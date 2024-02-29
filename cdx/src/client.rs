use futures::{Stream, StreamExt};
use reqwest::Client;
use std::time::Duration;

const TCP_KEEPALIVE_SECS: u64 = 20;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("HTTP client error: {0}")]
    HttpClientError(#[from] reqwest::Error),
    #[error("JSON decoding error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Invalid num pages response")]
    InvalidNumPages(Vec<Vec<String>>),
    #[error("Blocked query: {0}")]
    BlockedQuery(String),
}

#[derive(Clone, Debug)]
pub struct Page {
    pub url: String,
    pub content: String,
}

impl Page {
    fn new(url: String, content: String) -> Self {
        Self { url, content }
    }
}

pub struct IndexClient {
    underlying: Client,
    base: String,
    page_size: usize,
    delay: Duration,
}

impl IndexClient {
    pub fn new(base: String, page_size: usize, delay: Duration) -> Result<Self, Error> {
        Ok(Self {
            underlying: Client::builder()
                .tcp_keepalive(Some(std::time::Duration::from_secs(TCP_KEEPALIVE_SECS)))
                .build()?,
            base,
            page_size,
            delay,
        })
    }

    pub fn new_default() -> Result<Self, Error> {
        Self::new(
            "http://web.archive.org/web/timemap".to_string(),
            5,
            Duration::from_secs(15),
        )
    }

    async fn get_num_pages(&self, query: &str, exact: bool) -> Result<usize, Error> {
        let response = self
            .underlying
            .get(format!("{}/json", self.base))
            .query(&[
                ("url", query),
                ("matchType", if exact { "exact" } else { "prefix" }),
                ("pageSize", &self.page_size.to_string()),
                ("showNumPages", "true"),
            ])
            .send()
            .await?;

        let content = response.json::<Vec<Vec<String>>>().await?;

        if content.len() == 2
            && content[0].len() == 1
            && content[1].len() == 1
            && content[0][0] == "numpages"
        {
            content[1][0]
                .parse::<usize>()
                .map_err(|_| Error::InvalidNumPages(content))
        } else {
            Err(Error::InvalidNumPages(content))
        }
    }

    async fn get_page(&self, query: &str, exact: bool, page: usize) -> Result<Page, Error> {
        let request = self
            .underlying
            .get(format!("{}/json", self.base))
            .query(&[
                ("url", query),
                ("matchType", if exact { "exact" } else { "prefix" }),
                ("pageSize", &self.page_size.to_string()),
                ("fields", "urlkey,timestamp,original,mimetype,statuscode,digest,redirect,robotflags,length,offset,filename"),
                ("page", &page.to_string()),
            ])
            .build()?;

        let url = request.url().to_string();

        let response = self.underlying.execute(request).await?;
        let body = response.text().await?;

        Ok(Page::new(url, body))
    }

    pub async fn lookup<'a>(
        &'a self,
        query: &'a str,
        exact: bool,
        start_page: Option<usize>,
    ) -> Result<(usize, impl Stream<Item = Result<Page, Error>> + 'a), Error> {
        let num_pages = self.get_num_pages(query, exact).await?;
        let pages = futures::stream::iter(start_page.unwrap_or_default()..num_pages).then(
            move |page| async move {
                tokio::time::sleep(self.delay).await;

                self.get_page(query, exact, page).await
            },
        );

        Ok((num_pages, pages))
    }
}
