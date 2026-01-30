#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
IMAGE_NAME="hwpx-converter"
IMAGE_FILE="${SCRIPT_DIR}/${IMAGE_NAME}.tar.gz"

echo "=== HWPX Converter 배포 ==="

# 이미지 로드
if [ -f "${IMAGE_FILE}" ]; then
    echo "[1/2] 이미지 로드 중... ← ${IMAGE_FILE}"
    docker load -i "${IMAGE_FILE}"
else
    echo "오류: ${IMAGE_FILE} 파일을 찾을 수 없습니다."
    exit 1
fi

# 실행
echo "[2/2] 컨테이너 실행 중..."
docker compose -f "${SCRIPT_DIR}/docker-compose.yml" up -d

echo ""
echo "=== 완료 ==="
echo "서비스: http://localhost:9040"
echo "헬스체크: http://localhost:9040/api/v1/health"
echo "API 문서: http://localhost:9040/swagger-ui/"
echo ""
echo "관리 명령어:"
echo "  상태 확인: docker compose -f ${SCRIPT_DIR}/docker-compose.yml ps"
echo "  로그 확인: docker compose -f ${SCRIPT_DIR}/docker-compose.yml logs -f"
echo "  중지:     docker compose -f ${SCRIPT_DIR}/docker-compose.yml down"
