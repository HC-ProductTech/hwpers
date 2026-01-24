use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::AppState;
use crate::jsontohwpx::{self, ApiResponse, JsonToHwpxError};

// --- 요청/응답 스키마 ---

/// 변환 요청 바디 (OpenAPI 문서용)
#[derive(Deserialize, ToSchema)]
#[schema(example = json!({
    "responseCode": "0",
    "data": {
        "article": {
            "atclId": "DOC001",
            "subject": "문서 제목",
            "contents": [
                { "type": "text", "value": "본문 텍스트" }
            ]
        }
    }
}))]
pub struct ConvertRequest {
    /// 응답 코드 ("0"이면 정상)
    #[serde(rename = "responseCode")]
    pub response_code: String,
    /// 옵션
    #[serde(default)]
    pub options: Option<serde_json::Value>,
    /// 데이터
    pub data: serde_json::Value,
}

/// 에러 응답 구조
#[derive(Serialize, ToSchema)]
#[schema(example = json!({
    "error": {
        "code": "INVALID_JSON",
        "message": "JSON 파싱 실패: expected value at line 1 column 1"
    }
}))]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorDetail {
    /// 에러 코드
    pub code: String,
    /// 에러 메시지
    pub message: String,
    /// 상세 에러 목록
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<ErrorItem>,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorItem {
    /// 에러 발생 경로
    pub path: String,
    /// 상세 메시지
    pub message: String,
}

/// validate 응답 구조
#[derive(Serialize, ToSchema)]
#[schema(example = json!({ "valid": true, "errors": [] }))]
pub struct ValidateResponse {
    /// 유효성 결과
    pub valid: bool,
    /// 에러 목록 (유효하지 않은 경우)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// health 응답 구조
#[derive(Serialize, ToSchema)]
#[schema(example = json!({
    "status": "healthy",
    "version": "0.5.0",
    "uptime_seconds": 3600
}))]
pub struct HealthResponse {
    /// 서버 상태
    pub status: String,
    /// 서버 버전
    pub version: String,
    /// 가동 시간 (초)
    pub uptime_seconds: u64,
}

// --- 핸들러 ---

/// JSON을 HWPX 문서로 변환
///
/// JSON API 응답을 받아 HWPX(한글 문서) 바이너리 파일로 변환하여 반환합니다.
#[utoipa::path(
    post,
    path = "/api/v1/convert",
    request_body(content = ConvertRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "변환 성공 (HWPX 바이너리)", content_type = "application/vnd.hancom.hwpx"),
        (status = 400, description = "잘못된 입력", body = ErrorResponse),
        (status = 500, description = "변환 실패", body = ErrorResponse),
    ),
    tag = "변환"
)]
pub async fn convert(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let input: ApiResponse = serde_json::from_str(&body).map_err(|e| {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "INVALID_JSON".to_string(),
                message: format!("JSON 파싱 실패: {}", e),
                details: Vec::new(),
            },
        };
        (StatusCode::BAD_REQUEST, Json(resp))
    })?;

    if let Err(e) = input.validate() {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: e.error_code().to_string(),
                message: e.to_string(),
                details: Vec::new(),
            },
        };
        return Err((StatusCode::BAD_REQUEST, Json(resp)));
    }

    let atcl_id = input.data.article.atcl_id.trim().to_string();

    let bytes = jsontohwpx::convert(&input, &state.base_path).map_err(|e| {
        let (status, code) = match &e {
            JsonToHwpxError::Input(_) => (StatusCode::BAD_REQUEST, e.error_code()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, e.error_code()),
        };
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: code.to_string(),
                message: e.to_string(),
                details: Vec::new(),
            },
        };
        (status, Json(resp))
    })?;

    let filename = format!("{}.hwpx", atcl_id);
    let headers = [
        (
            header::CONTENT_TYPE,
            "application/vnd.hancom.hwpx".to_string(),
        ),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        ),
    ];

    Ok((headers, bytes))
}

/// JSON 입력 유효성 검증
///
/// JSON 데이터의 구조와 필수 필드를 검증합니다. 변환은 수행하지 않습니다.
#[utoipa::path(
    post,
    path = "/api/v1/validate",
    request_body(content = ConvertRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "검증 결과", body = ValidateResponse),
    ),
    tag = "검증"
)]
pub async fn validate(body: String) -> impl IntoResponse {
    let input: ApiResponse = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            let resp = ValidateResponse {
                valid: false,
                errors: vec![format!("JSON 파싱 실패: {}", e)],
            };
            return (StatusCode::OK, Json(resp));
        }
    };

    match input.validate() {
        Ok(()) => {
            let resp = ValidateResponse {
                valid: true,
                errors: Vec::new(),
            };
            (StatusCode::OK, Json(resp))
        }
        Err(e) => {
            let resp = ValidateResponse {
                valid: false,
                errors: vec![e.to_string()],
            };
            (StatusCode::OK, Json(resp))
        }
    }
}

/// 서버 상태 확인
///
/// 서버의 현재 상태, 버전, 가동 시간을 반환합니다.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "서버 상태", body = HealthResponse),
    ),
    tag = "상태"
)]
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().as_secs();
    let resp = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
    };
    Json(resp)
}
