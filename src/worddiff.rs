//! Word/char-level intra-line diff utilities.
//!
//! Pure text algorithms with no UI dependency: the diff renderer maps the
//! char-index ranges produced here to pixels via the laid-out galley.

/// Split a line into diff tokens: runs of word chars, runs of whitespace, and
/// each punctuation char as its own token. Word-level granularity gives clean,
/// readable change blocks instead of scattered single-character highlights.
fn diff_tokenize(s: &str) -> Vec<String> {
    #[derive(PartialEq, Clone, Copy)]
    enum Cls {
        Word,
        Space,
    }
    fn cls(c: char) -> Option<Cls> {
        if c.is_alphanumeric() || c == '_' {
            Some(Cls::Word)
        } else if c.is_whitespace() {
            Some(Cls::Space)
        } else {
            None
        } // punctuation → standalone token
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_cls: Option<Cls> = None;
    for c in s.chars() {
        match cls(c) {
            None => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    cur_cls = None;
                }
                out.push(c.to_string());
            }
            Some(k) => {
                if Some(k) == cur_cls {
                    cur.push(c);
                } else {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    cur.push(c);
                    cur_cls = Some(k);
                }
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Compute the char-index ranges (on the given side) that differ between
/// `before` and `after`, at word granularity, for intra-line change highlighting.
/// Trailing whitespace is ignored. If the line changed by more than ~60%, returns
/// no ranges so the line-level background alone conveys the change (avoids noisy
/// fragmented highlights on near-total rewrites). Ranges index into the line's
/// chars; the renderer maps them to pixels via the laid-out galley.
pub fn changed_word_ranges(before: &str, after: &str, left_side: bool) -> Vec<(usize, usize)> {
    let b = before.trim_end();
    let a = after.trim_end();
    let bt = diff_tokenize(b);
    let at = diff_tokenize(a);
    let n = bt.len();
    let m = at.len();
    if n == 0 && m == 0 {
        return Vec::new();
    }
    // Pathologically long lines (e.g. minified JS): the O(n*m) DP below would
    // stall the frame, so fall back to line-level background only. The cap also
    // keeps the LCS length within u16 range.
    if n.saturating_mul(m) > 100_000 {
        return Vec::new();
    }

    // LCS DP over tokens
    let mut dp = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if bt[i] == at[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let tok_chars = |tok: &str| tok.chars().count();
    let side = if left_side { &bt } else { &at };
    let total_cols: usize = side.iter().map(|tok| tok_chars(tok)).sum();

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut col = 0usize;
    let mut changed_cols = 0usize;
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if bt[i] == at[j] {
            col += tok_chars(if left_side { &bt[i] } else { &at[j] });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            if left_side {
                let w = tok_chars(&bt[i]);
                ranges.push((col, col + w));
                changed_cols += w;
                col += w;
            }
            i += 1;
        } else {
            if !left_side {
                let w = tok_chars(&at[j]);
                ranges.push((col, col + w));
                changed_cols += w;
                col += w;
            }
            j += 1;
        }
    }
    while i < n {
        if left_side {
            let w = tok_chars(&bt[i]);
            ranges.push((col, col + w));
            changed_cols += w;
            col += w;
        }
        i += 1;
    }
    while j < m {
        if !left_side {
            let w = tok_chars(&at[j]);
            ranges.push((col, col + w));
            changed_cols += w;
            col += w;
        }
        j += 1;
    }

    // Near-total rewrite: let the line background speak, skip word highlights
    if total_cols > 0 && changed_cols * 100 / total_cols >= 60 {
        return Vec::new();
    }

    // Merge adjacent/touching ranges into clean contiguous blocks
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for r in ranges {
        if let Some(last) = merged.last_mut()
            && r.0 <= last.1
        {
            last.1 = last.1.max(r.1);
        } else {
            merged.push(r);
        }
    }
    merged
}

#[derive(Clone, Debug, PartialEq)]
pub enum CharOp {
    Equal(String),
    Delete(String),
    Insert(String),
}

pub fn diff_chars(before: &str, after: &str) -> Vec<CharOp> {
    let b_chars: Vec<char> = before.chars().collect();
    let a_chars: Vec<char> = after.chars().collect();
    let b_len = b_chars.len();
    let a_len = a_chars.len();

    // Very long lines would make the O(n*m) DP table huge; show the whole
    // line as replaced instead of risking a memory spike / UI stall.
    if b_len.saturating_mul(a_len) > 1_000_000 {
        let mut ops = Vec::new();
        if !before.is_empty() {
            ops.push(CharOp::Delete(before.to_string()));
        }
        if !after.is_empty() {
            ops.push(CharOp::Insert(after.to_string()));
        }
        return ops;
    }

    let mut dp = vec![vec![0; a_len + 1]; b_len + 1];

    for i in 1..=b_len {
        for j in 1..=a_len {
            if b_chars[i - 1] == a_chars[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    let mut ops = Vec::new();
    let mut i = b_len;
    let mut j = a_len;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && b_chars[i - 1] == a_chars[j - 1] {
            ops.push(CharOp::Equal(b_chars[i - 1].to_string()));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            ops.push(CharOp::Insert(a_chars[j - 1].to_string()));
            j -= 1;
        } else if i > 0 && (j == 0 || dp[i - 1][j] >= dp[i][j - 1]) {
            ops.push(CharOp::Delete(b_chars[i - 1].to_string()));
            i -= 1;
        }
    }

    ops.reverse();

    let mut merged_ops = Vec::new();
    for op in ops {
        match (merged_ops.last_mut(), op) {
            (Some(CharOp::Equal(s1)), CharOp::Equal(s2)) => s1.push_str(&s2),
            (Some(CharOp::Delete(s1)), CharOp::Delete(s2)) => s1.push_str(&s2),
            (Some(CharOp::Insert(s1)), CharOp::Insert(s2)) => s1.push_str(&s2),
            (_, op) => merged_ops.push(op),
        }
    }

    merged_ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_chars() {
        let ops = diff_chars("hello world", "hello brave new world");
        assert_eq!(ops[0], CharOp::Equal("hello".to_string()));
        assert_eq!(ops[1], CharOp::Insert(" brave new".to_string()));
        assert_eq!(ops[2], CharOp::Equal(" world".to_string()));
    }

    #[test]
    fn test_changed_word_ranges_basic() {
        // One word changed: "foo" → "bar" inside identical surroundings
        let r = changed_word_ranges("let x = foo;", "let x = bar;", false);
        // "bar" occupies chars [8, 11)
        assert_eq!(r, vec![(8, 11)]);
    }

    #[test]
    fn test_changed_word_ranges_ignores_trailing_ws() {
        // Only trailing whitespace differs → no ranges
        let r = changed_word_ranges("value = 1", "value = 1   ", false);
        assert!(r.is_empty());
    }

    #[test]
    fn test_changed_word_ranges_near_total_rewrite() {
        // Mostly different → skip fragmented highlights (rely on line bg)
        let r = changed_word_ranges("abcd efgh", "wxyz 1234", false);
        assert!(r.is_empty());
    }
}
