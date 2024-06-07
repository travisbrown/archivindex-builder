use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

const FILE_PATTERN: &str = r#"^redirects-(.).csv$"#;

static FILE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(FILE_PATTERN).unwrap());

static FILE_PREFIXES: Lazy<HashSet<String>> = Lazy::new(|| {
    let mut prefixes = HashSet::new();
    prefixes.extend(('2'..='7').map(|c| c.to_string()));
    prefixes.extend(('A'..='Z').map(|c| c.to_string()));
    prefixes
});

pub fn is_valid_path<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref()
        .file_name()
        .and_then(|v| v.to_str())
        .and_then(|v| FILE_RE.captures(v))
        .and_then(|groups| groups.get(1))
        .map(|m| FILE_PREFIXES.contains(m.as_str()))
        .unwrap_or(false)
}

pub fn file_prefixes() -> Vec<String> {
    let mut result = FILE_PREFIXES.iter().cloned().collect::<Vec<_>>();
    result.sort();
    result
}
