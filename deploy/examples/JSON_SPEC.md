# HWPX Converter 입력 JSON 스펙

## 한눈에 보기

```
article_id (필수) ─── 문서 고유 ID, 출력 파일명으로 사용
title             ─── 문서 제목
metadata          ─── 작성자, 부서, 날짜 등 메타 정보
contents[]        ─── 본문 (텍스트, 테이블, 이미지 순서대로 삽입)
```

---

## 최소 필수 JSON

변환에 필요한 최소 구조입니다. (`05_minimal.json` 참고)

```json
{
  "schema_version": "1.1",
  "article_id": "DOC001",
  "title": "문서 제목",
  "contents": [
    { "type": "text", "value": "본문 내용입니다." }
  ]
}
```

> `article_id`는 **필수**이며 비어있으면 에러가 발생합니다.

---

## 전체 구조

```json
{
  "schema_version": "1.1",
  "article_id": "문서ID (필수)",
  "title": "문서 제목",

  "metadata": {
    "author": "작성자",
    "created_at": "2025-01-30T10:00:00+09:00",
    "updated_at": "2025-01-30T10:00:00+09:00",
    "department": "부서",
    "board_id": "게시판ID",
    "board_name": "게시판명",
    "board_path": ["상위게시판", "하위게시판"],
    "board_depth": 2,
    "folder_id": "폴더ID",
    "expiry": "보존기간",
    "views": 0,
    "likes": 0,
    "comments": 0
  },

  "attachments": [
    {
      "file_id": "파일ID",
      "file_name": "파일명.pdf",
      "file_extension": "pdf",
      "file_size": 1024000,
      "file_size_formatted": "1000.00 KB",
      "file_url": "https://example.com/file.pdf"
    }
  ],
  "attachment_count": 1,
  "total_attachment_size": 1024000,

  "contents": [
    { "type": "text", "value": "텍스트 내용" },
    { "type": "table", "value": "<table>...</table>" },
    { "type": "image", "url": "https://example.com/img.png" },
    { "type": "image", "base64": "iVBOR...", "format": "png" }
  ],

  "content_html": "<p>원본 HTML</p>"
}
```

---

## 필드별 설명

### 최상위 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `schema_version` | string | - | 스키마 버전. 현재 `"1.1"` |
| `article_id` | string | **O** | 문서 고유 ID. 출력 파일명(`{article_id}.hwpx`)에 사용 |
| `title` | string | - | 문서 제목 |
| `metadata` | object | - | 메타데이터 (아래 참고) |
| `contents` | array | - | 본문 콘텐츠 배열. 배열 순서대로 문서에 삽입 |
| `attachments` | array | - | 첨부파일 정보 (HWPX 변환에 미사용, 데이터 보존용) |
| `attachment_count` | number | - | 첨부파일 수 (HWPX 변환에 미사용) |
| `total_attachment_size` | number | - | 첨부파일 총 크기 bytes (HWPX 변환에 미사용) |
| `content_html` | string | - | 원본 HTML (HWPX 변환에 미사용, 데이터 보존용) |

### metadata 필드

| 필드 | 타입 | 설명 |
|------|------|------|
| `author` | string | 작성자 이름 |
| `created_at` | string | 작성일시 (ISO 8601) |
| `updated_at` | string | 수정일시 (ISO 8601) |
| `department` | string | 부서명 |
| `board_id` | string | 게시판 ID |
| `board_name` | string | 게시판 이름 |
| `board_path` | string[] | 게시판 경로 배열 |
| `board_depth` | number | 게시판 깊이 |
| `folder_id` | string | 폴더 ID |
| `expiry` | string | 보존 기간 (예: `"영구"`, `"1년"`, `"3년"`) |
| `views` | number | 조회수 |
| `likes` | number | 좋아요 수 |
| `comments` | number | 댓글 수 |

> metadata의 모든 필드는 선택입니다.
> `include_header=true` 옵션 사용 시 metadata 값이 본문 상단에 삽입됩니다.

---

## contents 타입별 작성법

### 1. text (텍스트)

```json
{ "type": "text", "value": "첫째 줄\n둘째 줄\n\n빈 줄 뒤 셋째 줄" }
```

- `\n` → 줄바꿈
- `\n\n` → 빈 줄 포함 줄바꿈

### 2. table (테이블)

```json
{ "type": "table", "value": "<table><tr><th>헤더1</th><th>헤더2</th></tr><tr><td>값1</td><td>값2</td></tr></table>" }
```

- HTML `<table>` 태그를 문자열로 전달
- `<th>`, `<td>` 사용 가능
- `colspan`, `rowspan` 속성으로 셀 병합 지원

**셀 병합 예시:**
```html
<table>
  <tr>
    <th colspan="3">전체 헤더</th>
  </tr>
  <tr>
    <td rowspan="2">병합</td>
    <td>A</td>
    <td>B</td>
  </tr>
  <tr>
    <td>C</td>
    <td>D</td>
  </tr>
</table>
```

### 3. image (이미지) — URL 방식

```json
{ "type": "image", "url": "https://example.com/photo.png" }
```

### 4. image (이미지) — Base64 방식

```json
{ "type": "image", "base64": "iVBORw0KGgo...", "format": "png" }
```

**지원 이미지 포맷:** PNG, JPEG, GIF, BMP, WebP, AVIF

---

## 예시 파일 목록

| 파일 | 설명 | 주요 포인트 |
|------|------|-------------|
| `01_simple_text.json` | 텍스트만 있는 공지사항 | 기본 구조, 줄바꿈 |
| `02_with_table.json` | 테이블 + 텍스트 + 첨부파일 정보 | 테이블 작성법, attachments |
| `03_complex_table.json` | colspan/rowspan 셀 병합 테이블 | 복잡한 테이블 병합 |
| `04_mixed_content.json` | 텍스트 + 테이블 혼합 + 첨부파일 2개 | 복합 콘텐츠, 다중 첨부 |
| `05_minimal.json` | 최소 필수 필드만 | 가장 간단한 형태 |

---

## 빠른 테스트

```bash
# 예시 파일로 변환 테스트
curl -X POST http://localhost:9040/api/v1/convert \
  -H "Content-Type: application/json" \
  -d @01_simple_text.json \
  --output test.hwpx

# JSON 검증만 (변환 없음)
curl -X POST http://localhost:9040/api/v1/validate \
  -H "Content-Type: application/json" \
  -d @01_simple_text.json
```
