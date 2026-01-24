use hwpers::jsontohwpx::api::{create_router, ServerConfig};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // tracing 초기화
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let config = ServerConfig::from_env();
    let addr = format!("{}:{}", config.host, config.port);

    tracing::info!(
        host = %config.host,
        port = config.port,
        max_request_size = config.max_request_size,
        base_path = %config.base_path.display(),
        "jsontohwpx-api 서버 시작"
    );

    let app = create_router(&config);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(addr = %addr, error = %e, "서버 바인딩 실패");
            std::process::exit(1);
        });

    tracing::info!(addr = %addr, "서버 대기 중");

    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "서버 실행 오류");
            std::process::exit(1);
        });
}
