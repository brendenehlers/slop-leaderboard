use std::{collections::HashMap, sync::Arc};

use axum::{Json, Router, extract::State, routing::get};
use common::{LeaderboardPayload, LeaderboardResponse};
use tokio::sync::RwLock;

type AppState = Arc<RwLock<HashMap<String, u32>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state: AppState = Arc::new(RwLock::new(HashMap::new()));

    let app = Router::new()
        .route("/", get(root))
        .route("/update_tokens", axum::routing::post(update_tokens))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root(State(state): State<AppState>) -> LeaderboardTemplate {
    let map = state.read().await;
    let mut entries: Vec<(String, u32)> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    LeaderboardTemplate { entries }
}

#[derive(askama::Template, askama_web::WebTemplate)]
#[template(path = "leaderboard.html")]
struct LeaderboardTemplate {
    entries: Vec<(String, u32)>,
}

async fn update_tokens(
    State(state): State<AppState>,
    Json(payload): Json<LeaderboardPayload>,
) -> Json<common::LeaderboardResponse> {
    let mut map = state.write().await;
    let entry = map.entry(payload.user.clone()).or_insert(0);
    if payload.tokens > *entry {
        *entry = payload.tokens;
    }

    Json(LeaderboardResponse { ok: true })
}
