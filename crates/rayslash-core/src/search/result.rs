use std::path::{Path, PathBuf};

use crate::{actions::CommandSpec, config::AliasConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub flair: String,
    pub subtitle: String,
    pub icon: SearchResultIcon,
    pub kind: SearchResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultIcon {
    Placeholder,
    Calculator,
    UnitConversion,
    CurrencyConversion,
    TimeLookup,
    WebSearch { label: String },
    App { path: Option<PathBuf> },
    ProjectFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultKind {
    Placeholder,
    NoResults { query: String },
    Calculator { expression: String, result: String },
    CalculatorError { expression: String, message: String },
    UnitConversion { expression: String, result: String },
    CurrencyConversion { expression: String, result: String },
    CurrencyConversionError { expression: String, message: String },
    TimeLookup { expression: String, result: String },
    TimeLookupError { expression: String, message: String },
    WebSearch { name: String, url: String },
    DefaultWebSearch { query: String },
    App { id: String, command: CommandSpec },
    Project { path: PathBuf },
    Alias { alias: AliasConfig },
}

impl SearchResult {
    pub fn project_path(&self) -> Option<&Path> {
        match &self.kind {
            SearchResultKind::Placeholder => None,
            SearchResultKind::NoResults { .. } => None,
            SearchResultKind::Calculator { .. } => None,
            SearchResultKind::CalculatorError { .. } => None,
            SearchResultKind::UnitConversion { .. } => None,
            SearchResultKind::CurrencyConversion { .. } => None,
            SearchResultKind::CurrencyConversionError { .. } => None,
            SearchResultKind::TimeLookup { .. } => None,
            SearchResultKind::TimeLookupError { .. } => None,
            SearchResultKind::WebSearch { .. } => None,
            SearchResultKind::DefaultWebSearch { .. } => None,
            SearchResultKind::App { .. } => None,
            SearchResultKind::Project { path } => Some(path),
            SearchResultKind::Alias { .. } => None,
        }
    }

    pub fn app_command(&self) -> Option<&CommandSpec> {
        match &self.kind {
            SearchResultKind::App { command, .. } => Some(command),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::Calculator { .. }
            | SearchResultKind::CalculatorError { .. }
            | SearchResultKind::UnitConversion { .. }
            | SearchResultKind::CurrencyConversion { .. }
            | SearchResultKind::CurrencyConversionError { .. }
            | SearchResultKind::TimeLookup { .. }
            | SearchResultKind::TimeLookupError { .. }
            | SearchResultKind::WebSearch { .. }
            | SearchResultKind::DefaultWebSearch { .. }
            | SearchResultKind::Project { .. }
            | SearchResultKind::Alias { .. } => None,
        }
    }

    pub fn calculator_result(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::Calculator { result, .. } => Some(result),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::CalculatorError { .. }
            | SearchResultKind::UnitConversion { .. }
            | SearchResultKind::CurrencyConversion { .. }
            | SearchResultKind::CurrencyConversionError { .. }
            | SearchResultKind::TimeLookup { .. }
            | SearchResultKind::TimeLookupError { .. }
            | SearchResultKind::WebSearch { .. }
            | SearchResultKind::DefaultWebSearch { .. }
            | SearchResultKind::App { .. }
            | SearchResultKind::Project { .. }
            | SearchResultKind::Alias { .. } => None,
        }
    }

    pub fn calculator_error_message(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::CalculatorError { message, .. } => Some(message),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::Calculator { .. }
            | SearchResultKind::UnitConversion { .. }
            | SearchResultKind::CurrencyConversion { .. }
            | SearchResultKind::CurrencyConversionError { .. }
            | SearchResultKind::TimeLookup { .. }
            | SearchResultKind::TimeLookupError { .. }
            | SearchResultKind::WebSearch { .. }
            | SearchResultKind::DefaultWebSearch { .. }
            | SearchResultKind::App { .. }
            | SearchResultKind::Project { .. }
            | SearchResultKind::Alias { .. } => None,
        }
    }

    pub fn is_no_results(&self) -> bool {
        matches!(self.kind, SearchResultKind::NoResults { .. })
    }

    pub fn unit_conversion_result(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::UnitConversion { result, .. } => Some(result),
            _ => None,
        }
    }

    pub fn currency_conversion_result(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::CurrencyConversion { result, .. } => Some(result),
            _ => None,
        }
    }

    pub fn currency_error_message(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::CurrencyConversionError { message, .. } => Some(message),
            _ => None,
        }
    }

    pub fn time_lookup_result(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::TimeLookup { result, .. } => Some(result),
            _ => None,
        }
    }

    pub fn time_lookup_error_message(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::TimeLookupError { message, .. } => Some(message),
            _ => None,
        }
    }

    pub fn web_search_url(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::WebSearch { url, .. } => Some(url),
            _ => None,
        }
    }

    pub fn default_web_search_query(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::DefaultWebSearch { query } => Some(query),
            _ => None,
        }
    }

    pub fn app_id(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::App { id, .. } => Some(id),
            _ => None,
        }
    }

    pub fn stable_id(&self) -> Option<String> {
        match &self.kind {
            SearchResultKind::App { id, .. } => Some(format!("app:{id}")),
            SearchResultKind::Project { path } => Some(format!("folder:{}", path.display())),
            SearchResultKind::Calculator { expression, .. }
            | SearchResultKind::CalculatorError { expression, .. } => {
                Some(format!("calculator:{}", expression.trim()))
            }
            SearchResultKind::UnitConversion { expression, .. } => {
                Some(format!("unit-conversion:{}", expression.trim()))
            }
            SearchResultKind::CurrencyConversion { expression, .. }
            | SearchResultKind::CurrencyConversionError { expression, .. } => {
                Some(format!("currency-conversion:{}", expression.trim()))
            }
            SearchResultKind::TimeLookup { expression, .. }
            | SearchResultKind::TimeLookupError { expression, .. } => {
                Some(format!("time-lookup:{}", expression.trim()))
            }
            SearchResultKind::WebSearch { name, url } => Some(format!("web-search:{name}:{url}")),
            SearchResultKind::DefaultWebSearch { query } => {
                Some(format!("default-web-search:{}", query.trim()))
            }
            SearchResultKind::Alias { alias } => Some(format!("alias:{}", alias.query.trim())),
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
            | SearchResultKind::CalculatorError { .. }
            | SearchResultKind::UnitConversion { .. }
            | SearchResultKind::CurrencyConversion { .. }
            | SearchResultKind::CurrencyConversionError { .. }
            | SearchResultKind::TimeLookup { .. }
            | SearchResultKind::TimeLookupError { .. }
            | SearchResultKind::WebSearch { .. }
            | SearchResultKind::DefaultWebSearch { .. }
            | SearchResultKind::Alias { .. } => None,
        }
    }

    pub fn alias(&self) -> Option<&AliasConfig> {
        match &self.kind {
            SearchResultKind::Alias { alias } => Some(alias),
            _ => None,
        }
    }
}
