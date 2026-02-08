# HWPX Converter 배포 가이드

## 1. 설치 및 실행

### 사전 요구사항

- Docker, Docker Compose 설치

### 실행

```bash
# 3개 파일을 같은 폴더에 복사
# - hwpx-converter.tar.gz
# - docker-compose.yml
# - load-and-run.sh

chmod +x load-and-run.sh
./load-and-run.sh
```

### 실행 결과

```
=== HWPX Converter 배포 ===
[1/2] 이미지 로드 중...
[2/2] 컨테이너 실행 중...

=== 완료 ===
서비스: http://localhost:9040
헬스체크: http://localhost:9040/api/v1/health
API 문서: http://localhost:9040/swagger-ui/
```

### 관리 명령어

```bash
# 상태 확인
docker ps --filter name=hwpx-converter

# 로그 확인
docker logs -f hwpx-converter

# 중지
docker compose down

# 재시작
docker compose restart
```

---

## 2. API 엔드포인트

| 메서드 | 경로 | 설명 |
|--------|------|------|
| `GET` | `/api/v1/health` | 서버 상태 확인 |
| `POST` | `/api/v1/convert` | 동기 변환 (JSON body, 즉시 HWPX 반환) |
| `POST` | `/api/v1/convert/file` | 동기 변환 (파일 업로드, 즉시 HWPX 반환) |
| `POST` | `/api/v1/convert/async` | 비동기 변환 (작업 ID 반환) |
| `GET` | `/api/v1/jobs/{id}` | 비동기 작업 상태 조회 |
| `GET` | `/api/v1/jobs/{id}/download` | 비동기 작업 결과 다운로드 |
| `POST` | `/api/v1/validate` | JSON 검증만 (변환 없음) |
| `GET` | `/swagger-ui/` | Swagger UI (API 문서) |

---

## 3. 헬스체크

```bash
curl http://localhost:9040/api/v1/health
```

응답:
```json
{
  "status": "healthy",
  "version": "0.5.0",
  "queue": { "pending": 0, "processing": 0, "completed": 0, "failed": 0 },
  "workers": { "active": 0, "max": 4 },
  "uptime_seconds": 120,
  "license": "valid"
}
```

---

## 4. 동기 변환 (POST /api/v1/convert)

JSON을 보내면 즉시 HWPX 파일을 응답으로 받습니다.

**응답 헤더:**

| 헤더 | 값 | 설명 |
|------|----|------|
| `Content-Type` | `application/vnd.hancom.hwpx` | HWPX 파일 MIME 타입 |
| `Content-Disposition` | `attachment; filename="{article_id}.hwpx"` | 다운로드 파일명 |
| `Content-Length` | 파일 크기 (bytes) | 응답 바디 크기 |

> 성공 시 HTTP 200과 함께 바이너리(HWPX 파일)가 응답 바디로 반환됩니다.

```bash
# 기본 변환
curl -X POST http://localhost:9040/api/v1/convert \
  -H "Content-Type: application/json" \
  -d '{
    "schema_version": "1.1",
    "article_id": "DOC001",
    "title": "테스트 문서",
    "metadata": {
      "author": "홍길동",
      "created_at": "2025-01-30T10:00:00+09:00",
      "updated_at": "2025-01-30T10:00:00+09:00",
      "department": "개발팀",
      "board_id": "BBNC100171030",
      "board_name": "공지사항",
      "board_path": ["BGF리테일게시판", "전사공지사항", "공지사항"],
      "board_depth": 3,
      "folder_id": "BFCC100171030",
      "expiry": "영구",
      "views": 0,
      "likes": 0,
      "comments": 0
    },
    "attachments": [],
    "attachment_count": 0,
    "total_attachment_size": 0,
    "contents": [
      { "type": "text", "value": "안녕하세요.\n변환 테스트입니다." },
      { "type": "table", "value": "<table><tr><th>이름</th><th>부서</th></tr><tr><td>홍길동</td><td>개발팀</td></tr></table>" }
    ],
    "content_html": "<p>안녕하세요.</p>"
  }' \
  --output DOC001.hwpx

# include_header 옵션: 메타데이터를 본문 상단에 삽입
curl -X POST "http://localhost:9040/api/v1/convert?include_header=true" \
  -H "Content-Type: application/json" \
  -d '{
    "schema_version": "1.1",
    "article_id": "DOC001",
    "title": "테스트 문서",
    "metadata": {
      "author": "홍길동",
      "department": "개발팀",
      "board_name": "공지사항",
      "board_path": ["BGF리테일게시판", "공지사항"],
      "board_depth": 2
    },
    "contents": [{ "type": "text", "value": "본문" }]
  }' \
  --output DOC001.hwpx
```

**`include_header=true` 삽입 결과:**

본문 상단에 메타데이터가 아래 형식으로 삽입됩니다:

```
제목: 테스트 문서
작성자: 홍길동
부서: 개발팀
작성일: 2025-01-30T10:00:00+09:00
게시판: 공지사항

(본문 내용)
```


---

## 4-1. 파일 업로드 동기 변환 (POST /api/v1/convert/file)

JSON 파일을 multipart/form-data로 업로드하면 즉시 HWPX 파일을 반환합니다.

```bash
# 기본 변환
curl -X POST http://localhost:9040/api/v1/convert/file \
  -F "file=@input.json" \
  --output result.hwpx

# include_header 옵션 포함
curl -X POST http://localhost:9040/api/v1/convert/file \
  -F "file=@input.json" \
  -F "include_header=true" \
  --output result.hwpx
```

| 필드 | 필수 | 설명 |
|------|------|------|
| `file` | O | JSON 파일 (multipart file) |
| `include_header` | - | `true`이면 메타데이터를 본문 상단에 삽입 |

---

## 5. 비동기 변환 (대용량 문서)

### 5-1. 변환 요청

```bash
# 기본 비동기 변환
curl -X POST http://localhost:9040/api/v1/convert/async \
  -H "Content-Type: application/json" \
  -d '{
    "schema_version": "1.1",
    "article_id": "DOC002",
    "title": "대용량 문서",
    "metadata": {
      "author": "홍길동",
      "department": "개발팀",
      "board_name": "공지사항"
    },
    "contents": [
      { "type": "text", "value": "내용..." }
    ]
  }'

# include_header 옵션 포함
curl -X POST "http://localhost:9040/api/v1/convert/async?include_header=true" \
  -H "Content-Type: application/json" \
  -d '{
    "schema_version": "1.1",
    "article_id": "DOC002",
    "title": "대용량 문서",
    "metadata": {
      "author": "홍길동",
      "department": "개발팀",
      "board_name": "공지사항"
    },
    "contents": [{ "type": "text", "value": "내용..." }]
  }'
```

응답:
```json
{
  "jobId": "550e8400-e29b-41d4-a716-446655440000",
  "status": "queued",
  "createdAt": "2025-01-30T10:00:00Z"
}
```

### 5-2. 상태 확인

```bash
curl http://localhost:9040/api/v1/jobs/{jobId}
```

응답:
```json
{
  "jobId": "550e8400-...",
  "status": "completed",
  "createdAt": "...",
  "completedAt": "...",
  "downloadUrl": "/api/v1/jobs/550e8400-.../download"
}
```

**status 값:**

| 상태 | 설명 |
|------|------|
| `queued` | 대기열에 등록됨 |
| `processing` | 변환 처리 중 |
| `completed` | 변환 완료 (다운로드 가능) |
| `failed` | 변환 실패 |

> `completed`일 때만 `downloadUrl`과 `completedAt` 필드가 포함됩니다.
> `failed`일 때는 `error` 필드에 실패 사유가 포함됩니다.

### 5-3. 결과 다운로드

```bash
curl http://localhost:9040/api/v1/jobs/{jobId}/download --output result.hwpx
```

> **작업 결과 보관 기간**: 비동기 변환 결과 파일은 기본 **24시간** 후 자동 삭제됩니다.
> 만료된 작업을 다운로드하면 HTTP 404가 반환됩니다.
> 보관 기간은 환경 변수 `FILE_EXPIRY_HOURS`로 변경할 수 있습니다 (10절 참고).

---

## 6. 제한사항

| 항목 | 값 | 설명 |
|------|----|------|
| 최대 요청 크기 | **50MB** (52,428,800 bytes) | 초과 시 HTTP 413 반환 |
| 비동기 작업 보관 | **24시간** | 이후 결과 파일 자동 삭제 |
| 비동기 워커 수 | **4** (기본) | 동시 처리 가능한 비동기 작업 수 |

> 위 값들은 환경 변수로 변경 가능합니다 (10절 참고).

---

## 7. JSON 검증 (POST /api/v1/validate)

변환 없이 JSON 형식만 검증합니다.

```bash
curl -X POST http://localhost:9040/api/v1/validate \
  -H "Content-Type: application/json" \
  -d '{
    "schema_version": "1.1",
    "article_id": "DOC003",
    "title": "검증 테스트",
    "contents": []
  }'
```

성공 응답:
```json
{ "valid": true }
```

실패 응답 (article_id 누락):
```json
{
  "valid": false,
  "errors": ["입력 에러: article_id가 비어있습니다"]
}
```

실패 응답 (JSON 파싱 실패):
```json
{
  "valid": false,
  "errors": ["JSON 파싱 실패: expected value at line 1 column 1"]
}
```

---

## 8. 입력 JSON 구조

```json
{
  "schema_version": "1.1",
  "article_id": "문서ID (필수, 출력 파일명으로 사용)",
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
    { "type": "text", "value": "텍스트 (\\n으로 줄바꿈)" },
    { "type": "table", "value": "<table>HTML 테이블</table>" },
    { "type": "image", "url": "https://example.com/img.png" },
    { "type": "image", "base64": "iVBOR...", "format": "png" }
  ],
  "content_html": "<p>원본 HTML</p>"
}
```

### 필드 설명

| 필드 | 필수 | 설명 |
|------|------|------|
| `schema_version` | - | 스키마 버전 (현재 `"1.1"`) |
| `article_id` | O | 문서 고유 ID (비어있으면 에러) |
| `title` | - | 문서 제목 |
| `metadata` | - | 메타데이터 객체 (author, department, board_name 등) |
| `attachments` | - | 첨부파일 배열 (파싱만, HWPX 변환에 미사용) |
| `attachment_count` | - | 첨부파일 수 |
| `total_attachment_size` | - | 첨부파일 총 크기 (bytes) |
| `contents` | - | 본문 콘텐츠 배열 |
| `content_html` | - | 원본 HTML (파싱만, HWPX 변환에 미사용) |

### 콘텐츠 타입

| type | 필드 | 설명 |
|------|------|------|
| `text` | `value` | 텍스트, `\n` 줄바꿈 지원 |
| `table` | `value` | HTML `<table>` (colspan/rowspan 지원) |
| `image` | `url` | 파일 경로 또는 HTTP URL |
| `image` | `base64` + `format` | Base64 인코딩 이미지 |

### 지원 이미지 포맷

PNG, JPEG, GIF, BMP, WebP, AVIF

---

## 9. 에러 응답

```json
{
  "error": {
    "code": "INVALID_JSON",
    "message": "JSON 파싱 실패: ...",
    "details": []
  }
}
```

| 코드 | HTTP | 설명 |
|------|------|------|
| `INVALID_JSON` | 400 | JSON 파싱 실패 |
| `INPUT_ERROR` | 400 | 입력 데이터 검증 실패 (article_id 누락 등) |
| `CONVERSION_ERROR` | 500 | 변환 중 오류 |

---

## 10. 환경 변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `PORT` | `9040` | 서버 포트 |
| `RUST_LOG` | `info` | 로그 레벨 |
| `MAX_REQUEST_SIZE` | `52428800` | 최대 요청 크기 (50MB) |
| `WORKER_COUNT` | `4` | 비동기 워커 수 |
| `FILE_EXPIRY_HOURS` | `24` | 생성 파일 만료 시간 |

docker-compose.yml에서 변경 가능합니다.

---

## 11. 트러블슈팅

### 컨테이너가 시작되지 않는 경우

```bash
# 로그 확인
docker logs hwpx-converter

# 포트 충돌 확인 (9040 포트를 다른 프로세스가 사용 중인지)
lsof -i :9040
# 또는
netstat -tlnp | grep 9040
```

포트가 충돌하면 `docker-compose.yml`에서 포트 매핑을 변경하세요:
```yaml
ports:
  - "9041:9040"  # 호스트 포트를 9041로 변경
```

### 헬스체크가 실패하는 경우

```bash
# 컨테이너 상태 확인
docker ps --filter name=hwpx-converter

# 직접 헬스체크
curl -v http://localhost:9040/api/v1/health
```

### 변환 시 HTTP 413 에러

요청 JSON 크기가 50MB를 초과하는 경우 발생합니다.
이미지를 base64로 포함하면 크기가 커질 수 있으므로, 이미지 수나 해상도를 줄여주세요.

### 비동기 다운로드 시 HTTP 404 에러

작업 결과 파일이 만료(기본 24시간)되어 삭제된 경우입니다.
변환 완료 후 빠르게 다운로드하거나, `FILE_EXPIRY_HOURS` 값을 늘려주세요.

### 컨테이너 재시작/업데이트

```bash
# 기존 컨테이너 중지 및 제거 후 재시작
docker compose down && ./load-and-run.sh
```
