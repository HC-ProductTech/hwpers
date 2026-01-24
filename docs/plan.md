# jsontohwpx 개발 체크리스트

> 기획서: [docs/spec.md](./spec.md)
> 프로젝트 컨텍스트: [CLAUDE.md](../CLAUDE.md)

---

## Phase 1: 핵심 변환 기능 + CLI

### M1: 프로젝트 초기화

- [ ] HC-ProductTech/hwpers 레포 clone
- [ ] HwpxWriter/HwpxReader API 확인 및 사용 가능 여부 점검
- [ ] `src/jsontohwpx/` 디렉토리 생성
  - [ ] `mod.rs`
  - [ ] `model.rs`
  - [ ] `converter.rs`
  - [ ] `text.rs`
  - [ ] `image.rs`
  - [ ] `table.rs`
  - [ ] `error.rs`
- [ ] `src/bin/jsontohwpx.rs` CLI 진입점 생성
- [ ] `src/lib.rs`에 `pub mod jsontohwpx;` 추가
- [ ] Cargo.toml에 의존성 추가
  - [ ] `clap = { version = "4", features = ["derive"] }`
  - [ ] `scraper = "0.21"`
  - [ ] `base64 = "0.22"`
  - [ ] `image = "0.25"`
  - [ ] `reqwest = { version = "0.12", features = ["blocking"] }`
- [ ] Cargo.toml에 `[[bin]]` 타겟 추가
- [ ] `cargo build` 성공 확인
- [ ] `cargo clippy -- -D warnings` 통과
- [ ] `cargo fmt --check` 통과

### M2: JSON 모델 정의

- [ ] **error.rs**: 에러 타입 정의
  - [ ] `JsonToHwpxError` enum (thiserror 기반)
  - [ ] `InputError` (잘못된 JSON, responseCode 비정상)
  - [ ] `ConversionError` (이미지 다운로드 실패, 포맷 변환 실패)
  - [ ] `IoError` (파일 쓰기 실패)
- [ ] **model.rs**: JSON 입력 모델 구현
  - [ ] `ApiResponse` (responseCode, responseText, options, data)
  - [ ] `Options` (includeHeader, headerFields)
  - [ ] `Data` → `Article` (atclId, subject, contents, regDt, regEmpName, regDeptName)
  - [ ] `Content` enum (Text, Image, Table)
  - [ ] `ImageContent` (url vs base64+format)
  - [ ] serde `Deserialize` derive
- [ ] **입력 검증 로직**
  - [ ] `responseCode` != "0" → `InputError` 반환
  - [ ] `contents` 빈 배열 → 경고 로그 + 빈 문서 생성 허용
  - [ ] 필수 필드 누락 검증 (atclId, subject)
  - [ ] `type` 필드 유효값 검증 (text/image/table)
- [ ] **단위 테스트**
  - [ ] 정상 JSON 파싱 테스트 (`simple_text.json`)
  - [ ] responseCode 에러 테스트
  - [ ] 잘못된 JSON 에러 테스트
  - [ ] 빈 contents 경고 테스트
  - [ ] 알 수 없는 type 에러 테스트

### M3: HwpxWriter 연동 기본 변환

- [ ] **converter.rs**: 메인 변환 로직 스캐폴드
  - [ ] `convert(input: &ApiResponse) -> Result<Vec<u8>>` 함수
  - [ ] `convert_to_file(input: &ApiResponse, path: &Path) -> Result<()>` 함수
  - [ ] HwpxWriter 인스턴스 생성
  - [ ] 용지 설정: A4 고정 (210mm × 297mm)
  - [ ] 메타데이터 설정 (subject, author 등)
  - [ ] `writer.to_bytes()` / `writer.save_to_file()` 호출
- [ ] **파이프라인 확인**
  - [ ] 빈 contents → 빈 HWPX 파일 생성
  - [ ] 생성된 파일이 유효한 ZIP 구조인지 확인
  - [ ] HwpxReader로 읽기 가능한지 검증
- [ ] **통합 테스트**
  - [ ] `tests/jsontohwpx_test.rs` 생성
  - [ ] 빈 문서 생성 테스트
  - [ ] HwpxReader로 검증하는 헬퍼 함수 작성

### M4: 텍스트 변환

- [ ] **text.rs**: 텍스트 변환 모듈
  - [ ] `\n` → 새 단락 (각 줄을 `add_paragraph()` 호출)
  - [ ] `\n\n` → 빈 단락 포함 (빈 줄도 `add_paragraph("")` 호출)
  - [ ] 빈 value → 빈 단락 하나 생성
  - [ ] 특수문자 (`<`, `>`, `&`) 그대로 전달
- [ ] **연속 text 요소 처리** (converter.rs)
  - [ ] 이전 요소가 text이고 현재도 text이면 사이에 빈 단락 추가
- [ ] **메타데이터 처리**
  - [ ] subject → 문서 속성 저장
  - [ ] regEmpName → 문서 작성자
  - [ ] regDeptName → 문서 속성
  - [ ] regDt → 문서 속성
- [ ] **includeHeader 옵션**
  - [ ] `true` → 본문 상단에 메타데이터 텍스트 삽입
  - [ ] headerFields로 포함 필드 선택
  - [ ] 구분선 (─────) 삽입
  - [ ] 굵은 스타일 적용 (`add_styled_paragraph`)
- [ ] **단위 테스트**
  - [ ] 단일 줄 텍스트
  - [ ] `\n` 포함 텍스트 (여러 단락)
  - [ ] `\n\n` 포함 텍스트 (빈 단락)
  - [ ] 연속 text 요소 사이 빈 단락
  - [ ] 특수문자 포함 텍스트
  - [ ] includeHeader=true 검증
  - [ ] headerFields 선택 검증
- [ ] **통합 테스트**
  - [ ] `simple_text.json` 변환 + HwpxReader 검증
  - [ ] `with_metadata_header.json` 변환 + 검증

### M5: 이미지 삽입

- [ ] **image.rs**: 이미지 처리 모듈
  - [ ] 포맷 감지 함수 (확장자 + 매직 바이트)
    - [ ] PNG (89 50 4E 47)
    - [ ] JPEG (FF D8 FF)
    - [ ] GIF (47 49 46)
    - [ ] BMP (42 4D)
    - [ ] WebP (52 49 46 46 ... 57 45 42 50)
    - [ ] AVIF
  - [ ] 네이티브 포맷 판별 (PNG, JPEG, GIF, BMP)
  - [ ] 비네이티브 → PNG 변환 (image 크레이트)
    - [ ] WebP → PNG
    - [ ] AVIF → PNG
  - [ ] GIF → 첫 프레임 추출 → PNG
- [ ] **이미지 소스 처리**
  - [ ] 로컬 파일 경로 (JSON 파일 위치 기준 상대 경로 해석)
  - [ ] 외부 URL 다운로드 (reqwest blocking)
    - [ ] 타임아웃 60초
    - [ ] 실패 시 전체 변환 중단 (에러 반환)
  - [ ] Base64 디코딩
    - [ ] format 필드로 포맷 결정
- [ ] **HwpxWriter 호출**
  - [ ] 네이티브 포맷: `add_image_from_file()` 또는 `add_image(bytes, format)`
  - [ ] 변환 필요 포맷: 변환 후 `add_image(png_bytes, Png)`
  - [ ] 이미지 크기: 원본 그대로 삽입
- [ ] **단위 테스트**
  - [ ] PNG 포맷 감지
  - [ ] JPEG 포맷 감지 (.jpg, .jpeg)
  - [ ] GIF 포맷 감지
  - [ ] WebP 포맷 감지
  - [ ] AVIF 포맷 감지
  - [ ] 매직 바이트 기반 감지 (확장자 없는 경우)
  - [ ] Base64 디코딩 테스트
  - [ ] 상대 경로 해석 테스트
  - [ ] WebP → PNG 변환 테스트
  - [ ] AVIF → PNG 변환 테스트
  - [ ] GIF 첫 프레임 추출 테스트
- [ ] **통합 테스트** (`tests/jsontohwpx_image_test.rs`)
  - [ ] `image_png.json` 변환 검증
  - [ ] `image_jpg.json` 변환 검증
  - [ ] `image_gif.json` 변환 검증
  - [ ] `image_webp.json` 변환 검증
  - [ ] `image_avif.json` 변환 검증
  - [ ] `with_image.json` (텍스트+이미지 혼합)
  - [ ] `with_image_base64.json` 변환 검증
  - [ ] `image_multi_format.json` 다양한 포맷 동시 처리
  - [ ] 외부 URL 다운로드 실패 → 에러 반환 테스트

### M6: 표 변환

- [ ] **table.rs**: HTML 테이블 파싱 모듈
  - [ ] scraper로 `<table>` 파싱
  - [ ] `<thead>` / `<tbody>` 구분 추출
  - [ ] `<tr>`, `<th>`, `<td>` 순회
  - [ ] `<th>` → 굵은 글씨 표시
  - [ ] 인라인 스타일 무시 (구조만 추출)
  - [ ] 셀 내부 텍스트만 추출 (이미지/중첩 테이블 없음 가정)
- [ ] **colspan/rowspan 처리**
  - [ ] HwpxWriter의 테이블 병합 API 확인
  - [ ] 지원 시: colspan/rowspan 반영
  - [ ] 미지원 시: 일반 셀로 fallback (병합 무시)
- [ ] **HwpxWriter 호출**
  - [ ] `HwpxTable::from_data(headers, rows)` 구성
  - [ ] `writer.add_table(table)` 호출
- [ ] **단위 테스트**
  - [ ] 단순 테이블 (2x3) 파싱
  - [ ] thead/tbody 구분 테스트
  - [ ] th 굵은 글씨 테스트
  - [ ] colspan 처리 테스트
  - [ ] rowspan 처리 테스트
  - [ ] 인라인 스타일 무시 테스트
  - [ ] 빈 셀 처리
- [ ] **통합 테스트** (`tests/jsontohwpx_table_test.rs`)
  - [ ] `with_table.json` 변환 검증
  - [ ] `table_merge.json` 변환 검증

### M7: CLI 완성

- [ ] **src/bin/jsontohwpx.rs**: clap 기반 CLI
  - [ ] 위치 인자: 입력 JSON 파일 경로
  - [ ] `-o, --output`: 출력 파일 경로 (기본: `{atclId}.hwpx`)
  - [ ] `--validate`: 검증만 수행
  - [ ] `--include-header`: includeHeader 강제 활성화
  - [ ] `--json`: 에러 출력을 JSON 형식으로
  - [ ] stdin 입력 지원 (`-`로 지정 또는 파이프)
- [ ] **출력 파일명 자동 생성**
  - [ ] `-o` 미지정 시 `{atclId}.hwpx` 자동 생성
  - [ ] `-o` 지정 시 지정된 경로 사용
- [ ] **에러 출력**
  - [ ] 기본: stderr에 텍스트 형식
  - [ ] `--json`: stderr에 JSON 형식 (`{ "error": { "code", "message", "details" } }`)
- [ ] **종료 코드**
  - [ ] 0 = 성공
  - [ ] 1 = 입력 에러
  - [ ] 2 = 변환 에러
  - [ ] 3 = IO 에러
- [ ] **진행 로그** (stderr)
  - [ ] `[1/N] JSON 파싱 중...`
  - [ ] `[2/N] 이미지 다운로드 중... (X개)`
  - [ ] `[3/N] 이미지 포맷 변환 중...`
  - [ ] `[4/N] HWPX 변환 중...`
  - [ ] `[5/N] 파일 저장: {filename}`
  - [ ] `완료!`
- [ ] **통합 테스트**
  - [ ] `full_document.json` 전체 파이프라인 검증
  - [ ] 잘못된 파일 경로 → exit code 1
  - [ ] 이미지 다운로드 실패 → exit code 2
  - [ ] 쓰기 불가 경로 → exit code 3
  - [ ] `--validate` 모드 검증
  - [ ] stdin 입력 검증

### Phase 1 완료 검증

- [ ] `cargo clippy -- -D warnings` 통과
- [ ] `cargo fmt --check` 통과
- [ ] `cargo test` 전체 통과
- [ ] 테스트 커버리지 80% 이상
- [ ] `unwrap()` 미사용 (테스트 코드 제외)
- [ ] public API에 문서 주석 (`///`)
- [ ] 성능: 기본 문서 변환 100ms 이내
- [ ] `full_document.json` → HWPX 생성 → 한글 오피스에서 열기 수동 확인

---

## Phase 2: REST API

### M8: REST API (동기)

- [ ] **의존성 추가** (Cargo.toml)
  - [ ] `axum = "0.7"`
  - [ ] `tokio = { version = "1", features = ["full"] }`
  - [ ] `tower = "0.5"`
  - [ ] `tower-http = { version = "0.5", features = ["cors", "trace", "limit"] }`
  - [ ] `tracing = "0.1"`
  - [ ] `tracing-subscriber = { version = "0.3", features = ["json"] }`
  - [ ] `uuid = { version = "1", features = ["v4"] }`
  - [ ] `chrono = { version = "0.4", features = ["serde"] }`
- [ ] **`[[bin]]` 타겟 추가**: `jsontohwpx-api`
- [ ] **src/bin/jsontohwpx_api.rs**: API 서버 진입점
  - [ ] tokio 런타임 설정
  - [ ] 환경변수 읽기 (HOST, PORT, MAX_REQUEST_SIZE 등)
  - [ ] tracing + JSON 로깅 초기화
  - [ ] axum Router 구성
  - [ ] 서버 시작 로그
- [ ] **src/jsontohwpx/api/mod.rs**: API 모듈
- [ ] **src/jsontohwpx/api/routes.rs**: 라우트 정의
  - [ ] `POST /api/v1/convert`
  - [ ] `POST /api/v1/validate`
  - [ ] `GET /api/v1/health`
- [ ] **src/jsontohwpx/api/handlers.rs**: 핸들러 구현
  - [ ] **convert 핸들러**
    - [ ] JSON body 파싱
    - [ ] 변환 실행
    - [ ] `Content-Type: application/vnd.hancom.hwpx` 응답
    - [ ] `Content-Disposition: attachment; filename="{atclId}.hwpx"` 헤더
    - [ ] 바이너리 직접 반환
  - [ ] **validate 핸들러**
    - [ ] JSON 파싱 + 검증
    - [ ] `{ "valid": true/false, "errors": [...] }` 반환
  - [ ] **health 핸들러**
    - [ ] 상세 정보 반환 (status, version, queue, workers, uptime)
- [ ] **에러 핸들링**
  - [ ] JSON 에러 응답 형식 (`{ "error": { "code", "message", "details" } }`)
  - [ ] 잘못된 JSON → 400
  - [ ] responseCode 비정상 → 400
  - [ ] 변환 실패 → 500
- [ ] **미들웨어**
  - [ ] 요청 크기 제한 (`MAX_REQUEST_SIZE`, 기본 50MB)
  - [ ] 요청별 tracing span (request_id, method, path)
  - [ ] CORS 설정
- [ ] **테스트**
  - [ ] 동기 변환 API 성공 테스트
  - [ ] 잘못된 입력 → 400 에러
  - [ ] validate 성공/실패 테스트
  - [ ] health 응답 검증
  - [ ] 요청 크기 초과 테스트

### M9: REST API (비동기)

- [ ] **src/jsontohwpx/api/queue.rs**: 인메모리 작업 큐
  - [ ] `tokio::sync::mpsc` 채널 기반 큐
  - [ ] 워커 수 설정 (`WORKER_COUNT`, 기본: CPU 코어 수)
  - [ ] 작업 수신 → 변환 실행 → 결과 저장
- [ ] **src/jsontohwpx/api/jobs.rs**: 작업 상태 관리
  - [ ] `Job` 구조체 (jobId, status, createdAt, completedAt, filePath)
  - [ ] status: `queued` | `processing` | `completed` | `failed`
  - [ ] 인메모리 저장소 (Arc<RwLock<HashMap>>)
- [ ] **엔드포인트 추가** (routes.rs)
  - [ ] `POST /api/v1/convert/async`
  - [ ] `GET /api/v1/jobs/{id}`
  - [ ] `GET /api/v1/jobs/{id}/download`
- [ ] **핸들러 구현** (handlers.rs)
  - [ ] **convert_async**: 큐에 작업 등록, jobId 즉시 반환
  - [ ] **get_job**: 작업 상태 조회
  - [ ] **download_job**: 완료된 파일 다운로드 (미완료 → 404)
- [ ] **파일 만료/정리**
  - [ ] 임시 파일 저장: `/tmp/jsontohwpx/{jobId}.hwpx`
  - [ ] 백그라운드 태스크: 24시간 경과 파일 삭제
  - [ ] `FILE_EXPIRY_HOURS` 환경변수로 설정 가능
- [ ] **Graceful Shutdown**
  - [ ] SIGTERM 수신 → 새 요청 거부
  - [ ] 진행 중 작업 30초간 대기
  - [ ] 30초 초과 → 강제 종료
- [ ] **health 업데이트**
  - [ ] queue 정보 (pending, processing, completed, failed)
  - [ ] workers 정보 (active, max)
  - [ ] uptime_seconds
- [ ] **테스트**
  - [ ] 비동기 변환 → 작업 ID 반환 테스트
  - [ ] 작업 상태 조회 (queued → processing → completed)
  - [ ] 완료 후 다운로드 테스트
  - [ ] 미완료 작업 다운로드 → 404
  - [ ] 실패 작업 상태 조회
  - [ ] Graceful shutdown 테스트

### Phase 2 완료 검증

- [ ] `cargo clippy -- -D warnings` 통과
- [ ] `cargo fmt --check` 통과
- [ ] `cargo test` 전체 통과
- [ ] API 동기 응답 시간: 기본 문서 200ms 이내
- [ ] 동시 요청 처리 검증
- [ ] curl/httpie로 수동 API 테스트
  - [ ] `POST /api/v1/convert` → HWPX 파일 저장
  - [ ] `POST /api/v1/convert/async` → jobId → 다운로드
  - [ ] `GET /api/v1/health` → 상태 확인

---

## Phase 3: Docker 컨테이너화

### M10: 컨테이너화

- [ ] **Dockerfile** (멀티 스테이지 빌드)
  - [ ] 빌드 스테이지: `rust:1.75`
  - [ ] 실행 스테이지: `debian:bookworm-slim`
  - [ ] ca-certificates 설치 (HTTPS 다운로드용)
  - [ ] `/tmp/jsontohwpx` 디렉토리 생성
  - [ ] EXPOSE 8080
- [ ] **docker-compose.yml**
  - [ ] API 서비스 단독 구성
  - [ ] 환경변수 설정
    - [ ] `RUST_LOG=info`
    - [ ] `HOST=0.0.0.0`
    - [ ] `PORT=8080`
    - [ ] `MAX_REQUEST_SIZE=52428800`
    - [ ] `WORKER_COUNT=4`
    - [ ] `FILE_EXPIRY_HOURS=24`
    - [ ] `IMAGE_DOWNLOAD_TIMEOUT=60`
  - [ ] healthcheck 구성 (curl /api/v1/health)
  - [ ] 리소스 제한 (memory: 1G, cpus: 2.0)
  - [ ] tmpfs 마운트 (/tmp/jsontohwpx:size=512M)
  - [ ] restart: unless-stopped
- [ ] **빌드 및 실행 확인**
  - [ ] `docker build -t jsontohwpx-api .` 성공
  - [ ] `docker-compose up` 정상 기동
  - [ ] healthcheck 통과
  - [ ] API 엔드포인트 정상 동작
- [ ] **검증**
  - [ ] 동기 변환 API 테스트 (curl)
  - [ ] 비동기 변환 API 테스트
  - [ ] 파일 만료 정리 동작 확인
  - [ ] Graceful shutdown (docker stop → 정상 종료)
  - [ ] 메모리 제한 내 동작 확인
  - [ ] 외부 이미지 URL 다운로드 (ca-certificates 동작)

### Phase 3 완료 검증

- [ ] Docker 이미지 크기 확인 (경량화)
- [ ] `docker-compose up -d` → `docker-compose down` 정상
- [ ] 장시간 운영 시 메모리 누수 없음
- [ ] 임시 파일 24시간 후 정리 확인

---

## 최종 검증 체크리스트

### 코드 품질
- [ ] `cargo clippy -- -D warnings` 0 경고
- [ ] `cargo fmt --check` 통과
- [ ] 테스트 커버리지 80% 이상
- [ ] `unwrap()` 미사용 (테스트 제외)
- [ ] `unsafe` 미사용 (또는 최소화)
- [ ] public API 문서 주석 완비
- [ ] 하드코딩된 시크릿 없음
- [ ] console.log / println! 디버그 코드 없음

### 성능
- [ ] 기본 문서 변환: 100ms 이내
- [ ] 대용량 문서 변환: 1초 이내
- [ ] API 동기 응답: 200ms 이내 (이미지 다운로드 제외)

### 안정성
- [ ] 잘못된 JSON → 명확한 에러 메시지
- [ ] 이미지 다운로드 실패 → 에러로 중단
- [ ] 대용량 요청 → 크기 제한으로 거부
- [ ] Graceful shutdown 정상 동작
- [ ] 빈 contents → 경고 후 빈 문서 생성

### 기능 완성도
- [ ] CLI: 모든 옵션 동작 (input, -o, --validate, --include-header, --json)
- [ ] 텍스트: `\n`/`\n\n` 처리, 연속 text 사이 빈 단락
- [ ] 이미지: PNG, JPG, GIF(첫 프레임), BMP, WebP→PNG, AVIF→PNG
- [ ] 이미지: 로컬 경로, 외부 URL, Base64
- [ ] 테이블: HTML 파싱, thead/tbody, th 굵게, 구조만 추출
- [ ] API: 동기 (/convert) + 비동기 (/convert/async)
- [ ] Docker: 빌드, 실행, healthcheck, 환경변수 설정
