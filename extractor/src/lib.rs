use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};
use std::borrow::Cow;
use std::collections::HashSet;

static TITLE_SEL: Lazy<Selector> = Lazy::new(|| Selector::parse(r#"head title"#).unwrap());
static BODY_PARA_SEL: Lazy<Selector> = Lazy::new(|| Selector::parse(r#"body"#).unwrap());
static LINK_SEL: Lazy<Selector> = Lazy::new(|| Selector::parse(r#"a"#).unwrap());

static GRAVATAR_IMG_SEL: Lazy<Selector> =
    Lazy::new(|| Selector::parse(r#"img[src *= "gravatar.com"]"#).unwrap());

static GRAVATAR_SRC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"gravatar\.com/avatar/([0-9a-f]+)").unwrap());

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("Invalid UTF-8: {0:?}")]
    InvalidUtf8(#[from] std::str::Utf8Error),
}

#[derive(Debug)]
pub struct Document<'a> {
    pub title: Cow<'a, str>,
    pub content: Vec<Cow<'a, str>>,
    pub links: Vec<Cow<'a, str>>,
    pub gravatar_hashes: HashSet<Cow<'a, str>>,
}

impl Document<'static> {
    pub fn parse(contents: &str) -> Result<Document<'static>, Error> {
        let html = Html::parse_document(contents);
        let doc = Document::extract(&html)?;

        Ok(doc.into_owned())
    }
}

impl<'a> Document<'a> {
    pub fn extract(html: &'a Html) -> Result<Self, Error> {
        let title = html
            .select(&TITLE_SEL)
            .flat_map(|body| body.text())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .max_by_key(|value| value.len())
            .map(|value| value.into())
            .unwrap_or_default();

        let content = html
            .select(&BODY_PARA_SEL)
            .flat_map(|body| body.text())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| value.into())
            .collect::<Vec<_>>();

        let links = html
            .select(&LINK_SEL)
            .filter_map(|element| element.attr("href"))
            .map(|value| value.trim())
            .filter(|value| value.starts_with("http"))
            .map(|value| value.into())
            .collect::<Vec<_>>();

        let matches = html
            .select(&GRAVATAR_IMG_SEL)
            .filter_map(|element| element.attr("src"))
            .flat_map(|src| {
                GRAVATAR_SRC_RE
                    .captures_iter(src)
                    .filter_map(|capture| capture.get(1))
            })
            .map(|hash_match| hash_match.as_str().into())
            .collect::<HashSet<_>>();

        Ok(Self {
            title,
            content,
            links,
            gravatar_hashes: matches,
        })
    }

    pub fn into_owned(self) -> Document<'static> {
        Document {
            title: self.title.into_owned().into(),
            content: self
                .content
                .into_iter()
                .map(|value| value.into_owned().into())
                .collect(),
            links: self
                .links
                .into_iter()
                .map(|value| value.into_owned().into())
                .collect(),
            gravatar_hashes: self
                .gravatar_hashes
                .into_iter()
                .map(|value| value.into_owned().into())
                .collect(),
        }
    }
}
