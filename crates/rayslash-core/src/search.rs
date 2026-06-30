use std::path::{Path, PathBuf};

use crate::projects::Project;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub kind: SearchResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultKind {
    Placeholder,
    Project { path: PathBuf },
}

impl SearchResult {
    pub fn project_path(&self) -> Option<&Path> {
        match &self.kind {
            SearchResultKind::Placeholder => None,
            SearchResultKind::Project { path } => Some(path),
        }
    }
}

pub fn placeholder_results() -> Vec<SearchResult> {
    vec![
        SearchResult {
            title: "Open applications".to_owned(),
            subtitle: "Desktop app search will land in Phase 3".to_owned(),
            kind: SearchResultKind::Placeholder,
        },
        SearchResult {
            title: "Find projects".to_owned(),
            subtitle: "Project folder search will land in Phase 2".to_owned(),
            kind: SearchResultKind::Placeholder,
        },
        SearchResult {
            title: "Calculate".to_owned(),
            subtitle: "Calculator support will land in Phase 4".to_owned(),
            kind: SearchResultKind::Placeholder,
        },
    ]
}

pub fn project_results(projects: &[Project], query: &str) -> Vec<SearchResult> {
    if projects.is_empty() {
        return placeholder_results();
    }

    let normalized_query = query.trim().to_lowercase();

    projects
        .iter()
        .filter(|project| {
            normalized_query.is_empty() || project.name.to_lowercase().contains(&normalized_query)
        })
        .map(|project| SearchResult {
            title: project.name.clone(),
            subtitle: project.path.display().to_string(),
            kind: SearchResultKind::Project {
                path: project.path.clone(),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_results_are_available() {
        assert_eq!(placeholder_results().len(), 3);
    }

    #[test]
    fn project_results_filter_by_case_insensitive_substring() {
        let projects = vec![
            Project {
                name: "rayslash".to_owned(),
                path: PathBuf::from("/tmp/rayslash"),
            },
            Project {
                name: "Other".to_owned(),
                path: PathBuf::from("/tmp/other"),
            },
        ];

        let results = project_results(&projects, "SLASH");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "rayslash");
        assert_eq!(results[0].project_path(), Some(Path::new("/tmp/rayslash")));
    }

    #[test]
    fn project_results_show_all_projects_for_empty_query() {
        let projects = vec![Project {
            name: "rayslash".to_owned(),
            path: PathBuf::from("/tmp/rayslash"),
        }];

        let results = project_results(&projects, "");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "rayslash");
    }

    #[test]
    fn project_results_preserve_placeholders_when_no_projects_exist() {
        let results = project_results(&[], "anything");

        assert_eq!(results, placeholder_results());
    }
}
