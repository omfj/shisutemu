use axum::{Router, http::header, response::IntoResponse, routing::get};

use crate::{state::AppState, view};

const TAILWINDCSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/styles.css"));

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(view::index))
        .route("/styles.css", get(styles))
        .with_state(state)
}

async fn styles() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], TAILWINDCSS)
}
