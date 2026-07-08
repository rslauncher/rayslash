mod matcher;
mod providers;
mod result;

use crate::apps::DesktopApp;
use crate::calc;
use crate::config::{AliasConfig, ProviderConfig, WebSearchConfig};
use crate::currency;
use crate::projects::Project;
use crate::ranking::RankingState;
use crate::{time_lookup, units, utility_actions, web_search};

use matcher::{boosted_score, fuzzy_matcher, fuzzy_pattern, search_result_order};
use nucleo_matcher::Utf32Str;
use providers::{
    alias_result, app_result, calculator_result, currency_conversion_result, currency_error_result,
    default_web_search_result, disabled_providers_result, no_results,
    placeholder_results_for_providers, project_result, time_lookup_error_result,
    time_lookup_result, unit_conversion_result, utility_action_error_result, utility_action_result,
    web_search_result,
};
pub use providers::{display_path, placeholder_results, project_results};
#[cfg(test)]
use providers::{display_path_for_home, project_result_with_subtitle};
pub use result::{SearchResult, SearchResultIcon, SearchResultKind};

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
    let query = query.trim();
    let mut utility_results = utility_results(query, providers, web_searches);

    if !providers.apps
        && !providers.folders
        && !providers.calculator
        && !providers.aliases
        && !providers.web_search
        && !providers.unit_conversion
        && !providers.currency_conversion
        && !providers.time_lookup
        && utility_results.is_empty()
    {
        return vec![disabled_providers_result()];
    }

    let enabled_projects = if providers.folders { projects } else { &[] };
    let enabled_apps = if providers.apps { apps } else { &[] };
    let enabled_aliases = if providers.aliases { aliases } else { &[] };
    if enabled_projects.is_empty() && enabled_apps.is_empty() && enabled_aliases.is_empty() {
        return if query.is_empty() || utility_results.is_empty() {
            placeholder_results_for_providers(providers)
        } else {
            utility_results
        };
    }

    if query.is_empty() {
        let mut results = enabled_projects
            .iter()
            .map(project_result)
            .chain(enabled_apps.iter().map(app_result))
            .chain(enabled_aliases.iter().map(alias_result))
            .collect::<Vec<_>>();
        results.sort_by(search_result_order);
        return results;
    }

    let pattern = fuzzy_pattern(query);
    let mut matcher = fuzzy_matcher();
    let mut char_buf = Vec::new();

    let mut matches = Vec::new();

    for project in enabled_projects {
        let haystack = Utf32Str::new(&project.name, &mut char_buf);
        if let Some(score) = pattern.score(haystack, &mut matcher) {
            let result = project_result(project);
            let boosted_score = boosted_score(&result, score, query, ranking);
            matches.push((result, score, boosted_score));
        }
    }

    for app in enabled_apps {
        if let Some(score) = app_match_score(app, &pattern, &mut matcher, &mut char_buf) {
            let result = app_result(app);
            let boosted_score = boosted_score(&result, score, query, ranking);
            matches.push((result, score, boosted_score));
        }
    }

    for alias in enabled_aliases {
        let name_score = {
            let haystack = Utf32Str::new(&alias.name, &mut char_buf);
            pattern.score(haystack, &mut matcher)
        };
        let query_score = {
            let haystack = Utf32Str::new(&alias.query, &mut char_buf);
            pattern.score(haystack, &mut matcher)
        };
        if let Some(score) = name_score.max(query_score) {
            let result = alias_result(alias);
            matches.push((result, score, score));
        }
    }

    matches.sort_by(
        |(a, a_score, a_boosted_score), (b, b_score, b_boosted_score)| {
            b_boosted_score
                .cmp(a_boosted_score)
                .then_with(|| b_score.cmp(a_score))
                .then_with(|| search_result_order(a, b))
        },
    );

    let mut results = matches
        .into_iter()
        .map(|(result, _score, _boosted_score)| result)
        .collect::<Vec<_>>();

    utility_results.append(&mut results);
    let results = utility_results;

    if results.is_empty() {
        return vec![no_results(query, providers)];
    }

    results
}

fn utility_results(
    query: &str,
    providers: &ProviderConfig,
    web_searches: &[WebSearchConfig],
) -> Vec<SearchResult> {
    if query.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut suppress_calculator = false;

    if let Some(action) = utility_actions::parse_query(query) {
        suppress_calculator = true;
        match action {
            Ok(action) => results.push(utility_action_result(action)),
            Err(error) => results.push(utility_action_error_result(
                &error.expression,
                error.message,
            )),
        }
    }

    if providers.unit_conversion
        && let Some(conversion) = units::convert_query(query)
    {
        suppress_calculator = true;
        results.push(unit_conversion_result(conversion));
    } else if units::looks_like_conversion_query(query) {
        suppress_calculator = true;
    }

    if let Some(request) = currency::parse_query(query) {
        suppress_calculator = true;
        if providers.currency_conversion {
            match currency::convert_request(&request) {
                Ok(conversion) => results.push(currency_conversion_result(conversion)),
                Err(error) => results.push(currency_error_result(
                    &request.expression,
                    error.to_string(),
                )),
            }
        }
    }

    if let Some(request) = time_lookup::parse_query(query) {
        suppress_calculator = true;
        if providers.time_lookup {
            match time_lookup::lookup_request(&request) {
                Ok(lookup) => results.push(time_lookup_result(lookup)),
                Err(error) => results.push(time_lookup_error_result(
                    &request.expression,
                    error.to_string(),
                )),
            }
        }
    }

    if providers.web_search {
        let web_result_count_before = results.len();
        let custom_searches = web_search::matching_web_searches(web_searches, query)
            .into_iter()
            .map(web_search_result)
            .collect::<Vec<_>>();
        let has_custom_search = !custom_searches.is_empty();
        results.extend(custom_searches);

        if !has_custom_search && let Some(search_terms) = web_search::default_search_terms(query) {
            results.push(default_web_search_result(search_terms));
        }

        if results.len() > web_result_count_before {
            suppress_calculator = true;
        }
    }

    if providers.calculator
        && !suppress_calculator
        && let Some(calculation) = calc::calculate(query)
    {
        results.push(calculator_result(calculation));
    }

    results
}

fn app_match_score(
    app: &DesktopApp,
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
}
