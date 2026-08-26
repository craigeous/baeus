use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLineKind {
    Unchanged,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub old_line_number: Option<usize>,
    pub new_line_number: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub lines: Vec<DiffLine>,
    pub added_count: usize,
    pub removed_count: usize,
    pub unchanged_count: usize,
}

impl DiffResult {
    pub fn has_changes(&self) -> bool {
        self.added_count > 0 || self.removed_count > 0
    }

    pub fn total_lines(&self) -> usize {
        self.lines.len()
    }
}

pub fn compute_diff(original: &str, modified: &str) -> DiffResult {
    let old_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = modified.lines().collect();

    // Simple LCS-based diff
    let lcs = longest_common_subsequence(&old_lines, &new_lines);

    let mut result_lines = Vec::new();
    let mut added_count = 0;
    let mut removed_count = 0;
    let mut unchanged_count = 0;

    let mut old_idx = 0;
    let mut new_idx = 0;
    let mut lcs_idx = 0;

    while old_idx < old_lines.len() || new_idx < new_lines.len() {
        if old_idx < old_lines.len()
            && lcs_idx < lcs.len()
            && old_lines[old_idx] == lcs[lcs_idx]
            && new_idx < new_lines.len()
            && new_lines[new_idx] == lcs[lcs_idx]
        {
            result_lines.push(DiffLine {
                kind: DiffLineKind::Unchanged,
                content: old_lines[old_idx].to_string(),
                old_line_number: Some(old_idx + 1),
                new_line_number: Some(new_idx + 1),
            });
            unchanged_count += 1;
            old_idx += 1;
            new_idx += 1;
            lcs_idx += 1;
        } else if old_idx < old_lines.len()
            && (lcs_idx >= lcs.len() || old_lines[old_idx] != lcs[lcs_idx])
        {
            result_lines.push(DiffLine {
                kind: DiffLineKind::Removed,
                content: old_lines[old_idx].to_string(),
                old_line_number: Some(old_idx + 1),
                new_line_number: None,
            });
            removed_count += 1;
            old_idx += 1;
        } else if new_idx < new_lines.len() {
            result_lines.push(DiffLine {
                kind: DiffLineKind::Added,
                content: new_lines[new_idx].to_string(),
                old_line_number: None,
                new_line_number: Some(new_idx + 1),
            });
            added_count += 1;
            new_idx += 1;
        }
    }

    DiffResult { lines: result_lines, added_count, removed_count, unchanged_count }
}

fn longest_common_subsequence<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<&'a str> {
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            result.push(a[i - 1]);
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    result.reverse();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_changes() {
        let text = "line1\nline2\nline3";
        let result = compute_diff(text, text);

        assert!(!result.has_changes());
        assert_eq!(result.unchanged_count, 3);
        assert_eq!(result.added_count, 0);
        assert_eq!(result.removed_count, 0);
    }

    #[test]
    fn test_added_lines() {
        let original = "line1\nline3";
        let modified = "line1\nline2\nline3";
        let result = compute_diff(original, modified);

        assert!(result.has_changes());
        assert_eq!(result.added_count, 1);
        assert_eq!(result.removed_count, 0);
        assert_eq!(result.unchanged_count, 2);
    }

    #[test]
    fn test_removed_lines() {
        let original = "line1\nline2\nline3";
        let modified = "line1\nline3";
        let result = compute_diff(original, modified);

        assert!(result.has_changes());
        assert_eq!(result.removed_count, 1);
        assert_eq!(result.added_count, 0);
    }

    #[test]
    fn test_modified_lines() {
        let original = "line1\nold-line\nline3";
        let modified = "line1\nnew-line\nline3";
        let result = compute_diff(original, modified);

        assert!(result.has_changes());
        assert_eq!(result.removed_count, 1);
        assert_eq!(result.added_count, 1);
        assert_eq!(result.unchanged_count, 2);
    }

    #[test]
    fn test_empty_original() {
        let result = compute_diff("", "line1\nline2");
        assert!(result.has_changes());
        assert_eq!(result.added_count, 2);
    }

    #[test]
    fn test_empty_modified() {
        let result = compute_diff("line1\nline2", "");
        assert!(result.has_changes());
        assert_eq!(result.removed_count, 2);
    }

    #[test]
    fn test_diff_line_numbers() {
        let original = "a\nb\nc";
        let modified = "a\nx\nc";
        let result = compute_diff(original, modified);

        let unchanged: Vec<_> =
            result.lines.iter().filter(|l| l.kind == DiffLineKind::Unchanged).collect();
        assert_eq!(unchanged.len(), 2);
        assert!(
            unchanged.iter().all(|l| l.old_line_number.is_some() && l.new_line_number.is_some())
        );
    }

    #[test]
    fn test_diff_result_total_lines() {
        let original = "a\nb\nc";
        let modified = "a\nx\nc";
        let result = compute_diff(original, modified);
        assert_eq!(result.total_lines(), 4); // a, -b, +x, c
    }

    #[test]
    fn test_yaml_diff() {
        let original = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: old-pod\n";
        let modified = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: new-pod\n";
        let result = compute_diff(original, modified);

        assert!(result.has_changes());
        assert_eq!(result.unchanged_count, 3);
        assert_eq!(result.removed_count, 1);
        assert_eq!(result.added_count, 1);
    }
}
