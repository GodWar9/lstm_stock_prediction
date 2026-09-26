use crate::{artifacts::arrow_bytes, contract::*};
use arrow2::{
    array::Float64Array,
    io::ipc::read::{read_file_metadata, FileReader},
};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode, Uri},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    convert::Infallible,
    fs,
    io::Cursor,
    path::{Path as FsPath, PathBuf},
    time::Duration,
};
use tokio_stream::StreamExt;
use utoipa::OpenApi;

#[derive(Clone)]
pub struct AppState {
    root: PathBuf,
}

type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;
fn error(status: StatusCode, code: &str, message: impl ToString) -> (StatusCode, Json<ApiError>) {
    (
        status,
        Json(ApiError {
            code: code.into(),
            message: message.to_string(),
        }),
    )
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn safe_path(root: &FsPath, id: &str) -> ApiResult<PathBuf> {
    if !valid_id(id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "Use an artifact ID, not a path",
        ));
    }
    let base = root.canonicalize().map_err(|_| {
        error(
            StatusCode::NOT_FOUND,
            "not_found",
            "Artifact directory does not exist",
        )
    })?;
    let path = base
        .join(id)
        .canonicalize()
        .map_err(|_| error(StatusCode::NOT_FOUND, "not_found", "Artifact not found"))?;
    if !path.starts_with(&base) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_path",
            "Artifact escapes its root",
        ));
    }
    Ok(path)
}
fn json_file<T: serde::de::DeserializeOwned>(path: &FsPath) -> ApiResult<T> {
    let bytes = fs::read(path)
        .map_err(|_| error(StatusCode::NOT_FOUND, "not_found", "Artifact not available"))?;
    serde_json::from_slice(&bytes).map_err(|_| {
        error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_artifact",
            "Artifact JSON is malformed",
        )
    })
}
fn read_manifest(state: &AppState, id: &str) -> ApiResult<RunManifest> {
    let dir = safe_path(&state.root.join("reports/runs"), id)?;
    let m: RunManifest = json_file(&dir.join("manifest.json"))?;
    if m.schema_version != 1 || m.run_id != id {
        return Err(error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_schema",
            "Run ID or schema version is unsupported",
        ));
    }
    Ok(m)
}

#[utoipa::path(get, path = "/api/runs", responses((status = 200, body = [RunManifest]), (status = 422, body = ApiError)))]
async fn runs(State(s): State<AppState>) -> ApiResult<Json<Vec<RunManifest>>> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(s.root.join("reports/runs")) {
        for entry in entries.flatten() {
            if entry.path().join("manifest.json").is_file() {
                result.push(read_manifest(&s, &entry.file_name().to_string_lossy())?);
            }
        }
    }
    result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(result))
}

#[utoipa::path(get, path = "/api/runs/{run_id}/manifest", params(("run_id" = String, Path)), responses((status = 200, body = RunManifest), (status = 404, body = ApiError)))]
async fn manifest(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Response> {
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=31536000, immutable")],
        Json(read_manifest(&s, &id)?),
    )
        .into_response())
}

#[derive(Deserialize, Default)]
struct SeriesQuery {
    max_points: Option<usize>,
    asof: Option<i64>,
    instrument: Option<String>,
}

// Bucket extrema selection preserves first/last and local extremes in every value column.
fn selected_indices(columns: &[(String, Vec<f64>)], limit: usize) -> Vec<usize> {
    let n = columns.first().map_or(0, |(_, v)| v.len());
    if n <= limit {
        return (0..n).collect();
    }
    let values = columns.len().saturating_sub(1).max(1);
    let buckets = ((limit - 2) / (2 * values)).max(1);
    let mut selected = BTreeSet::from([0, n - 1]);
    for bucket in 0..buckets {
        let start = 1 + bucket * (n - 2) / buckets;
        let end = 1 + (bucket + 1) * (n - 2) / buckets;
        for (_, col) in columns.iter().skip(1) {
            if let Some(i) = (start..end).min_by(|&a, &b| col[a].total_cmp(&col[b])) {
                selected.insert(i);
            }
            if let Some(i) = (start..end).max_by(|&a, &b| col[a].total_cmp(&col[b])) {
                selected.insert(i);
            }
        }
    }
    selected.into_iter().collect()
}

#[utoipa::path(get, path = "/api/runs/{run_id}/{artifact}", params(("run_id" = String, Path), ("artifact" = String, Path), ("max_points" = Option<usize>, Query), ("asof" = Option<i64>, Query), ("instrument" = Option<String>, Query)), responses((status = 200, description = "Arrow IPC or JSON artifact"), (status = 400, body = ApiError), (status = 404, body = ApiError)))]
async fn artifact(
    State(s): State<AppState>,
    Path((id, name)): Path<(String, String)>,
    query: Result<Query<SeriesQuery>, axum::extract::rejection::QueryRejection>,
) -> ApiResult<Response> {
    let Query(q) = query.map_err(|e| error(StatusCode::BAD_REQUEST, "invalid_query", e))?;
    let m = read_manifest(&s, &id)?;
    let allowed = [
        "equity.arrow",
        "trades.arrow",
        "signals.arrow",
        "simulation/bands.arrow",
        "validation",
        "simulation.json",
        "report.json",
    ];
    if !allowed.contains(&name.as_str()) {
        return Err(error(
            StatusCode::NOT_FOUND,
            "not_found",
            "Unknown artifact",
        ));
    }
    let filename = if name == "validation" {
        "validation.json"
    } else if name == "simulation/bands.arrow" {
        "bands.arrow"
    } else {
        &name
    };
    if !m.artifacts.values().any(|p| p == filename) {
        return Err(error(
            StatusCode::NOT_FOUND,
            "capability_unavailable",
            "This run did not produce the requested artifact",
        ));
    }
    let dir = safe_path(&s.root.join("reports/runs"), &id)?;
    let file = dir
        .join(filename)
        .canonicalize()
        .map_err(|_| error(StatusCode::NOT_FOUND, "not_found", "Artifact is missing"))?;
    if !file.starts_with(&dir) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_path",
            "Artifact escapes run directory",
        ));
    }
    let bytes = fs::read(file)
        .map_err(|_| error(StatusCode::NOT_FOUND, "not_found", "Cannot read artifact"))?;
    if !filename.ends_with(".arrow") {
        return Ok((
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            bytes,
        )
            .into_response());
    }
    let limit = q.max_points.unwrap_or(2000);
    if !(32..=100_000).contains(&limit) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "max_points must be between 32 and 100000",
        ));
    }
    if name == "signals.arrow" && q.asof.is_none() {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "missing_asof",
            "Signals require asof in UTC milliseconds",
        ));
    }
    let mut cursor = Cursor::new(bytes);
    let metadata = read_file_metadata(&mut cursor)
        .map_err(|e| error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_arrow", e))?;
    let mut columns: Vec<(String, Vec<f64>)> = metadata
        .schema
        .fields
        .iter()
        .map(|f| (f.name.clone(), Vec::new()))
        .collect();
    for batch in FileReader::new(cursor, metadata, None, None) {
        let batch =
            batch.map_err(|e| error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_arrow", e))?;
        for (i, array) in batch.arrays().iter().enumerate() {
            let a = array
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or_else(|| {
                    error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "invalid_arrow",
                        "Expected Float64 series",
                    )
                })?;
            columns[i].1.extend(a.values().iter().copied());
        }
    }
    let matches_instrument = q
        .instrument
        .as_ref()
        .is_none_or(|instrument| m.instruments.iter().any(|i| &i.id == instrument));
    if !matches_instrument {
        for (_, v) in &mut columns {
            v.clear();
        }
    }
    if let Some(asof) = q.asof {
        let indices: Vec<_> = columns
            .first()
            .map(|(_, v)| {
                v.iter()
                    .enumerate()
                    .filter_map(|(i, t)| (*t <= asof as f64).then_some(i))
                    .collect()
            })
            .unwrap_or_default();
        for (_, v) in &mut columns {
            *v = indices.iter().map(|&i| v[i]).collect();
        }
    }
    let indices = selected_indices(&columns, limit);
    let reduced: Vec<_> = columns
        .iter()
        .map(|(name, v)| (name.as_str(), indices.iter().map(|&i| v[i]).collect()))
        .collect();
    let output = arrow_bytes(&reduced)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, "encode_failed", e))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.apache.arrow.file"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        output,
    )
        .into_response())
}

fn read_model(s: &AppState, id: &str) -> ApiResult<ModelArtifact> {
    let dir = safe_path(&s.root.join("models"), id)?;
    Ok(ModelArtifact {
        artifact_id: id.into(),
        metadata: json_file(&dir.join("metadata.json"))?,
        training_log: if dir.join("training_log.json").exists() {
            Some(json_file(&dir.join("training_log.json"))?)
        } else {
            None
        },
        validation: if dir.join("validation.json").exists() {
            Some(json_file(&dir.join("validation.json"))?)
        } else {
            None
        },
        onnx_present: dir.join("model.onnx").is_file(),
    })
}
#[utoipa::path(get, path = "/api/models", responses((status = 200, body = [ModelArtifact])))]
async fn models(State(s): State<AppState>) -> ApiResult<Json<Vec<ModelArtifact>>> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(s.root.join("models")) {
        for e in entries.flatten() {
            if e.path().join("metadata.json").exists() {
                result.push(read_model(&s, &e.file_name().to_string_lossy())?);
            }
        }
    }
    result.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));
    Ok(Json(result))
}
#[utoipa::path(get, path = "/api/models/{artifact_id}", params(("artifact_id" = String, Path)), responses((status = 200, body = ModelArtifact), (status = 404, body = ApiError)))]
async fn model(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ModelArtifact>> {
    Ok(Json(read_model(&s, &id)?))
}

#[utoipa::path(get, path = "/api/events", responses((status = 200, description = "SSE completed artifact snapshots; no job launcher")))]
async fn events(
    State(s): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream =
        tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(3)))
            .map(move |_| {
                let completed = fs::read_dir(s.root.join("reports/runs"))
                    .map(|entries| {
                        entries
                            .flatten()
                            .filter(|e| e.path().join("manifest.json").exists())
                            .count()
                    })
                    .unwrap_or(0);
                Ok(Event::default()
                    .event("artifacts")
                    .data(serde_json::json!({"completed_runs":completed}).to_string()))
            });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(rust_embed::RustEmbed)]
#[folder = "$OUT_DIR/assets/"]
struct Assets;
async fn spa(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") {
        return error(StatusCode::NOT_FOUND, "not_found", "Unknown API endpoint").into_response();
    }
    let asset = Assets::get(if path.is_empty() { "index.html" } else { path }).or_else(|| {
        if !path.contains('.') {
            Assets::get("index.html")
        } else {
            None
        }
    });
    match asset {
        Some(file) => {
            let mime = if !path.contains('.') {
                "text/html".into()
            } else {
                mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string()
            };
            (
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, "no-cache".into()),
                ],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => error(
            StatusCode::NOT_FOUND,
            "not_found",
            "Asset missing; rebuild web and quantctl",
        )
        .into_response(),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(runs, manifest, artifact, models, model, events),
    components(schemas(RunManifest, Provenance, Instrument, ApiError, ModelArtifact))
)]
struct ApiDoc;
pub fn openapi() -> String {
    ApiDoc::openapi()
        .to_pretty_json()
        .expect("OpenAPI serialization")
}

pub fn router(root: PathBuf) -> Router {
    Router::new()
        .route("/api/runs", get(runs))
        .route("/api/runs/{run_id}/manifest", get(manifest))
        .route("/api/runs/{run_id}/{*artifact}", get(artifact))
        .route("/api/models", get(models))
        .route("/api/models/{artifact_id}", get(model))
        .route("/api/events", get(events))
        .route(
            "/api/openapi.json",
            get(|| async { ([(header::CONTENT_TYPE, "application/json")], openapi()) }),
        )
        .fallback(spa)
        .with_state(AppState { root })
}
pub async fn serve(root: PathBuf, port: u16) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!("Research inspector: http://{}", listener.local_addr()?);
    axum::serve(listener, router(root))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;
    #[tokio::test]
    async fn empty_list_missing_run_and_read_only() {
        let tmp = tempfile::tempdir().unwrap();
        for (method, path, status) in [
            ("GET", "/api/runs", 200),
            ("GET", "/api/models", 200),
            ("GET", "/api/runs/missing/manifest", 404),
            ("POST", "/api/runs", 405),
            ("GET", "/api/unknown", 404),
            ("GET", "/backtest", 200),
        ] {
            let response = router(tmp.path().into())
                .oneshot(
                    axum::http::Request::builder()
                        .method(method)
                        .uri(path)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status, "{method} {path}");
        }
    }
    #[test]
    fn rejects_paths_and_preserves_extremes() {
        for id in ["../a", "a/b", "a\\b", "", "a.json"] {
            assert!(!valid_id(id));
        }
        let mut y = vec![0.0; 1000];
        y[501] = 100.0;
        y[502] = -100.0;
        let columns = vec![
            ("timestamp_ms".into(), (0..1000).map(f64::from).collect()),
            ("nav".into(), y),
        ];
        let selected = selected_indices(&columns, 32);
        assert!(selected.len() <= 32);
        for i in [0, 501, 502, 999] {
            assert!(selected.contains(&i));
        }
    }
}
