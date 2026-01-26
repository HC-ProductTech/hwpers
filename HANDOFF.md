# jsontohwpx 프로젝트

## 프로젝트 목적

**JSON API 응답을 HWPX(한글 문서)로 변환**하는 도구입니다.

게시판 시스템 등에서 내려오는 JSON 데이터를 한글 문서로 변환하여 저장/배포할 수 있게 합니다.

---

## 주요 변환 대상

### 1. 메타데이터 (헤더)

문서 상단에 작성자 정보를 포함할 수 있습니다.

| 필드 | 설명 |
|------|------|
| `subject` | 문서 제목 |
| `regEmpName` | 작성자 |
| `regDeptName` | 부서 |
| `regDt` | 작성일시 |

```json
{
  "options": {
    "includeHeader": true,
    "headerFields": ["subject", "regEmpName", "regDeptName", "regDt"]
  },
  "data": {
    "article": {
      "atclId": "DOC001",
      "subject": "문서 제목",
      "regEmpName": "홍길동",
      "regDeptName": "개발팀",
      "regDt": "2025-01-24 10:00:00",
      "contents": [...]
    }
  }
}
```

### 2. 콘텐츠 타입

JSON의 `contents` 배열에 포함되는 콘텐츠 타입들:

| 타입 | 설명 | 예시 |
|------|------|------|
| **text** | 일반 텍스트 | 본문, 줄바꿈(`\n`) 지원 |
| **table** | HTML 테이블 | `<table>`, colspan/rowspan 지원 |
| **image** | 이미지 | 파일 경로, HTTP URL, Base64 |

```json
{
  "contents": [
    { "type": "text", "value": "제목입니다\n\n본문 내용..." },
    { "type": "table", "value": "<table><tr><td>A</td><td>B</td></tr></table>" },
    { "type": "image", "url": "https://example.com/image.png" }
  ]
}
```

---

## 기반 오픈소스

### hwpers ([GitHub](https://github.com/HC-ProductTech/hwpers))

Rust로 작성된 한글 문서(HWP/HWPX) 라이브러리입니다.

**hwpers가 제공하는 기능:**

| 기능 | 설명 |
|------|------|
| HwpReader | HWP 파일 읽기/파싱 |
| HwpxReader | HWPX 파일 읽기/파싱 |
| HwpxWriter | HWPX 파일 생성 |
| 텍스트 | 서식 있는 텍스트 (볼드, 이탤릭, 색상 등) |
| 테이블 | 테이블 생성, 셀 병합 |
| 이미지 | PNG, JPEG, BMP, GIF 삽입 |
| 페이지 레이아웃 | 용지 크기, 여백, 방향 설정 |

---

## 추가 개발 내용

hwpers는 저수준 API만 제공하므로, **JSON → HWPX 변환**을 위한 고수준 모듈을 추가 개발했습니다.

### 1. 변환 모듈 (`src/jsontohwpx/`)

| 파일 | 역할 |
|------|------|
| `model.rs` | JSON 입력 구조 정의 (serde) |
| `converter.rs` | 메인 변환 로직 (contents 순회 → HwpxWriter 호출) |
| `text.rs` | 텍스트 처리 (줄바꿈 → 단락 분리) |
| `table.rs` | **HTML 테이블 파싱** → HWPX 테이블 변환 |
| `image.rs` | 이미지 로드 + **포맷 변환** (WebP/AVIF → PNG) |
| `error.rs` | 에러 타입 정의 |

#### 주요 추가 기능

**메타데이터 헤더** (`model.rs`, `converter.rs`)
- `includeHeader` 옵션으로 문서 상단에 작성자 정보 삽입
- `headerFields`로 포함할 필드 선택 (제목, 작성자, 부서, 작성일)

**HTML 테이블 파싱** (`table.rs`)
- `<table>`, `<tr>`, `<td>`, `<th>` 태그 파싱
- `colspan`, `rowspan` 속성 지원
- hwpers의 `HwpxTable` API로 변환

**이미지 포맷 변환** (`image.rs`)
- HWPX가 지원하지 않는 WebP, AVIF → PNG 자동 변환
- HTTP URL에서 이미지 다운로드
- Base64 인코딩 이미지 처리
- 매직 바이트로 포맷 자동 감지

### 2. CLI 도구 (`src/bin/jsontohwpx.rs`)

```bash
# 기본 사용
jsontohwpx input.json -o output.hwpx

# stdin에서 읽기
cat input.json | jsontohwpx - -o output.hwpx

# 검증만 (변환 없음)
jsontohwpx input.json --validate
```

### 3. REST API (`src/jsontohwpx/api/`)

| 엔드포인트 | 설명 |
|------------|------|
| `POST /api/v1/convert` | 동기 변환 (즉시 HWPX 반환) |
| `POST /api/v1/convert/async` | 비동기 변환 (작업 ID 반환) |
| `GET /api/v1/jobs/:id/download` | 비동기 결과 다운로드 |
| `POST /api/v1/validate` | JSON 검증만 |
| `GET /api/v1/health` | 서버 상태 확인 |

**API 서버 실행:**
```bash
cargo run --bin jsontohwpx-api
# http://localhost:8080/swagger-ui/
```

### 4. Docker화

```yaml
# docker-compose.yml
services:
  jsontohwpx:
    build: .
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info
      - WORKER_COUNT=4
```

```bash
# 실행
docker compose up -d

# API 호출
curl -X POST http://localhost:8080/api/v1/convert \
  -H "Content-Type: application/json" \
  -d @input.json \
  --output output.hwpx
```

---

## 프로젝트 구조

```
jsontohwpx/
├── src/
│   ├── hwpx/                  # [hwpers] HWPX Reader/Writer
│   ├── jsontohwpx/            # [추가] 변환 모듈
│   │   ├── model.rs           #   JSON 모델
│   │   ├── converter.rs       #   변환 로직
│   │   ├── text.rs            #   텍스트 처리
│   │   ├── table.rs           #   HTML 테이블 파싱
│   │   ├── image.rs           #   이미지 처리
│   │   └── api/               #   REST API
│   └── bin/
│       ├── jsontohwpx.rs      # [추가] CLI
│       └── jsontohwpx_api.rs  # [추가] API 서버
├── Dockerfile                 # [추가]
└── docker-compose.yml         # [추가]
```

---

## 요약

| 항목 | 내용 |
|------|------|
| **목적** | JSON → HWPX 변환 |
| **콘텐츠** | 메타데이터(작성자/부서/일시), text, table (HTML), image |
| **기반** | hwpers (Rust HWP 라이브러리) |
| **추가 개발** | 메타데이터 헤더, HTML 파싱, 이미지 포맷 변환, CLI, REST API |
| **배포** | Docker 컨테이너 |
