//! Seeds Cortex-shipped skills into the user's home skills directories at app start.
//!
//! Skills are embedded into the binary via `include_str!` and written to each
//! target provider's skills root on launch. A `.cortex-shipped-hash` sidecar
//! records what we last shipped, so we can upgrade in place without clobbering
//! user edits.
//!
//! Each skill is seeded once per target provider. For example, the orchestrate
//! skill is written to both `~/.agents/skills/cortex-orchestrate/` (Cortex's
//! own skill palette) and `~/.claude/skills/cortex-orchestrate/` (Claude Code's
//! `/slash` command autocomplete).
//!
//! Source of truth for content lives at `cortex-skills/` in the repo root.

use std::fs;
use std::io::Write as _;
use std::path::Path;

use ai::skills::{home_skills_path, SkillProvider};
use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

const HASH_SIDECAR: &str = ".cortex-shipped-hash";

struct CortexSkill {
    /// Embedded content of the skill's `SKILL.md`.
    content: &'static str,
    /// Subdirectory under the provider's home skills root, e.g. `cortex-orchestrate`.
    /// The `cortex-` prefix keeps Cortex-shipped skills visually distinct from
    /// any upstream `warpdotdev/common-skills` sync into the same root.
    dest_subdir: &'static str,
    /// Provider whose home skills directory the skill is written into.
    provider: SkillProvider,
}

const ORCHESTRATE_SKILL: &str = include_str!("../../cortex-skills/orchestrate/SKILL.md");

const SHIPPED_SKILLS: &[CortexSkill] = &[
    CortexSkill {
        content: ORCHESTRATE_SKILL,
        dest_subdir: "cortex-orchestrate",
        provider: SkillProvider::Agents,
    },
    CortexSkill {
        content: ORCHESTRATE_SKILL,
        dest_subdir: "cortex-orchestrate",
        provider: SkillProvider::Claude,
    },
];

/// Write Cortex-shipped skills to the user's home skills directory if they
/// are missing, or upgrade them in place if the shipped content has changed
/// and the on-disk file has not been edited by the user.
///
/// Errors are logged and swallowed — skill seeding must never block startup.
pub fn seed_cortex_skills() {
    for skill in SHIPPED_SKILLS {
        match seed_one(skill) {
            Ok(SeedOutcome::Wrote) => {
                log::info!("cortex skill seeded: {}", skill.dest_subdir);
            }
            Ok(SeedOutcome::UpToDate) => {}
            Ok(SeedOutcome::Skipped(reason)) => {
                log::info!("cortex skill {} skipped: {reason}", skill.dest_subdir);
            }
            Err(err) => {
                log::warn!("cortex skill {} seed failed: {err:#}", skill.dest_subdir);
            }
        }
    }
}

enum SeedOutcome {
    Wrote,
    UpToDate,
    Skipped(&'static str),
}

fn seed_one(skill: &CortexSkill) -> Result<SeedOutcome> {
    let Some(root) = home_skills_path(skill.provider) else {
        return Ok(SeedOutcome::Skipped("no home_skills_path for provider"));
    };
    let dest_dir = root.join(skill.dest_subdir);
    let skill_path = dest_dir.join("SKILL.md");
    let sidecar_path = dest_dir.join(HASH_SIDECAR);

    let shipped_hash = sha256_hex(skill.content.as_bytes());

    if skill_path.exists() {
        let Some(prev_shipped) = fs::read_to_string(&sidecar_path)
            .ok()
            .map(|s| s.trim().to_string())
        else {
            // No marker: user-authored or pre-marker install. Don't clobber.
            return Ok(SeedOutcome::Skipped(
                "SKILL.md present with no shipped-hash marker",
            ));
        };
        let on_disk = fs::read(&skill_path)
            .with_context(|| format!("read {}", skill_path.display()))?;
        let on_disk_hash = sha256_hex(&on_disk);
        if on_disk_hash != prev_shipped {
            return Ok(SeedOutcome::Skipped("SKILL.md modified since shipping"));
        }
        if prev_shipped == shipped_hash {
            return Ok(SeedOutcome::UpToDate);
        }
        // Pristine but stale: fall through and overwrite.
    }

    fs::create_dir_all(&dest_dir)
        .with_context(|| format!("create {}", dest_dir.display()))?;
    atomic_write(&skill_path, skill.content.as_bytes())?;
    atomic_write(&sidecar_path, shipped_hash.as_bytes())?;
    Ok(SeedOutcome::Wrote)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", path.display()))?;
    let mut tmp = NamedTempFile::new_in(parent)
        .with_context(|| format!("temp file for {}", path.display()))?;
    tmp.write_all(bytes)
        .with_context(|| format!("write {}", path.display()))?;
    tmp.flush()
        .with_context(|| format!("flush {}", path.display()))?;
    tmp.persist(path)
        .map(|_| ())
        .map_err(|err| anyhow::Error::from(err.error))
        .with_context(|| format!("persist {}", path.display()))?;
    Ok(())
}
