#[macro_use]
extern crate rocket;

use aib_auth::{
    model::{
        providers::{GitHub, Google, Twitter},
        IsProvider,
    },
    Authorizer,
};
use aib_auth_sqlx::SqlxAuthDb;
use aib_indexer::{query::Range, Index};
use aib_manager::model::Pattern;
use rocket::{
    fairing::{AdHoc, Fairing},
    http::CookieJar,
    serde::json::Json,
    Build, Rocket, State,
};
use rocket_db_pools::{Connection, Database as PoolDatabase};
use rocket_oauth2::{OAuth2, OAuthConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod auth;
mod error;
mod result;
mod time;

use time::NaiveDateParam;

const DEFAULT_SEARCH_LIMIT: usize = 10;
const DEFAULT_SEARCH_SNIPPET_MAX_CHARS: usize = 200;
const DEFAULT_FIRST_YEAR: u16 = 2004;
const USER_AGENT: &str = "archivindex-builder";

fn provider_fairing<P: IsProvider>() -> impl Fairing {
    OAuth2::<P>::fairing(P::provider().name())
}

#[derive(Deserialize)]
pub struct AppConfig {
    domain: Option<String>,
    authorization: String,
    index: PathBuf,
    default_login_redirect_uri: rocket::http::uri::Reference<'static>,
}

#[derive(PoolDatabase)]
#[database("sqlite_auth")]
pub struct AuthDb(sqlx::SqlitePool);

#[derive(PoolDatabase)]
#[database("sqlite_data")]
pub struct DataDb(sqlx::SqlitePool);

type SqliteAuthorizer = Authorizer<SqlxAuthDb>;

#[get("/patterns")]
async fn patterns(
    mut data_db_connection: Connection<DataDb>,
) -> Result<Json<Vec<Pattern>>, error::Error> {
    Ok(Json(
        aib_manager::db::pattern::get_all(&mut *data_db_connection.as_mut()).await?,
    ))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Query {
    #[serde(rename = "searchTerm")]
    search_term: String,
    current: usize,
    filters: Vec<Filter>,
    #[serde(rename = "resultsPerPage")]
    results_per_page: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Filter {
    field: String,
    values: Vec<String>,
    #[serde(rename = "type")]
    filter_type: String,
}

#[post("/search", data = "<query>")]
async fn search_post(
    query: Json<Query>,
    cookies: &CookieJar<'_>,
    index: &State<Index>,
    auth_db_connection: Connection<AuthDb>,
    mut data_db_connection: Connection<DataDb>,
    authorizer: &State<SqliteAuthorizer>,
) -> Result<Json<result::SearchResult>, error::Error> {
    let db = aib_manager::db::Db::new(&mut data_db_connection);

    let index_query = aib_indexer::Query::new(
        &query.search_term,
        None,
        None,
        query
            .0
            .filters
            .iter()
            .filter(|filter| filter.field == "pattern")
            .flat_map(|filter| &filter.values)
            .cloned()
            .collect(),
        query
            .0
            .filters
            .iter()
            .filter(|filter| filter.field == "year")
            .flat_map(|filter| {
                filter
                    .values
                    .iter()
                    .map(|value| value.parse::<u16>().unwrap_or(0))
            })
            .collect(),
    );

    let search_result = aib_manager::search::search(
        index,
        db,
        DEFAULT_SEARCH_SNIPPET_MAX_CHARS,
        &index_query,
        query.0.results_per_page,
        (query.0.current - 1) * query.0.results_per_page,
    )
    .await;

    if let Err(error) = &search_result {
        log::error!("{:?}", error);
    }

    let search_result = search_result?;

    Ok(Json(search_result.into()))
}

#[get("/search?<query>&<email>&<start>&<end>&<pattern>&<year>&<limit>&<offset>")]
async fn search(
    query: String,
    email: Option<String>,
    start: Option<NaiveDateParam>,
    end: Option<NaiveDateParam>,
    pattern: Option<Vec<String>>,
    year: Option<Vec<u16>>,
    limit: Option<usize>,
    offset: Option<usize>,
    cookies: &CookieJar<'_>,
    index: &State<Index>,
    auth_db_connection: Connection<AuthDb>,
    mut data_db_connection: Connection<DataDb>,
    authorizer: &State<SqliteAuthorizer>,
) -> Result<Json<result::SearchResult>, error::Error> {
    let db = aib_manager::db::Db::new(&mut data_db_connection);

    let date_range = Range::new(start, end).map(|range| range.map(|value| value.into()));

    let query = aib_indexer::Query::new(
        &query,
        email.as_deref(),
        date_range,
        pattern.unwrap_or_default(),
        year.unwrap_or_default(),
    );

    let search_result = aib_manager::search::search(
        index,
        db,
        DEFAULT_SEARCH_SNIPPET_MAX_CHARS,
        &query,
        limit.unwrap_or(DEFAULT_SEARCH_LIMIT),
        offset.unwrap_or(0),
    )
    .await;

    if let Err(error) = &search_result {
        log::error!("{:?}", error);
    }

    let search_result = search_result?;

    Ok(Json(search_result.into()))
}

#[launch]
fn rocket() -> _ {
    let cors = rocket_cors::CorsOptions::default()
        .allowed_origins(rocket_cors::AllowedOrigins::all())
        .allowed_methods(
            vec![
                rocket::http::Method::Get,
                rocket::http::Method::Post,
                rocket::http::Method::Patch,
            ]
            .into_iter()
            .map(From::from)
            .collect(),
        )
        .allow_credentials(true);

    rocket::build()
        .attach(AdHoc::config::<AppConfig>())
        .attach(AdHoc::try_on_ignite(
            "Open authorization databases",
            |rocket| async {
                match init_authorization(&rocket).await {
                    Some(authorizer) => Ok(rocket.manage(authorizer)),
                    None => Err(rocket),
                }
            },
        ))
        .attach(AuthDb::init())
        .attach(DataDb::init())
        .attach(AdHoc::try_on_ignite("index", |rocket| async {
            match init_index(&rocket).await {
                Ok(index) => Ok(rocket.manage(index)),
                Err(error) => {
                    log::error!("{:?}", error);
                    Err(rocket)
                }
            }
        }))
        .attach(cors.to_cors().unwrap())
        .attach(provider_fairing::<GitHub>())
        .attach(provider_fairing::<Google>())
        .attach(provider_fairing::<Twitter>())
        .mount(
            "/",
            routes![
                patterns,
                search,
                search_post,
                auth::login::status,
                auth::login::logout,
                auth::login::github,
                auth::login::google,
                auth::login::twitter,
                auth::callback::github,
                auth::callback::google,
                auth::callback::twitter,
            ],
        )
}

async fn init_index(rocket: &Rocket<Build>) -> Result<Index, error::InitError> {
    let config = rocket
        .state::<AppConfig>()
        .ok_or_else(|| error::InitError::MissingConfig)?;

    let data_db = DataDb::fetch(rocket).expect("Database not initialized");

    let patterns = aib_manager::db::pattern::get_all(&mut *data_db.acquire().await?).await?;
    let pattern_slugs = patterns
        .iter()
        .map(|pattern| pattern.slug.as_str())
        .collect::<Vec<_>>();

    let mut index = Index::open(&config.index, &pattern_slugs, DEFAULT_FIRST_YEAR)?;
    log::info!("Created index");
    index.initialize_surt_ids()?;
    log::info!("Initialized SURT IDs");

    Ok(index)
}

async fn init_authorization(rocket: &Rocket<Build>) -> Option<SqliteAuthorizer> {
    let google_config = OAuthConfig::from_figment(rocket.figment(), "google").ok()?;
    let twitter_config = OAuthConfig::from_figment(rocket.figment(), "twitter").ok()?;
    let config = rocket.state::<AppConfig>()?;

    Authorizer::open(
        &config.authorization,
        USER_AGENT,
        google_config.client_id(),
        google_config.client_secret(),
        twitter_config.client_id(),
        twitter_config.client_secret(),
        twitter_config.redirect_uri()?,
    )
    .await
    .ok()
}
