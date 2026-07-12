use crate::projects::Project;
use crate::ranking::RankingState;
use nucleo_matcher::{
    Config, Matcher,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use super::{SearchResult, SearchResultKind};

pub(crate) fn boosted_score(
    result: &SearchResult,
    score: u32,
    query: &str,
    ranking: Option<&RankingState>,
) -> u32 {
    let Some(ranking) = ranking else {
        return score;
    };

    result
        .learning_id()
        .map(|id| {
            let boost = if title_starts_with_query(&result.title, query) {
                ranking.boost_for(&id, query)
            } else {
                0
            };
            score.saturating_add(boost)
        })
        .unwrap_or(score)
}

pub(super) fn project_order(a: &Project, b: &Project) -> std::cmp::Ordering {
    a.name
        .to_lowercase()
        .cmp(&b.name.to_lowercase())
        .then_with(|| a.path.cmp(&b.path))
}

pub(crate) fn search_result_order(a: &SearchResult, b: &SearchResult) -> std::cmp::Ordering {
    a.title
        .to_lowercase()
        .cmp(&b.title.to_lowercase())
        .then_with(|| result_type_order(&a.kind).cmp(&result_type_order(&b.kind)))
        .then_with(|| a.subtitle.cmp(&b.subtitle))
}

pub(crate) fn fuzzy_pattern(query: &str) -> Pattern {
    Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    )
}

pub(crate) fn fuzzy_matcher() -> Matcher {
    let mut config = Config::DEFAULT;
    config.prefer_prefix = true;
    Matcher::new(config)
}

fn title_starts_with_query(title: &str, query: &str) -> bool {
    let query = query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    if query.is_empty() {
        return false;
    }
    let title = title.to_lowercase();
    if title.starts_with(&query) {
        return true;
    }
    let compact_title = title
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect::<String>();
    let compact_query = query
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect::<String>();
    compact_title.starts_with(&compact_query)
}

fn result_type_order(kind: &SearchResultKind) -> u8 {
    match kind {
        SearchResultKind::Module { .. } => 0,
        SearchResultKind::App { .. } => 1,
        SearchResultKind::Project { .. } => 2,
        SearchResultKind::Placeholder | SearchResultKind::NoResults { .. } => 4,
    }
}
