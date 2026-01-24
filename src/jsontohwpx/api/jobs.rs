use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::RwLock;
use utoipa::ToSchema;

/// 작업 상태
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

/// 작업 정보
#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub file_path: Option<PathBuf>,
    pub atcl_id: Option<String>,
    pub error_message: Option<String>,
}

/// 작업 상태 조회 응답
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "jobId": "550e8400-e29b-41d4-a716-446655440000",
    "status": "completed",
    "createdAt": "2025-01-24T09:00:00Z",
    "completedAt": "2025-01-24T09:00:02Z",
    "downloadUrl": "/api/v1/jobs/550e8400-e29b-41d4-a716-446655440000/download"
}))]
pub struct JobResponse {
    pub job_id: String,
    pub status: JobStatus,
    #[schema(value_type = String, format = "date-time")]
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = "date-time")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 비동기 변환 요청 응답
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "jobId": "550e8400-e29b-41d4-a716-446655440000",
    "status": "queued",
    "createdAt": "2025-01-24T09:00:00Z"
}))]
pub struct AsyncConvertResponse {
    pub job_id: String,
    pub status: JobStatus,
    #[schema(value_type = String, format = "date-time")]
    pub created_at: DateTime<Utc>,
}

/// 인메모리 작업 저장소
#[derive(Clone)]
pub struct JobStore {
    jobs: Arc<RwLock<HashMap<String, Job>>>,
}

impl Default for JobStore {
    fn default() -> Self {
        Self::new()
    }
}

impl JobStore {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 새 작업 생성 및 저장
    pub async fn create_job(&self, id: String) -> Job {
        let job = Job {
            id: id.clone(),
            status: JobStatus::Queued,
            created_at: Utc::now(),
            completed_at: None,
            file_path: None,
            atcl_id: None,
            error_message: None,
        };
        self.jobs.write().await.insert(id, job.clone());
        job
    }

    /// 작업 상태를 Processing으로 변경
    pub async fn set_processing(&self, id: &str) {
        if let Some(job) = self.jobs.write().await.get_mut(id) {
            job.status = JobStatus::Processing;
        }
    }

    /// 작업 완료 처리
    pub async fn set_completed(&self, id: &str, file_path: PathBuf, atcl_id: String) {
        if let Some(job) = self.jobs.write().await.get_mut(id) {
            job.status = JobStatus::Completed;
            job.completed_at = Some(Utc::now());
            job.file_path = Some(file_path);
            job.atcl_id = Some(atcl_id);
        }
    }

    /// 작업 실패 처리
    pub async fn set_failed(&self, id: &str, error: String) {
        if let Some(job) = self.jobs.write().await.get_mut(id) {
            job.status = JobStatus::Failed;
            job.completed_at = Some(Utc::now());
            job.error_message = Some(error);
        }
    }

    /// 작업 조회
    pub async fn get_job(&self, id: &str) -> Option<Job> {
        self.jobs.read().await.get(id).cloned()
    }

    /// 통계 조회
    pub async fn stats(&self) -> JobStats {
        let jobs = self.jobs.read().await;
        let mut stats = JobStats::default();
        for job in jobs.values() {
            match job.status {
                JobStatus::Queued => stats.pending += 1,
                JobStatus::Processing => stats.processing += 1,
                JobStatus::Completed => stats.completed += 1,
                JobStatus::Failed => stats.failed += 1,
            }
        }
        stats
    }

    /// 만료된 작업 정리 (파일 삭제 + 작업 제거)
    pub async fn cleanup_expired(&self, expiry_hours: u64) {
        let now = Utc::now();
        let mut jobs = self.jobs.write().await;
        let expired_ids: Vec<String> = jobs
            .iter()
            .filter(|(_, job)| {
                let age = now.signed_duration_since(job.created_at);
                age.num_hours() >= expiry_hours as i64
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in &expired_ids {
            if let Some(job) = jobs.remove(id) {
                if let Some(path) = &job.file_path {
                    let _ = std::fs::remove_file(path);
                }
            }
        }

        if !expired_ids.is_empty() {
            tracing::info!(count = expired_ids.len(), "만료된 작업 정리 완료");
        }
    }
}

/// 작업 통계
#[derive(Default, Serialize, ToSchema)]
pub struct JobStats {
    pub pending: u64,
    pub processing: u64,
    pub completed: u64,
    pub failed: u64,
}
