use std::fs;
use std::path::{Path, PathBuf};

use super::{
    accent_for_path, load_projects_from, project_for_path, AllSentinel, Project, ProjectsConfig,
    SubProjects,
};

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
fn preserves_file_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(
        &dir,
        r##"{
            "projects": [
                { "name": "Beta",  "cwd": "/b", "color": "#aaaaaa" },
                { "name": "Alpha", "cwd": "/a", "color": "#bbbbbb" },
                { "name": "Gamma", "cwd": "/g", "color": "#cccccc" }
            ]
        }"##,
    );
    let projects = load_projects_from(&path);
    let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["Beta", "Alpha", "Gamma"]);
}

#[test]
fn round_trip_serde() {
    let original = ProjectsConfig {
        projects: vec![super::Project {
            name: "Acme".into(),
            cwd: PathBuf::from("C:\\projects\\acme-services"),
            color: "#2bd7fb".into(),
            sub_projects: None,
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
        sub_projects: None,
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
fn sub_projects_omitted_loads_as_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(
        &dir,
        r##"{"projects": [{ "name": "X", "cwd": "/x", "color": "#888888" }]}"##,
    );
    let projects = load_projects_from(&path);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].sub_projects, None);
}

#[test]
fn sub_projects_all_string_form() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(
        &dir,
        r##"{"projects": [{
            "name": "X", "cwd": "/x", "color": "#888888",
            "sub_projects": "all"
        }]}"##,
    );
    let projects = load_projects_from(&path);
    assert_eq!(
        projects[0].sub_projects,
        Some(SubProjects::All(AllSentinel::All))
    );
}

#[test]
fn sub_projects_named_array_form() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(
        &dir,
        r##"{"projects": [{
            "name": "X", "cwd": "/x", "color": "#888888",
            "sub_projects": ["foo", "bar"]
        }]}"##,
    );
    let projects = load_projects_from(&path);
    assert_eq!(
        projects[0].sub_projects,
        Some(SubProjects::Named(vec!["foo".into(), "bar".into()]))
    );
}

#[test]
fn resolved_sub_projects_named_preserves_order() {
    let p = Project {
        name: "X".into(),
        cwd: PathBuf::from("/parent"),
        color: "#888888".into(),
        sub_projects: Some(SubProjects::Named(vec!["c".into(), "a".into(), "b".into()])),
    };
    let subs = p.resolved_sub_projects();
    let names: Vec<&str> = subs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["c", "a", "b"]);
    assert_eq!(subs[0].cwd, PathBuf::from("/parent/c"));
}

#[test]
fn resolved_sub_projects_all_filters_and_sorts() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("Beta")).unwrap();
    fs::create_dir(dir.path().join("alpha")).unwrap();
    fs::create_dir(dir.path().join(".hidden")).unwrap();
    fs::write(dir.path().join("file.txt"), "").unwrap();
    let p = Project {
        name: "Parent".into(),
        cwd: dir.path().to_path_buf(),
        color: "#888888".into(),
        sub_projects: Some(SubProjects::All(AllSentinel::All)),
    };
    let names: Vec<String> = p.resolved_sub_projects().into_iter().map(|s| s.name).collect();
    assert_eq!(names, vec!["alpha".to_string(), "Beta".to_string()]);
}

#[test]
fn resolved_sub_projects_none_is_empty() {
    let p = Project {
        name: "X".into(),
        cwd: PathBuf::from("/whatever"),
        color: "#888888".into(),
        sub_projects: None,
    };
    assert!(p.resolved_sub_projects().is_empty());
}

#[test]
fn accent_for_path_returns_parent_color_for_non_sub_path() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("sub_a")).unwrap();
    fs::create_dir(dir.path().join("sub_b")).unwrap();
    // Create the nested dir so canonicalize succeeds symmetrically with the
    // project's cwd (matters on macOS where /var → /private/var).
    fs::create_dir(dir.path().join("sub_a").join("nested")).unwrap();
    let projects = vec![Project {
        name: "Parent".into(),
        cwd: dir.path().to_path_buf(),
        color: "#112233".into(),
        sub_projects: Some(SubProjects::Named(vec!["sub_a".into(), "sub_b".into()])),
    }];
    // A path deep inside the parent but not at a sub-project root → parent color.
    let deep = dir.path().join("sub_a").join("nested");
    let accent = accent_for_path(&deep, &projects).unwrap();
    assert_eq!(accent.r, 0x11);
    assert_eq!(accent.g, 0x22);
    assert_eq!(accent.b, 0x33);
}

#[test]
fn accent_for_path_returns_gradient_for_exact_sub_match() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("sub_a")).unwrap();
    fs::create_dir(dir.path().join("sub_b")).unwrap();
    let projects = vec![Project {
        name: "Parent".into(),
        cwd: dir.path().to_path_buf(),
        color: "#112233".into(),
        sub_projects: Some(SubProjects::Named(vec!["sub_a".into(), "sub_b".into()])),
    }];
    // Exact sub-project cwd → gradient shade (different from parent).
    let sub_path = dir.path().join("sub_a");
    let accent = accent_for_path(&sub_path, &projects).unwrap();
    let parent_accent = warp_core::ui::color::hex_color::coloru_from_hex_string("#112233").unwrap();
    assert_ne!(
        (accent.r, accent.g, accent.b),
        (parent_accent.r, parent_accent.g, parent_accent.b),
        "sub-project should not get parent's exact color"
    );
}

#[test]
fn tilde_expanded_at_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_json(
        &dir,
        r##"{"projects": [{ "name": "Cfg", "cwd": "~/foo/bar", "color": "#999999" }]}"##,
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
