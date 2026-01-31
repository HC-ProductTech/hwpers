use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::openapi::schema::{Object, ObjectBuilder, SchemaType, Type};
use utoipa::ToSchema;
use uuid::Uuid;

use super::jobs::{AsyncConvertResponse, JobResponse, JobStats, JobStatus};
use super::queue::ConvertJob;
use super::AppState;
use crate::jsontohwpx::{self, ArticleDocument, ConvertOptions, JsonToHwpxError};

// --- 요청/응답 스키마 ---

/// 변환 요청 바디 (OpenAPI 문서용)
#[derive(Deserialize, ToSchema)]
#[schema(example = json!({
    "article_id": "DOC001",
    "title": "문서 제목",
    "metadata": {
        "author": "홍길동",
        "department": "개발팀",
        "created_at": "2025-01-30T10:00:00+09:00"
    },
    "contents": [
        { "type": "text", "value": "본문 텍스트" }
    ]
}))]
pub struct ConvertRequest {
    /// 게시글 고유 ID
    pub article_id: String,
    /// 제목
    pub title: Option<String>,
    /// 메타데이터
    pub metadata: Option<serde_json::Value>,
    /// 본문 콘텐츠
    pub contents: Option<Vec<serde_json::Value>>,
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
    "queue": { "pending": 0, "processing": 0, "completed": 10, "failed": 1 },
    "workers": { "active": 0, "max": 4 },
    "uptime_seconds": 3600,
    "license": "valid"
}))]
pub struct HealthResponse {
    /// 서버 상태
    pub status: String,
    /// 서버 버전
    pub version: String,
    /// 작업 큐 통계
    pub queue: JobStats,
    /// 워커 정보
    pub workers: WorkerInfo,
    /// 가동 시간 (초)
    pub uptime_seconds: u64,
    /// 라이선스 상태 ("valid", "expired", 또는 미설정 시 None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// 워커 상태 정보
#[derive(Serialize, ToSchema)]
pub struct WorkerInfo {
    /// 현재 활성 워커 수
    pub active: u64,
    /// 최대 워커 수
    pub max: u64,
}

// --- 핸들러 ---

/// JSON을 HWPX 문서로 변환 (동기)
///
/// JSON 문서를 받아 HWPX(한글 문서) 바이너리 파일로 변환하여 즉시 반환합니다.
#[utoipa::path(
    post,
    path = "/api/v1/convert",
    params(
        ("include_header" = Option<String>, Query, description = "메타데이터를 본문 상단에 삽입할지 여부 (true/false)")
    ),
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
    Query(query): Query<ConvertQuery>,
    body: String,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let input: ArticleDocument = serde_json::from_str(&body).map_err(|e| {
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

    let article_id = input.article_id.trim().to_string();
    let base_path = state.base_path.clone();
    let include_header = parse_include_header(query.include_header.as_deref());
    let options = ConvertOptions {
        include_header,
        ..ConvertOptions::default()
    };

    // spawn_blocking으로 감싸서 blocking reqwest와 tokio 런타임 충돌 방지
    let convert_result =
        tokio::task::spawn_blocking(move || jsontohwpx::convert(&input, &options, &base_path))
            .await
            .map_err(|e| {
                let resp = ErrorResponse {
                    error: ErrorDetail {
                        code: "INTERNAL_ERROR".to_string(),
                        message: format!("변환 작업 실행 실패: {}", e),
                        details: Vec::new(),
                    },
                };
                (StatusCode::INTERNAL_SERVER_ERROR, Json(resp))
            })?;

    let bytes = convert_result.map_err(|e| {
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

    let filename = format!("{}.hwpx", article_id);
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

/// 변환 엔드포인트 공통 query parameter
#[derive(Debug, Deserialize)]
pub struct ConvertQuery {
    #[serde(default)]
    pub include_header: Option<String>,
}

/// include_header 값을 bool로 파싱
fn parse_include_header(value: Option<&str>) -> bool {
    value.is_some_and(|v| v == "true" || v == "1")
}

/// include_header 필드의 커스텀 스키마 (true/false enum, default=false)
fn include_header_schema() -> Object {
    ObjectBuilder::new()
        .schema_type(SchemaType::Type(Type::String))
        .enum_values(Some(["false", "true"]))
        .default(Some(serde_json::Value::String("false".to_string())))
        .description(Some("메타데이터를 본문 상단에 삽입할지 여부"))
        .build()
}

/// 파일 업로드 변환 요청 (OpenAPI 문서용)
#[derive(ToSchema)]
pub struct ConvertFileRequest {
    /// JSON 파일
    #[schema(value_type = String, format = Binary)]
    pub file: String,
    /// 메타데이터를 본문 상단에 삽입할지 여부
    #[schema(schema_with = include_header_schema)]
    pub include_header: String,
}

/// JSON 파일을 업로드하여 HWPX 문서로 변환 (동기)
///
/// multipart/form-data로 JSON 파일을 업로드하면 HWPX 바이너리를 즉시 반환합니다.
/// `file` 필드에 JSON 파일, `include_header` 필드(선택)에 bool 값을 전달합니다.
#[utoipa::path(
    post,
    path = "/api/v1/convert/file",
    request_body(content = ConvertFileRequest, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "변환 성공 (HWPX 바이너리)", content_type = "application/vnd.hancom.hwpx"),
        (status = 400, description = "잘못된 입력", body = ErrorResponse),
        (status = 500, description = "변환 실패", body = ErrorResponse),
    ),
    tag = "변환"
)]
pub async fn convert_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut include_header = false;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "MULTIPART_ERROR".to_string(),
                message: format!("multipart 파싱 실패: {}", e),
                details: Vec::new(),
            },
        };
        (StatusCode::BAD_REQUEST, Json(resp))
    })? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let bytes = field.bytes().await.map_err(|e| {
                    let resp = ErrorResponse {
                        error: ErrorDetail {
                            code: "MULTIPART_ERROR".to_string(),
                            message: format!("파일 읽기 실패: {}", e),
                            details: Vec::new(),
                        },
                    };
                    (StatusCode::BAD_REQUEST, Json(resp))
                })?;
                file_bytes = Some(bytes.to_vec());
            }
            "include_header" => {
                let text = field.text().await.unwrap_or_default();
                include_header = text == "true" || text == "1";
            }
            _ => {}
        }
    }

    let file_bytes = file_bytes.ok_or_else(|| {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "MISSING_FILE".to_string(),
                message: "file 필드가 필요합니다".to_string(),
                details: Vec::new(),
            },
        };
        (StatusCode::BAD_REQUEST, Json(resp))
    })?;

    let input: ArticleDocument = serde_json::from_slice(&file_bytes).map_err(|e| {
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

    let article_id = input.article_id.trim().to_string();
    let base_path = state.base_path.clone();
    let options = ConvertOptions {
        include_header,
        ..ConvertOptions::default()
    };

    let convert_result =
        tokio::task::spawn_blocking(move || jsontohwpx::convert(&input, &options, &base_path))
            .await
            .map_err(|e| {
                let resp = ErrorResponse {
                    error: ErrorDetail {
                        code: "INTERNAL_ERROR".to_string(),
                        message: format!("변환 작업 실행 실패: {}", e),
                        details: Vec::new(),
                    },
                };
                (StatusCode::INTERNAL_SERVER_ERROR, Json(resp))
            })?;

    let bytes = convert_result.map_err(|e| {
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

    let filename = format!("{}.hwpx", article_id);
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

/// JSON을 HWPX 문서로 변환 (비동기)
///
/// 변환 작업을 큐에 등록하고 즉시 작업 ID를 반환합니다.
/// 작업 상태는 GET /api/v1/jobs/{id}로 확인할 수 있습니다.
#[utoipa::path(
    post,
    path = "/api/v1/convert/async",
    params(
        ("include_header" = Option<String>, Query, description = "메타데이터를 본문 상단에 삽입할지 여부 (true/false)")
    ),
    request_body(content = ConvertRequest, content_type = "application/json"),
    responses(
        (status = 202, description = "작업 등록 완료", body = AsyncConvertResponse),
        (status = 400, description = "잘못된 입력", body = ErrorResponse),
        (status = 503, description = "큐 용량 초과", body = ErrorResponse),
    ),
    tag = "변환"
)]
pub async fn convert_async(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ConvertQuery>,
    body: String,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let input: ArticleDocument = serde_json::from_str(&body).map_err(|e| {
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

    let job_id = Uuid::new_v4().to_string();
    let job = state.job_store.create_job(job_id.clone()).await;

    let include_header = parse_include_header(query.include_header.as_deref());
    let convert_job = ConvertJob {
        job_id: job_id.clone(),
        input,
        options: ConvertOptions {
            include_header,
            ..ConvertOptions::default()
        },
        base_path: state.base_path.clone(),
        output_dir: state.output_dir.clone(),
    };

    if let Err(e) = state.queue.submit(convert_job).await {
        state.job_store.set_failed(&job_id, e.clone()).await;
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "QUEUE_ERROR".to_string(),
                message: e,
                details: Vec::new(),
            },
        };
        return Err((StatusCode::SERVICE_UNAVAILABLE, Json(resp)));
    }

    let resp = AsyncConvertResponse {
        job_id,
        status: JobStatus::Queued,
        created_at: job.created_at,
    };

    Ok((StatusCode::ACCEPTED, Json(resp)))
}

/// 작업 상태 조회
///
/// 비동기 변환 작업의 현재 상태를 조회합니다.
#[utoipa::path(
    get,
    path = "/api/v1/jobs/{id}",
    params(("id" = String, Path, description = "작업 ID (UUID)")),
    responses(
        (status = 200, description = "작업 상태", body = JobResponse),
        (status = 404, description = "작업을 찾을 수 없음", body = ErrorResponse),
    ),
    tag = "작업"
)]
pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<JobResponse>, (StatusCode, Json<ErrorResponse>)> {
    let job = state.job_store.get_job(&id).await.ok_or_else(|| {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "NOT_FOUND".to_string(),
                message: format!("작업을 찾을 수 없습니다: {}", id),
                details: Vec::new(),
            },
        };
        (StatusCode::NOT_FOUND, Json(resp))
    })?;

    let download_url = if job.status == JobStatus::Completed {
        Some(format!("/api/v1/jobs/{}/download", job.id))
    } else {
        None
    };

    let resp = JobResponse {
        job_id: job.id,
        status: job.status,
        created_at: job.created_at,
        completed_at: job.completed_at,
        download_url,
        error: job.error_message,
    };

    Ok(Json(resp))
}

/// 완료된 작업 파일 다운로드
///
/// 변환이 완료된 HWPX 파일을 다운로드합니다.
#[utoipa::path(
    get,
    path = "/api/v1/jobs/{id}/download",
    params(("id" = String, Path, description = "작업 ID (UUID)")),
    responses(
        (status = 200, description = "HWPX 파일 다운로드", content_type = "application/vnd.hancom.hwpx"),
        (status = 404, description = "파일을 찾을 수 없음", body = ErrorResponse),
    ),
    tag = "작업"
)]
pub async fn download_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let job = state.job_store.get_job(&id).await.ok_or_else(|| {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "NOT_FOUND".to_string(),
                message: format!("작업을 찾을 수 없습니다: {}", id),
                details: Vec::new(),
            },
        };
        (StatusCode::NOT_FOUND, Json(resp))
    })?;

    if job.status != JobStatus::Completed {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "NOT_READY".to_string(),
                message: format!("작업이 아직 완료되지 않았습니다 (상태: {:?})", job.status),
                details: Vec::new(),
            },
        };
        return Err((StatusCode::NOT_FOUND, Json(resp)));
    }

    let file_path = job.file_path.ok_or_else(|| {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "FILE_MISSING".to_string(),
                message: "결과 파일을 찾을 수 없습니다".to_string(),
                details: Vec::new(),
            },
        };
        (StatusCode::NOT_FOUND, Json(resp))
    })?;

    let bytes = tokio::fs::read(&file_path).await.map_err(|e| {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "IO_ERROR".to_string(),
                message: format!("파일 읽기 실패: {}", e),
                details: Vec::new(),
            },
        };
        (StatusCode::INTERNAL_SERVER_ERROR, Json(resp))
    })?;

    let atcl_id = job.atcl_id.unwrap_or_else(|| id.clone());
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
    let input: ArticleDocument = match serde_json::from_str(&body) {
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
/// 서버의 현재 상태, 큐 통계, 워커 정보, 가동 시간을 반환합니다.
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
    let stats = state.job_store.stats().await;

    let (status, license) = match state.license_expiry {
        Some(expiry) => {
            let today = chrono::Local::now().date_naive();
            if today > expiry {
                ("expired".to_string(), Some("expired".to_string()))
            } else {
                ("healthy".to_string(), Some("valid".to_string()))
            }
        }
        None => ("healthy".to_string(), None),
    };

    let resp = HealthResponse {
        status,
        version: env!("CARGO_PKG_VERSION").to_string(),
        queue: stats,
        workers: WorkerInfo {
            active: state.queue.active_workers(),
            max: state.queue.max_workers(),
        },
        uptime_seconds: uptime,
        license,
    };
    Json(resp)
}
