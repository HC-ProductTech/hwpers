# jsontohwpx JSON 스펙

JSON API 응답을 HWPX(한글 문서) 파일로 변환하기 위한 입력 JSON 형식을 정의합니다.

## 목차

- [전체 구조](#전체-구조)
- [필드 상세](#필드-상세)
  - [최상위 필드](#최상위-필드)
  - [options](#options)
  - [data](#data)
  - [article](#article)
  - [contents](#contents)
- [콘텐츠 타입](#콘텐츠-타입)
  - [text](#text)
  - [image](#image)
  - [table](#table)
- [예제](#예제)
- [검증 규칙](#검증-규칙)

---

## 전체 구조

```json
{
  "responseCode": "0",
  "responseText": "SUCCESS",
  "options": {
    "includeHeader": false,
    "headerFields": []
  },
  "data": {
    "article": {
      "atclId": "문서ID",
      "subject": "문서 제목",
      "contents": [],
      "regDt": "2026-01-25 PM 12:00:00",
      "regEmpName": "작성자명",
      "regDeptName": "부서명"
    }
  }
}
```

---

## 필드 상세

### 최상위 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `responseCode` | string | **필수** | 응답 코드. `"0"`이면 정상 |
| `responseText` | string | 선택 | 응답 메시지 (예: "SUCCESS") |
| `options` | object | 선택 | 변환 옵션 |
| `data` | object | **필수** | 문서 데이터 |

### options

| 필드 | 타입 | 기본값 | 설명 |
|------|------|--------|------|
| `includeHeader` | boolean | `false` | 문서 상단에 헤더(작성자, 부서, 일시) 포함 여부 |
| `headerFields` | string[] | `[]` | 헤더에 포함할 필드 목록 (예: `["subject", "regEmpName"]`) |

### data

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `article` | object | **필수** | 문서 본문 |

### article

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `atclId` | string | **필수** | 문서 고유 ID (출력 파일명으로 사용됨) |
| `subject` | string | 선택 | 문서 제목 |
| `contents` | array | 선택 | 본문 콘텐츠 배열 |
| `regDt` | string | 선택 | 작성일시 (예: "2026-01-25 PM 12:00:00") |
| `regEmpName` | string | 선택 | 작성자 이름 |
| `regDeptName` | string | 선택 | 작성자 부서명 |

### contents

콘텐츠 배열의 각 요소는 `type` 필드로 구분됩니다.

| type | 설명 |
|------|------|
| `text` | 텍스트 콘텐츠 |
| `image` | 이미지 콘텐츠 |
| `table` | 표 콘텐츠 (HTML) |

---

## 콘텐츠 타입

### text

일반 텍스트를 삽입합니다.

```json
{
  "type": "text",
  "value": "텍스트 내용입니다.\n줄바꿈도 지원합니다."
}
```

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `type` | string | **필수** | `"text"` |
| `value` | string | **필수** | 텍스트 내용. `\n`으로 줄바꿈 |

### image

이미지를 삽입합니다. `url` 또는 `base64` 중 하나를 사용합니다.

#### URL 방식 (파일 경로 또는 HTTP URL)

```json
{
  "type": "image",
  "url": "./images/photo.png"
}
```

```json
{
  "type": "image",
  "url": "https://example.com/image.jpg"
}
```

#### Base64 방식

```json
{
  "type": "image",
  "base64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
  "format": "png"
}
```

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `type` | string | **필수** | `"image"` |
| `url` | string | 조건부 | 이미지 파일 경로 또는 HTTP(S) URL |
| `base64` | string | 조건부 | Base64 인코딩된 이미지 데이터 |
| `format` | string | 선택 | Base64 사용 시 이미지 포맷 (예: `"png"`, `"jpg"`) |

**지원 포맷:** PNG, JPEG, GIF, WebP, AVIF

**참고:**
- `url`과 `base64` 중 하나만 지정
- `url`이 상대 경로인 경우 `--base-path` 옵션 기준으로 해석
- HTTP URL은 타임아웃 60초

### table

HTML `<table>` 태그로 표를 삽입합니다.

```json
{
  "type": "table",
  "value": "<table><tr><th>헤더1</th><th>헤더2</th></tr><tr><td>값1</td><td>값2</td></tr></table>"
}
```

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `type` | string | **필수** | `"table"` |
| `value` | string | **필수** | HTML 테이블 문자열 |

**지원 HTML 요소:**

| 태그 | 설명 |
|------|------|
| `<table>` | 테이블 컨테이너 |
| `<thead>`, `<tbody>` | 테이블 섹션 (선택) |
| `<tr>` | 행 |
| `<th>` | 헤더 셀 (굵은 글씨) |
| `<td>` | 일반 셀 |

**지원 속성:**

| 속성 | 적용 대상 | 설명 |
|------|-----------|------|
| `colspan` | `<td>`, `<th>` | 가로 셀 병합 |
| `rowspan` | `<td>`, `<th>` | 세로 셀 병합 |

**테이블 예제:**

```html
<table>
  <tr>
    <th colspan="2">병합된 헤더</th>
  </tr>
  <tr>
    <td rowspan="2">세로 병합</td>
    <td>값1</td>
  </tr>
  <tr>
    <td>값2</td>
  </tr>
</table>
```

---

## 예제

### 기본 텍스트 문서

```json
{
  "responseCode": "0",
  "data": {
    "article": {
      "atclId": "DOC001",
      "subject": "공지사항",
      "contents": [
        { "type": "text", "value": "안녕하세요.\n\n중요한 공지사항입니다." }
      ]
    }
  }
}
```

### 이미지 포함 문서

```json
{
  "responseCode": "0",
  "data": {
    "article": {
      "atclId": "DOC002",
      "subject": "이미지 첨부 문서",
      "contents": [
        { "type": "text", "value": "아래 이미지를 참고하세요." },
        { "type": "image", "url": "https://example.com/diagram.png" },
        { "type": "text", "value": "감사합니다." }
      ]
    }
  }
}
```

### 표 포함 문서

```json
{
  "responseCode": "0",
  "data": {
    "article": {
      "atclId": "DOC003",
      "subject": "실적 보고서",
      "contents": [
        { "type": "text", "value": "2026년 1분기 실적입니다." },
        {
          "type": "table",
          "value": "<table><tr><th>항목</th><th>금액</th></tr><tr><td>매출</td><td>100억</td></tr><tr><td>영업이익</td><td>20억</td></tr></table>"
        }
      ]
    }
  }
}
```

### 헤더 포함 문서

```json
{
  "responseCode": "0",
  "options": {
    "includeHeader": true
  },
  "data": {
    "article": {
      "atclId": "DOC004",
      "subject": "회의록",
      "contents": [
        { "type": "text", "value": "금일 회의 내용을 공유합니다." }
      ],
      "regDt": "2026-01-25 PM 02:00:00",
      "regEmpName": "홍길동",
      "regDeptName": "개발팀"
    }
  }
}
```

### 복합 문서

```json
{
  "responseCode": "0",
  "responseText": "SUCCESS",
  "options": {
    "includeHeader": true,
    "headerFields": ["subject", "regEmpName", "regDeptName", "regDt"]
  },
  "data": {
    "article": {
      "atclId": "BA000000000000000001",
      "subject": "[공지] 시스템 점검 안내",
      "contents": [
        {
          "type": "text",
          "value": "안녕하세요. IT인프라팀입니다.\n\n아래와 같이 시스템 점검을 실시합니다."
        },
        {
          "type": "table",
          "value": "<table><tr><th>항목</th><th>내용</th></tr><tr><td>일시</td><td>2026-01-26 02:00 ~ 06:00</td></tr><tr><td>대상</td><td>전사 그룹웨어</td></tr><tr><td>영향</td><td>서비스 일시 중단</td></tr></table>"
        },
        {
          "type": "text",
          "value": "점검 시간 동안 서비스 이용이 불가합니다.\n양해 부탁드립니다."
        },
        {
          "type": "image",
          "url": "./notice_image.png"
        },
        {
          "type": "text",
          "value": "감사합니다."
        }
      ],
      "regDt": "2026-01-25 AM 10:00:00",
      "regEmpName": "김철수",
      "regDeptName": "IT인프라팀"
    }
  }
}
```

---

## 검증 규칙

변환 시 다음 조건을 검증합니다:

| 규칙 | 에러 코드 | 설명 |
|------|-----------|------|
| `responseCode`가 `"0"`이어야 함 | `INVALID_RESPONSE_CODE` | 다른 값이면 변환 거부 |
| `atclId`가 비어있지 않아야 함 | `MISSING_DATA` | 공백만 있는 경우도 에러 |
| `data.article` 필드 필수 | `MISSING_DATA` | 누락 시 에러 |
| 유효한 `type` 값 | `INVALID_JSON` | `text`, `image`, `table` 외 불가 |
| 테이블이 비어있지 않아야 함 | `CONVERSION_ERROR` | 행/열이 0개면 에러 |

---

## TypeScript 타입 정의

```typescript
interface ApiResponse {
  responseCode: string;
  responseText?: string;
  options?: Options;
  data: Data;
}

interface Options {
  includeHeader?: boolean;
  headerFields?: string[];
}

interface Data {
  article: Article;
}

interface Article {
  atclId: string;
  subject?: string;
  contents?: Content[];
  regDt?: string;
  regEmpName?: string;
  regDeptName?: string;
}

type Content = TextContent | ImageContent | TableContent;

interface TextContent {
  type: 'text';
  value: string;
}

interface ImageContent {
  type: 'image';
  url?: string;
  base64?: string;
  format?: string;
}

interface TableContent {
  type: 'table';
  value: string;
}
```

---

## 변경 이력

| 버전 | 날짜 | 변경 내용 |
|------|------|----------|
| 0.5.0 | 2026-01-25 | 초기 버전 |
