use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;

use super::jobs::JobStore;
use crate::jsontohwpx;
use crate::jsontohwpx::{ArticleDocument, ConvertOptions};

/// 큐에 전달되는 변환 작업
pub struct ConvertJob {
    pub job_id: String,
    pub input: ArticleDocument,
    pub options: ConvertOptions,
    pub base_path: PathBuf,
    pub output_dir: PathBuf,
}

/// 작업 큐 및 워커 관리
#[derive(Clone)]
pub struct JobQueue {
    sender: mpsc::Sender<ConvertJob>,
    active_workers: Arc<AtomicU64>,
    max_workers: u64,
}

impl JobQueue {
    /// 새 작업 큐 생성 및 워커 시작
    pub fn new(worker_count: u64, job_store: JobStore) -> Self {
        let (tx, rx) = mpsc::channel::<ConvertJob>(1000);
        let active_workers = Arc::new(AtomicU64::new(0));

        let queue = Self {
            sender: tx,
            active_workers: active_workers.clone(),
            max_workers: worker_count,
        };

        // 워커 풀 시작
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        for worker_id in 0..worker_count {
            let rx = rx.clone();
            let store = job_store.clone();
            let active = active_workers.clone();

            tokio::spawn(async move {
                loop {
                    let job = {
                        let mut rx = rx.lock().await;
                        rx.recv().await
                    };

                    match job {
                        Some(convert_job) => {
                            active.fetch_add(1, Ordering::SeqCst);
                            process_job(&store, convert_job, worker_id).await;
                            active.fetch_sub(1, Ordering::SeqCst);
                        }
                        None => {
                            tracing::info!(worker_id, "워커 종료");
                            break;
                        }
                    }
                }
            });
        }

        queue
    }

    /// 작업을 큐에 추가
    pub async fn submit(&self, job: ConvertJob) -> Result<(), String> {
        self.sender
            .send(job)
            .await
            .map_err(|_| "큐가 닫혔습니다".to_string())
    }

    /// 현재 활성 워커 수
    pub fn active_workers(&self) -> u64 {
        self.active_workers.load(Ordering::SeqCst)
    }

    /// 최대 워커 수
    pub fn max_workers(&self) -> u64 {
        self.max_workers
    }
}

/// 개별 작업 처리
async fn process_job(store: &JobStore, job: ConvertJob, worker_id: u64) {
    let job_id = job.job_id.clone();
    tracing::info!(worker_id, job_id = %job_id, "작업 처리 시작");

    store.set_processing(&job_id).await;

    // 변환 실행 (blocking 작업이므로 spawn_blocking 사용)
    let input = job.input;
    let options = job.options;
    let base_path = job.base_path;
    let output_dir = job.output_dir;
    let jid = job_id.clone();

    let result = tokio::task::spawn_blocking(move || {
        let article_id = input.article_id.trim().to_string();
        match jsontohwpx::convert(&input, &options, &base_path) {
            Ok(bytes) => {
                let file_path = output_dir.join(format!("{}.hwpx", jid));
                std::fs::create_dir_all(&output_dir).ok();
                match std::fs::write(&file_path, bytes) {
                    Ok(()) => Ok((file_path, article_id)),
                    Err(e) => Err(format!("파일 저장 실패: {}", e)),
                }
            }
            Err(e) => Err(format!("변환 실패: {}", e)),
        }
    })
    .await;

    match result {
        Ok(Ok((file_path, article_id))) => {
            store.set_completed(&job_id, file_path, article_id).await;
            tracing::info!(worker_id, job_id = %job_id, "작업 완료");
        }
        Ok(Err(e)) => {
            store.set_failed(&job_id, e.clone()).await;
            tracing::error!(worker_id, job_id = %job_id, error = %e, "작업 실패");
        }
        Err(e) => {
            let msg = format!("작업 패닉: {}", e);
            store.set_failed(&job_id, msg.clone()).await;
            tracing::error!(worker_id, job_id = %job_id, error = %msg, "작업 패닉");
        }
    }
}
