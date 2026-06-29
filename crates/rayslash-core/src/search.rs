#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
}

pub fn placeholder_results() -> Vec<SearchResult> {
    vec![
        SearchResult {
            title: "Open applications".to_owned(),
            subtitle: "Desktop app search will land in Phase 3".to_owned(),
        },
        SearchResult {
            title: "Find projects".to_owned(),
            subtitle: "Project folder search will land in Phase 2".to_owned(),
        },
        SearchResult {
            title: "Calculate".to_owned(),
            subtitle: "Calculator support will land in Phase 4".to_owned(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_results_are_available() {
        assert_eq!(placeholder_results().len(), 3);
    }
}
