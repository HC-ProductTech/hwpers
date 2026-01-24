use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

use hwpers::jsontohwpx::api::{create_router, ServerConfig};

fn test_config() -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".to_string(),
        port: 0,
        max_request_size: 50 * 1024 * 1024,
        base_path: std::path::PathBuf::from("examples/jsontohwpx"),
        ..Default::default()
    }
}

fn test_config_with_output(output_dir: std::path::PathBuf) -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".to_string(),
        port: 0,
        max_request_size: 50 * 1024 * 1024,
        base_path: std::path::PathBuf::from("examples/jsontohwpx"),
        output_dir,
        ..Default::default()
    }
}

/// 비동기 변환 후 상태가 completed가 될 때까지 폴링
async fn poll_job_completed(app: &Router, job_id: &str) -> serde_json::Value {
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let req = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/jobs/{}", job_id))
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let status = json["status"].as_str().unwrap_or("");
        if status == "completed" || status == "failed" {
            return json;
        }
    }
    panic!("작업이 시간 내에 완료되지 않음");
}

fn simple_json() -> &'static str {
    r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TEST001",
                "subject": "테스트",
                "contents": [
                    { "type": "text", "value": "안녕하세요" }
                ]
            }
        }
    }"#
}

// --- convert 핸들러 테스트 ---

#[tokio::test]
async fn test_convert_success() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert")
        .header("content-type", "application/json")
        .body(Body::from(simple_json()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Content-Type 확인
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(content_type, "application/vnd.hancom.hwpx");

    // Content-Disposition 확인
    let disposition = resp
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(disposition.contains("TEST001.hwpx"));

    // 응답 바디가 유효한 ZIP(HWPX)인지 확인
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert!(!body.is_empty());
    assert_eq!(&body[0..2], &[0x50, 0x4B], "유효한 ZIP 파일이어야 함");
}

#[tokio::test]
async fn test_convert_invalid_json() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert")
        .header("content-type", "application/json")
        .body(Body::from("이것은 JSON이 아닙니다"))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "INVALID_JSON");
}

#[tokio::test]
async fn test_convert_invalid_response_code() {
    let app = create_router(&test_config());

    let json = r#"{
        "responseCode": "99",
        "responseText": "FAIL",
        "data": {
            "article": {
                "atclId": "ERR001",
                "subject": "에러",
                "contents": []
            }
        }
    }"#;

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert")
        .header("content-type", "application/json")
        .body(Body::from(json))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "INPUT_ERROR");
}

#[tokio::test]
async fn test_convert_empty_table_error() {
    let app = create_router(&test_config());

    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TBL_ERR",
                "subject": "빈테이블",
                "contents": [
                    { "type": "table", "value": "<table></table>" }
                ]
            }
        }
    }"#;

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert")
        .header("content-type", "application/json")
        .body(Body::from(json))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["error"]["code"].as_str().is_some());
}

#[tokio::test]
async fn test_convert_with_table() {
    let app = create_router(&test_config());

    let json = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "TBL001",
                "subject": "테이블문서",
                "contents": [
                    { "type": "text", "value": "표 앞" },
                    { "type": "table", "value": "<table><tr><td>A</td><td>B</td></tr><tr><td>1</td><td>2</td></tr></table>" },
                    { "type": "text", "value": "표 뒤" }
                ]
            }
        }
    }"#;

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert")
        .header("content-type", "application/json")
        .body(Body::from(json))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[0..2], &[0x50, 0x4B]);
}

// --- validate 핸들러 테스트 ---

#[tokio::test]
async fn test_validate_success() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/validate")
        .header("content-type", "application/json")
        .body(Body::from(simple_json()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["valid"], true);
    // errors는 비어있으면 생략됨
    let errors = json["errors"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(errors, 0);
}

#[tokio::test]
async fn test_validate_invalid_json() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/validate")
        .header("content-type", "application/json")
        .body(Body::from("{broken"))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["valid"], false);
    assert!(!json["errors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_validate_bad_response_code() {
    let app = create_router(&test_config());

    let json = r#"{
        "responseCode": "1",
        "data": {
            "article": {
                "atclId": "X",
                "subject": "",
                "contents": []
            }
        }
    }"#;

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/validate")
        .header("content-type", "application/json")
        .body(Body::from(json))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["valid"], false);
}

// --- health 핸들러 테스트 ---

#[tokio::test]
async fn test_health() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "healthy");
    assert!(json["version"].as_str().is_some());
    assert!(json["uptime_seconds"].as_u64().is_some());
}

// --- 요청 크기 제한 테스트 ---

#[tokio::test]
async fn test_request_size_limit() {
    let config = ServerConfig {
        max_request_size: 100, // 100바이트 제한
        ..test_config()
    };
    let app = create_router(&config);

    // 100바이트 초과 요청
    let large_body = "x".repeat(200);
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert")
        .header("content-type", "application/json")
        .body(Body::from(large_body))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

// --- 404 테스트 ---

#[tokio::test]
async fn test_unknown_route() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/unknown")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// --- Swagger UI / OpenAPI 테스트 ---

#[tokio::test]
async fn test_openapi_json() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("GET")
        .uri("/api-docs/openapi.json")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["info"]["title"], "jsontohwpx API");
    assert!(json["paths"]["/api/v1/convert"].is_object());
    assert!(json["paths"]["/api/v1/validate"].is_object());
    assert!(json["paths"]["/api/v1/health"].is_object());
}

#[tokio::test]
async fn test_swagger_ui_redirect() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("GET")
        .uri("/swagger-ui")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    // Swagger UI는 /swagger-ui/ 로 리다이렉트하거나 200 반환
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::MOVED_PERMANENTLY
            || resp.status() == StatusCode::TEMPORARY_REDIRECT
            || resp.status() == StatusCode::SEE_OTHER,
    );
}

// --- include_header 옵션 테스트 ---

#[tokio::test]
async fn test_convert_with_include_header() {
    let app = create_router(&test_config());

    let json = r#"{
        "responseCode": "0",
        "options": { "includeHeader": true },
        "data": {
            "article": {
                "atclId": "HDR001",
                "subject": "헤더포함",
                "contents": [
                    { "type": "text", "value": "본문" }
                ],
                "regDt": "2025-01-24 AM 10:00:00",
                "regEmpName": "홍길동",
                "regDeptName": "개발팀"
            }
        }
    }"#;

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert")
        .header("content-type", "application/json")
        .body(Body::from(json))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[0..2], &[0x50, 0x4B]);
}

// --- 비동기 변환 API 테스트 ---

#[tokio::test]
async fn test_convert_async_success() {
    let tmp = tempfile::tempdir().unwrap();
    let app = create_router(&test_config_with_output(tmp.path().to_path_buf()));

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert/async")
        .header("content-type", "application/json")
        .body(Body::from(simple_json()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "queued");
    assert!(json["jobId"].as_str().is_some());
    assert!(json["createdAt"].as_str().is_some());

    let job_id = json["jobId"].as_str().unwrap();

    // 작업 완료 대기
    let result = poll_job_completed(&app, job_id).await;
    assert_eq!(result["status"], "completed");
    assert!(result["downloadUrl"].as_str().is_some());
}

#[tokio::test]
async fn test_convert_async_invalid_json() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert/async")
        .header("content-type", "application/json")
        .body(Body::from("not json"))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "INVALID_JSON");
}

#[tokio::test]
async fn test_convert_async_invalid_response_code() {
    let app = create_router(&test_config());

    let json_body = r#"{
        "responseCode": "99",
        "data": {
            "article": {
                "atclId": "ERR001",
                "subject": "에러",
                "contents": []
            }
        }
    }"#;

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert/async")
        .header("content-type", "application/json")
        .body(Body::from(json_body))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_job_not_found() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/jobs/nonexistent-id")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn test_download_job_not_found() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/jobs/nonexistent-id/download")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_convert_async_download() {
    let tmp = tempfile::tempdir().unwrap();
    let app = create_router(&test_config_with_output(tmp.path().to_path_buf()));

    // 비동기 변환 요청
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert/async")
        .header("content-type", "application/json")
        .body(Body::from(simple_json()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let job_id = json["jobId"].as_str().unwrap().to_string();

    // 완료 대기
    let result = poll_job_completed(&app, &job_id).await;
    assert_eq!(result["status"], "completed");

    // 다운로드
    let download_url = result["downloadUrl"].as_str().unwrap();
    let req = Request::builder()
        .method("GET")
        .uri(download_url)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(content_type, "application/vnd.hancom.hwpx");

    let disposition = resp
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(disposition.contains("TEST001.hwpx"));

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert!(!body.is_empty());
    assert_eq!(&body[0..2], &[0x50, 0x4B], "유효한 ZIP 파일이어야 함");
}

#[tokio::test]
async fn test_convert_async_failed_job() {
    let tmp = tempfile::tempdir().unwrap();
    let app = create_router(&test_config_with_output(tmp.path().to_path_buf()));

    // 빈 테이블로 변환 실패 유도
    let json_body = r#"{
        "responseCode": "0",
        "data": {
            "article": {
                "atclId": "FAIL001",
                "subject": "실패테스트",
                "contents": [
                    { "type": "table", "value": "<table></table>" }
                ]
            }
        }
    }"#;

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/convert/async")
        .header("content-type", "application/json")
        .body(Body::from(json_body))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let job_id = json["jobId"].as_str().unwrap();

    // 실패 대기
    let result = poll_job_completed(&app, job_id).await;
    assert_eq!(result["status"], "failed");
    assert!(result["error"].as_str().is_some());
}

#[tokio::test]
async fn test_health_with_queue_info() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "healthy");
    assert!(json["queue"]["pending"].as_u64().is_some());
    assert!(json["queue"]["processing"].as_u64().is_some());
    assert!(json["queue"]["completed"].as_u64().is_some());
    assert!(json["queue"]["failed"].as_u64().is_some());
    assert!(json["workers"]["active"].as_u64().is_some());
    assert!(json["workers"]["max"].as_u64().is_some());
}

#[tokio::test]
async fn test_openapi_includes_async_paths() {
    let app = create_router(&test_config());

    let req = Request::builder()
        .method("GET")
        .uri("/api-docs/openapi.json")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["paths"]["/api/v1/convert/async"].is_object());
    assert!(json["paths"]["/api/v1/jobs/{id}"].is_object());
    assert!(json["paths"]["/api/v1/jobs/{id}/download"].is_object());
}
