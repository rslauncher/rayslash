use std::path::{Path, PathBuf};

use crate::actions::CommandSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub icon: SearchResultIcon,
    pub kind: SearchResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultIcon {
    Placeholder,
    Calculator,
    App { path: Option<PathBuf> },
    ProjectFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultKind {
    Placeholder,
    NoResults { query: String },
    Calculator { expression: String, result: String },
    CalculatorError { expression: String, message: String },
    App { id: String, command: CommandSpec },
    Project { path: PathBuf },
}

impl SearchResult {
    pub fn project_path(&self) -> Option<&Path> {
        match &self.kind {
            SearchResultKind::Placeholder => None,
            SearchResultKind::NoResults { .. } => None,
            SearchResultKind::Calculator { .. } => None,
            SearchResultKind::CalculatorError { .. } => None,
            SearchResultKind::App { .. } => None,
            SearchResultKind::Project { path } => Some(path),
        }
    }

    pub fn app_command(&self) -> Option<&CommandSpec> {
        match &self.kind {
            SearchResultKind::App { command, .. } => Some(command),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::Calculator { .. }
            | SearchResultKind::CalculatorError { .. }
            | SearchResultKind::Project { .. } => None,
        }
    }

    pub fn calculator_result(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::Calculator { result, .. } => Some(result),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::CalculatorError { .. }
            | SearchResultKind::App { .. }
            | SearchResultKind::Project { .. } => None,
        }
    }

    pub fn calculator_error_message(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::CalculatorError { message, .. } => Some(message),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::Calculator { .. }
            | SearchResultKind::App { .. }
            | SearchResultKind::Project { .. } => None,
        }
    }

    pub fn is_no_results(&self) -> bool {
        matches!(self.kind, SearchResultKind::NoResults { .. })
    }

    pub fn stable_id(&self) -> Option<String> {
        match &self.kind {
            SearchResultKind::App { id, .. } => Some(format!("app:{id}")),
            SearchResultKind::Project { path } => Some(format!("folder:{}", path.display())),
            SearchResultKind::Calculator { expression, .. }
            | SearchResultKind::CalculatorError { expression, .. } => {
                Some(format!("calculator:{}", expression.trim()))
            }
            SearchResultKind::NoResults { query } => Some(format!("no-results:{}", query.trim())),
            SearchResultKind::Placeholder => None,
        }
    }

    pub fn learning_id(&self) -> Option<String> {
        match &self.kind {
            SearchResultKind::App { .. } | SearchResultKind::Project { .. } => self.stable_id(),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::Calculator { .. }
            | SearchResultKind::CalculatorError { .. } => None,
        }
    }
}
