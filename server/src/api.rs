use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use ssmgr_shared::{ApiResponse, Sample, ScanDir, StrudelConfig};
use serde::Deserialize;
use std::collections::HashMap;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct SampleQuery {
    search: Option<String>,
    category: Option<String>,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
pub struct CategoryRequest {
    pub category: String,
}

#[derive(Deserialize)]
pub struct ScanDirRequest {
    pub path: String,
    pub label: String,
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = Router::new()
        .route("/samples", get(list_samples))
        .route("/samples/:id", get(get_sample))
        .route("/samples/:id/toggle", post(toggle_sample))
        .route("/samples/:id/category", post(add_category))
        .route("/samples/:id/category", delete(remove_category))
        .route("/samples/:id/analyze", post(analyze_sample))
        .route("/scan-dirs", get(list_scan_dirs))
        .route("/scan-dirs", post(add_scan_dir))
        .route("/scan-dirs/:id", delete(remove_scan_dir))
        .route("/rescan", post(rescan))
        .route("/strudel.json", get(get_strudel_config));

    Router::new()
        .nest("/api", api)
        .nest_service("/samples", ServeDir::new("/"))
        .layer(cors)
        .with_state(state)
}

async fn list_samples(
    State(state): State<AppState>,
    Query(query): Query<SampleQuery>,
) -> impl IntoResponse {
    let samples = state
        .filter_samples(
            query.search.as_deref(),
            query.category.as_deref(),
            query.enabled,
        )
        .await;
    Json(ApiResponse::ok(samples))
}

async fn get_sample(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(e) => {
            return Json(ApiResponse::<Sample>::err(e.to_string()));
        }
    };

    let samples = state.get_samples().await;
    if let Some(sample) = samples.into_iter().find(|s| s.id == uuid) {
        Json(ApiResponse::ok(sample))
    } else {
        Json(ApiResponse::err("Sample not found".to_string()))
    }
}

async fn toggle_sample(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(e) => {
            return Json(ApiResponse::<bool>::err(e.to_string()));
        }
    };

    match state.toggle_sample(uuid).await {
        Some(enabled) => Json(ApiResponse::ok(enabled)),
        None => Json(ApiResponse::err("Sample not found".to_string())),
    }
}

async fn add_category(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CategoryRequest>,
) -> impl IntoResponse {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(e) => {
            return Json(ApiResponse::<bool>::err(e.to_string()));
        }
    };

    if state.add_category(uuid, req.category).await {
        Json(ApiResponse::ok(true))
    } else {
        Json(ApiResponse::err("Sample not found".to_string()))
    }
}

async fn remove_category(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CategoryRequest>,
) -> impl IntoResponse {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(e) => {
            return Json(ApiResponse::<bool>::err(e.to_string()));
        }
    };

    if state.remove_category(uuid, &req.category).await {
        Json(ApiResponse::ok(true))
    } else {
        Json(ApiResponse::err("Sample not found".to_string()))
    }
}

async fn analyze_sample(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(e) => {
            return Json(ApiResponse::<f64>::err(e.to_string()));
        }
    };

    match state.analyze_sample_bpm(uuid).await {
        Some(bpm) => Json(ApiResponse::ok(bpm)),
        None => Json(ApiResponse::err("Failed to analyze BPM".to_string())),
    }
}

async fn list_scan_dirs(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    Json(ApiResponse::ok(config.scan_dirs.clone()))
}

async fn add_scan_dir(
    State(state): State<AppState>,
    Json(req): Json<ScanDirRequest>,
) -> impl IntoResponse {
    let label = &req.label;
    let path = &req.path;
    state.add_scan_dir(path.clone(), label.clone()).await;
    info!("Added scan directory: {} -> {}", label, path);
    Json(ApiResponse::ok(true))
}

async fn remove_scan_dir(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uuid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(e) => {
            return Json(ApiResponse::<bool>::err(e.to_string()));
        }
    };

    state.remove_scan_dir(uuid).await;
    Json(ApiResponse::ok(true))
}

async fn rescan(State(state): State<AppState>) -> impl IntoResponse {
    info!("Starting rescan...");
    let (added, updated, removed) = state.rescan().await;
    info!(
        "Rescan complete: {} added, {} updated, {} removed",
        added.len(),
        updated.len(),
        removed.len()
    );

    let result = HashMap::from([
        ("added", added.len()),
        ("updated", updated.len()),
        ("removed", removed.len()),
    ]);
    Json(ApiResponse::ok(result))
}

async fn get_strudel_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.generate_strudel_config().await;
    Json(ApiResponse::ok(config))
}
