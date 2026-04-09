use std::{collections::HashMap, convert::Infallible, sync::Arc};

use askama::Template;
use async_stream::stream;
use axum::{
    Json, Router,
    extract::State,
    response::sse::{Event, Sse},
    routing::get,
};
use common::{LeaderboardPayload, LeaderboardResponse};
use tokio::sync::{RwLock, broadcast};

#[derive(Clone)]
struct AppState {
    map: Arc<RwLock<HashMap<String, u32>>>,
    tx: broadcast::Sender<()>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (tx, _) = broadcast::channel(16);
    let state = AppState {
        map: Arc::new(RwLock::new(HashMap::new())),
        tx,
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/sse", get(sse_handler))
        .route("/update_tokens", axum::routing::post(update_tokens))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn sorted_entries(map: &HashMap<String, u32>) -> Vec<(String, u32)> {
    let mut entries: Vec<(String, u32)> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
}

async fn root(State(state): State<AppState>) -> LeaderboardTemplate {
    let map = state.map.read().await;
    LeaderboardTemplate { entries: sorted_entries(&map).await }
}

#[derive(Template, askama_web::WebTemplate)]
#[template(path = "leaderboard.html")]
struct LeaderboardTemplate {
    entries: Vec<(String, u32)>,
}

#[derive(Template)]
#[template(path = "leaderboard_rows.html")]
struct LeaderboardRowsTemplate {
    entries: Vec<(String, u32)>,
}

async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.tx.subscribe();

    let s = stream! {
        loop {
            let entries = {
                let map = state.map.read().await;
                sorted_entries(&map).await
            };
            let html = LeaderboardRowsTemplate { entries }.render().unwrap_or_default();
            yield Ok::<Event, Infallible>(Event::default().event("update").data(html));

            match rx.recv().await {
                Ok(_) => continue,
                Err(_) => break,
            }
        }
    };

    Sse::new(s)
}

async fn update_tokens(
    State(state): State<AppState>,
    Json(payload): Json<LeaderboardPayload>,
) -> Json<LeaderboardResponse> {
    println!("POST /update_tokens {:?}", payload);
    let mut map = state.map.write().await;
    let entry = map.entry(payload.user.clone()).or_insert(0);
    if payload.tokens > *entry {
        *entry = payload.tokens;
        drop(map);
        let _ = state.tx.send(());
    }
    Json(LeaderboardResponse { ok: true })
}
