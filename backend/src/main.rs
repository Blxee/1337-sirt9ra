use axum::{Router, extract::State, routing::get, serve};
use chrono::{DateTime, TimeZone, Utc};
use dotenvy::dotenv;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, error::Error};
use tokio::net::TcpListener;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
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
        .route("/", get(get_rankings))
        .with_state((client, access_token));

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    println!("listening at {}..", listener.local_addr().unwrap());
    serve(listener, app).await?;
    Ok(())
}

async fn get_rankings(State((client, access_token)): State<(Client, String)>) -> String {
    let mut users: Vec<CursusUser> = fetch_students(&client, &access_token).await.unwrap();

    users = users
        .into_iter()
        .filter(|s| s.begin_at.unwrap() > Utc.with_ymd_and_hms(2025, 10, 1, 0, 0, 0).unwrap())
        .collect();

    users.sort_by_key(|s| (s.level * 100.) as u64);

    let mut rankings = String::new();

    for (i, user) in users.iter().enumerate() {
        rankings.push_str(&format!(
            "{:3}(lvl-{:2.2}): {}\n",
            users.len() - i,
            user.level,
            user.user.usual_full_name.as_ref().unwrap(),
        ));
    }
    rankings
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

    let students: Vec<CursusUser> = students
        .into_iter()
        .filter(|s: &CursusUser| s.user.pool_year.as_deref() == Some("2025"))
        .collect();

    return Ok(students);
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
