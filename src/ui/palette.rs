/// Fuzzy matching for the command palette: subsequence matching with
/// bonuses for prefixes, word boundaries, and consecutive runs, plus
/// per-character match indices for highlight rendering.
/// Score `query` against `text`. Returns the score (higher is better) and
/// the byte-mapped character indices in `text` that matched, or `None`
/// when `query` is not a subsequence of `text`. Matching is case-insensitive.
pub fn fuzzy_match(query: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }

    let q: Vec<char> = query.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    // Original-case chars for exact-case bonus.
    let t_raw: Vec<char> = text.chars().collect();
    let q_raw: Vec<char> = query.chars().collect();

    let mut matched: Vec<usize> = Vec::with_capacity(q.len());
    let mut cursor = 0usize;
    for qc in &q {
        let mut found = None;
        for (i, tc) in t.iter().enumerate().skip(cursor) {
            if tc == qc {
                found = Some(i);
                break;
            }
        }
        let i = found?;
        matched.push(i);
        cursor = i + 1;
    }

    let mut score: i32 = 1000;
    // Prefer matches that start early and hitlean short titles.
    score -= *matched.first().unwrap_or(&0) as i32 * 8;
    score -= t.len() as i32 * 2;
    // Exact-substring hits outrank scattered ones.
    if text.to_lowercase().contains(&query.to_lowercase()) {
        score += 200;
    }
    for (k, &idx) in matched.iter().enumerate() {
        let prev_boundary = idx == 0
            || matches!(
                t[idx - 1],
                '/' | ' ' | '-' | '_' | '.' | '\\' | '(' | ':'
            );
        if prev_boundary {
            score += 30;
        }
        if k > 0 && matched[k - 1] + 1 == idx {
            score += 45; // consecutive run
        }
        if t_raw.get(idx) == q_raw.get(k) {
            score += 5; // exact case
        }
    }

    Some((score, matched))
}

/// Zed-style query normalization: trim, collapse whitespace runs and
/// double colons, underscores become spaces.
pub fn normalize_query(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_char: Option<char> = None;
    for raw in input.trim().chars() {
        let ch = if raw == '_' { ' ' } else { raw };
        match (last_char, ch) {
            (Some(':'), ':') => continue,
            (Some(prev), c) if prev.is_whitespace() && c.is_whitespace() => continue,
            _ => {
                last_char = Some(ch);
            }
        }
        result.push(ch);
    }
    result
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteItem {
    pub category: &'static str,
    pub title: &'static str,
    pub action: u8,
    pub shortcut: Option<&'static str>,
}

/// A filtered entry with its fuzzy score and highlight indices.
#[derive(Debug, Clone)]
pub struct PaletteMatch {
    pub item: PaletteItem,
    pub score: i32,
    pub matched: Vec<usize>,
}

/// Filters and ranks `items` for `query`. Empty queries return everything
/// in declared order; otherwise subsequence matches rank by score.
/// Recently/often-used actions (Zed command history) always float above
/// unused ones. Stable sort preserves declared order for full ties.
pub fn filter_palette(
    items: &[PaletteItem],
    query: &str,
    usage: &std::collections::HashMap<u8, u32>,
) -> Vec<PaletteMatch> {
    let query = normalize_query(query);
    if query.is_empty() {
        let mut out: Vec<PaletteMatch> = items
            .iter()
            .map(|item| PaletteMatch {
                item: *item,
                score: 0,
                matched: Vec::new(),
            })
            .collect();
        out.sort_by_key(|m| std::cmp::Reverse(usage.get(&m.item.action).copied().unwrap_or(0)));
        return out;
    }
    let mut out: Vec<PaletteMatch> = items
        .iter()
        .filter_map(|item| {
            // Title hits outrank category hits; matching the two fields
            // separately avoids allocating a concatenated haystack.
            if let Some((score, matched)) = fuzzy_match(&query, item.title) {
                Some(PaletteMatch {
                    item: *item,
                    score,
                    matched,
                })
            } else {
                fuzzy_match(&query, item.category).map(|(score, _)| PaletteMatch {
                    item: *item,
                    score: score - 150,
                    matched: Vec::new(),
                })
            }
        })
        .collect();
    out.sort_by_key(|m| {
        (
            std::cmp::Reverse(usage.get(&m.item.action).copied().unwrap_or(0)),
            std::cmp::Reverse(m.score),
        )
    });
    out
}

/// Splits `title` into `(text, highlighted)` runs for rendering.
/// `matched` must be ascending (as produced by `fuzzy_match`); the merge
/// walk is linear instead of scanning per character.
pub fn highlight_runs(title: &str, matched: &[usize]) -> Vec<(String, bool)> {
    debug_assert!(matched.windows(2).all(|w| w[0] <= w[1]));
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut hits = matched.iter().peekable();
    let mut current_hit = false;
    let mut started = false;
    for (i, ch) in title.chars().enumerate() {
        while hits.peek().is_some_and(|&&m| m < i) {
            hits.next();
        }
        let hit = hits.peek().is_some_and(|&&m| m == i);
        if !started {
            current_hit = hit;
            started = true;
        }
        if hit != current_hit {
            runs.push((std::mem::take(&mut current), current_hit));
            current_hit = hit;
        }
        current.push(ch);
    }
    if started || !current.is_empty() {
        runs.push((current, current_hit));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_query_matches_everything() {
        assert_eq!(fuzzy_match("", "Anything").unwrap().1, Vec::<usize>::new());
    }

    #[test]
    fn test_exact_prefix_wins_over_scattered() {
        let (s_prefix, _) = fuzzy_match("con", "Connect to SFTP Server...").unwrap();
        let (s_scatter, _) = fuzzy_match("con", "Disconnect Remote Session").unwrap();
        assert!(s_prefix > s_scatter);
    }

    #[test]
    fn test_subsequence_and_case_insensitive() {
        assert!(fuzzy_match("nd", "New Directory...").is_some());
        assert!(fuzzy_match("ND", "New Directory...").is_some());
        assert!(fuzzy_match("xyz", "New Directory...").is_none());
    }

    #[test]
    fn test_consecutive_beats_gapped() {
        let (s_tight, _) = fuzzy_match("nw", "New Directory...").unwrap();
        let (s_loose, _) = fuzzy_match("nw", "Open New Window").unwrap();
        assert!(s_tight > s_loose);
    }

    #[test]
    fn test_filter_keeps_order_on_empty_and_ranks() {
        use std::collections::HashMap;
        let empty: HashMap<u8, u32> = HashMap::new();
        let items = vec![
            PaletteItem { category: "A", title: "Connect", action: 0, shortcut: None },
            PaletteItem { category: "B", title: "Disconnect", action: 1, shortcut: None },
        ];
        let all = filter_palette(&items, "", &empty);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].item.title, "Connect");

        let ranked = filter_palette(&items, "con", &empty);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].item.title, "Connect");

        assert!(filter_palette(&items, "zzz", &empty).is_empty());
    }

    #[test]
    fn test_used_commands_rank_first() {
        use std::collections::HashMap;
        let mut usage = HashMap::new();
        usage.insert(1u8, 5u32);
        let items = vec![
            PaletteItem { category: "A", title: "Connect", action: 0, shortcut: None },
            PaletteItem { category: "B", title: "Disconnect", action: 1, shortcut: None },
        ];
        let all = filter_palette(&items, "", &usage);
        assert_eq!(all[0].item.title, "Disconnect");
        let ranked = filter_palette(&items, "con", &usage);
        assert_eq!(ranked[0].item.title, "Disconnect");
    }

    #[test]
    fn test_normalize_query() {
        assert_eq!(normalize_query("  editor:  backspace  "), "editor: backspace");
        assert_eq!(normalize_query("a__b"), "a b");
        assert_eq!(normalize_query("a::::b"), "a:b");
    }

    #[test]
    fn test_highlight_runs_split_correctly() {
        let runs = highlight_runs("Connect", &[0, 1, 2]);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0], ("Con".to_string(), true));
        assert_eq!(runs[1], ("nect".to_string(), false));
    }
}
