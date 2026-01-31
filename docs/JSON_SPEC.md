# 게시판 게시글 JSON 구조화 스펙

> **Version**: 1.0  
> **Date**: 2026-01-31  
> **Purpose**: 그룹웨어 게시판 API 응답을 LLM/RAG 처리에 최적화된 JSON 구조로 변환하기 위한 스펙

---

## 1. 개요

그룹웨어 게시판 API의 원본 응답 데이터를 LLM이 효율적으로 소비할 수 있는 구조화된 JSON으로 변환한다. 불필요한 필드를 제거하고, 날짜·URL 등을 표준 형식으로 정규화하며, 본문 콘텐츠를 타입별로 분리하여 처리 효율을 높인다.

---

## 2. 전체 구조

```json
{
  "schema_version": "1.0",
  "article_id": "string",
  "title": "string",
  "metadata": { ... },
  "attachments": [ ... ],
  "attachment_count": 0,
  "total_attachment_size": 0,
  "contents": [ ... ],
  "content_html": "string"
}
```

---

## 3. 필드 정의

### 3.1 최상위 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `schema_version` | string | O | 스키마 버전. 하위 호환성 관리용. 현재 `"1.0"` |
| `article_id` | string | O | 게시글 고유 ID. 원본 API의 `articleId` 매핑 |
| `title` | string | O | 게시글 제목 |
| `metadata` | object | O | 작성자, 날짜, 부서 등 메타 정보 |
| `attachments` | array | O | 첨부파일 목록. 없으면 빈 배열 `[]` |
| `attachment_count` | number | O | 첨부파일 개수 |
| `total_attachment_size` | number | O | 첨부파일 총 용량 (bytes) |
| `contents` | array | O | 구조화된 본문 콘텐츠 배열 |
| `content_html` | string | O | 원본 HTML 보존 (원본 복원용) |

### 3.2 metadata

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `author` | string | O | 작성자. `regEmpName` 사용 (예: `"이중호"`) |
| `created_at` | string | O | 작성일시. ISO 8601 형식 |
| `updated_at` | string | O | 수정일시. ISO 8601 형식 |
| `department` | string | O | 작성 부서명. 원본 `regDeptName` 매핑 |
| `board_id` | string | O | 게시판 ID. 원본 `board.brdId` 매핑 |
| `board_name` | string | O | 게시판 이름. 원본 `board.brdName` 매핑 |
| `folder_id` | string | O | 게시판 폴더 ID. 원본 `board.fldId` 매핑 |
| `expiry` | string | O | 게시 만료일. `"99991231"` → `"영구"` 변환 |
| `views` | number | O | 조회수 |
| `likes` | number | O | 좋아요 수 |
| `comments` | number | O | 댓글 수 |

### 3.3 attachments (배열 요소)

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `file_id` | string | O | 파일 고유 ID |
| `file_name` | string | O | 파일명 (확장자 포함) |
| `file_extension` | string | O | 확장자 (소문자, 예: `"pdf"`) |
| `file_size` | number | O | 파일 크기 (bytes) |
| `file_size_formatted` | string | O | 읽기 쉬운 파일 크기 (예: `"2.79 MB"`) |
| `file_url` | string | O | 파일 다운로드 URL (**절대경로**) |

### 3.4 contents (배열 요소)

본문을 순서대로 타입별로 분리한 배열. 원본 HTML의 등장 순서를 그대로 보존한다.

#### 타입: `text`

```json
{
  "type": "text",
  "value": "본문 텍스트...\n줄바꿈 포함"
}
```

- 줄바꿈은 `\n`으로 표현
- 본문 내 링크는 마크다운 형식으로 포함: `[텍스트](URL)`
- 별도의 `link` 타입은 두지 않음

#### 타입: `image`

```json
{
  "type": "image",
  "url": "https://gw.hancom.com/ekp/service/file/fileView?module=board&..."
}
```

- URL은 **절대경로**로 변환

#### 타입: `table`

```json
{
  "type": "table",
  "value": "<table><tbody><tr><td>구분</td><td>일정</td></tr>...</tbody></table>"
}
```

- HTML 구조를 유지하되, **스타일 정보를 모두 제거**하여 경량화
- 아래 테이블 클린업 규칙 참조

---

## 4. 테이블 클린업 규칙

원본 테이블 HTML에서 의미 없는 스타일 정보를 제거하고, 구조와 텍스트만 보존한다.

### 4.1 제거 대상

| 대상 | 예시 |
|------|------|
| 모든 `style` 속성 | `style="width: 233px; border: 1px solid..."` |
| `<p>` 래퍼 태그 | `<p style="text-align: center;">텍스트</p>` → `텍스트` |
| `<span>` 래퍼 태그 | `<span style="font-family: ...">텍스트</span>` → `텍스트` |
| 인라인 스타일 속성 | `width`, `height`, `padding`, `background`, `vertical-align` 등 |
| 폰트 관련 속성 | `font-family`, `font-size`, `font-weight`, `letter-spacing` 등 |
| 레이아웃 속성 | `border-collapse`, `table-layout`, `text-align`, `word-break` 등 |

### 4.2 보존 대상

| 대상 | 이유 |
|------|------|
| `<table>`, `<tbody>`, `<tr>`, `<td>` | 테이블 구조 |
| `colspan`, `rowspan` 속성 | 병합 셀 표현 (존재하는 경우에만) |
| `<br>` | 셀 내 줄바꿈 (HTML 컨텍스트이므로 `\n`이 아닌 `<br>` 사용) |
| 실제 텍스트 콘텐츠 | 의미 있는 데이터 |

### 4.3 변환 규칙

1. `<p>` 태그 내 텍스트를 추출하고, 연속된 `<p>` 태그 사이는 `<br>`로 연결
2. `<span>` 태그를 제거하고 내부 텍스트만 유지
3. 모든 `style` 속성 삭제
4. `colspan`/`rowspan`이 없는 경우 해당 속성을 생략
5. 빈 셀은 `<td></td>`로 유지

### 4.4 변환 예시

**Before** (원본):
```html
<td style="width: 233px; height: 34px; padding: 1px 6px; border: 1px solid rgb(0,0,0); vertical-align: middle;">
  <p style="text-align: center; word-break: keep-all; line-height: 1.5;">
    <span style="font-family: 'Malgun Gothic'; font-size: 10pt;">목표등록 및 확정</span>
  </p>
  <p style="text-align: center; word-break: keep-all; line-height: 1.5;">
    <span style="font-family: 'Malgun Gothic'; font-size: 10pt;">동료평가자 선정</span>
  </p>
</td>
```

**After** (클린업):
```html
<td>목표등록 및 확정<br>동료평가자 선정</td>
```

---

## 5. 데이터 변환 규칙

### 5.1 날짜 형식

원본 포맷을 **ISO 8601** 형식으로 변환한다.

| 원본 | 변환 결과 |
|------|-----------|
| `"2025-10-22 PM 03:42:26"` | `"2025-10-22T15:42:26+09:00"` |

- 타임존은 KST (`+09:00`) 고정

### 5.2 파일 크기 포맷

`file_size` (bytes)를 `file_size_formatted`로 변환한다.

| 범위 | 단위 | 예시 |
|------|------|------|
| < 1,024 | B | `"512 B"` |
| < 1,048,576 | KB | `"45.2 KB"` |
| < 1,073,741,824 | MB | `"2.79 MB"` |
| ≥ 1,073,741,824 | GB | `"1.05 GB"` |

- 소수점 2자리까지 표시

### 5.3 만료일 매핑

| 원본 값 | 변환 결과 |
|---------|-----------|
| `"99991231"` | `"영구"` |
| 기타 날짜 | 해당 날짜 그대로 (예: `"2026-12-31"`) |

### 5.4 URL 처리

- 모든 URL(첨부파일, 이미지)은 **절대경로**로 변환
- 기본 도메인: `https://gw.hancom.com`

### 5.5 링크 처리

- 본문 내 `<a href="...">텍스트</a>` 링크는 별도 타입을 두지 않음
- `text` 타입의 `value` 안에 마크다운 형식으로 포함: `[텍스트](URL)`

---

## 6. 완성 예시

```json
{
  "schema_version": "1.0",
  "article_id": "BA38881508324312267670",
  "title": "2025년 인사평가 가이드 및 매뉴얼 공지드립니다. (10/27~)",
  "metadata": {
    "author": "이중호",
    "created_at": "2025-10-22T14:07:15+09:00",
    "updated_at": "2025-10-22T15:32:27+09:00",
    "department": "피플앤컬처팀",
    "board_id": "BB388815082858609858498",
    "board_name": "사내게시판",
    "folder_id": "BF388815082746214361498",
    "expiry": "영구",
    "views": 399,
    "likes": 0,
    "comments": 0
  },
  "attachments": [
    {
      "file_id": "BD388865310914458195726",
      "file_name": "첨부 1. 2025 평가 진행계획.pdf",
      "file_extension": "pdf",
      "file_size": 2929817,
      "file_size_formatted": "2.79 MB",
      "file_url": "https://gw.hancom.com/ekp/service/file/fileView?module=board&fileUrl=/C100171030/board/file/2025/10/22&fileName=288203c2-5da0-4ca4-9846-62b190c8c69d.pdf"
    },
    {
      "file_id": "BD388832376357315262606",
      "file_name": "첨부 2. 2025 인사평가 시스템 매뉴얼_팀원용.pdf",
      "file_extension": "pdf",
      "file_size": 2137848,
      "file_size_formatted": "2.04 MB",
      "file_url": "https://gw.hancom.com/ekp/service/file/fileView?module=board&fileUrl=/C100171030/board/file/2025/10/22&fileName=8fdd4492-bc8e-4322-8cee-3ebcded2f7c8.pdf"
    },
    {
      "file_id": "BD388832376425834042633",
      "file_name": "첨부 3. 2025 인사평가 시스템 매뉴얼_팀장용.pdf",
      "file_extension": "pdf",
      "file_size": 2699044,
      "file_size_formatted": "2.57 MB",
      "file_url": "https://gw.hancom.com/ekp/service/file/fileView?module=board&fileUrl=/C100171030/board/file/2025/10/22&fileName=9b8fc2e5-73c7-4bc2-971e-f84940fe5b68.pdf"
    }
  ],
  "attachment_count": 3,
  "total_attachment_size": 7766709,
  "contents": [
    {
      "type": "text",
      "value": "안녕하세요. 피플앤컬처팀 입니다.\n\n올해 초부터 진행해온 성장피드백과 중간목표 설정을 통해 각자의 성과와 성장을 지속적으로 점검해왔습니다.\n이제 그 과정의 결과를 종합적으로 평가하는 시점입니다.\n임직원 여러분들께서는 가이드 및 매뉴얼 참고하시어 정해진 기간 내 인사평가가 마무리 될 수 있도록\n많은 협조 부탁드립니다.\n\n                                            - 다  음 -\n\n■ 평가대상기간 : 2025. 1. 1. ~ 2025. 12. 31.\n\n■ 평가대상: 정규직 임직원\n  - 기존재직자 (2025년 휴/복직자중 연간근무일수 90일 이상인 직원)\n  - 2025년 9월30일 이전 입사자(신입/경력)\n\n■ 평가시스템\n  - 그룹웨어 메인 하단 퀵링크에서 '전사자원관리' 클릭\n  - 일반업무(인사/회계)_HC에서 [평가] 인사평가 클릭\n  ※ 기존 등록한 2025년 분기 목표를 불러와서 목표등록이 가능합니다. 자세한 사항은 매뉴얼 참고 부탁드립니다.\n\n■ 평가단계\n\n  ▶ 팀원 프로세스"
    },
    {
      "type": "image",
      "url": "https://gw.hancom.com/ekp/service/file/fileView?module=board&fileUrl=/C100171030/board/image/2025/10/22&fileName=3f91d84a-36c4-49a2-b8b7-f6add6fcc0df.png"
    },
    {
      "type": "text",
      "value": "  ▶ 직책보임자 프로세스 (팀원 프로세스 이후 진행)"
    },
    {
      "type": "image",
      "url": "https://gw.hancom.com/ekp/service/file/fileView?module=board&fileUrl=/C100171030/board/image/2025/10/22&fileName=e10c49fe-aaa4-4aa6-bc23-4148d64f2c73.png"
    },
    {
      "type": "text",
      "value": "  ※ 자세한 사항은 첨부해드린 매뉴얼 참고 부탁드립니다.\n\n■ 직원 평가일정"
    },
    {
      "type": "table",
      "value": "<table><tbody><tr><td>구분</td><td>일정</td><td>비고</td></tr><tr><td>인사평가 시스템 오픈</td><td>10/27(월)</td><td></td></tr><tr><td>목표등록 및 확정<br>동료평가자 선정</td><td>10/27(월) ~ 10/28(화)</td><td>동료평가자는 팀장/팀원 협의 후 선정<br>(동료평가자는 최소 3인이상, 다른부서도 가능)</td></tr><tr><td>자기평가, 상향&amp;동료평가</td><td>10/29(수) ~ 10/31(금)</td><td>자기평가 및 상향,동료평가 등록</td></tr><tr><td>본부 점수 확정 및<br>팀 별 점수 배분</td><td>11/03(월) ~ 11/04(화)</td><td>본부 점수를 토대로<br>각 팀별 점수까지 산출</td></tr><tr><td>1차 (팀장)조정 평가 및<br>CM대상자 선정</td><td>11/05(수) ~ 11/07(금)</td><td>팀장 1차 조정평가 후<br>실장 확정까지 완료</td></tr><tr><td>본부별 CM미팅</td><td>11/10(월) ~ 11/12(수)</td><td>본부별 가점자 선정</td></tr><tr><td>CM결과 확정 및 최종보고</td><td>11/13(목) ~ 11/14(금)</td><td></td></tr><tr><td>평가결과 공지 및 피드백</td><td>11/17(월) ~ 11/18(화)</td><td>피드백 후 이의제기까지 진행</td></tr><tr><td>평가 확정</td><td>11/19(수)</td><td></td></tr></tbody></table>"
    },
    {
      "type": "text",
      "value": "※ 상기 일정은 회사 사정에 따라 다소 변동될 수 있습니다.\n\n■ 주의사항\n  - ERP평가 진행 시 가급적 윈도우OS PC로 진행하여 주시기 바랍니다. Mac사용 시 특정 코드 등록으로 인한 오류 발생이 일어날 수 있습니다.\n  - 평가시스템은 평가단계별 일정에 따라 시스템상으로 일괄 조정되므로 평가일정에 맞추어 정해진 기한 내에 평가를 진행하여 주시기 바랍니다.\n  - 동료/상향평가 점수는 참고로 활용되며 평가점수와 연동되지는 않습니다. 조정평가에 대한 참고자료로 활용되는 사항오니 참고 부탁드립니다.\n    ※ 동료/상향평가 시 익명으로 진행되지 않습니다. 감안하여 평가 진행하여 주시기 바랍니다.\n      (상향평가 시 팀장은 평가결과를 볼 수 없고 실장이 확인 가능합니다)\n\n기타 궁금하신 사항은 언제든 피플앤컬처팀(담당: 이중호님)에게 문의하여주시기 바랍니다.\n\n감사합니다."
    }
  ],
  "content_html": "<!-- 원본 HTML 전체 보존 -->"
}
```

---

## 변경 이력

| 버전 | 날짜 | 변경 내용 |
|------|------|-----------|
| 1.0 | 2026-01-31 | 초기 스펙 확정. 테이블 HTML 클린업 규칙 추가, content_text 제거, 링크 마크다운 통합 |
