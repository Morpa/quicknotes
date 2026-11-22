//! Small fuzzy matcher for the command palette: case-insensitive subsequence
//! matching that favors consecutive runs and word starts.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub score: i32,
    /// Char indices of `text` that matched, for highlighting.
    pub indices: Vec<usize>,
}

/// Returns `None` when `query` is not a subsequence of `text`. An empty query
/// matches everything with score 0.
pub fn fuzzy_match(query: &str, text: &str) -> Option<Match> {
    let needle: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if needle.is_empty() {
        return Some(Match {
            score: 0,
            indices: Vec::new(),
        });
    }

    let chars: Vec<char> = text.chars().collect();
    let lowered: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();

    let mut indices = Vec::with_capacity(needle.len());
    let mut score = 0;
    let mut position = 0;
    let mut previous: Option<usize> = None;

    for target in needle {
        let found = (position..lowered.len()).find(|&index| lowered[index] == target)?;

        score += 10;
        if previous.is_some_and(|last| last + 1 == found) {
            score += 15;
        }
        if found == 0 || !chars[found - 1].is_alphanumeric() {
            score += 12;
        }
        // Gaps cost a little, so tighter matches win.
        if let Some(last) = previous {
            score -= (found - last - 1).min(8) as i32;
        } else {
            score -= found.min(8) as i32;
        }

        indices.push(found);
        previous = Some(found);
        position = found + 1;
    }

    Some(Match { score, indices })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_a_subsequence() {
        assert!(fuzzy_match("nwi", "New item").is_some());
        assert!(fuzzy_match("xyz", "New item").is_none());
        assert!(fuzzy_match("tn", "New item").is_none());
    }

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(fuzzy_match("", "anything").unwrap().score, 0);
    }

    #[test]
    fn is_case_insensitive_and_reports_indices() {
        let found = fuzzy_match("NI", "New item").unwrap();
        assert_eq!(found.indices, vec![0, 4]);
    }

    #[test]
    fn prefers_consecutive_and_word_start_matches() {
        let tight = fuzzy_match("sort", "Cycle sort order").unwrap();
        let loose = fuzzy_match("sort", "Show all tags or tools").unwrap();
        assert!(tight.score > loose.score);
    }
}
