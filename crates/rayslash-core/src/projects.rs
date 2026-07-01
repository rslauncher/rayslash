use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
}

pub fn scan_project_roots(roots: &[PathBuf]) -> Vec<Project> {
    let mut projects = roots
        .iter()
        .flat_map(|root| scan_project_root(root).unwrap_or_default())
        .collect::<Vec<_>>();

    projects.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.path.cmp(&b.path))
    });
    projects
}

fn scan_project_root(root: &Path) -> io::Result<Vec<Project>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut projects = Vec::new();

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }

        projects.push(Project {
            name,
            path: entry.path(),
        });
    }

    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_project_roots_lists_immediate_visible_directories() {
        let root = unique_temp_dir("rayslash-project-root");
        fs::create_dir(root.join("alpha")).expect("create alpha");
        fs::create_dir(root.join("Beta")).expect("create beta");
        fs::create_dir(root.join(".hidden")).expect("create hidden");
        fs::write(root.join("README.md"), "not a directory").expect("create file");
        fs::create_dir(root.join("alpha").join("nested")).expect("create nested");

        let projects = scan_project_roots(std::slice::from_ref(&root));

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

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn scan_project_roots_ignores_missing_roots() {
        let root = std::env::temp_dir().join(format!(
            "rayslash-missing-project-root-{}",
            std::process::id()
        ));

        let projects = scan_project_roots(&[root]);

        assert!(projects.is_empty());
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("create temp dir");
        path
    }
}
