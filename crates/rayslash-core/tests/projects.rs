mod fixtures;

use fixtures::TempDir;
use rayslash_core::projects::{self, Project};

#[test]
fn scan_project_roots_lists_immediate_visible_directories() {
    let root = TempDir::new("rayslash-project-root");
    root.create_dir_all("alpha").expect("create alpha");
    root.create_dir_all("Beta").expect("create beta");
    root.create_dir_all(".hidden").expect("create hidden");
    root.write("README.md", "not a directory")
        .expect("create file");
    root.create_dir_all("alpha/nested").expect("create nested");

    let projects = projects::scan_project_roots(&[root.path().to_path_buf()]);

    assert_eq!(
        projects,
        vec![
            Project {
                name: "alpha".to_owned(),
                path: root.join("alpha")
            },
            Project {
                name: "Beta".to_owned(),
                path: root.join("Beta")
            },
        ]
    );
}

#[test]
fn scan_project_roots_ignores_missing_roots() {
    let root = std::env::temp_dir().join(format!(
        "rayslash-missing-project-root-{}",
        std::process::id()
    ));

    let projects = projects::scan_project_roots(&[root]);

    assert!(projects.is_empty());
}
