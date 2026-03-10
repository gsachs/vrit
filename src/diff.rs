// Myers diff algorithm — computes shortest edit scripts between two text sequences
use colored::Colorize;

#[derive(Debug, PartialEq)]
pub enum Edit {
    Equal(String),
    Insert(String),
    Delete(String),
}

/// Compute the Myers diff between two slices of lines.
pub fn myers_diff(old: &[&str], new: &[&str]) -> Vec<Edit> {
    let n = old.len();
    let m = new.len();

    if n == 0 && m == 0 {
        return Vec::new();
    }

    let max = n + m;

    // Bail out when edit distance would exhaust memory from frontier vector cloning
    const MAX_EDIT_DISTANCE: usize = 10_000;
    if max > MAX_EDIT_DISTANCE {
        let mut edits = Vec::with_capacity(n + m);
        for line in old {
            edits.push(Edit::Delete(line.to_string()));
        }
        for line in new {
            edits.push(Edit::Insert(line.to_string()));
        }
        return edits;
    }

    // v[k] stores the furthest reaching x on diagonal k
    // Diagonal k = x - y, stored at index k + max
    let mut v = vec![0usize; 2 * max + 1];
    let mut trace: Vec<Vec<usize>> = Vec::new();

    'outer: for d in 0..=max {
        trace.push(v.clone());
        let mut new_v = v.clone();

        let d_i = d as isize;
        let mut k = -d_i;
        while k <= d_i {
            let idx = (k + max as isize) as usize;
            let mut x = if k == -d_i
                || (k != d_i && v[(k - 1 + max as isize) as usize] < v[(k + 1 + max as isize) as usize])
            {
                v[(k + 1 + max as isize) as usize]
            } else {
                v[(k - 1 + max as isize) as usize] + 1
            };
            let mut y = (x as isize - k) as usize;

            while x < n && y < m && old[x] == new[y] {
                x += 1;
                y += 1;
            }

            new_v[idx] = x;

            if x >= n && y >= m {
                v = new_v;
                trace.push(v.clone()); // final frontier snapshot needed for backtracking
                break 'outer;
            }
            k += 2;
        }
        v = new_v;
    }

    // Backtrack to reconstruct the edit path
    let mut edits = Vec::new();
    let mut x = n;
    let mut y = m;

    // trace.len() - 1 is the final snapshot; we iterate from the last edit step back to 0
    let num_d = trace.len() - 1; // number of d-levels recorded (the extra push at break adds one)

    for d in (0..num_d).rev() {
        let k = x as isize - y as isize;

        // Determine where we came from at step d
        let (prev_x, prev_y) = if d == 0 {
            // d=0 means no edits — all remaining moves are diagonal (equal)
            (0usize, 0usize)
        } else {
            let d_i = d as isize;
            let v_prev = &trace[d];
            let prev_k = if k == -d_i
                || (k != d_i
                    && v_prev[(k - 1 + max as isize) as usize]
                        < v_prev[(k + 1 + max as isize) as usize])
            {
                k + 1
            } else {
                k - 1
            };
            let px = v_prev[(prev_k + max as isize) as usize];
            let py = (px as isize - prev_k) as usize;
            (px, py)
        };

        // Diagonal moves (equal lines) — walk back from (x,y) toward the edit point
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            edits.push(Edit::Equal(old[x].to_string()));
        }

        // The non-diagonal edit at this d-level (skip for d=0)
        if d > 0 {
            if x == prev_x {
                y -= 1;
                edits.push(Edit::Insert(new[y].to_string()));
            } else {
                x -= 1;
                edits.push(Edit::Delete(old[x].to_string()));
            }
        }
    }

    edits.reverse();
    edits
}

/// A hunk in unified diff format.
pub struct Hunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<Edit>,
}

/// Group edits into hunks with context lines.
pub fn make_hunks(edits: &[Edit], context: usize) -> Vec<Hunk> {
    if edits.is_empty() {
        return Vec::new();
    }

    // Find change ranges (non-Equal edits)
    let mut changes: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < edits.len() {
        if !matches!(edits[i], Edit::Equal(_)) {
            let start = i;
            while i < edits.len() && !matches!(edits[i], Edit::Equal(_)) {
                i += 1;
            }
            changes.push((start, i));
        } else {
            i += 1;
        }
    }

    if changes.is_empty() {
        return Vec::new();
    }

    // Merge nearby changes into hunks
    let mut hunks = Vec::new();
    let mut group_start = changes[0].0;
    let mut group_end = changes[0].1;

    for &(start, end) in &changes[1..] {
        if start <= group_end + 2 * context { // merge hunks whose context lines would overlap
            group_end = end;
        } else {
            hunks.push(build_hunk(edits, group_start, group_end, context));
            group_start = start;
            group_end = end;
        }
    }
    hunks.push(build_hunk(edits, group_start, group_end, context));
    hunks
}

fn build_hunk(edits: &[Edit], change_start: usize, change_end: usize, context: usize) -> Hunk {
    let hunk_start = change_start.saturating_sub(context);
    let hunk_end = (change_end + context).min(edits.len());

    let lines: Vec<Edit> = edits[hunk_start..hunk_end]
        .iter()
        .map(|e| match e {
            Edit::Equal(s) => Edit::Equal(s.clone()),
            Edit::Insert(s) => Edit::Insert(s.clone()),
            Edit::Delete(s) => Edit::Delete(s.clone()),
        })
        .collect();

    // Count old/new lines
    let mut old_count = 0;
    let mut new_count = 0;

    // Calculate old_start by counting old lines before hunk_start
    let mut old_line = 0;
    let mut new_line = 0;
    for edit in &edits[..hunk_start] {
        match edit {
            Edit::Equal(_) => {
                old_line += 1;
                new_line += 1;
            }
            Edit::Delete(_) => old_line += 1,
            Edit::Insert(_) => new_line += 1,
        }
    }
    let old_start = old_line + 1;
    let new_start = new_line + 1;

    for line in &lines {
        match line {
            Edit::Equal(_) => {
                old_count += 1;
                new_count += 1;
            }
            Edit::Delete(_) => old_count += 1,
            Edit::Insert(_) => new_count += 1,
        }
    }

    Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
        lines,
    }
}

/// Format a diff in unified format with colors.
pub fn format_unified(old_name: &str, new_name: &str, edits: &[Edit], colored: bool) -> String {
    let hunks = make_hunks(edits, 3);
    if hunks.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    let header_a = format!("--- a/{old_name}");
    let header_b = format!("+++ b/{new_name}");
    if colored {
        out.push_str(&format!("{}\n", header_a.bold()));
        out.push_str(&format!("{}\n", header_b.bold()));
    } else {
        out.push_str(&format!("{header_a}\n"));
        out.push_str(&format!("{header_b}\n"));
    }

    for hunk in &hunks {
        let header = format!(
            "@@ -{},{} +{},{} @@",
            hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
        );
        if colored {
            out.push_str(&format!("{}\n", header.cyan()));
        } else {
            out.push_str(&format!("{header}\n"));
        }

        for line in &hunk.lines {
            match line {
                Edit::Equal(s) => out.push_str(&format!(" {s}\n")),
                Edit::Delete(s) => {
                    let text = format!("-{s}");
                    if colored {
                        out.push_str(&format!("{}\n", text.red()));
                    } else {
                        out.push_str(&format!("{text}\n"));
                    }
                }
                Edit::Insert(s) => {
                    let text = format!("+{s}");
                    if colored {
                        out.push_str(&format!("{}\n", text.green()));
                    } else {
                        out.push_str(&format!("{text}\n"));
                    }
                }
            }
        }
    }

    out
}

/// Check if content looks like a binary file (null bytes in first 8KB).
pub fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(8192);
    data[..check_len].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_diff() {
        let edits = myers_diff(&[], &[]);
        assert!(edits.is_empty());
    }

    #[test]
    fn all_insertions() {
        let edits = myers_diff(&[], &["a", "b"]);
        assert_eq!(edits, vec![Edit::Insert("a".into()), Edit::Insert("b".into())]);
    }

    #[test]
    fn all_deletions() {
        let edits = myers_diff(&["a", "b"], &[]);
        assert_eq!(edits, vec![Edit::Delete("a".into()), Edit::Delete("b".into())]);
    }

    #[test]
    fn no_changes() {
        let edits = myers_diff(&["a", "b", "c"], &["a", "b", "c"]);
        assert_eq!(
            edits,
            vec![
                Edit::Equal("a".into()),
                Edit::Equal("b".into()),
                Edit::Equal("c".into()),
            ]
        );
    }

    #[test]
    fn mixed_edits() {
        let old = vec!["a", "b", "c"];
        let new = vec!["a", "x", "c"];
        let edits = myers_diff(&old, &new);

        // Should have: Equal(a), Delete(b), Insert(x), Equal(c)
        let has_delete_b = edits.iter().any(|e| *e == Edit::Delete("b".into()));
        let has_insert_x = edits.iter().any(|e| *e == Edit::Insert("x".into()));
        let has_equal_a = edits.iter().any(|e| *e == Edit::Equal("a".into()));
        let has_equal_c = edits.iter().any(|e| *e == Edit::Equal("c".into()));
        assert!(has_delete_b);
        assert!(has_insert_x);
        assert!(has_equal_a);
        assert!(has_equal_c);
    }

    #[test]
    fn unified_format_output() {
        let old = vec!["a", "b", "c"];
        let new = vec!["a", "x", "c"];
        let edits = myers_diff(&old, &new);
        let output = format_unified("test.txt", "test.txt", &edits, false);
        assert!(output.contains("--- a/test.txt"));
        assert!(output.contains("+++ b/test.txt"));
        assert!(output.contains("@@"));
        assert!(output.contains("-b"));
        assert!(output.contains("+x"));
    }

    #[test]
    fn binary_detection() {
        assert!(is_binary(&[0x00, 0x01, 0x02]));
        assert!(!is_binary(b"hello world"));
        assert!(!is_binary(&[]));
    }
}
