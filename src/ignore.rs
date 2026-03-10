// Parses .vritignore glob patterns and matches file paths
use std::fs;
use std::path::Path;

pub struct IgnoreRules {
    patterns: Vec<Pattern>,
}

struct Pattern {
    segments: Vec<SegmentMatcher>,
    dir_only: bool,
    is_recursive: bool,
}

enum SegmentMatcher {
    Literal(String),
    Glob(GlobPattern),
    DoubleWildcard,
}

struct GlobPattern {
    parts: Vec<GlobPart>,
}

enum GlobPart {
    Literal(String),
    Star,
    Question,
}

impl IgnoreRules {
    pub fn load(repo_root: &Path) -> IgnoreRules {
        let path = repo_root.join(".vritignore");
        let content = fs::read_to_string(&path).unwrap_or_default();
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> IgnoreRules {
        let patterns = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| Pattern::parse(l))
            .collect();
        IgnoreRules { patterns }
    }

    pub fn is_ignored(&self, path: &str, is_dir: bool) -> bool {
        // Always ignore .vrit directory
        if path == ".vrit" || path.starts_with(".vrit/") {
            return true;
        }
        self.patterns.iter().any(|p| p.matches(path, is_dir))
    }
}

impl Pattern {
    fn parse(raw: &str) -> Option<Pattern> {
        let mut s = raw.to_string();
        let dir_only = s.ends_with('/');
        if dir_only {
            s.pop();
        }

        let is_recursive = s.contains("**");
        let segments: Vec<SegmentMatcher> = s
            .split('/')
            .filter(|seg| !seg.is_empty())
            .map(|seg| {
                if seg == "**" {
                    SegmentMatcher::DoubleWildcard
                } else if seg.contains('*') || seg.contains('?') {
                    SegmentMatcher::Glob(GlobPattern::parse(seg))
                } else {
                    SegmentMatcher::Literal(seg.to_string())
                }
            })
            .collect();

        if segments.is_empty() {
            return None;
        }
        Some(Pattern {
            segments,
            dir_only,
            is_recursive,
        })
    }

    fn matches(&self, path: &str, is_dir: bool) -> bool {
        if self.dir_only && !is_dir {
            return false;
        }

        let path_parts: Vec<&str> = path.split('/').collect();

        // Single-segment patterns without slashes match any path component
        if self.segments.len() == 1 && !self.is_recursive {
            return path_parts
                .iter()
                .any(|part| self.segment_matches(&self.segments[0], part));
        }

        self.match_segments(&self.segments, &path_parts)
    }

    fn match_segments(&self, segments: &[SegmentMatcher], path_parts: &[&str]) -> bool {
        if segments.is_empty() {
            return true;
        }
        if path_parts.is_empty() {
            return segments.iter().all(|s| matches!(s, SegmentMatcher::DoubleWildcard));
        }

        match &segments[0] {
            SegmentMatcher::DoubleWildcard => {
                // ** matches zero or more directories
                for i in 0..=path_parts.len() {
                    if self.match_segments(&segments[1..], &path_parts[i..]) {
                        return true;
                    }
                }
                false
            }
            _ => {
                if self.segment_matches(&segments[0], path_parts[0]) {
                    self.match_segments(&segments[1..], &path_parts[1..])
                } else {
                    false
                }
            }
        }
    }

    fn segment_matches(&self, segment: &SegmentMatcher, part: &str) -> bool {
        match segment {
            SegmentMatcher::Literal(s) => s == part,
            SegmentMatcher::Glob(g) => g.matches(part),
            SegmentMatcher::DoubleWildcard => true,
        }
    }
}

impl GlobPattern {
    fn parse(pattern: &str) -> GlobPattern {
        let mut parts = Vec::new();
        let mut literal = String::new();
        for ch in pattern.chars() {
            match ch {
                '*' => {
                    if !literal.is_empty() {
                        parts.push(GlobPart::Literal(std::mem::take(&mut literal)));
                    }
                    parts.push(GlobPart::Star);
                }
                '?' => {
                    if !literal.is_empty() {
                        parts.push(GlobPart::Literal(std::mem::take(&mut literal)));
                    }
                    parts.push(GlobPart::Question);
                }
                _ => literal.push(ch),
            }
        }
        if !literal.is_empty() {
            parts.push(GlobPart::Literal(literal));
        }
        GlobPattern { parts }
    }

    fn matches(&self, text: &str) -> bool {
        self.match_parts(&self.parts, text)
    }

    fn match_parts(&self, parts: &[GlobPart], text: &str) -> bool {
        if parts.is_empty() {
            return text.is_empty();
        }
        match &parts[0] {
            GlobPart::Literal(s) => {
                text.starts_with(s.as_str()) && self.match_parts(&parts[1..], &text[s.len()..])
            }
            GlobPart::Question => {
                !text.is_empty() && self.match_parts(&parts[1..], &text[text.chars().next().unwrap().len_utf8()..])
            }
            GlobPart::Star => {
                for i in 0..=text.len() {
                    if self.match_parts(&parts[1..], &text[i..]) {
                        return true;
                    }
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_file_pattern() {
        let rules = IgnoreRules::parse("*.o");
        assert!(rules.is_ignored("foo.o", false));
        assert!(rules.is_ignored("dir/foo.o", false));
        assert!(!rules.is_ignored("foo.c", false));
    }

    #[test]
    fn directory_pattern() {
        let rules = IgnoreRules::parse("build/");
        assert!(rules.is_ignored("build", true));
        assert!(!rules.is_ignored("build", false));
    }

    #[test]
    fn double_wildcard() {
        let rules = IgnoreRules::parse("**/logs");
        assert!(rules.is_ignored("logs", true));
        assert!(rules.is_ignored("a/b/logs", true));
    }

    #[test]
    fn path_pattern() {
        let rules = IgnoreRules::parse("target/debug");
        assert!(rules.is_ignored("target/debug", true));
        assert!(!rules.is_ignored("other/debug", true));
    }

    #[test]
    fn vrit_dir_always_ignored() {
        let rules = IgnoreRules::parse("");
        assert!(rules.is_ignored(".vrit", true));
        assert!(rules.is_ignored(".vrit/objects", true));
    }

    #[test]
    fn question_mark() {
        let rules = IgnoreRules::parse("?.txt");
        assert!(rules.is_ignored("a.txt", false));
        assert!(!rules.is_ignored("ab.txt", false));
    }

    #[test]
    fn comments_and_empty_lines() {
        let rules = IgnoreRules::parse("# comment\n\n*.o\n");
        assert!(rules.is_ignored("foo.o", false));
    }
}
