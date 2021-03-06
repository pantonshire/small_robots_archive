mod clone_data;
mod respond;
mod error;
mod page;
mod services;
mod robots;
mod search;

use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::ops::Add;

use actix_web::{self as aw, get, HttpServer, App, web, HttpRequest};
use actix_files as fs;
use sqlx::postgres::PgPool;
use maud::{html, PreEscaped};
use serde::Deserialize;

use clone_data::CloneData;
use error::{SiteError, SiteReportError, SiteReportResult, IntoReport};
use respond::MarkupResponse;
use robots::{Linkable, Named, Displayable, RobotPreview, RobotFull};

const DEFAULT_BIND_ADDR: &str = "[::1]:8080";

const BIND_ADDR_VAR: &str = "BIND_ADDRESS";
const DB_URL_VAR: &str = "DATABASE_URL";
const ARCHIVE_META_NAME_VAR: &str = "SBB_ARCHIVE_META_NAME";
const ARCHIVE_META_URL_PREFIX_VAR: &str = "SBB_ARCHIVE_META_URL_PREFIX";

const THH_BOOK_URL: &str
    = "https://www.hive.co.uk/Product/Thomas-Heasman-Hunt/Small-Robots--A-collection-of-one-hundred-mostly-useful-robot-friends/24078313";
const THH_REDBUBBLE_URL: &str
    = "https://www.redbubble.com/people/smolrobots/shop";
const THH_PATREON_URL: &str
    = "https://www.patreon.com/thomasheasmanhunt/posts";
const THH_COMMISION_URL: &str
    = "https://docs.google.com/forms/d/e/1FAIpQLSfQBDf0no0bVolIk90sgiMTHL9PpvVwDjGh6hOegCsPe4TXZg/viewform";

#[derive(Debug)]
enum ServerError {
    DbError(Box<sqlx::Error>),
    IoError(Box<io::Error>),
    EnvError(Box<env::VarError>),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::DbError(err) => err.fmt(f),
            Self::IoError(err) => err.fmt(f),
            Self::EnvError(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<sqlx::Error> for ServerError {
    fn from(err: sqlx::Error) -> Self {
        Self::DbError(Box::new(err))
    }
}

impl From<io::Error> for ServerError {
    fn from(err: io::Error) -> Self {
        Self::IoError(Box::new(err))
    }
}

impl From<env::VarError> for ServerError {
    fn from(err: env::VarError) -> Self {
        Self::EnvError(Box::new(err))
    }
}

#[get("/")]
async fn landing_page(pool: CloneData<PgPool>) -> SiteReportResult<MarkupResponse> {
    let latest: Vec<RobotPreview> = sqlx::query_as(
        "SELECT \
            id, robot_number, ident, prefix, suffix, plural, content_warning, image_thumb_path, \
            alt, custom_alt \
        FROM robots \
        ORDER BY tweet_time DESC \
        LIMIT 20"
    )
    .fetch_all(&*pool)
    .await
    .map_err(|err| err.into_report("failed to get latest robots"))?;

    Ok(MarkupResponse::ok(page::archive_page(
        "Small Robots Archive",
        html! {
            div class="section" {
                p {
                    "Welcome to the Small Robots Archive, a fan-made site dedicated to all of the 
                    mechanical friends drawn by the wonderful "
                    a class="link_text" href="https://twitter.com/smolrobots" { "@smolrobots" }
                    "."
                }

                p {
                    "If you'd like to support "
                    a class="link_text" href="https://twitter.com/smolrobots" { "@smolrobots" }
                    ", you can:"
                }

                ul {
                    li { a class="link_text" href=(THH_BOOK_URL) { "Buy their book!" } }
                    li { a class="link_text" href=(THH_REDBUBBLE_URL) { "Visit their Redbubble shop!" } }
                    li { a class="link_text" href=(THH_PATREON_URL) { "Become a patron!" } }
                    li { a class="link_text" href=(THH_COMMISION_URL) { "Commission your very own small robot!!!" } }
                }
            }

            div class="section" {
                h2 { "Recent robots" }
                ul class="robots_row" {
                    @for robot in &latest {
                        li class="robot_container" {
                            a href=(robot.page_link()) class="link_area" {
                                @if let Some(image_resource_url) = robot.image_resource_url() {
                                    img
                                        src=(image_resource_url)
                                        alt=(robot.image_alt())
                                        draggable="false";
                                } @else {
                                    img alt="Image not found";
                                }
                                h3 { (robot.full_name()) }
                                h3 class="robot_number" { "#"(robot.robot_number) }
                            }
                        }
                    }
                }

                p {
                    a class="link_text" href="/all" { "See all robots" }
                }
            }
        }
    )))
}

//TODO: render content warnings
async fn render_all_robots(pool: PgPool, page: u32) -> SiteReportResult<MarkupResponse> {
    const PAGE_SIZE: u32 = 48;

    let num_robots: robots::Count = sqlx::query_as("SELECT COUNT(*) AS count FROM robots")
        .fetch_one(&pool)
        .await
        .map_err(|err| err.into_report("failed to count rows in robots table"))?;

    let num_pages = num_robots.pages(PAGE_SIZE);

    let limit = PAGE_SIZE as i64;
    let offset = (PAGE_SIZE * page) as i64;

    let robots: Vec<RobotPreview> = sqlx::query_as(
        "SELECT \
            id, robot_number, ident, prefix, suffix, plural, content_warning, image_thumb_path, \
            alt, custom_alt \
        FROM robots \
        ORDER BY robot_number, id \
        LIMIT $1 \
        OFFSET $2"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|err| err.into_report(format!("failed to get robots page {}", page)))?;

    let pagination = Pagination::try_new(page, num_pages);

    let pagination_menu = pagination.map(|pagination| html! {
        nav class="pagination" {
            ul {
                li class="pagination_item_major" {
                    @if let Some(prev_page) = pagination.prev_page {
                        a class="pagination_number_other" href=(format!("/all/{}", prev_page.add(1))) { "Previous" }
                    } @else {
                        span class="pagination_disabled no_select" { "Previous" }
                    }
                }

                @if let Some(first_page) = pagination.first_page {
                    li class="pagination_item_minor" {
                        a class="pagination_number_other" href=(format!("/all/{}", first_page.add(1))) { (first_page.add(1)) }
                    }

                    li class="pagination_item_minor" {
                        span class="pagination_elipsis no_select" { (PreEscaped("&hellip;")) }
                    }
                }

                @for n in pagination.min_range_page .. pagination.current_page {
                    li class="pagination_item_minor" {
                        a class="pagination_number_other" href=(format!("/all/{}", n.add(1))) { (n.add(1)) }
                    }
                }

                li class="pagination_item_minor" {
                    span class="pagination_number_current no_select" { (pagination.current_page.add(1)) }
                }

                @for n in (pagination.current_page ..= pagination.max_range_page).skip(1) {
                    li class="pagination_item_minor" {
                        a class="pagination_number_other" href=(format!("/all/{}", n.add(1))) { (n.add(1)) }
                    }
                }

                @if let Some(last_page) = pagination.last_page {
                    li class="pagination_item_minor" {
                        span class="pagination_elipsis no_select" { (PreEscaped("&hellip;")) }
                    }

                    li class="pagination_item_minor" {
                        a class="pagination_number_other" href=(format!("/all/{}", last_page.add(1))) { (last_page.add(1)) }
                    }
                }

                li class="pagination_item_major" {
                    @if let Some(next_page) = pagination.next_page {
                        a class="pagination_number_other" href=(format!("/all/{}", next_page.add(1))) { "Next" }
                    }  @else {
                        span class="pagination_disabled no_select" { "Next" }
                    }
                }
            }
        }
    });

    Ok(MarkupResponse::ok(page::archive_page(
        "All robots",
        html! {
            div class="section" {
                h2 { "All robots" }
                ul class="robots_grid" {
                    @for robot in &robots {
                        li class="robot_container" {
                            a href=(robot.page_link()) class="link_area" {
                                @if let Some(image_resource_url) = robot.image_resource_url() {
                                    img
                                        src=(image_resource_url)
                                        alt=(robot.image_alt())
                                        draggable="false";
                                } @else {
                                    img alt="Image not found";
                                }
                                h3 { (robot.full_name()) }
                                h3 class="robot_number" { "#"(robot.robot_number) }
                            }
                        }
                    }
                }
            }

            @if let Some(pagination_menu) = pagination_menu {
                div class="section" {
                    (pagination_menu)
                }
            }
        }
    )))
}

#[get("/all")]
async fn all_robots(pool: CloneData<PgPool>) -> SiteReportResult<MarkupResponse> {
    render_all_robots(pool.inner, 0).await
}

#[get("/all/{page}")]
async fn all_robots_paged(pool: CloneData<PgPool>, page: web::Path<u32>) -> SiteReportResult<MarkupResponse> {
    let page = page.into_inner();

    let page = page
        .checked_sub(1)
        .ok_or_else(|| SiteError::BadRequest.report(format!("invalid page number {}", page)))?;

    render_all_robots(pool.inner, page).await
}

#[derive(Deserialize)]
struct SearchQuery {
    query: String,
}

#[get("/search")]
async fn search_robots(pool: CloneData<PgPool>, query: web::Query<SearchQuery>) -> SiteReportResult<MarkupResponse> {
    const MAX_QUERY_CHARS: usize = 64;

    let search_query = query.query.chars().take(MAX_QUERY_CHARS).collect::<String>();

    let robots = search::search(&*pool, &search_query).await?;

    Ok(MarkupResponse::ok(page::archive_page(
        "All robots",
        html! {
            div class="section" {
                h2 class="word_break" { "Search results for \"" (search_query) "\"" }
                ul class="robots_grid" {
                    @for robot in &robots {
                        li class="robot_container" {
                            a href=(robot.page_link()) class="link_area" {
                                @if let Some(image_resource_url) = robot.image_resource_url() {
                                    img
                                        src=(image_resource_url)
                                        alt=(robot.image_alt())
                                        draggable="false";
                                } @else {
                                    img alt="Image not found";
                                }
                                h3 { (robot.full_name()) }
                                h3 class="robot_number" { "#"(robot.robot_number) }
                            }
                        }
                    }
                }
            }
        }
    )))
}

fn render_robot(meta: &InstanceMeta, robot: RobotFull) -> MarkupResponse {
    let full_name = robot.full_name();

    let tweet_link = format!("https://twitter.com/smolrobots/status/{}", robot.tweet_id);

    let permalink = meta.url_prefix
        .as_deref()
        .map(|prefix| format!("{}/robot/{}/{}", prefix, robot.robot_number, robot.ident));

    let robot_content = html! {
        div class="robot_content" {
            @if let Some(image_resource_url) = robot.image_resource_url() {
                div class="robot_image_full_container" {
                    a href=(tweet_link) {
                        img
                            class="robot_image_full"
                            src=(image_resource_url)
                            alt=(robot.image_alt())
                            draggable="false";
                    }
                }
            }

            div class="robot_description" {
                p {
                    (robot.body)
                }

                p {
                    a class="link_text" href=(tweet_link) { "Go to original Tweet" }
                }

                @if let Some(permalink) = permalink {
                    p {
                        "Permalink: " a class="link_text" href=(permalink) { (permalink) }
                    }
                }
            }
        }
    };

    MarkupResponse::ok(page::archive_page(
        &full_name,
        html! {
            div class="section" {
                h2 class="robot_title word_break" {
                    span class="robot_number" { "#" (robot.robot_number) } " " (full_name)
                }

                @match robot.content_warning.as_deref() {
                    Some(content_warning) => {
                        details {
                            summary { "(Click to expand) Content warning: " (content_warning) }
                            (robot_content)
                        }
                    }

                    None => {
                        (robot_content)
                    }
                }
            }
        }
    ))
}

#[get("/robot/{number}/{ident}")]
async fn robot_page(
    meta: web::Data<InstanceMeta>,
    pool: CloneData<PgPool>,
    path: web::Path<(i32, String)>
) -> SiteReportResult<MarkupResponse>
{
    let (number, ident) = path.into_inner();

    let robot: RobotFull = sqlx::query_as(
        "SELECT \
            id, robot_number, ident, prefix, suffix, plural, content_warning, image_path, \
            alt, custom_alt, body, tweet_id \
        FROM robots \
        WHERE (robot_number, ident) = ($1, $2)"
    )
    .bind(number)
    .bind(&ident)
    .fetch_optional(&*pool)
    .await
    .map_err(SiteError::from)
    .and_then(|robot| robot.ok_or(SiteError::NotFound))
    .map_err(|err| err.report(format!("failed to get robot {}/{}", number, ident)))?;

    Ok(render_robot(&meta, robot))
}

#[get("/daily")]
async fn daily_robot(
    meta: web::Data<InstanceMeta>,
    pool: CloneData<PgPool>
) -> SiteReportResult<MarkupResponse>
{
    let robot: RobotFull = sqlx::query_as(
        "SELECT \
            id, robot_number, ident, prefix, suffix, plural, content_warning, image_path, \
            alt, custom_alt, body, tweet_id \
        FROM robots \
        WHERE id IN (SELECT robot_id FROM past_dailies ORDER BY posted_on DESC LIMIT 1) \
        LIMIT 1",
    )
    .fetch_one(&*pool)
    .await
    .map_err(|err| err.into_report("failed to get daily robot"))?;

    Ok(render_robot(&meta, robot))
}

#[get("/random")]
async fn random_robot(
    meta: web::Data<InstanceMeta>,
    pool: CloneData<PgPool>
) -> SiteReportResult<MarkupResponse>
{
    let robot: RobotFull = sqlx::query_as(
        "SELECT \
            id, robot_number, ident, prefix, suffix, plural, content_warning, image_path, \
            alt, custom_alt, body, tweet_id \
        FROM robots \
        LIMIT 1 \
        OFFSET FLOOR(RANDOM() * (SELECT COUNT (*) FROM robots))",
    )
    .fetch_one(&*pool)
    .await
    .map_err(|err| err.into_report("failed to get random robot"))?;

    Ok(render_robot(&meta, robot))
}

#[get("/about")]
async fn about_page(meta: web::Data<InstanceMeta>) -> MarkupResponse {
    let instance_name = meta.name
        .as_deref()
        .unwrap_or("[not set]");

    MarkupResponse::ok(page::archive_page(
        "About",
        html! {
            div class="section" {
                h2 id="about" { "About this site" }
                p {
                    "This is a fan-made site archiving the robots drawn by "
                    a class="link_text" href="https://twitter.com/smolrobots" { "@smolrobots" }
                    " on Twitter! " (PreEscaped("&#129302;"))
                }
                p {
                    "It is a continuation of the wonderful "
                    a class="link_text" href="https://twitter.com/aguitarpenter" { "@aguitarpenter" }
                    "'s "
                    a class="link_text" href="https://smolrobots.snekkeren.co.uk" { "original archive site" }
                    ", so special thanks to him for starting this whole thing!!"
                }
            }
            div class="section" {
                h2 id="issues" { "Reporting issues" }
                p {
                    "If you find something wrong with this site, please "
                    a class="link_text" href="https://github.com/Pantonshire/small_robots_archive/issues/new" { "open an issue" }
                    " on GitHub."
                }
            }
            div class="section" {
                h2 id="setup" { "Setting up your own archive instance" }
                p {
                    "This is an open-source project; the source code can be found at "
                    a class="link_text" href="https://github.com/Pantonshire/small_robots_archive" { "Pantonshire/small_robots_archive" }
                    " on GitHub. You are free to clone the repository and set up your own instance of this archive!"
                }
                p {
                    "If you would like to set up your own instance, see "
                    a class="link_text" href="https://github.com/Pantonshire/SmolBotBot/blob/main/README.md" { "the README in the SmolBotBot repository" }
                    " ."
                }
            }
            div class="section" {
                h2 id="contact" { "Contact" }
                p {
                    "If you'd like to contact me directly, I'm "
                    a class="link_text" href="https://twitter.com/pantonshire" { "@pantonshire" }
                    " on Twitter or "
                    a class="link_text" href="https://tech.lgbt/@pantonshire" { "@pantonshire@tech.lgbt" }
                    " on Mastodon."
                }
            }
            div class="section" {
                h2 id="instance" { "Instance info" }
                ul {
                    li { "Instance name: " (instance_name) }
                }
            }
        }
    ))
}

#[get("/bootstrap/ids")]
async fn bootstrap_ids() -> aw::Result<fs::NamedFile> {
    fs::NamedFile::open("./generated/bootstrap/ids")
        .map_err(aw::Error::from)
        .map(|f| f.set_content_type(mime::TEXT_PLAIN))
}

#[get("/bootstrap/alt")]
async fn bootstrap_alt() -> aw::Result<fs::NamedFile> {
    fs::NamedFile::open("./generated/bootstrap/alt.json")
        .map_err(aw::Error::from)
        .map(|f| f.set_content_type(mime::APPLICATION_JSON))
}

async fn not_found(req: HttpRequest) -> SiteReportError {
    SiteError::NotFound
        .report(req.path().to_owned())
}

#[derive(Clone, Debug)]
struct Pagination {
    current_page: u32,
    min_range_page: u32,
    max_range_page: u32,
    first_page: Option<u32>,
    last_page: Option<u32>,
    prev_page: Option<u32>,
    next_page: Option<u32>,
}

impl Pagination {
    /// Pages are zero-indexed
    fn try_new(current_page: u32, num_pages: u32) -> Option<Self> {
        const TOTAL_SPACES: u32 = 9;
        const ADJACENT_SPACES: u32 = TOTAL_SPACES / 2;

        if num_pages <= current_page {
            return None;
        }

        let last_page = num_pages - 1;
        let prev_page = (current_page > 0).then(|| current_page - 1);
        let next_page = (current_page < last_page).then(|| current_page + 1);

        if num_pages <= TOTAL_SPACES {
            return Some(Pagination {
                current_page,
                min_range_page: 0,
                max_range_page: last_page,
                first_page: None,
                last_page: None,
                prev_page,
                next_page,
            });
        }

        let centre = current_page
            .clamp(ADJACENT_SPACES, last_page - ADJACENT_SPACES);

        let (min_range_page, first_page) = match centre - ADJACENT_SPACES {
            min if min > 0 => (min + 1, Some(0)),
            min => (min, None),
        };

        let (max_range_page, last_page) = match centre + ADJACENT_SPACES {
            max if max < last_page => (max - 1, Some(last_page)),
            max => (max, None),
        };
    
        Some(Pagination {
            current_page,
            min_range_page,
            max_range_page,
            first_page,
            last_page,
            prev_page,
            next_page,
        })
    }
}

/// Metadata about this instance of the Small Robots Archive.
#[derive(Clone, Debug)]
struct InstanceMeta {
    /// A name for this specific instance of the archive.
    name: Option<String>,

    /// The scheme to use for permalinks.
    url_prefix: Option<String>,
}

impl InstanceMeta {
    fn new_env() -> Result<InstanceMeta, ServerError> {
        Ok(InstanceMeta {
            name: env_var_opt(ARCHIVE_META_NAME_VAR)?,
            url_prefix: env_var_opt(ARCHIVE_META_URL_PREFIX_VAR)?,
        })
    }
}

fn env_var_opt<K>(key: K) -> Result<Option<String>, env::VarError>
where
    K: AsRef<OsStr>
{
    match env::var(key) {
        Ok(val) => match val.is_empty() {
            true => Ok(None),
            false => Ok(Some(val)),
        },
        Err(env::VarError::NotPresent) => Ok(None),
        Err(err @ env::VarError::NotUnicode(_)) => Err(err),
    }
}

#[aw::main]
async fn main() -> Result<(), ServerError> {
    #[cfg(feature = "dotenv")] {
        dotenv::dotenv().ok();
    }

    env_logger::init();

    let instance_meta = {
        let meta = InstanceMeta::new_env()?;
        web::Data::new(meta)
    };

    let pool = {
        let db_url = env::var(DB_URL_VAR)?;
        PgPool::connect(&db_url).await?
    };

    let app_factory = move || {
        App::new()
            .app_data(instance_meta.clone())
            .app_data(CloneData::new(pool.clone()))
            .service(fs::Files::new("/static", "./static"))
            .service(fs::Files::new("/robot_images", "./generated/robot_images"))
            .service(bootstrap_ids)
            .service(bootstrap_alt)
            .service(landing_page)
            .service(all_robots)
            .service(all_robots_paged)
            .service(robot_page)
            .service(search_robots)
            .service(daily_robot)
            .service(random_robot)
            .service(about_page)
            .default_service(web::route().to(not_found))
    };

    let http_server = HttpServer::new(app_factory);

    let http_server = match env::var(BIND_ADDR_VAR) {
        Ok(addrs) => {
            let mut http_server = http_server;
            for addr in addrs.split_whitespace() {
                http_server = http_server.bind(addr)?;
            }
            http_server
        },
        Err(env::VarError::NotPresent) => http_server.bind(DEFAULT_BIND_ADDR)?,
        Err(err) => return Err(err.into()),
    };

    http_server
        .run()
        .await
        .map_err(ServerError::from)
}
