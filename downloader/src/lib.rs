use aib_core::{digest::Sha1Digest, entry::UrlParts, timestamp::Timestamp};
use bytes::{Buf, Bytes};
use futures::future::{BoxFuture, FutureExt};
use reqwest::{header::LOCATION, redirect, Client, Response, StatusCode};
use std::time::Duration;
use thiserror::Error;

const MAX_RETRIES: usize = 7;
const RETRY_BASE_DURATION_MS: u64 = 60_000;
const TCP_KEEPALIVE_DURATION: Duration = Duration::from_secs(20);
const DEFAULT_REQUEST_TIMEOUT_DURATION: Duration = Duration::from_secs(60);

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("HTTP client error: {0:?}")]
    Client(#[from] reqwest::Error),
    #[error("Unexpected redirect: {0:?}")]
    UnexpectedRedirect(Option<String>),
    #[error("Unexpected redirect URL: {0:?}")]
    UnexpectedRedirectUrl(String),
    #[error("Unexpected status code: {0:?}")]
    UnexpectedStatus(StatusCode),
    #[error("Invalid UTF-8: {0:?}")]
    InvalidUtf8(#[from] std::str::Utf8Error),
}

#[derive(Debug, Eq, PartialEq)]
pub struct RedirectResolution {
    pub url: String,
    pub timestamp: Timestamp,
    pub content: Bytes,
    pub valid_initial_content: bool,
    pub valid_digest: bool,
}

#[derive(Clone, Debug)]
pub struct Downloader {
    client: Client,
}

impl Downloader {
    pub fn new(request_timeout: Duration) -> reqwest::Result<Self> {
        let tcp_keepalive = Some(TCP_KEEPALIVE_DURATION);

        Ok(Self {
            client: Client::builder()
                .timeout(request_timeout)
                .tcp_keepalive(tcp_keepalive)
                .redirect(redirect::Policy::none())
                .build()?,
        })
    }

    fn wayback_url(url: &str, timestamp: Timestamp, original: bool) -> String {
        format!(
            "http://web.archive.org/web/{}{}/{}",
            timestamp,
            if original { "id_" } else { "if_" },
            url
        )
    }

    pub async fn resolve_redirect(
        &self,
        url: &str,
        timestamp: Timestamp,
        expected_digest: Sha1Digest,
    ) -> Result<RedirectResolution, Error> {
        let initial_url = Self::wayback_url(url, timestamp, true);
        let initial_response = self.client.head(&initial_url).send().await?;

        match initial_response.status() {
            StatusCode::FOUND => {
                match initial_response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string)
                {
                    Some(location) => {
                        let info = location
                            .parse::<UrlParts>()
                            .map_err(|_| Error::UnexpectedRedirectUrl(location))?;

                        let guess = aib_core::redirect::make_redirect_html(&info.url);
                        let mut guess_bytes = guess.as_bytes();
                        let guess_digest = aib_core::digest::compute_digest(&mut guess_bytes)?;

                        let mut valid_initial_content = true;
                        let mut valid_digest = true;

                        let content = if guess_digest == expected_digest {
                            Bytes::from(guess)
                        } else {
                            //log::warn!("Invalid guess, re-requesting");
                            let direct_bytes =
                                self.client.get(&initial_url).send().await?.bytes().await?;
                            let direct_digest = aib_core::digest::compute_digest(
                                &mut direct_bytes.clone().reader(),
                            )?;
                            valid_initial_content = false;
                            valid_digest = direct_digest == expected_digest;

                            direct_bytes
                        };

                        let actual_url = self
                            .direct_resolve_redirect(&info.url, info.timestamp)
                            .await?;

                        let actual_info = actual_url
                            .parse::<UrlParts>()
                            .map_err(|_| Error::UnexpectedRedirectUrl(actual_url))?;

                        Ok(RedirectResolution {
                            url: actual_info.url,
                            timestamp: actual_info.timestamp,
                            content,
                            valid_initial_content,
                            valid_digest,
                        })
                    }
                    None => Err(Error::UnexpectedRedirect(None)),
                }
            }
            other => Err(Error::UnexpectedStatus(other)),
        }
    }

    async fn direct_resolve_redirect(
        &self,
        url: &str,
        timestamp: Timestamp,
    ) -> Result<String, Error> {
        let response = self
            .client
            .head(Self::wayback_url(url, timestamp, true))
            .send()
            .await?;

        match response.status() {
            StatusCode::FOUND => {
                match response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string)
                {
                    Some(location) => Ok(location),
                    None => Err(Error::UnexpectedRedirect(None)),
                }
            }
            other => Err(Error::UnexpectedStatus(other)),
        }
    }

    pub async fn resolve_redirect_shallow(
        &self,
        url: &str,
        timestamp: Timestamp,
        expected_digest: Sha1Digest,
    ) -> Result<(UrlParts, String, bool), Error> {
        let initial_url = Self::wayback_url(url, timestamp, true);
        let initial_response = self.client.head(&initial_url).send().await?;

        match initial_response.status() {
            StatusCode::FOUND => {
                match redirect_location(&initial_response) {
                    Some(location) => {
                        let info = location
                            .parse::<UrlParts>()
                            .map_err(|_| Error::UnexpectedRedirectUrl(location.to_string()))?;

                        let guess = aib_core::redirect::make_redirect_html(&info.url);
                        let mut guess_bytes = guess.as_bytes();
                        let guess_digest = aib_core::digest::compute_digest(&mut guess_bytes)?;

                        let (content, valid_digest) = if guess_digest == expected_digest {
                            (guess, true)
                        } else {
                            //log::warn!("Invalid guess, re-requesting");
                            let direct_bytes =
                                self.client.get(&initial_url).send().await?.bytes().await?;
                            let direct_digest = aib_core::digest::compute_digest(
                                &mut direct_bytes.clone().reader(),
                            )?;
                            (
                                std::str::from_utf8(&direct_bytes)?.to_string(),
                                direct_digest == expected_digest,
                            )
                        };

                        Ok((info, content, valid_digest))
                    }
                    None => Err(Error::UnexpectedRedirect(None)),
                }
            }
            other => Err(Error::UnexpectedStatus(other)),
        }
    }

    pub async fn download<'a>(
        &'a self,
        url: &'a str,
        timestamp: Timestamp,
        original: bool,
    ) -> Result<Option<Download>, Error> {
        let strategy = tokio_retry::strategy::ExponentialBackoff::from_millis(2)
            .factor(RETRY_BASE_DURATION_MS / 2)
            .map(tokio_retry::strategy::jitter)
            .take(MAX_RETRIES);

        let mut count = 0;

        let download = tokio_retry::RetryIf::spawn(
            strategy,
            || {
                count += 1;
                self.download_once(url, timestamp, original)
            },
            |error: &_| match error {
                Error::UnexpectedStatus(StatusCode::TOO_MANY_REQUESTS) => true,
                Error::UnexpectedStatus(status_code) if status_code.is_server_error() => true,
                Error::Client(error) if error.is_body() => true,
                Error::Client(_) => true,
                _ => false,
            },
        )
        .await;

        match download {
            Ok(download) => Ok(Some(download)),
            Err(Error::UnexpectedStatus(StatusCode::NOT_FOUND)) => Ok(None),
            Err(other) => Err(other),
        }
    }

    fn download_once<'a>(
        &'a self,
        url: &'a str,
        timestamp: Timestamp,
        original: bool,
    ) -> BoxFuture<'a, Result<Download, Error>> {
        async move {
            let response = self
                .client
                .get(Self::wayback_url(url, timestamp, original))
                .send()
                .await?;

            match response.status() {
                StatusCode::OK => Ok(Download {
                    bytes: response.bytes().await?,
                    redirects: vec![],
                }),
                StatusCode::FOUND => match redirect_location(&response) {
                    Some(location) => {
                        let url_parts = location
                            .parse::<UrlParts>()
                            .map_err(|_| Error::UnexpectedRedirectUrl(location.to_string()))?;

                        let mut result = self
                            .download_once(&url_parts.url, url_parts.timestamp, original)
                            .await?;

                        result.redirects.push(url_parts);

                        Ok(result)
                    }
                    None => Err(Error::UnexpectedRedirect(None)),
                },
                other => Err(Error::UnexpectedStatus(other)),
            }
        }
        .boxed()
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new(DEFAULT_REQUEST_TIMEOUT_DURATION).unwrap()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Download {
    pub bytes: Bytes,
    pub redirects: Vec<UrlParts>,
}

fn redirect_location(response: &Response) -> Option<&str> {
    response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
}
