use nucleo_matcher::Utf32Str;

use crate::search::{
    matcher::{fuzzy_matcher, fuzzy_pattern},
    providers::{app_result, project_result},
};

use super::{
    Provider, ProviderConfig, ProviderContext, ProviderId, ProviderMetadata, ProviderOutcome,
    ProviderPermissions, ProviderResult,
};

struct FoldersProvider;
struct AppsProvider;

static FOLDERS_PROVIDER: FoldersProvider = FoldersProvider;
static APPS_PROVIDER: AppsProvider = AppsProvider;

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

static PROVIDERS: [&'static dyn Provider; 2] = [&FOLDERS_PROVIDER, &APPS_PROVIDER];
static CATALOG: [&ProviderMetadata; 2] = [&APPS_METADATA, &FOLDERS_METADATA];

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
