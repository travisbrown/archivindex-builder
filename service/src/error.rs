use rocket::{
    http::Status,
    request::Request,
    response::{Responder, Result},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("JSON error")]
    Json(#[from] serde_json::Error),
    #[error("OAuth 2.0 error")]
    Oauth2(#[from] rocket_oauth2::Error),
    #[error("SQLx error")]
    Sqlx(#[from] sqlx::Error),
    #[error("Search error")]
    Search(#[from] aib_manager::search::Error),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Authorization error")]
    Authorization(#[from] aib_auth::Error<aib_auth_sqlx::Error>),
    #[error("Google OpenID error")]
    GoogleOpenId(#[from] aib_auth::google::Error),
    #[error("Twitter OAuth error")]
    TwitterOAuth(#[from] aib_auth::twitter::Error),
}

impl<'r, 'o: 'r> Responder<'r, 'o> for Error {
    fn respond_to(self, req: &'r Request<'_>) -> Result<'o> {
        match self {
            Self::Unauthorized => Status::Unauthorized.respond_to(req),
            _ => Status::InternalServerError.respond_to(req),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InitError {
    #[error("Index error")]
    Index(#[from] aib_indexer::Error),
    #[error("Missing config")]
    MissingConfig,
    #[error("SQLx error")]
    Sqlx(#[from] sqlx::Error),
}
