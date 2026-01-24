# jsontohwpx 개발 기획서

## 1. 프로젝트 개요

### 1.1 목적
JSON 형식의 구조화된 데이터를 HWPX (한글 오피스 XML) 문서로 변환하는 도구를 개발한다.

### 1.2 프로젝트 기반
- 레포지토리: [HC-ProductTech/hwpers](https://github.com/HC-ProductTech/hwpers)를 clone하여 개발
- hwpers 레포를 기반으로 jsontohwpx 기능을 추가하는 구조
- 내부 모듈(`crate::hwpx::HwpxWriter` 등)을 직접 사용
- 별도의 git 의존성 불필요 — 같은 크레이트 내에서 개발
- 업스트림 동기화 불필요 — 독립적으로 유지

### 1.3 개발 단계

| 단계 | 목표 | 산출물 |
|------|------|--------|
| 1차 | 핵심 변환 기능 + CLI | 라이브러리 + CLI 바이너리 |
| 2차 | REST API 서비스 | HTTP API 서버 (동기 + 비동기) |
| 3차 | 컨테이너화 | Dockerfile + docker-compose.yml |

---

## 2. HWPX 파일 형식

> **구현 참고**: HWPX 파일 생성은 hwpers의 `HwpxWriter`가 담당한다.
> 아래 구조는 이해를 위한 참고 자료이며, 직접 구현하지 않는다.

### 2.1 개요
HWPX는 한글과컴퓨터의 개방형 문서 형식으로, ZIP 아카이브 내에 XML 파일들을 포함하는 구조이다.

### 2.2 파일 구조

```
document.hwpx (ZIP 아카이브)
├── mimetype                          # "application/hwp+zip" (비압축)
├── META-INF/
│   ├── container.xml                 # 루트 파일 참조
│   └── manifest.xml                  # 파일 목록 및 MIME 타입
├── Contents/
│   ├── content.hpf                   # 문서 메타데이터 및 구조 정의
│   ├── header.xml                    # 문서 설정 (용지, 여백 등)
│   ├── section0.xml                  # 본문 섹션 (텍스트, 표 등)
│   └── BinData/                      # 이미지 등 바이너리 데이터
│       └── image1.png
└── Preview/
    └── PrvText.txt                   # 미리보기용 텍스트
```

### 2.3 HwpxWriter API 매핑

본 프로젝트에서 사용하는 hwpers `HwpxWriter`의 주요 API:

| JSON contents type | HwpxWriter 메서드 | 비고 |
|--------------------|-------------------|------|
| `text` (단순) | `add_paragraph(text)` | `\n`으로 분리하여 각각 호출 |
| `text` (스타일) | `add_styled_paragraph(text, style)` | includeHeader 시 굵은 제목 등 |
| `image` (경로) | `add_image_from_file(path, format)` | 파일에서 직접 로드 |
| `image` (base64) | `add_image(bytes, format)` | 디코딩 후 바이트 전달 |
| `table` | `HwpxTable::from_data()` + `add_table()` | HTML 파싱 후 셀 데이터 구성 |

**HwpxWriter 주요 타입:**
```rust
// 텍스트 스타일
HwpxTextStyle::new().bold().size(14).color(0x333333)

// 혼합 스타일 단락
let runs = vec![
    StyledText::new("일반 "),
    StyledText::with_style("굵게", HwpxTextStyle::new().bold()),
];
writer.add_mixed_styled_paragraph(runs)?;

// 이미지
writer.add_image_from_file("./test_img.png", HwpxImageFormat::Png)?;
writer.add_image(png_bytes, HwpxImageFormat::Png)?;

// 테이블
let table = HwpxTable::from_data(headers, rows);
writer.add_table(table)?;

// 저장
writer.to_bytes()?;          // Vec<u8>
writer.save_to_file(path)?;  // 파일 저장
```

**검증 (테스트용):**
```rust
use hwpers::hwpx::HwpxReader;

let doc = HwpxReader::from_file(path)?;
let text = doc.extract_text();
assert!(text.contains("예상 텍스트"));
```

> **참고**: 같은 크레이트 내이므로 `use crate::hwpx::{HwpxWriter, ...}` 로 접근.

---

## 3. JSON 입력 형식 설계

### 3.1 기본 구조

API 응답 형태의 JSON을 입력으로 받는다. `contents` 배열의 순서대로 문서에 내용이 삽입된다.

```json
{
  "responseCode": "0",
  "responseText": "SUCCESS",
  "options": {
    "includeHeader": false,
    "headerFields": ["subject", "regEmpName", "regDeptName", "regDt"]
  },
  "data": {
    "article": {
      "atclId": "고유 식별자",
      "subject": "문서 제목",
      "contents": [],
      "regDt": "등록일시",
      "regEmpName": "작성자명",
      "regDeptName": "작성 부서명"
    }
  }
}
```

### 3.2 responseCode 처리

- `responseCode`가 `"0"`이 아닌 경우 에러로 처리하고 변환을 중단한다.
- 에러 메시지에 `responseCode`와 `responseText` 값을 포함한다.

### 3.3 메타데이터 처리

`article`의 `subject`, `regEmpName`, `regDeptName`, `regDt`는 HWPX 문서의 메타데이터에 저장된다.

#### options.includeHeader

`true`로 설정하면 메타데이터가 본문 상단에 텍스트로도 삽입된다:

```
제목: [subject]
작성자: [regEmpName]
부서: [regDeptName]
작성일: [regDt]
─────────────────────────
(본문 시작)
```

`headerFields` 배열로 어떤 필드를 헤더에 포함할지 선택 가능하다.

### 3.4 contents 요소 타입

`contents`는 순서가 있는 배열이며, 각 요소의 `type`에 따라 처리된다.
- `contents`만 사용한다 (다른 필드의 본문 데이터는 무시).
- `atclCn` 등 다른 본문 필드가 있어도 무시한다.

#### 빈 contents 처리

`contents`가 빈 배열(`[]`)인 경우:
- 경고 로그를 출력한다.
- 본문 없이 메타데이터만 포함된 빈 HWPX 문서를 생성한다.

#### 연속된 text 요소 처리

연속된 `text` 타입 요소 사이에는 빈 단락 하나를 추가하여 구분한다.

#### type: "text"

```json
{
  "type": "text",
  "value": "텍스트 내용\n\n줄바꿈으로 단락 구분"
}
```

**줄바꿈 처리 규칙:**
- `\n` = 새로운 단락 생성 (각 줄이 하나의 단락)
- `\n\n` = 빈 단락 포함 (단락 사이에 빈 줄 추가)

**특수문자 처리:**
- `<`, `>`, `&` 등 특수문자는 그대로 HwpxWriter에 전달한다.
- HwpxWriter가 내부적으로 XML 이스케이프를 처리한다고 가정한다.

#### type: "image" (파일 경로)

```json
{
  "type": "image",
  "url": "./test_img.png"
}
```

**경로 해석:**
- 상대 경로는 JSON 파일 위치를 기준으로 해석한다.
- 외부 URL(`http://`, `https://`)도 지원한다 — 다운로드하여 사용.

#### type: "image" (Base64)

```json
{
  "type": "image",
  "base64": "iVBORw0KGgoAAAANSUhEUg...",
  "format": "png"
}
```

- `url` 또는 `base64` 중 하나를 사용
- `format`: base64 사용 시 이미지 형식 지정

#### 이미지 크기

- 원본 크기 그대로 삽입한다.
- 크기 조절은 향후 옵션으로 추가 가능.

#### 이미지 다운로드 정책

- 외부 URL 이미지 다운로드 실패 시 전체 변환을 중단한다 (에러 반환).
- 다운로드 타임아웃: 60초.
- URL 화이트리스트: 2차 개발에서 추가 고려.

#### 지원 이미지 포맷

| 포맷 | 확장자 | 처리 방식 |
|------|--------|-----------|
| PNG | `.png` | 그대로 삽입 |
| JPEG | `.jpg`, `.jpeg` | 그대로 삽입 |
| GIF | `.gif` | 첫 프레임을 정적 이미지로 삽입 |
| BMP | `.bmp` | 그대로 삽입 |
| WebP | `.webp` | PNG로 변환 후 삽입 |
| AVIF | `.avif` | PNG로 변환 후 삽입 |

- **HEIC 미지원**: libheif C 라이브러리 의존성을 피하기 위해 제외.
- 변환에는 `image` 크레이트 사용.
- 이미지 포맷은 파일 확장자 + 매직 바이트로 판별.
- GIF 애니메이션의 경우 첫 프레임만 사용.

#### type: "table" (HTML 형식)

```json
{
  "type": "table",
  "value": "<table><thead><tr><th>헤더1</th><th>헤더2</th></tr></thead><tbody><tr><td>값1</td><td>값2</td></tr></tbody></table>"
}
```

**처리 규칙:**
- 표준 HTML `<table>` 태그 구조만 파싱한다.
- `<thead>`, `<tbody>` 구분 지원.
- `<th>` 태그는 굵은 글씨로 처리.
- 인라인 스타일은 무시한다 (구조만 추출).
- `colspan`/`rowspan` 셀 병합: HwpxWriter가 지원하면 반영, 미지원 시 일반 셀로 fallback.
- 테이블 셀 내부에는 텍스트만 존재한다고 가정 (이미지/중첩 테이블 없음).

---

## 4. 1차 개발: 핵심 변환 기능 + CLI

### 4.1 목표
- JSON 파일을 읽어 HWPX 파일로 변환하는 CLI 도구
- 라이브러리로도 사용 가능

### 4.2 출력 파일명

- 출력 파일명은 `{atclId}.hwpx`로 자동 생성한다.
- CLI에서 `-o` 옵션으로 명시적 지정도 가능.
- 명시적 지정 시 지정된 경로를 우선 사용.

### 4.3 CLI 인터페이스

```bash
# 기본 사용 (출력: {atclId}.hwpx)
jsontohwpx input.json

# 출력 파일 명시 지정
jsontohwpx input.json -o output.hwpx

# 표준 입력에서 읽기
cat input.json | jsontohwpx -o output.hwpx

# 검증만 수행 (변환하지 않음)
jsontohwpx --validate input.json

# includeHeader 옵션 강제 활성화
jsontohwpx input.json --include-header

# JSON 형식 에러 출력
jsontohwpx input.json --json
```

### 4.4 CLI 에러 출력

**기본 모드 (텍스트):**
- stderr에 사람이 읽기 쉬운 형식으로 출력
- 예: `Error: 이미지 다운로드 실패 (url: https://...)`

**JSON 모드 (`--json` 플래그):**
- stderr에 JSON 형식으로 에러 출력
- 자동화 스크립트 연동에 유용
```json
{
  "error": {
    "code": "IMAGE_DOWNLOAD_FAILED",
    "message": "이미지 다운로드 실패",
    "details": { "url": "https://..." }
  }
}
```

### 4.5 CLI 종료 코드

| 코드 | 의미 |
|------|------|
| 0 | 성공 |
| 1 | 입력 에러 (잘못된 JSON, 파일 미존재, responseCode 비정상) |
| 2 | 변환 에러 (이미지 다운로드 실패, 포맷 변환 실패) |
| 3 | IO 에러 (파일 쓰기 실패) |

### 4.6 CLI 진행 표시

간단한 단계별 로그 메시지를 stderr에 출력:
```
[1/5] JSON 파싱 중...
[2/5] 이미지 다운로드 중... (2개)
[3/5] 이미지 포맷 변환 중...
[4/5] HWPX 변환 중...
[5/5] 파일 저장: BA000000000000000000006.hwpx
완료!
```

### 4.7 용지 설정

- A4 고정 (210mm × 297mm)
- 기본 여백 사용 (HwpxWriter 기본값)
- 폰트: HwpxWriter 기본 폰트 사용

### 4.8 프로젝트 구조

hwpers 레포를 clone한 후 아래 파일들을 추가한다:

```
hwpers/                         # clone한 레포 기반
├── Cargo.toml                  # [[bin]] 타겟 추가
├── src/
│   ├── lib.rs                  # (기존) hwpers 라이브러리
│   ├── hwpx/                   # (기존) HwpxWriter, HwpxReader 등
│   ├── bin/
│   │   ├── jsontohwpx.rs      # [추가] CLI 진입점
│   │   └── jsontohwpx_api.rs  # [추가] API 서버 진입점 (2차)
│   └── jsontohwpx/            # [추가] 변환 모듈
│       ├── mod.rs
│       ├── model.rs           # JSON 입력 모델 (article, contents, options)
│       ├── converter.rs       # JSON → HwpxWriter 호출 (메인 변환 로직)
│       ├── text.rs            # 텍스트 변환 (\n 파싱, 단락 생성)
│       ├── image.rs           # 이미지 로드/포맷변환/다운로드
│       ├── table.rs           # HTML 테이블 파싱 (scraper)
│       ├── error.rs           # jsontohwpx 에러 타입
│       └── api/               # [추가] REST API 모듈 (2차)
│           ├── mod.rs
│           ├── routes.rs
│           ├── handlers.rs
│           ├── queue.rs       # 비동기 작업 큐
│           └── jobs.rs        # 작업 상태 관리
├── tests/
│   ├── jsontohwpx_test.rs             # 변환 통합 테스트
│   ├── jsontohwpx_image_test.rs       # 이미지 포맷 테스트
│   └── jsontohwpx_table_test.rs       # 테이블 변환 테스트
├── examples/
│   └── jsontohwpx/
│       ├── simple_text.json
│       ├── with_image.json
│       ├── with_image_base64.json
│       ├── image_png.json / image_jpg.json / ...
│       ├── image_multi_format.json
│       ├── with_table.json
│       ├── table_merge.json
│       ├── with_metadata_header.json
│       ├── full_document.json
│       └── test_img.{png,jpg,jpeg,gif,avif,webp}
└── docs/
    └── spec.md
```

### 4.9 의존성 (Cargo.toml 추가분)

```toml
# 기존 hwpers 의존성 유지 (serde, zip, quick-xml 등 이미 포함)

# [추가] jsontohwpx에 필요한 의존성
[dependencies]
clap = { version = "4", features = ["derive"] }
scraper = "0.21"            # HTML 테이블 파싱
base64 = "0.22"             # Base64 이미지 디코딩
image = "0.25"              # 이미지 포맷 변환 (WebP, AVIF → PNG)
reqwest = { version = "0.12", features = ["blocking"] }  # 이미지 URL 다운로드

# [추가] 바이너리 타겟
[[bin]]
name = "jsontohwpx"
path = "src/bin/jsontohwpx.rs"
```

> **참고**: `serde`, `serde_json`, `thiserror`, `anyhow`, `zip`, `quick-xml` 등은
> hwpers에 이미 포함되어 있으므로 추가 불필요.
> **HEIC 제외**: `libheif-rs` 의존성 불필요.

### 4.10 테스트 전략

#### 검증 방법
- **HwpxReader 기반**: 생성된 HWPX를 `HwpxReader`로 읽어 텍스트/구조 검증
- 자동화된 단위/통합 테스트 작성
- 한글 오피스에서 열어 확인하는 수동 검증은 별도 단계

#### examples 파일 활용

| 파일명 | 설명 | 검증 포인트 |
|--------|------|-------------|
| `simple_text.json` | 텍스트만 포함 | 줄바꿈, 단락 분리, 메타데이터 |
| `with_image.json` | PNG 이미지 경로 | 이미지 삽입, contents 순서 |
| `with_image_base64.json` | Base64 이미지 | Base64 디코딩, 이미지 삽입 |
| `image_*.json` | 각 포맷별 이미지 | 포맷별 처리 검증 |
| `image_multi_format.json` | 여러 포맷 혼합 | 다양한 포맷 동시 처리 |
| `with_table.json` | HTML 테이블 | HTML 파싱, HWPX 표 생성 |
| `table_merge.json` | colspan/rowspan | 셀 병합 처리 |
| `with_metadata_header.json` | includeHeader | 본문 상단 메타데이터 텍스트 |
| `full_document.json` | 종합 문서 | 전체 기능 통합 검증 |

---

## 5. 2차 개발: REST API

### 5.1 목표
- HTTP API로 JSON을 받아 HWPX 파일을 반환
- 동기(즉시 응답) + 비동기(작업 큐) 모두 지원
- 인증 없음 (내부 서비스 용도)

### 5.2 기술 스택
- 웹 프레임워크: `axum`
- 비동기 런타임: `tokio`
- 로깅: `tracing` + JSON 구조화 출력
- HTTP 클라이언트: `reqwest` (이미지 다운로드용)

### 5.3 API 설계

#### POST /api/v1/convert (동기)
JSON을 HWPX로 변환하여 바이너리 직접 반환 (변환 완료까지 대기)

**Request:**
```http
POST /api/v1/convert
Content-Type: application/json

{
  "responseCode": "0",
  "responseText": "SUCCESS",
  "options": { ... },
  "data": { "article": { ... } }
}
```

**Response (성공):**
```http
HTTP/1.1 200 OK
Content-Type: application/vnd.hancom.hwpx
Content-Disposition: attachment; filename="{atclId}.hwpx"

<binary hwpx data>
```

#### POST /api/v1/convert/async (비동기)
변환 작업을 큐에 등록하고 즉시 작업 ID 반환

**Request:** 동기와 동일

**Response:**
```json
{
  "jobId": "uuid-string",
  "status": "queued",
  "createdAt": "2025-01-24T09:00:00Z"
}
```

#### GET /api/v1/jobs/{id} (작업 상태 확인)

**Response:**
```json
{
  "jobId": "uuid-string",
  "status": "completed",
  "createdAt": "2025-01-24T09:00:00Z",
  "completedAt": "2025-01-24T09:00:02Z",
  "downloadUrl": "/api/v1/jobs/{id}/download"
}
```

**status 값:** `queued` | `processing` | `completed` | `failed`

#### GET /api/v1/jobs/{id}/download (파일 다운로드)

**Response:**
```http
HTTP/1.1 200 OK
Content-Type: application/vnd.hancom.hwpx
Content-Disposition: attachment; filename="{atclId}.hwpx"

<binary hwpx data>
```

- 완료된 작업의 파일만 다운로드 가능
- 미완료/실패 작업은 404 또는 상태 메시지 반환

#### POST /api/v1/validate
JSON 입력의 유효성만 검증

**Response:**
```json
{
  "valid": true,
  "errors": []
}
```

#### GET /api/v1/health
서버 상태 상세 정보 반환

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "queue": {
    "pending": 3,
    "processing": 2,
    "completed": 150,
    "failed": 1
  },
  "workers": {
    "active": 2,
    "max": 4
  },
  "uptime_seconds": 86400
}
```

### 5.4 비동기 작업 관리

#### 큐 구현
- 인메모리 큐 사용 (별도 Redis 불필요)
- `tokio::sync::mpsc` 채널 기반
- 워커 수: 환경변수로 설정 가능 (기본값: CPU 코어 수)

#### 파일 만료 정책
- 생성된 HWPX 파일은 24시간 후 자동 삭제
- 백그라운드 태스크로 주기적 정리
- 임시 파일 저장 위치: `/tmp/jsontohwpx/`

#### Graceful Shutdown
- SIGTERM 수신 시 새 요청 거부
- 진행 중인 변환 작업은 30초간 완료 대기
- 30초 초과 시 강제 종료

### 5.5 요청 제한

| 설정 | 환경변수 | 기본값 |
|------|----------|--------|
| 최대 요청 크기 | `MAX_REQUEST_SIZE` | 50MB |
| 이미지 다운로드 타임아웃 | `IMAGE_DOWNLOAD_TIMEOUT` | 60초 |
| 워커 수 | `WORKER_COUNT` | CPU 코어 수 |
| 파일 만료 시간 | `FILE_EXPIRY_HOURS` | 24시간 |

### 5.6 로깅

- `tracing` 크레이트 기반
- JSON 구조화 로그 형식 (ELK 등 로그 수집 시스템 연동 용이)
- 로그 레벨: 환경변수 `RUST_LOG`로 제어
- 요청별 span 생성 (request_id, method, path)

### 5.7 에러 응답 형식

```json
{
  "error": {
    "code": "INVALID_JSON",
    "message": "입력 JSON 형식이 올바르지 않습니다",
    "details": [
      {
        "path": "$.data.article.contents[0]",
        "message": "type 필드가 누락되었습니다"
      }
    ]
  }
}
```

### 5.8 추가 의존성

```toml
[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.5", features = ["cors", "trace", "limit"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json"] }
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }

[[bin]]
name = "jsontohwpx-api"
path = "src/bin/jsontohwpx_api.rs"
```

### 5.9 2차에서 추가 고려 사항

- URL 화이트리스트 (이미지 다운로드 허용 도메인 제한)
- 배치 처리 (여러 JSON을 한번에 변환)

---

## 6. 3차 개발: Docker 컨테이너화

### 6.1 목표
- 멀티 스테이지 빌드로 경량 이미지 생성
- docker-compose로 단독 서비스 구성
- 임시 파일은 컨테이너 `/tmp` 사용

### 6.2 Dockerfile

```dockerfile
# 빌드 스테이지
FROM rust:1.75 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN cargo build --release --bin jsontohwpx-api

# 실행 스테이지
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/jsontohwpx-api /usr/local/bin/
RUN mkdir -p /tmp/jsontohwpx
EXPOSE 8080
CMD ["jsontohwpx-api"]
```

### 6.3 docker-compose.yml

```yaml
version: '3.8'

services:
  api:
    build: .
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info
      - HOST=0.0.0.0
      - PORT=8080
      - MAX_REQUEST_SIZE=52428800      # 50MB
      - WORKER_COUNT=4
      - FILE_EXPIRY_HOURS=24
      - IMAGE_DOWNLOAD_TIMEOUT=60
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/api/v1/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    restart: unless-stopped
    deploy:
      resources:
        limits:
          memory: 1G
          cpus: '2.0'
    tmpfs:
      - /tmp/jsontohwpx:size=512M
```

### 6.4 환경 변수 종합

| 변수명 | 기본값 | 설명 |
|--------|--------|------|
| `HOST` | `0.0.0.0` | 서버 바인딩 주소 |
| `PORT` | `8080` | 서버 포트 |
| `RUST_LOG` | `info` | 로그 레벨 |
| `MAX_REQUEST_SIZE` | `52428800` | 최대 요청 크기 (50MB, 바이트) |
| `WORKER_COUNT` | CPU 코어 수 | 동시 변환 워커 수 |
| `FILE_EXPIRY_HOURS` | `24` | 생성 파일 만료 시간 |
| `IMAGE_DOWNLOAD_TIMEOUT` | `60` | 이미지 다운로드 타임아웃 (초) |

---

## 7. 개발 일정 (마일스톤)

### M1: 프로젝트 초기화
- HC-ProductTech/hwpers 레포 clone
- jsontohwpx 모듈 및 바이너리 타겟 추가
- 추가 의존성(clap, scraper, base64, image, reqwest) 추가
- 빌드 확인

### M2: JSON 모델 정의
- 입력 JSON 스키마 정의 (article, contents, options)
- serde 역직렬화 모델 구현
- 입력 검증 로직 (responseCode 검사 포함)
- 에러 타입 정의 (error.rs)

### M3: HwpxWriter 연동 기본 변환
- converter 모듈에서 `crate::hwpx::HwpxWriter` 호출
- JSON → 빈 문서 생성 파이프라인 확인
- 한글 오피스에서 열리는지 확인

### M4: 텍스트 변환
- type: "text" → 단락 변환
- `\n` → 새 단락, `\n\n` → 빈 단락 포함
- 연속 text 요소 사이 빈 단락 추가
- 메타데이터 → 문서 속성 저장
- includeHeader 옵션 처리

### M5: 이미지 삽입
- 파일 경로 방식 이미지 삽입 (JSON 파일 기준 상대 경로)
- 외부 URL 방식 이미지 다운로드 + 삽입
- Base64 방식 이미지 삽입
- 네이티브 포맷 지원 (PNG, JPG/JPEG, GIF 첫 프레임, BMP)
- 비네이티브 포맷 변환 (WebP, AVIF → PNG)
- 이미지 포맷 자동 감지 (확장자 + 매직 바이트)
- 다운로드 실패 시 전체 변환 중단

### M6: 표 변환
- HTML `<table>` 파싱 (scraper 크레이트)
- HWPX 표 생성 (`HwpxTable::from_data()`)
- colspan/rowspan: HwpxWriter 지원 확인 후 반영 또는 fallback
- 인라인 스타일 무시 (구조만 추출)

### M7: CLI 완성
- clap 기반 CLI 구현
- 출력 파일명 자동 생성 ({atclId}.hwpx)
- 에러 출력 (텍스트 기본 + --json 옵션)
- 종료 코드 (0/1/2/3)
- 진행 로그 출력
- 1차 개발 완료

### M8: REST API (동기)
- axum 서버 구성
- POST /api/v1/convert (동기, 바이너리 응답)
- POST /api/v1/validate
- GET /api/v1/health (상세 정보)
- 에러 핸들링 (JSON 에러 응답)
- 요청 크기 제한
- tracing + JSON 로깅

### M9: REST API (비동기)
- 인메모리 작업 큐 구현
- POST /api/v1/convert/async
- GET /api/v1/jobs/{id}
- GET /api/v1/jobs/{id}/download
- 워커 풀 관리
- 파일 만료/정리 태스크
- Graceful shutdown (30초 타임아웃)

### M10: 컨테이너화
- Dockerfile 작성 (멀티 스테이지)
- docker-compose 구성
- 환경 변수 설정
- 헬스 체크 구현
- 배포 준비 완료

---

## 8. 품질 기준

### 8.1 코드 품질
- `cargo clippy -- -D warnings` 통과
- `cargo fmt --check` 통과
- 테스트 커버리지 80% 이상

### 8.2 성능 목표
- 기본 문서 (10페이지 이하) 변환: 100ms 이내
- 대용량 문서 (100페이지) 변환: 1초 이내
- API 동기 응답 시간 (기본 문서, 이미지 다운로드 제외): 200ms 이내

### 8.3 안정성
- 잘못된 JSON 입력에 대한 명확한 에러 메시지
- 메모리 안전성 (unsafe 최소화)
- 대용량 입력에 대한 크기 제한 (설정 가능)
- Graceful shutdown 지원

---

## 9. 기술 결정 요약

| 항목 | 결정 | 비고 |
|------|------|------|
| responseCode | "0" 아니면 에러 | 변환 중단 |
| 줄바꿈 | `\n` = 새 단락, `\n\n` = 빈 단락 포함 | |
| 이미지 경로 기준 | JSON 파일 위치 | CLI 기준 |
| 테이블 스타일 | 구조만 (인라인 무시) | |
| 본문 소스 | contents 배열만 | atclCn 무시 |
| 폰트 | HwpxWriter 기본값 | |
| 출력 파일명 | {atclId}.hwpx | -o로 오버라이드 가능 |
| 이미지 크기 | 원본 그대로 | 향후 옵션 추가 가능 |
| 외부 URL | 다운로드 지원 | 타임아웃 60초 |
| HEIC | 미지원 | libheif 의존성 제외 |
| GIF | 첫 프레임 | 정적 이미지로 삽입 |
| 다운로드 실패 | 전체 변환 중단 | 에러 반환 |
| 빈 contents | 경고 후 빈 문서 생성 | |
| 연속 text | 사이에 빈 단락 | |
| 특수문자 | 그대로 전달 | HwpxWriter 자동 처리 가정 |
| 용지 | A4 고정 | |
| API 인증 | 없음 | 내부 서비스 |
| API 방식 | 동기 + 비동기 모두 | |
| API 응답 (동기) | 바이너리 직접 | |
| API 응답 (비동기) | 다운로드 URL | 24시간 후 만료 |
| 큐 | 인메모리 (tokio channel) | |
| 워커 수 | 설정 가능 (기본: CPU 코어) | |
| 요청 크기 | 설정 가능 (기본: 50MB) | |
| 로깅 | tracing + JSON | |
| Docker | API 단독 서비스 | Redis 불필요 |
| Shutdown | 30초 타임아웃 후 강제 | |
| CLI 에러 | 텍스트 기본 + --json | |
| Exit code | 0/1/2/3 | 입력/변환/IO 에러 구분 |
| 진행 표시 | 단계별 로그 | stderr |
| 테스트 검증 | HwpxReader | 자동화 테스트 |
| colspan/rowspan | HwpxWriter 지원 시 반영 | fallback: 일반 셀 |
| Health check | 상세 정보 | 큐/워커/메모리 |
| URL 화이트리스트 | 2차에서 고려 | |
| 배치 처리 | 2차에서 고려 | |
| 업스트림 동기화 | 없음 | 독립 fork |
