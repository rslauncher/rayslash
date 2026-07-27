pub(crate) mod matcher;
pub(crate) mod providers;
mod result;

use crate::apps::DesktopApp;
use crate::config::{AliasConfig, ProviderConfig, WebSearchConfig};
use crate::projects::Project;
use crate::providers::{
    ProviderAction, ProviderContext, ProviderExecutionHint, ProviderId, builtin_providers,
    query_execution_hint as provider_query_execution_hint,
};
use crate::ranking::RankingState;

use matcher::{boosted_score, search_result_order};
use providers::{disabled_providers_result, no_results, placeholder_results_for_providers};
pub use providers::{display_path, placeholder_results, project_results};
#[cfg(test)]
use providers::{display_path_for_home, project_result_with_subtitle};
pub use result::{ModuleAction, SearchResult, SearchResultIcon, SearchResultKind};

pub fn mixed_results(projects: &[Project], apps: &[DesktopApp], query: &str) -> Vec<SearchResult> {
    mixed_results_with_aliases(projects, apps, &[], query)
}

pub fn mixed_results_with_aliases(
    projects: &[Project],
    apps: &[DesktopApp],
    aliases: &[AliasConfig],
    query: &str,
) -> Vec<SearchResult> {
    mixed_results_with_ranking(
        projects,
        apps,
        aliases,
        query,
        &ProviderConfig::default(),
        None,
    )
}

pub fn mixed_results_with_providers(
    projects: &[Project],
    apps: &[DesktopApp],
    query: &str,
    providers: &ProviderConfig,
) -> Vec<SearchResult> {
    mixed_results_with_ranking(projects, apps, &[], query, providers, None)
}

pub fn mixed_results_with_ranking(
    projects: &[Project],
    apps: &[DesktopApp],
    aliases: &[AliasConfig],
    query: &str,
    providers: &ProviderConfig,
    ranking: Option<&RankingState>,
) -> Vec<SearchResult> {
    mixed_results_with_ranking_and_web_searches(
        projects,
        apps,
        aliases,
        &[],
        query,
        providers,
        ranking,
    )
}

pub fn mixed_results_with_ranking_and_web_searches(
    projects: &[Project],
    apps: &[DesktopApp],
    aliases: &[AliasConfig],
    web_searches: &[WebSearchConfig],
    query: &str,
    providers: &ProviderConfig,
    ranking: Option<&RankingState>,
) -> Vec<SearchResult> {
    mixed_results_with_ranking_and_web_searches_impl(
        projects,
        apps,
        aliases,
        web_searches,
        query,
        providers,
        ranking,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn mixed_results_with_ranking_and_web_searches_limited(
    projects: &[Project],
    apps: &[DesktopApp],
    aliases: &[AliasConfig],
    web_searches: &[WebSearchConfig],
    query: &str,
    providers: &ProviderConfig,
    ranking: Option<&RankingState>,
    limit: usize,
) -> Vec<SearchResult> {
    mixed_results_with_ranking_and_web_searches_impl(
        projects,
        apps,
        aliases,
        web_searches,
        query,
        providers,
        ranking,
        Some(limit),
    )
}

#[allow(clippy::too_many_arguments)]
fn mixed_results_with_ranking_and_web_searches_impl(
    projects: &[Project],
    apps: &[DesktopApp],
    aliases: &[AliasConfig],
    web_searches: &[WebSearchConfig],
    query: &str,
    providers: &ProviderConfig,
    ranking: Option<&RankingState>,
    limit: Option<usize>,
) -> Vec<SearchResult> {
    let query = query.trim();
    let context = ProviderContext {
        query,
        projects,
        apps,
        aliases,
        web_searches,
        legacy_config: providers,
        ranking,
        result_limit: limit,
    };
    let provider_registry = builtin_providers();

    if !provider_registry
        .iter()
        .any(|provider| provider.config(&context).enabled)
    {
        return vec![disabled_providers_result()];
    }

    let mut outcomes = Vec::with_capacity(provider_registry.len());
    for provider in provider_registry {
        let output = provider.run(&context);
        outcomes.push(output);
    }

    let data_sources_empty = (!provider_enabled(&context, &ProviderId::CORE_FOLDERS)
        || projects.is_empty())
        && (!provider_enabled(&context, &ProviderId::CORE_APPS) || apps.is_empty());
    if data_sources_empty {
        let results = outcomes
            .into_iter()
            .flat_map(|outcome| outcome.results)
            .map(|provider_result| provider_result.result)
            .collect::<Vec<_>>();
        return if query.is_empty() || results.is_empty() {
            placeholder_results_for_providers(providers)
        } else {
            results
        };
    }

    if query.is_empty() {
        let mut results = outcomes
            .into_iter()
            .flat_map(|outcome| outcome.results)
            .map(|provider_result| provider_result.result)
            .collect::<Vec<_>>();
        sort_results_with_limit(&mut results, limit);
        return results;
    }

    if outcomes.iter().any(|outcome| outcome.exclusive) {
        return outcomes
            .into_iter()
            .filter(|outcome| !is_data_provider(&outcome.provider_id))
            .flat_map(|outcome| outcome.results)
            .map(|provider_result| provider_result.result)
            .collect();
    }

    let mut matches = Vec::new();
    let mut priority_results = Vec::new();
    for provider_result in outcomes.into_iter().flat_map(|outcome| outcome.results) {
        if let Some(score) = provider_result.match_score {
            let ranking_eligible = provider_result.ranking_eligible
                && matches!(
                    provider_result.action,
                    ProviderAction::LaunchApp { .. } | ProviderAction::OpenFolder(_)
                );
            let boosted_score = if ranking_eligible {
                boosted_score(&provider_result.result, score, query, ranking)
            } else {
                score
            };
            matches.push((provider_result.result, score, boosted_score));
        } else {
            priority_results.push(provider_result.result);
        }
    }

    let ranked_order =
        |(a, a_score, a_boosted_score): &(SearchResult, u32, u32),
         (b, b_score, b_boosted_score): &(SearchResult, u32, u32)| {
            b_boosted_score
                .cmp(a_boosted_score)
                .then_with(|| b_score.cmp(a_score))
                .then_with(|| search_result_order(a, b))
        };
    let ranked_limit = limit.map(|limit| limit.saturating_sub(priority_results.len()));
    if let Some(limit) = ranked_limit
        && matches.len() > limit
    {
        if limit == 0 {
            matches.clear();
        } else {
            matches.select_nth_unstable_by(limit, ranked_order);
            matches.truncate(limit);
        }
    }
    matches.sort_by(ranked_order);

    let mut ranked_results = matches
        .into_iter()
        .map(|(result, _score, _boosted_score)| result)
        .collect::<Vec<_>>();

    priority_results.append(&mut ranked_results);
    let mut results = priority_results;
    if let Some(limit) = limit {
        results.truncate(limit);
    }

    if results.is_empty() {
        return vec![no_results(query, providers)];
    }

    results
}

fn sort_results_with_limit(results: &mut Vec<SearchResult>, limit: Option<usize>) {
    if let Some(limit) = limit
        && results.len() > limit
    {
        if limit == 0 {
            results.clear();
            return;
        }
        results.select_nth_unstable_by(limit, search_result_order);
        results.truncate(limit);
    }
    results.sort_by(search_result_order);
}

pub fn query_execution_hint(query: &str, providers: &ProviderConfig) -> ProviderExecutionHint {
    provider_query_execution_hint(&ProviderContext {
        query: query.trim(),
        projects: &[],
        apps: &[],
        aliases: &[],
        web_searches: &[],
        legacy_config: providers,
        ranking: None,
        result_limit: None,
    })
}

fn provider_enabled(context: &ProviderContext<'_>, id: &ProviderId) -> bool {
    builtin_providers()
        .iter()
        .find(|provider| &provider.metadata().id == id)
        .is_some_and(|provider| provider.config(context).enabled)
}

fn is_data_provider(id: &ProviderId) -> bool {
    matches!(
        id,
        id if id == &ProviderId::CORE_APPS
            || id == &ProviderId::CORE_FOLDERS
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn display_path_shortens_paths_under_home() {
        let home = PathBuf::from("/home/example");
        let path = home.join("Documents/Projects/rayslash");

        assert_eq!(
            display_path_for_home(&path, &home),
            "~/Documents/Projects/rayslash"
        );
    }

    #[test]
    fn display_path_shortens_home_itself() {
        let home = PathBuf::from("/home/example");

        assert_eq!(display_path_for_home(&home, &home), "~");
    }

    #[test]
    fn display_path_keeps_paths_outside_home_unchanged() {
        let home = PathBuf::from("/home/example");
        let path = PathBuf::from("/tmp/rayslash");

        assert_eq!(display_path_for_home(&path, &home), "/tmp/rayslash");
    }

    #[test]
    fn project_results_use_shortened_subtitle_without_changing_launch_path() {
        let home = PathBuf::from("/home/example");
        let path = home.join("Projects/rayslash");
        let project = Project {
            name: "rayslash".to_owned(),
            path: path.clone(),
        };

        let result = project_result_with_subtitle(&project, display_path_for_home(&path, &home));

        assert_eq!(result.subtitle, "~/Projects/rayslash");
        assert_eq!(result.project_path(), Some(path.as_path()));
        assert_eq!(result.icon, SearchResultIcon::ProjectFolder);
    }

    #[test]
    fn limited_search_is_the_exact_prefix_of_full_ranking() {
        let projects = ["Zulu", "Alpha", "Echo", "Bravo", "Delta"]
            .into_iter()
            .map(|name| Project {
                name: name.to_owned(),
                path: PathBuf::from("/tmp").join(name),
            })
            .collect::<Vec<_>>();
        let providers = ProviderConfig::default();

        for query in ["", "a"] {
            let full = mixed_results_with_ranking_and_web_searches(
                &projects,
                &[],
                &[],
                &[],
                query,
                &providers,
                None,
            );
            let limited = mixed_results_with_ranking_and_web_searches_limited(
                &projects,
                &[],
                &[],
                &[],
                query,
                &providers,
                None,
                3,
            );
            assert_eq!(limited, full.into_iter().take(3).collect::<Vec<_>>());
        }
    }
}
