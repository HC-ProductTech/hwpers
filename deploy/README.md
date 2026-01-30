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
| `POST` | `/api/v1/convert` | 동기 변환 (즉시 HWPX 반환) |
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
  "uptime_seconds": 120
}
```

---

## 4. 동기 변환 (POST /api/v1/convert)

JSON을 보내면 즉시 HWPX 파일을 응답으로 받습니다.

```bash
curl -X POST http://localhost:9040/api/v1/convert \
  -H "Content-Type: application/json" \
  -d '{
    "responseCode": "0",
    "data": {
      "article": {
        "atclId": "DOC001",
        "subject": "테스트 문서",
        "contents": [
          { "type": "text", "value": "안녕하세요.\n변환 테스트입니다." },
          { "type": "table", "value": "<table><tr><th>이름</th><th>부서</th></tr><tr><td>홍길동</td><td>개발팀</td></tr></table>" }
        ],
        "regEmpName": "홍길동",
        "regDeptName": "개발팀",
        "regDt": "2025-01-30 10:00:00"
      }
    },
    "options": {
      "includeHeader": true,
      "headerFields": ["subject", "regEmpName", "regDeptName", "regDt"]
    }
  }' \
  --output DOC001.hwpx

echo "생성됨: DOC001.hwpx"
```

---

## 5. 비동기 변환 (대용량 문서)

### 5-1. 변환 요청

```bash
curl -X POST http://localhost:9040/api/v1/convert/async \
  -H "Content-Type: application/json" \
  -d '{
    "responseCode": "0",
    "data": {
      "article": {
        "atclId": "DOC002",
        "subject": "대용량 문서",
        "contents": [
          { "type": "text", "value": "내용..." }
        ]
      }
    }
  }'
```

응답:
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "queued",
  "created_at": "2025-01-30T10:00:00Z"
}
```

### 5-2. 상태 확인

```bash
curl http://localhost:9040/api/v1/jobs/{job_id}
```

응답:
```json
{
  "job_id": "550e8400-...",
  "status": "completed",
  "created_at": "...",
  "completed_at": "..."
}
```

### 5-3. 결과 다운로드

```bash
curl http://localhost:9040/api/v1/jobs/{job_id}/download --output result.hwpx
```

---

## 6. JSON 검증 (POST /api/v1/validate)

변환 없이 JSON 형식만 검증합니다.

```bash
curl -X POST http://localhost:9040/api/v1/validate \
  -H "Content-Type: application/json" \
  -d '{
    "responseCode": "0",
    "data": {
      "article": {
        "atclId": "DOC003",
        "subject": "검증 테스트",
        "contents": []
      }
    }
  }'
```

응답:
```json
{ "valid": true, "errors": [] }
```

---

## 7. 입력 JSON 구조

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
      "atclId": "문서ID (필수, 출력 파일명으로 사용)",
      "subject": "문서 제목",
      "contents": [
        { "type": "text", "value": "텍스트 (\\n으로 줄바꿈)" },
        { "type": "table", "value": "<table>HTML 테이블</table>" },
        { "type": "image", "url": "https://example.com/img.png" },
        { "type": "image", "base64": "iVBOR...", "format": "png" }
      ],
      "regDt": "작성일시",
      "regEmpName": "작성자",
      "regDeptName": "부서"
    }
  }
}
```

### 필드 설명

| 필드 | 필수 | 설명 |
|------|------|------|
| `responseCode` | O | `"0"`이어야 정상 처리 |
| `data.article.atclId` | O | 문서 고유 ID |
| `data.article.subject` | - | 문서 제목 |
| `data.article.contents` | - | 본문 콘텐츠 배열 |
| `options.includeHeader` | - | `true`면 메타데이터를 문서 상단에 삽입 |
| `options.headerFields` | - | 포함할 메타데이터 필드 목록 |

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

## 8. 에러 응답

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
| `INVALID_RESPONSE_CODE` | 400 | responseCode가 "0"이 아님 |
| `MISSING_DATA` | 400 | data 또는 article 누락 |
| `CONVERSION_ERROR` | 500 | 변환 중 오류 |

---

## 9. 환경 변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `PORT` | `9040` | 서버 포트 |
| `RUST_LOG` | `info` | 로그 레벨 |
| `MAX_REQUEST_SIZE` | `52428800` | 최대 요청 크기 (50MB) |
| `WORKER_COUNT` | `4` | 비동기 워커 수 |
| `FILE_EXPIRY_HOURS` | `24` | 생성 파일 만료 시간 |

docker-compose.yml에서 변경 가능합니다.
