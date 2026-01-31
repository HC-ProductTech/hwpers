# jsontohwpx - JSON to HWPX 변환기

## 프로젝트 개요

JSON 데이터를 HWPX (한글 오피스 XML) 문서로 변환하는 Rust 라이브러리 및 REST API 서비스.

### 프로젝트 기반
- [HC-ProductTech/hwpers](https://github.com/HC-ProductTech/hwpers)를 clone하여 개발
- 같은 크레이트 내에서 `crate::hwpx::HwpxWriter` 직접 사용
- jsontohwpx는 hwpers에 추가되는 바이너리 타겟 + 모듈

## 개발 명령어

```bash
# 빌드
cargo build
cargo build --release

# 테스트
cargo test                                    # 전체 테스트
cargo test jsontohwpx                         # jsontohwpx 관련 테스트만
cargo test -- --nocapture                     # 출력 포함

# 코드 품질
cargo clippy -- -D warnings
cargo fmt
cargo fmt --check

# jsontohwpx CLI 실행
cargo run --bin jsontohwpx -- input.json              # 출력: {article_id}.hwpx
cargo run --bin jsontohwpx -- input.json -o out.hwpx  # 출력 지정
cargo run --bin jsontohwpx -- --validate input.json   # 검증만
cargo run --bin jsontohwpx -- input.json --json       # JSON 에러 출력

# API 서버 (2차 개발)
cargo run --bin jsontohwpx-api
# 동기: POST /api/v1/convert
# 비동기: POST /api/v1/convert/async → GET /api/v1/jobs/{id}/download
```

## 아키텍처

hwpers 레포를 clone한 후 아래 파일들을 추가:

```
hwpers/                          # clone 기반
├── src/
│   ├── lib.rs                   # (기존) hwpers 라이브러리
│   ├── hwpx/                    # (기존) HwpxWriter, HwpxReader
│   ├── bin/
│   │   └── jsontohwpx.rs       # [추가] CLI 진입점
│   └── jsontohwpx/             # [추가] 변환 모듈
│       ├── mod.rs
│       ├── model.rs            # JSON 입력 모델 (ArticleDocument, Metadata, ConvertOptions)
│       ├── converter.rs        # 메인 변환 로직
│       ├── text.rs             # 텍스트 → add_paragraph
│       ├── image.rs            # 이미지 로드/포맷변환 → add_image
│       ├── table.rs            # HTML 파싱 → HwpxTable → add_table
│       ├── error.rs            # 에러 타입
│       └── api/                # REST API (2차 개발)
│           ├── mod.rs
│           ├── routes.rs
│           ├── handlers.rs
│           ├── queue.rs       # 인메모리 작업 큐
│           └── jobs.rs        # 작업 상태 관리
├── tests/
│   └── jsontohwpx_*.rs         # [추가] 변환 테스트
└── examples/jsontohwpx/         # [추가] JSON 예시 + 테스트 이미지
```

> **핵심**: `use crate::hwpx::{HwpxWriter, HwpxReader, ...}`로 내부 모듈 직접 접근.
> HWPX 생성은 HwpxWriter가 처리, 본 모듈은 JSON 파싱 → API 호출만 담당.

## JSON 입력 형식

```json
{
  "schema_version": "1.0",
  "article_id": "고유ID",
  "title": "문서 제목",
  "metadata": {
    "author": "작성자",
    "department": "부서",
    "created_at": "2025-01-30T10:00:00+09:00",
    "board_name": "게시판명",
    "expiry": "보존기간"
  },
  "contents": [
    { "type": "text", "value": "텍스트" },
    { "type": "image", "url": "./path.png" },
    { "type": "image", "base64": "...", "format": "png" },
    { "type": "table", "value": "<table>...</table>" }
  ],
  "content_html": "<html>...</html>",
  "attachments": []
}
```

- `article_id` 필수, 비어있으면 에러 처리
- `contents` 배열 순서대로 문서에 삽입
- `content_html`, `attachments` 필드는 파싱되지만 HWPX 변환에 미사용
- `--include-header` CLI 옵션: true면 메타데이터를 본문 상단에 텍스트로 삽입
- `type: "text"`: `\n` = 새 단락, `\n\n` = 빈 단락 포함, 연속 text 요소 사이 빈 단락
- `type: "table"`: HTML `<table>` 구조만 파싱 (인라인 스타일 무시)
- `type: "image"`: 경로는 JSON 파일 기준, 외부 URL 다운로드 지원
- 출력 파일명: `{article_id}.hwpx` 자동 생성 (-o로 오버라이드)

## 이미지 포맷 지원

| 포맷 | HWPX 네이티브 | 처리 |
|------|---------------|------|
| PNG, JPG/JPEG, BMP | O | 그대로 삽입 |
| GIF | O | 첫 프레임을 정적 이미지로 삽입 |
| WebP, AVIF | X | PNG 변환 후 삽입 (`image` 크레이트) |

- **HEIC 미지원** (libheif C 라이브러리 의존성 제외)
- 포맷 감지: 확장자 우선, 확장자 없으면 매직 바이트로 판별
- 이미지 크기: 원본 그대로 삽입
- 외부 URL: 다운로드 지원 (타임아웃 60초, 실패 시 전체 변환 중단)

## 변환 흐름

```
JSON 입력 → 파싱(serde) → contents 순회 → HwpxWriter 호출 → .hwpx 출력
                                              ↓
                              text  → add_paragraph()
                              image → add_image() / add_image_from_file()
                              table → HwpxTable::new() + add_table()
```

## 개발 원칙

### TDD 필수
1. **Red**: 실패하는 테스트 먼저 작성
2. **Green**: 최소 코드로 통과
3. **Refactor**: 코드 품질 개선

### Tidy First
- 구조적 변경과 기능적 변경을 분리
- 리팩토링 커밋과 기능 커밋 분리

### 커밋 워크플로우
`/plan` → 구현 (TDD) → `/review` → `/verify` → `/commit-push-pr`

## 테스트 전략

### examples 기반 테스트
`examples/jsontohwpx/` 폴더에 JSON 예시 파일을 두고, 이를 변환하여 결과 검증:
- **HwpxReader 기반 검증**: 생성된 HWPX를 HwpxReader로 읽어 텍스트/구조 확인
- XML 내용 파싱하여 기대값과 비교
- 한글 오피스에서 열리는지 수동 확인 (별도 단계)

### 단위 테스트
각 모듈에 `#[cfg(test)]` 블록으로 단위 테스트 작성.

### 통합 테스트
`tests/` 디렉토리에 엔드투엔드 변환 테스트 배치.

## 코드 품질 기준

- `cargo clippy` 경고 0건
- `cargo fmt --check` 통과
- 테스트 커버리지 80% 이상
- `unwrap()` 사용 금지 (테스트 코드 제외)
- 모든 public API에 문서 주석 (`///`)
