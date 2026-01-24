pub mod handlers;

use std::sync::Arc;
use std::time::Instant;

use axum::extract::DefaultBodyLimit;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use handlers::{
    ConvertRequest, ErrorDetail, ErrorItem, ErrorResponse, HealthResponse, ValidateResponse,
};

/// OpenAPI 문서 정의
#[derive(OpenApi)]
#[openapi(
    info(
        title = "jsontohwpx API",
        version = "0.5.0",
        description = "JSON API 응답을 HWPX(한글 문서) 파일로 변환하는 REST API",
        license(name = "MIT OR Apache-2.0")
    ),
    paths(
        handlers::convert,
        handlers::validate,
        handlers::health,
    ),
    components(schemas(
        ConvertRequest,
        ErrorResponse,
        ErrorDetail,
        ErrorItem,
        ValidateResponse,
        HealthResponse,
    )),
    tags(
        (name = "변환", description = "JSON → HWPX 변환"),
        (name = "검증", description = "입력 데이터 유효성 검증"),
        (name = "상태", description = "서버 상태 확인"),
    )
)]
pub struct ApiDoc;

/// API 서버 공유 상태
#[derive(Clone)]
pub struct AppState {
    pub start_time: Instant,
    pub base_path: std::path::PathBuf,
}

/// API 서버 설정
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_request_size: usize,
    pub base_path: std::path::PathBuf,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            max_request_size: 50 * 1024 * 1024, // 50MB
            base_path: std::path::PathBuf::from("."),
        }
    }
}

impl ServerConfig {
    /// 환경변수로부터 설정 읽기
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(host) = std::env::var("HOST") {
            config.host = host;
        }
        if let Ok(port) = std::env::var("PORT") {
            if let Ok(p) = port.parse() {
                config.port = p;
            }
        }
        if let Ok(size) = std::env::var("MAX_REQUEST_SIZE") {
            if let Ok(s) = size.parse() {
                config.max_request_size = s;
            }
        }
        if let Ok(path) = std::env::var("BASE_PATH") {
            config.base_path = std::path::PathBuf::from(path);
        }

        config
    }
}

/// axum Router 생성 (Swagger UI 포함)
pub fn create_router(config: &ServerConfig) -> Router {
    let state = Arc::new(AppState {
        start_time: Instant::now(),
        base_path: config.base_path.clone(),
    });

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/api/v1/convert", axum::routing::post(handlers::convert))
        .route("/api/v1/validate", axum::routing::post(handlers::validate))
        .route("/api/v1/health", axum::routing::get(handlers::health))
        .layer(DefaultBodyLimit::max(config.max_request_size))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
