use once_cell::sync::Lazy;

const REDIRECT_HTML_PATTERN: &str =
    r#"^<html><body>You are being <a href="([^"]+)">redirected</a>\.</body></html>$"#;

/// Attempt to guess the contents of a redirect page stored by the Wayback
/// Machine.
///
/// When an item is listed as a 302 redirect in CDX results, the content of
/// the page usually (but not always) has the following format, where the
/// URL is the value of the location header.
pub fn make_redirect_html(url: &str) -> String {
    format!(
        "<html><body>You are being <a href=\"{}\">redirected</a>.</body></html>",
        url
    )
}

pub fn parse_redirect_html(content: &str) -> Option<&str> {
    static REDIRECT_HTML_RE: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(REDIRECT_HTML_PATTERN).unwrap());

    REDIRECT_HTML_RE
        .captures(content)
        .and_then(|groups| groups.get(1))
        .map(|m| m.as_str())
}
