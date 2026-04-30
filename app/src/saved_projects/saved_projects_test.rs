use std::fs;
use std::path::{Path, PathBuf};

use super::{load_projects_from, project_for_path, Project, ProjectsConfig};

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
            name: "Acme".into(),
            cwd: PathBuf::from("D:\\acme-services"),
            color: "#2bd7fb".into(),
            rank: 2,
        }],
    };
    let serialized = serde_json::to_string(&original).unwrap();
    let parsed: ProjectsConfig = serde_json::from_str(&serialized).unwrap();
    assert_eq!(original, parsed);
}

fn project(name: &str, cwd: &str) -> Project {
    Project {
        name: name.into(),
        cwd: PathBuf::from(cwd),
        color: "#ff00ff".into(),
        rank: 0,
    }
}

#[test]
fn project_for_path_exact_match() {
    let projects = vec![project("Alpha", "/projects/alpha")];
    let hit = project_for_path(Path::new("/projects/alpha"), &projects);
    assert_eq!(hit.map(|p| p.name.as_str()), Some("Alpha"));
}

#[test]
fn project_for_path_subdirectory_match() {
    let projects = vec![project("Alpha", "/projects/alpha")];
    let hit = project_for_path(Path::new("/projects/alpha/src/lib"), &projects);
    assert_eq!(hit.map(|p| p.name.as_str()), Some("Alpha"));
}

#[test]
fn project_for_path_nested_longest_prefix_wins() {
    let projects = vec![
        project("Outer", "/projects"),
        project("Inner", "/projects/alpha"),
    ];
    let hit = project_for_path(Path::new("/projects/alpha/src"), &projects);
    assert_eq!(hit.map(|p| p.name.as_str()), Some("Inner"));
}

#[test]
fn project_for_path_no_match_when_unrelated() {
    let projects = vec![
        project("Alpha", "/projects/alpha"),
        project("Beta", "/projects/beta"),
    ];
    assert!(project_for_path(Path::new("/elsewhere"), &projects).is_none());
}

#[test]
fn project_for_path_partial_component_is_not_a_prefix() {
    // `/projects/alphabet` should NOT match a project at `/projects/alpha`,
    // because Path::starts_with is component-wise, not byte-wise.
    let projects = vec![project("Alpha", "/projects/alpha")];
    assert!(project_for_path(Path::new("/projects/alphabet"), &projects).is_none());
}

#[test]
fn project_for_path_empty_projects_returns_none() {
    let projects: Vec<Project> = vec![];
    assert!(project_for_path(Path::new("/anywhere"), &projects).is_none());
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
