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
