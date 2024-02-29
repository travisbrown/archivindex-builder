use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

const FILE_PATTERN: &str = r#"^redirects-(.).csv$"#;

pub fn is_valid_path<P: AsRef<Path>>(path: P) -> bool {
    lazy_static::lazy_static! {
        static ref FILE_RE: Regex = Regex::new(FILE_PATTERN).unwrap();
    }

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

lazy_static::lazy_static! {
    static ref FILE_PREFIXES: HashSet<String> = {
        let mut prefixes = HashSet::new();
        prefixes.extend(('2'..='7').map(|c| c.to_string()));
        prefixes.extend(('A'..='Z').map(|c| c.to_string()));
        prefixes
    };
}
