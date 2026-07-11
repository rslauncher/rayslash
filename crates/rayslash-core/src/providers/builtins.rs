use nucleo_matcher::Utf32Str;

use crate::{
    calc, currency,
    search::{
        matcher::{fuzzy_matcher, fuzzy_pattern},
        providers::{
            alias_result, app_result, calculator_result, currency_conversion_result,
            currency_error_result, project_result, time_lookup_error_result, time_lookup_result,
            unit_conversion_result, utility_action_error_result, utility_action_result,
            web_search_result,
        },
    },
    time_lookup, units, utility_actions, web_search,
};

use super::{
    Provider, ProviderConfig, ProviderContext, ProviderExecutionHint, ProviderId, ProviderMetadata,
    ProviderOutcome, ProviderPermissions, ProviderResult,
};

const REMOTE_DEBOUNCE_MS: u64 = 450;

struct UtilityProvider;
struct UnitsProvider;
struct CurrencyProvider;
struct TimeProvider;
struct WebSearchProvider;
struct CalculatorProvider;
struct FoldersProvider;
struct AppsProvider;
struct AliasesProvider;

static UTILITY_PROVIDER: UtilityProvider = UtilityProvider;
static UNITS_PROVIDER: UnitsProvider = UnitsProvider;
static CURRENCY_PROVIDER: CurrencyProvider = CurrencyProvider;
static TIME_PROVIDER: TimeProvider = TimeProvider;
static WEB_SEARCH_PROVIDER: WebSearchProvider = WebSearchProvider;
static CALCULATOR_PROVIDER: CalculatorProvider = CalculatorProvider;
static FOLDERS_PROVIDER: FoldersProvider = FoldersProvider;
static APPS_PROVIDER: AppsProvider = AppsProvider;
static ALIASES_PROVIDER: AliasesProvider = AliasesProvider;

static UTILITY_METADATA: ProviderMetadata = ProviderMetadata {
    id: ProviderId::TIMERS,
    name: "Timers & system actions",
    description: "Schedule timers, reminders, and power actions.",
    module_id: Some(ProviderId::TIMERS),
    ranking_eligible: true,
    permissions: ProviderPermissions {
        network: false,
        filesystem: false,
        process: true,
        clipboard: false,
    },
};

static UNITS_METADATA: ProviderMetadata = ProviderMetadata {
    id: ProviderId::UNITS,
    name: "Units",
    description: "Convert common units locally.",
    module_id: Some(ProviderId::UNITS),
    ranking_eligible: false,
    permissions: ProviderPermissions {
        network: false,
        filesystem: false,
        process: false,
        clipboard: true,
    },
};

static CURRENCY_METADATA: ProviderMetadata = ProviderMetadata {
    id: ProviderId::CURRENCY,
    name: "Currency",
    description: "Convert currencies with cached live rates.",
    module_id: Some(ProviderId::CURRENCY),
    ranking_eligible: false,
    permissions: ProviderPermissions {
        network: true,
        filesystem: false,
        process: false,
        clipboard: true,
    },
};

static TIME_METADATA: ProviderMetadata = ProviderMetadata {
    id: ProviderId::TIME,
    name: "Time",
    description: "Check local time for places.",
    module_id: Some(ProviderId::TIME),
    ranking_eligible: false,
    permissions: ProviderPermissions {
        network: true,
        filesystem: false,
        process: false,
        clipboard: true,
    },
};

static WEB_SEARCH_METADATA: ProviderMetadata = ProviderMetadata {
    id: ProviderId::WEB_SEARCH,
    name: "Web Search",
    description: "Search the web with keyword triggers.",
    module_id: Some(ProviderId::WEB_SEARCH),
    ranking_eligible: false,
    permissions: ProviderPermissions {
        network: true,
        filesystem: false,
        process: true,
        clipboard: false,
    },
};

static CALCULATOR_METADATA: ProviderMetadata = ProviderMetadata {
    id: ProviderId::CALCULATOR,
    name: "Calculator",
    description: "Calculate expressions and linear equations.",
    module_id: Some(ProviderId::CALCULATOR),
    ranking_eligible: false,
    permissions: ProviderPermissions {
        network: false,
        filesystem: false,
        process: false,
        clipboard: true,
    },
};

static FOLDERS_METADATA: ProviderMetadata = ProviderMetadata {
    id: ProviderId::CORE_FOLDERS,
    name: "Folders",
    description: "Search configured folder sources.",
    module_id: None,
    ranking_eligible: true,
    permissions: ProviderPermissions {
        network: false,
        filesystem: true,
        process: true,
        clipboard: false,
    },
};

static APPS_METADATA: ProviderMetadata = ProviderMetadata {
    id: ProviderId::CORE_APPS,
    name: "Applications",
    description: "Search installed desktop applications.",
    module_id: None,
    ranking_eligible: true,
    permissions: ProviderPermissions {
        network: false,
        filesystem: true,
        process: true,
        clipboard: false,
    },
};

static ALIASES_METADATA: ProviderMetadata = ProviderMetadata {
    id: ProviderId::ALIASES,
    name: "Aliases",
    description: "Open configured quick links and commands.",
    module_id: Some(ProviderId::ALIASES),
    ranking_eligible: false,
    permissions: ProviderPermissions {
        network: true,
        filesystem: true,
        process: true,
        clipboard: false,
    },
};

static PROVIDERS: [&'static dyn Provider; 9] = [
    &UTILITY_PROVIDER,
    &UNITS_PROVIDER,
    &CURRENCY_PROVIDER,
    &TIME_PROVIDER,
    &WEB_SEARCH_PROVIDER,
    &CALCULATOR_PROVIDER,
    &FOLDERS_PROVIDER,
    &APPS_PROVIDER,
    &ALIASES_PROVIDER,
];

static CATALOG: [&ProviderMetadata; 9] = [
    &APPS_METADATA,
    &FOLDERS_METADATA,
    &CALCULATOR_METADATA,
    &ALIASES_METADATA,
    &WEB_SEARCH_METADATA,
    &UNITS_METADATA,
    &CURRENCY_METADATA,
    &TIME_METADATA,
    &UTILITY_METADATA,
];

pub fn builtin_providers() -> &'static [&'static dyn Provider] {
    &PROVIDERS
}

pub fn builtin_provider_catalog() -> &'static [&'static ProviderMetadata] {
    &CATALOG
}

fn result(
    metadata: &'static ProviderMetadata,
    result: crate::search::SearchResult,
    match_score: Option<u32>,
) -> ProviderResult {
    ProviderResult::new(
        metadata.id.clone(),
        result,
        match_score,
        metadata.ranking_eligible,
    )
}

fn outcome(metadata: &'static ProviderMetadata) -> ProviderOutcome {
    ProviderOutcome::empty(metadata.id.clone())
}

impl Provider for UtilityProvider {
    fn metadata(&self) -> &'static ProviderMetadata {
        &UTILITY_METADATA
    }

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig {
        ProviderConfig::new(context.legacy_config.utility_actions)
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut output = outcome(self.metadata());
        if context.query.is_empty() || !self.config(context).enabled {
            return output;
        }

        if let Some(action) = utility_actions::parse_query(context.query) {
            output.matched_query = true;
            output.suppresses_calculator = true;
            let search_result = match action {
                Ok(action) => utility_action_result(action),
                Err(error) => utility_action_error_result(&error.expression, error.message),
            };
            output
                .results
                .push(result(self.metadata(), search_result, None));
        } else if let Some(action) = utility_actions::fuzzy_system_action(context.query) {
            let searchable_name = match &action {
                utility_actions::UtilityAction::System(action) => action.expression.clone(),
                utility_actions::UtilityAction::Timer(action) => action.expression.clone(),
            };
            let pattern = fuzzy_pattern(context.query);
            let mut matcher = fuzzy_matcher();
            let mut char_buf = Vec::new();
            let haystack = Utf32Str::new(&searchable_name, &mut char_buf);
            let score = pattern.score(haystack, &mut matcher).unwrap_or(1);
            output.results.push(result(
                self.metadata(),
                utility_action_result(action),
                Some(score),
            ));
        }
        output
    }
}

impl Provider for UnitsProvider {
    fn metadata(&self) -> &'static ProviderMetadata {
        &UNITS_METADATA
    }

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig {
        ProviderConfig::new(context.legacy_config.unit_conversion)
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut output = outcome(self.metadata());
        if context.query.is_empty() {
            return output;
        }

        if self.config(context).enabled
            && let Some(conversion) = units::convert_query(context.query)
        {
            output.matched_query = true;
            output.suppresses_calculator = true;
            output.results.push(result(
                self.metadata(),
                unit_conversion_result(conversion),
                None,
            ));
        } else if units::looks_like_conversion_query(context.query) {
            output.matched_query = true;
            output.suppresses_calculator = true;
        }
        output
    }
}

impl Provider for CurrencyProvider {
    fn metadata(&self) -> &'static ProviderMetadata {
        &CURRENCY_METADATA
    }

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig {
        ProviderConfig::new(context.legacy_config.currency_conversion)
    }

    fn execution_hint(&self, context: &ProviderContext<'_>) -> ProviderExecutionHint {
        if self.config(context).enabled
            && currency::parse_query(context.query)
                .is_some_and(|request| request.base != request.quote)
        {
            ProviderExecutionHint::DebouncedNetwork {
                debounce_ms: REMOTE_DEBOUNCE_MS,
            }
        } else {
            ProviderExecutionHint::Local
        }
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut output = outcome(self.metadata());
        let Some(request) = currency::parse_query(context.query) else {
            return output;
        };
        output.matched_query = true;
        output.suppresses_calculator = true;
        if !self.config(context).enabled {
            return output;
        }

        let search_result = match currency::convert_request(&request) {
            Ok(conversion) => currency_conversion_result(conversion),
            Err(error) => currency_error_result(&request.expression, error.to_string()),
        };
        output
            .results
            .push(result(self.metadata(), search_result, None));
        output
    }
}

impl Provider for TimeProvider {
    fn metadata(&self) -> &'static ProviderMetadata {
        &TIME_METADATA
    }

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig {
        ProviderConfig::new(context.legacy_config.time_lookup)
    }

    fn execution_hint(&self, context: &ProviderContext<'_>) -> ProviderExecutionHint {
        if self.config(context).enabled && time_lookup::parse_query(context.query).is_some() {
            ProviderExecutionHint::DebouncedNetwork {
                debounce_ms: REMOTE_DEBOUNCE_MS,
            }
        } else {
            ProviderExecutionHint::Local
        }
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut output = outcome(self.metadata());
        let Some(request) = time_lookup::parse_query(context.query) else {
            return output;
        };
        output.matched_query = true;
        output.suppresses_calculator = true;
        if !self.config(context).enabled {
            return output;
        }

        output.exclusive = true;
        match time_lookup::lookup_request(&request) {
            Ok(lookups) => output.results.extend(
                lookups
                    .into_iter()
                    .map(|lookup| result(self.metadata(), time_lookup_result(lookup), None)),
            ),
            Err(error) => output.results.push(result(
                self.metadata(),
                time_lookup_error_result(&request.expression, error.to_string()),
                None,
            )),
        }
        output
    }
}

impl Provider for WebSearchProvider {
    fn metadata(&self) -> &'static ProviderMetadata {
        &WEB_SEARCH_METADATA
    }

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig {
        ProviderConfig::new(context.legacy_config.web_search)
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut output = outcome(self.metadata());
        if context.query.is_empty() || !self.config(context).enabled {
            return output;
        }

        output.results.extend(
            web_search::matching_web_searches(context.web_searches, context.query)
                .into_iter()
                .map(|search| result(self.metadata(), web_search_result(search), None)),
        );
        if !output.results.is_empty() {
            output.matched_query = true;
            output.exclusive = true;
            output.suppresses_calculator = true;
        }
        output
    }
}

impl Provider for CalculatorProvider {
    fn metadata(&self) -> &'static ProviderMetadata {
        &CALCULATOR_METADATA
    }

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig {
        ProviderConfig::new(context.legacy_config.calculator)
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut output = outcome(self.metadata());
        if context.query.is_empty() || !self.config(context).enabled {
            return output;
        }
        if let Some(calculation) = calc::calculate(context.query) {
            output.matched_query = true;
            output.results.push(result(
                self.metadata(),
                calculator_result(calculation),
                None,
            ));
        }
        output
    }
}

impl Provider for FoldersProvider {
    fn metadata(&self) -> &'static ProviderMetadata {
        &FOLDERS_METADATA
    }

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig {
        ProviderConfig::new(context.legacy_config.folders)
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut output = outcome(self.metadata());
        if !self.config(context).enabled {
            return output;
        }
        if context.query.is_empty() {
            output.results.extend(
                context
                    .projects
                    .iter()
                    .map(|project| result(self.metadata(), project_result(project), None)),
            );
            return output;
        }

        let pattern = fuzzy_pattern(context.query);
        let mut matcher = fuzzy_matcher();
        let mut char_buf = Vec::new();
        for project in context.projects {
            let haystack = Utf32Str::new(&project.name, &mut char_buf);
            if let Some(score) = pattern.score(haystack, &mut matcher) {
                output.results.push(result(
                    self.metadata(),
                    project_result(project),
                    Some(score),
                ));
            }
        }
        output
    }
}

impl Provider for AppsProvider {
    fn metadata(&self) -> &'static ProviderMetadata {
        &APPS_METADATA
    }

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig {
        ProviderConfig::new(context.legacy_config.apps)
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut output = outcome(self.metadata());
        if !self.config(context).enabled {
            return output;
        }
        if context.query.is_empty() {
            output.results.extend(
                context
                    .apps
                    .iter()
                    .map(|app| result(self.metadata(), app_result(app), None)),
            );
            return output;
        }

        let pattern = fuzzy_pattern(context.query);
        let mut matcher = fuzzy_matcher();
        let mut char_buf = Vec::new();
        for app in context.apps {
            if let Some(score) = app_match_score(app, &pattern, &mut matcher, &mut char_buf) {
                output
                    .results
                    .push(result(self.metadata(), app_result(app), Some(score)));
            }
        }
        output
    }
}

impl Provider for AliasesProvider {
    fn metadata(&self) -> &'static ProviderMetadata {
        &ALIASES_METADATA
    }

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig {
        ProviderConfig::new(context.legacy_config.aliases)
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut output = outcome(self.metadata());
        if !self.config(context).enabled {
            return output;
        }
        if context.query.is_empty() {
            output.results.extend(
                context
                    .aliases
                    .iter()
                    .map(|alias| result(self.metadata(), alias_result(alias), None)),
            );
            return output;
        }

        let pattern = fuzzy_pattern(context.query);
        let mut matcher = fuzzy_matcher();
        let mut char_buf = Vec::new();
        for alias in context.aliases {
            let name_score = score_text(&alias.name, &pattern, &mut matcher, &mut char_buf);
            let query_score = score_text(&alias.query, &pattern, &mut matcher, &mut char_buf);
            if let Some(score) = name_score.max(query_score) {
                output
                    .results
                    .push(result(self.metadata(), alias_result(alias), Some(score)));
            }
        }
        output
    }
}

fn app_match_score(
    app: &crate::apps::DesktopApp,
    pattern: &nucleo_matcher::pattern::Pattern,
    matcher: &mut nucleo_matcher::Matcher,
    char_buf: &mut Vec<char>,
) -> Option<u32> {
    let mut score = score_text(&app.name, pattern, matcher, char_buf);

    for term in app
        .localized_names
        .iter()
        .chain(app.keywords.iter())
        .map(String::as_str)
        .chain(app.generic_name.as_deref())
        .chain(app.comment.as_deref())
    {
        score = score.max(score_text(term, pattern, matcher, char_buf));
    }

    score
}

fn score_text(
    text: &str,
    pattern: &nucleo_matcher::pattern::Pattern,
    matcher: &mut nucleo_matcher::Matcher,
    char_buf: &mut Vec<char>,
) -> Option<u32> {
    let haystack = Utf32Str::new(text, char_buf);
    pattern.score(haystack, matcher)
}
