use axum::{
    extract::State,
    response::{
        sse::{Event, Sse},
        Html, IntoResponse,
    },
    routing::get,
    Json, Router,
};
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

use crate::manager::SharedGameManager;

pub fn create_router(manager: SharedGameManager) -> Router {
    Router::new()
        .route("/", get(index_page))
        .route("/api/games", get(get_games))
        .route("/api/leaderboard", get(get_leaderboard))
        .route("/api/stream", get(sse_handler))
        .with_state(manager)
        .layer(CorsLayer::permissive())
}

async fn index_page() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn get_games(
    State(manager): State<SharedGameManager>,
) -> impl IntoResponse {
    let mgr = manager.lock().await;
    let active = mgr.get_active_games();
    let finished = mgr.get_finished_games().to_vec();

    Json(serde_json::json!({
        "active": active,
        "finished": finished,
    }))
}

async fn get_leaderboard(
    State(manager): State<SharedGameManager>,
) -> impl IntoResponse {
    let mgr = manager.lock().await;
    let leaderboard = mgr.get_leaderboard();
    Json(leaderboard)
}

async fn sse_handler(
    State(manager): State<SharedGameManager>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = manager.lock().await.broadcast_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(data) => Some(Ok(Event::default().data(data))),
        Err(_) => None,
    });
    Sse::new(stream)
}
