use axum::{
    Json, Router,
    extract::{Query, State},
    http::{self, HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect},
    routing::get,
    serve,
};
use chrono::{DateTime, Duration, TimeZone, Utc};
use dotenvy::dotenv;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{
    prelude::FromRow,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{env, error::Error, str::FromStr, time::Instant};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Debug, FromRow)]
struct Usur {
    id: i64,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").unwrap();
    let options = SqliteConnectOptions::from_str(&database_url)
        .unwrap()
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    // 3. Create a temporary table for this demonstration
    sqlx::query!(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE
         );",
    )
    .execute(&pool)
    .await?;

    // 4. Insert data using runtime parameter binding (prevents SQL injection)
    let name = "Alice";
    let email = "alice@example.com";

    sqlx::query!(
        "INSERT OR IGNORE INTO users (name, email) VALUES (?, ?)",
        name,
        email
    )
    .execute(&pool)
    .await?;

    // 5. Query and map data to your struct using query_as
    let users = sqlx::query_as::<_, Usur>("SELECT id, name, email FROM users")
        .fetch_all(&pool)
        .await?;

    println!("Retrieved Users: {:#?}", users);
    return Ok(());

    // Replace these with your actual 42 API application credentials
    let client_id = env::var("INTRA_42_CLIENT_ID").unwrap();
    let client_secret = env::var("INTRA_42_CLIENT_SECRET").unwrap();

    let client = Client::new();

    // 1. Authenticate and get the access token via POST request
    let token_params = [
        ("grant_type", "client_credentials"),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
    ];

    let token_res = client
        .post("https://api.intra.42.fr/oauth/token")
        .form(&token_params)
        .send()
        .await?;

    let token_data: TokenResponse = token_res.json().await?;
    let access_token = token_data.access_token;

    let app = Router::new()
        .fallback_service(ServeDir::new("assets"))
        .route("/api/rankings", get(get_rankings))
        .route("/api/auth", get(login_handler))
        .route("/api/callback", get(callback_handler))
        .route("/api/avatar", get(proxy_avatar))
        .with_state((client, access_token));

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    println!("listening at {}..", listener.local_addr().unwrap());
    serve(listener, app).await?;
    Ok(())
}

#[derive(Serialize)]
struct Rank {
    name: String,
    level: f64,
    image: String,
}

async fn get_rankings(State((client, access_token)): State<(Client, String)>) -> Json<Vec<Rank>> {
    let mut users: Vec<CursusUser> = fetch_students(&client, &access_token).await.unwrap();

    // users = users
    //     .into_iter()
    //     .filter(|s| s.begin_at.unwrap() > Utc.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap())
    //     .collect();

    users.sort_by_key(|s| (s.level * 100.) as u64);

    let mut rankings = Vec::new();

    for user in users.into_iter() {
        rankings.push(Rank {
            name: user.user.usual_full_name.unwrap(),
            level: user.level,
            image: user.user.image.unwrap().versions.unwrap().small.unwrap(),
        });
    }
    Json(rankings)
}

async fn fetch_students(
    client: &Client,
    access_token: &String,
) -> Result<Vec<CursusUser>, Box<dyn Error>> {
    let mut students = Vec::new();

    for page in 1.. {
        let api_res = client
            .get("https://api.intra.42.fr/v2/cursus_users")
            .query(&[
                ("filter[campus_id]", "16"),
                ("filter[cursus_id]", "21"),
                ("page[size]", "100"),
                ("page[number]", &page.to_string()),
            ])
            .bearer_auth(&access_token)
            .send()
            .await?;

        let users: Vec<CursusUser> = api_res.json().await?;

        if users.len() == 0 {
            break;
        }

        students.extend(users);
        break;
    }

    // let students: Vec<CursusUser> = students
    //     .into_iter()
    //     .filter(|s: &CursusUser| s.user.pool_year.as_deref() == Some("2025"))
    //     .collect();

    return Ok(students);
}

pub async fn login_handler() -> Redirect {
    let client_id = env::var("INTRA_42_CLIENT_ID").unwrap();

    let auth_url = format!(
        "https://api.intra.42.fr/oauth/authorize?client_id={}&redirect_uri={}&response_type=code",
        client_id,
        urlencoding::encode("http://localhost:8080/api/callback")
    );
    Redirect::temporary(&auth_url)
}

#[derive(Deserialize, Debug)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct CallbackResult {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: String,
    scope: String,
    created_at: u64,
    secret_valid_until: u64,
}

pub async fn callback_handler(
    Query(query): Query<CallbackQuery>,
    State((client, _)): State<(Client, String)>,
) -> impl IntoResponse {
    let code = query.code.unwrap();
    let client_id = env::var("INTRA_42_CLIENT_ID").unwrap();
    let client_secret = env::var("INTRA_42_CLIENT_SECRET").unwrap();
    let redirect_uri = "http://localhost:8080/api/callback";

    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("code", &code),
        ("redirect_uri", redirect_uri),
    ];

    let res: CallbackResult = client
        .post("https://api.intra.42.fr/oauth/token")
        .form(&params)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let mut headers = HeaderMap::new();
    let cookie_value = format!(
        "user_token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        res.access_token, res.expires_in
    );
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie_value).unwrap(),
    );
    headers.insert(header::LOCATION, HeaderValue::from_static("/"));
    return (StatusCode::FOUND, headers).into_response();
}

#[derive(Deserialize)]
pub struct AvatarQuery {
    pub url: String,
}

pub async fn proxy_avatar(
    Query(params): Query<AvatarQuery>,
    State((client, access_token)): State<(Client, String)>,
) -> impl IntoResponse {
    // 1. Restrict proxying strictly to 42 CDN URLs for security
    if !params.url.starts_with("https://cdn.intra.42.fr/") {
        return (StatusCode::BAD_REQUEST, "Invalid image URL").into_response();
    }

    // 2. Fetch the image server-side with Bearer auth, User-Agent, and 42 Referer
    let res = match client
        .get(&params.url)
        .bearer_auth(&access_token)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => resp,
        Ok(resp) => {
            println!(
                "CDN returned status: {} for URL: {}",
                resp.status(),
                params.url
            );
            return (
                StatusCode::BAD_GATEWAY,
                format!("CDN error: {}", resp.status()),
            )
                .into_response();
        }
        Err(e) => {
            println!("Network error fetching avatar: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Network error: {}", e),
            )
                .into_response();
        }
    };

    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("image/jpeg"));

    let bytes = match res.bytes().await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read image bytes",
            )
                .into_response();
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );

    (StatusCode::OK, headers, bytes).into_response()
}

// ##################

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursusUser {
    pub id: u64,
    pub begin_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub grade: Option<String>,
    pub level: f64,
    pub skills: Vec<serde_json::Value>,
    pub cursus_id: u64,
    pub has_coalition: bool,
    pub blackholed_at: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub user: User,
    pub cursus: Cursus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub email: String,
    pub login: String,
    pub first_name: String,
    pub last_name: String,
    pub usual_full_name: Option<String>,
    pub usual_first_name: Option<String>,
    pub url: String,
    pub phone: Option<String>,
    pub displayname: String,
    pub kind: String,
    pub image: Option<UserImage>,

    #[serde(rename = "staff?")]
    pub is_staff: bool,

    pub correction_point: i64,
    pub pool_month: Option<String>,
    pub pool_year: Option<String>,
    pub location: Option<String>,
    pub wallet: i64,
    pub anonymize_date: Option<DateTime<Utc>>,
    pub data_erasure_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub alumnized_at: Option<DateTime<Utc>>,

    #[serde(rename = "alumni?")]
    pub is_alumni: bool,

    #[serde(rename = "active?")]
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserImage {
    pub link: Option<String>,
    pub versions: Option<ImageVersions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageVersions {
    pub large: Option<String>,
    pub medium: Option<String>,
    pub small: Option<String>,
    pub micro: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursus {
    pub id: u64,
    pub created_at: DateTime<Utc>,
    pub name: String,
    pub slug: String,
    pub kind: String,
}
