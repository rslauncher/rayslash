use std::{borrow::Cow, path::PathBuf};

use crate::{
    actions::CommandSpec,
    apps::DesktopApp,
    config::{AliasConfig, ProviderConfig as LegacyProviderConfig, WebSearchConfig},
    projects::Project,
    ranking::RankingState,
    search::{SearchResult, SearchResultKind},
    utility_actions::UtilityAction,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderId(Cow<'static, str>);

impl ProviderId {
    pub const CORE_APPS: Self = Self::builtin("rayslash.core.apps");
    pub const CORE_FOLDERS: Self = Self::builtin("rayslash.core.folders");
    pub const CALCULATOR: Self = Self::builtin("rayslash.calculator");
    pub const UNITS: Self = Self::builtin("rayslash.units");
    pub const CURRENCY: Self = Self::builtin("rayslash.currency");
    pub const TIME: Self = Self::builtin("rayslash.time");
    pub const WEB_SEARCH: Self = Self::builtin("rayslash.web-search");
    pub const TIMERS: Self = Self::builtin("rayslash.timers");
    pub const ALIASES: Self = Self::builtin("rayslash.aliases");
    pub const CORE_FALLBACK: Self = Self::builtin("rayslash.core.fallback");

    pub const fn builtin(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    pub fn new(id: impl Into<String>) -> Self {
        Self(Cow::Owned(id.into()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProviderPermissions {
    pub network: bool,
    pub filesystem: bool,
    pub process: bool,
    pub clipboard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderMetadata {
    pub id: ProviderId,
    pub name: &'static str,
    pub description: &'static str,
    pub module_id: Option<ProviderId>,
    pub ranking_eligible: bool,
    pub permissions: ProviderPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    pub enabled: bool,
}

impl ProviderConfig {
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderExecutionHint {
    Local,
    DebouncedNetwork { debounce_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderHealth {
    Ready,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDiagnostics {
    pub provider_id: ProviderId,
    pub health: ProviderHealth,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderAction {
    None,
    Dismiss,
    CopyText(String),
    ShowMessage(String),
    OpenUrl(String),
    OpenDefaultWebSearch(String),
    OpenFolder(PathBuf),
    LaunchApp {
        id: String,
        command: CommandSpec,
        desktop_file: PathBuf,
        dbus_activatable: bool,
        startup_wm_class: Option<String>,
    },
    LaunchAlias(AliasConfig),
    RunUtility(UtilityAction),
}

impl ProviderAction {
    pub fn from_result(result: &SearchResult) -> Self {
        match &result.kind {
            SearchResultKind::Placeholder => Self::None,
            SearchResultKind::NoResults { .. } => Self::Dismiss,
            SearchResultKind::Calculator { result, .. }
            | SearchResultKind::UnitConversion { result, .. }
            | SearchResultKind::CurrencyConversion { result, .. }
            | SearchResultKind::TimeLookup { result, .. } => Self::CopyText(result.clone()),
            SearchResultKind::CalculatorError { message, .. }
            | SearchResultKind::CurrencyConversionError { message, .. }
            | SearchResultKind::TimeLookupError { message, .. }
            | SearchResultKind::UtilityActionError { message, .. } => {
                Self::ShowMessage(message.clone())
            }
            SearchResultKind::UtilityAction { action } => Self::RunUtility(action.clone()),
            SearchResultKind::WebSearch { url, .. } => Self::OpenUrl(url.clone()),
            SearchResultKind::DefaultWebSearch { query } => {
                Self::OpenDefaultWebSearch(query.clone())
            }
            SearchResultKind::App {
                id,
                command,
                desktop_file,
                dbus_activatable,
                startup_wm_class,
            } => Self::LaunchApp {
                id: id.clone(),
                command: command.clone(),
                desktop_file: desktop_file.clone(),
                dbus_activatable: *dbus_activatable,
                startup_wm_class: startup_wm_class.clone(),
            },
            SearchResultKind::Project { path } => Self::OpenFolder(path.clone()),
            SearchResultKind::Alias { alias } => Self::LaunchAlias(alias.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResult {
    pub provider_id: ProviderId,
    pub result: SearchResult,
    pub action: ProviderAction,
    pub match_score: Option<u32>,
    pub ranking_eligible: bool,
}

impl ProviderResult {
    pub fn new(
        provider_id: ProviderId,
        result: SearchResult,
        match_score: Option<u32>,
        ranking_eligible: bool,
    ) -> Self {
        let action = ProviderAction::from_result(&result);
        Self {
            provider_id,
            result,
            action,
            match_score,
            ranking_eligible,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOutcome {
    pub provider_id: ProviderId,
    pub results: Vec<ProviderResult>,
    pub matched_query: bool,
    pub exclusive: bool,
    pub suppresses_calculator: bool,
    pub execution_hint: ProviderExecutionHint,
}

impl ProviderOutcome {
    pub fn empty(provider_id: ProviderId) -> Self {
        Self {
            provider_id,
            results: Vec::new(),
            matched_query: false,
            exclusive: false,
            suppresses_calculator: false,
            execution_hint: ProviderExecutionHint::Local,
        }
    }
}

pub struct ProviderContext<'a> {
    pub query: &'a str,
    pub projects: &'a [Project],
    pub apps: &'a [DesktopApp],
    pub aliases: &'a [AliasConfig],
    pub web_searches: &'a [WebSearchConfig],
    pub legacy_config: &'a LegacyProviderConfig,
    pub ranking: Option<&'a RankingState>,
}
