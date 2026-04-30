use std::fs;
use std::path::PathBuf;

use super::{load_projects_from, ProjectsConfig};

fn write_json(dir: &tempfile::TempDir, contents: &str) -> PathBuf {
    let path = dir.path().join("projects.json");
    fs::write(&path, contents).unwrap();
    path
}

#[test]
fn missing_file_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nope.json");
    assert!(load_projects_from(&path).is_empty());
}

#[test]
fn malformed_json_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(&dir, "{ this is not json");
    assert!(load_projects_from(&path).is_empty());
}

#[test]
fn empty_projects_array_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(&dir, r#"{"projects": []}"#);
    assert!(load_projects_from(&path).is_empty());
}

#[test]
fn sorts_by_rank_ascending() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(
        &dir,
        r##"{
            "projects": [
                { "name": "Beta",  "cwd": "/b", "color": "#aaaaaa", "rank": 3 },
                { "name": "Alpha", "cwd": "/a", "color": "#bbbbbb", "rank": 1 },
                { "name": "Gamma", "cwd": "/g", "color": "#cccccc", "rank": 2 }
            ]
        }"##,
    );
    let projects = load_projects_from(&path);
    let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "Gamma", "Beta"]);
}

#[test]
fn round_trip_serde() {
    let original = ProjectsConfig {
        projects: vec![super::Project {
            name: "Kenect".into(),
            cwd: PathBuf::from("D:\\kenect-services"),
            color: "#2bd7fb".into(),
            rank: 2,
        }],
    };
    let serialized = serde_json::to_string(&original).unwrap();
    let parsed: ProjectsConfig = serde_json::from_str(&serialized).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn tilde_expanded_at_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(
        &dir,
        r##"{"projects": [{ "name": "Cfg", "cwd": "~/foo/bar", "color": "#999999", "rank": 1 }]}"##,
    );
    let projects = load_projects_from(&path);
    let cwd = &projects[0].cwd;
    assert!(!cwd.to_string_lossy().starts_with('~'), "cwd={cwd:?} still has tilde");
    let home = dirs::home_dir().unwrap();
    assert!(
        cwd.starts_with(&home),
        "cwd={cwd:?} should start with home dir {home:?}"
    );
}
