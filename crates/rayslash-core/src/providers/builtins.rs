use nucleo_matcher::Utf32Str;
use std::thread;

use crate::search::{
    display_path,
    matcher::{fuzzy_matcher, fuzzy_pattern, title_starts_with_query},
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

        output.results = project_results_for_slice(context.projects, context, self.metadata());
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

        output.results = app_results_for_slice(context.apps, context, self.metadata());
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
    let order = |(a, a_index): &(&crate::projects::Project, usize),
                 (b, b_index): &(&crate::projects::Project, usize)| {
        cmp_case_folded(&a.name, &b.name)
            .then_with(|| display_path(&a.path).cmp(&display_path(&b.path)))
            .then_with(|| a_index.cmp(b_index))
    };
    let projects = bounded_prefix(
        context
            .projects
            .iter()
            .enumerate()
            .map(|(index, project)| (project, index)),
        limit,
        order,
    );
    projects
        .into_iter()
        .map(|(project, _)| result(metadata, project_result(project), None))
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
    let order =
        |(a, a_subtitle, a_index): &(&crate::apps::DesktopApp, &str, usize),
         (b, b_subtitle, b_index): &(&crate::apps::DesktopApp, &str, usize)| {
            cmp_case_folded(&a.name, &b.name)
                .then_with(|| a_subtitle.cmp(b_subtitle))
                .then_with(|| a_index.cmp(b_index))
        };
    let apps = bounded_prefix(
        context
            .apps
            .iter()
            .enumerate()
            .map(|(index, app)| (app, app_result_subtitle(app), index)),
        limit,
        order,
    );
    apps.into_iter()
        .map(|(app, _, _)| result(metadata, app_result(app), None))
        .collect()
}

fn cmp_case_folded(left: &str, right: &str) -> std::cmp::Ordering {
    if left.is_ascii() && right.is_ascii() {
        for (left, right) in left.bytes().zip(right.bytes()) {
            match left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase()) {
                std::cmp::Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        return left.len().cmp(&right.len());
    }
    left.chars()
        .flat_map(char::to_lowercase)
        .cmp(right.chars().flat_map(char::to_lowercase))
}

fn bounded_prefix<T>(
    values: impl IntoIterator<Item = T>,
    limit: usize,
    mut order: impl FnMut(&T, &T) -> std::cmp::Ordering + Copy,
) -> Vec<T> {
    if limit == 0 {
        return Vec::new();
    }
    let mut selected = Vec::with_capacity(limit);
    for value in values {
        if selected.len() < limit {
            selected.push(value);
            let mut index = selected.len() - 1;
            while index > 0 {
                let parent = (index - 1) / 2;
                if !order(&selected[parent], &selected[index]).is_lt() {
                    break;
                }
                selected.swap(parent, index);
                index = parent;
            }
            continue;
        }
        if order(&value, &selected[0]).is_lt() {
            selected[0] = value;
            let mut parent = 0;
            loop {
                let left = parent * 2 + 1;
                if left >= selected.len() {
                    break;
                }
                let right = left + 1;
                let child =
                    if right < selected.len() && order(&selected[left], &selected[right]).is_lt() {
                        right
                    } else {
                        left
                    };
                if !order(&selected[parent], &selected[child]).is_lt() {
                    break;
                }
                selected.swap(parent, child);
                parent = child;
            }
        }
    }
    selected.sort_by(&mut order);
    selected
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
                scope.spawn(move || project_results_for_slice(projects, context, metadata))
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
    thread::scope(|scope| {
        context
            .apps
            .chunks(chunk_size)
            .map(|apps| scope.spawn(move || app_results_for_slice(apps, context, metadata)))
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    })
}

fn project_results_for_slice(
    projects: &[crate::projects::Project],
    context: &ProviderContext<'_>,
    metadata: &'static ProviderMetadata,
) -> Vec<ProviderResult> {
    let pattern = fuzzy_pattern(context.query);
    let mut matcher = fuzzy_matcher();
    let mut char_buf = Vec::new();
    let mut matches = projects
        .iter()
        .filter_map(|project| {
            let haystack = Utf32Str::new(&project.name, &mut char_buf);
            let score = pattern.score(haystack, &mut matcher)?;
            let boosted = candidate_boost(
                score,
                &project.name,
                || format!("folder:{}", project.path.display()),
                context,
            );
            Some((project, score, boosted))
        })
        .collect::<Vec<_>>();
    let order = |(a, a_score, a_boosted): &(&crate::projects::Project, u32, u32),
                 (b, b_score, b_boosted): &(&crate::projects::Project, u32, u32)| {
        b_boosted
            .cmp(a_boosted)
            .then_with(|| b_score.cmp(a_score))
            .then_with(|| cmp_case_folded(&a.name, &b.name))
            .then_with(|| display_path(&a.path).cmp(&display_path(&b.path)))
    };
    limit_matches(&mut matches, context.result_limit, order);
    matches
        .into_iter()
        .map(|(project, score, _)| result(metadata, project_result(project), Some(score)))
        .collect()
}

fn app_results_for_slice(
    apps: &[crate::apps::DesktopApp],
    context: &ProviderContext<'_>,
    metadata: &'static ProviderMetadata,
) -> Vec<ProviderResult> {
    let pattern = fuzzy_pattern(context.query);
    let mut matcher = fuzzy_matcher();
    let mut char_buf = Vec::new();
    let mut matches = apps
        .iter()
        .filter_map(|app| {
            let score = app_match_score(app, &pattern, &mut matcher, &mut char_buf)?;
            let boosted = candidate_boost(score, &app.name, || format!("app:{}", app.id), context);
            Some((app, score, boosted))
        })
        .collect::<Vec<_>>();
    let order = |(a, a_score, a_boosted): &(&crate::apps::DesktopApp, u32, u32),
                 (b, b_score, b_boosted): &(&crate::apps::DesktopApp, u32, u32)| {
        b_boosted
            .cmp(a_boosted)
            .then_with(|| b_score.cmp(a_score))
            .then_with(|| cmp_case_folded(&a.name, &b.name))
            .then_with(|| app_result_subtitle(a).cmp(app_result_subtitle(b)))
    };
    limit_matches(&mut matches, context.result_limit, order);
    matches
        .into_iter()
        .map(|(app, score, _)| result(metadata, app_result(app), Some(score)))
        .collect()
}

fn candidate_boost(
    score: u32,
    title: &str,
    learning_id: impl FnOnce() -> String,
    context: &ProviderContext<'_>,
) -> u32 {
    let Some(ranking) = context.ranking else {
        return score;
    };
    if ranking.entries.is_empty() || !title_starts_with_query(title, context.query) {
        return score;
    }
    score.saturating_add(ranking.boost_for(&learning_id(), context.query))
}

fn limit_matches<T>(
    matches: &mut Vec<T>,
    limit: Option<usize>,
    mut order: impl FnMut(&T, &T) -> std::cmp::Ordering + Copy,
) {
    let Some(limit) = limit else {
        return;
    };
    if matches.len() > limit {
        if limit == 0 {
            matches.clear();
            return;
        }
        matches.select_nth_unstable_by(limit, order);
        matches.truncate(limit);
    }
    matches.sort_by(&mut order);
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
