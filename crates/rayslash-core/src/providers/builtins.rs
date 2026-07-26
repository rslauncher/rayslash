use nucleo_matcher::Utf32Str;
use std::thread;

use crate::search::{
    display_path,
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
            output.results = empty_project_results(context, self.metadata());
            return output;
        }

        if context.projects.len() >= 10_000 {
            output.results = parallel_project_results(context, self.metadata());
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
            output.results = empty_app_results(context, self.metadata());
            return output;
        }

        if context.apps.len() >= 10_000 {
            output.results = parallel_app_results(context, self.metadata());
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

fn empty_project_results(
    context: &ProviderContext<'_>,
    metadata: &'static ProviderMetadata,
) -> Vec<ProviderResult> {
    let Some(limit) = context.result_limit else {
        return context
            .projects
            .iter()
            .map(|project| result(metadata, project_result(project), None))
            .collect();
    };
    let mut projects = context
        .projects
        .iter()
        .enumerate()
        .map(|(index, project)| {
            (
                project,
                project.name.to_lowercase(),
                display_path(&project.path),
                index,
            )
        })
        .collect::<Vec<_>>();
    let order = |(_, a_name, a_subtitle, a_index): &(_, String, String, usize),
                 (_, b_name, b_subtitle, b_index): &(_, String, String, usize)| {
        a_name
            .cmp(b_name)
            .then_with(|| a_subtitle.cmp(b_subtitle))
            .then_with(|| a_index.cmp(b_index))
    };
    select_prefix(&mut projects, limit, order);
    projects
        .into_iter()
        .map(|(project, _, _, _)| result(metadata, project_result(project), None))
        .collect()
}

fn empty_app_results(
    context: &ProviderContext<'_>,
    metadata: &'static ProviderMetadata,
) -> Vec<ProviderResult> {
    let Some(limit) = context.result_limit else {
        return context
            .apps
            .iter()
            .map(|app| result(metadata, app_result(app), None))
            .collect();
    };
    let mut apps = context
        .apps
        .iter()
        .enumerate()
        .map(|(index, app)| {
            (
                app,
                app.name.to_lowercase(),
                app_result_subtitle(app),
                index,
            )
        })
        .collect::<Vec<_>>();
    let order = |(_, a_name, a_subtitle, a_index): &(_, String, &str, usize),
                 (_, b_name, b_subtitle, b_index): &(_, String, &str, usize)| {
        a_name
            .cmp(b_name)
            .then_with(|| a_subtitle.cmp(b_subtitle))
            .then_with(|| a_index.cmp(b_index))
    };
    select_prefix(&mut apps, limit, order);
    apps.into_iter()
        .map(|(app, _, _, _)| result(metadata, app_result(app), None))
        .collect()
}

fn select_prefix<T>(
    values: &mut Vec<T>,
    limit: usize,
    mut order: impl FnMut(&T, &T) -> std::cmp::Ordering + Copy,
) {
    if values.len() > limit {
        if limit == 0 {
            values.clear();
            return;
        }
        values.select_nth_unstable_by(limit, order);
        values.truncate(limit);
    }
    values.sort_by(&mut order);
}

fn parallel_project_results(
    context: &ProviderContext<'_>,
    metadata: &'static ProviderMetadata,
) -> Vec<ProviderResult> {
    let workers = search_worker_count(context.projects.len());
    let chunk_size = context.projects.len().div_ceil(workers);
    thread::scope(|scope| {
        context
            .projects
            .chunks(chunk_size)
            .map(|projects| {
                scope.spawn(move || {
                    let pattern = fuzzy_pattern(context.query);
                    let mut matcher = fuzzy_matcher();
                    let mut char_buf = Vec::new();
                    projects
                        .iter()
                        .filter_map(|project| {
                            let haystack = Utf32Str::new(&project.name, &mut char_buf);
                            pattern
                                .score(haystack, &mut matcher)
                                .map(|score| result(metadata, project_result(project), Some(score)))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    })
}

fn parallel_app_results(
    context: &ProviderContext<'_>,
    metadata: &'static ProviderMetadata,
) -> Vec<ProviderResult> {
    let workers = search_worker_count(context.apps.len());
    let chunk_size = context.apps.len().div_ceil(workers);
    let candidate_limit = context.result_limit.filter(|_| {
        context
            .ranking
            .is_none_or(|ranking| ranking.entries.is_empty())
    });
    thread::scope(|scope| {
        context
            .apps
            .chunks(chunk_size)
            .map(|apps| {
                scope.spawn(move || {
                    let pattern = fuzzy_pattern(context.query);
                    let mut matcher = fuzzy_matcher();
                    let mut char_buf = Vec::new();
                    let mut matches = apps
                        .iter()
                        .filter_map(|app| {
                            app_match_score(app, &pattern, &mut matcher, &mut char_buf)
                                .map(|score| (app, score))
                        })
                        .collect::<Vec<_>>();
                    limit_app_matches(&mut matches, candidate_limit);
                    matches
                        .into_iter()
                        .map(|(app, score)| result(metadata, app_result(app), Some(score)))
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    })
}

fn limit_app_matches(matches: &mut Vec<(&crate::apps::DesktopApp, u32)>, limit: Option<usize>) {
    let Some(limit) = limit else {
        return;
    };
    let order = |(a, a_score): &(&crate::apps::DesktopApp, u32),
                 (b, b_score): &(&crate::apps::DesktopApp, u32)| {
        b_score
            .cmp(a_score)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| app_result_subtitle(a).cmp(app_result_subtitle(b)))
    };
    if matches.len() > limit {
        matches.select_nth_unstable_by(limit, order);
        matches.truncate(limit);
    }
    matches.sort_by(order);
}

fn app_result_subtitle(app: &crate::apps::DesktopApp) -> &str {
    app.comment
        .as_deref()
        .or(app.generic_name.as_deref())
        .unwrap_or("Application")
}

fn search_worker_count(item_count: usize) -> usize {
    thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8)
        .min(item_count.max(1))
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
