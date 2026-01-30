use hwpers::jsontohwpx::api::{build_state, create_router_with_state, ServerConfig};
use tokio::signal;
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
    let file_expiry_hours = config.file_expiry_hours;

    // 라이선스 만료 체크 (서버 시작 시)
    if let Some(expiry) = config.license_expiry {
        let today = chrono::Local::now().date_naive();
        if today > expiry {
            tracing::error!(
                expiry = %expiry,
                today = %today,
                "라이선스가 만료되었습니다"
            );
            std::process::exit(1);
        }
        tracing::info!(expiry = %expiry, "라이선스 유효");
    }

    tracing::info!(
        host = %config.host,
        port = config.port,
        max_request_size = config.max_request_size,
        base_path = %config.base_path.display(),
        output_dir = %config.output_dir.display(),
        worker_count = config.worker_count,
        file_expiry_hours = config.file_expiry_hours,
        "jsontohwpx-api 서버 시작"
    );

    let state = build_state(&config);
    let app = create_router_with_state(state.clone(), config.max_request_size);

    // 파일 만료 정리 백그라운드 태스크
    let cleanup_store = state.job_store.clone();
    tokio::spawn(async move {
        let interval = tokio::time::Duration::from_secs(3600); // 1시간마다
        loop {
            tokio::time::sleep(interval).await;
            cleanup_store.cleanup_expired(file_expiry_hours).await;
        }
    });

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(addr = %addr, error = %e, "서버 바인딩 실패");
            std::process::exit(1);
        });

    tracing::info!(addr = %addr, "서버 대기 중");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "서버 실행 오류");
            std::process::exit(1);
        });

    tracing::info!("서버 정상 종료");
}

/// Graceful shutdown 시그널 대기
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Ctrl+C 핸들러 설치 실패");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("SIGTERM 핸들러 설치 실패")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Ctrl+C 수신, 서버 종료 시작");
        }
        _ = terminate => {
            tracing::info!("SIGTERM 수신, 서버 종료 시작");
        }
    }
}
