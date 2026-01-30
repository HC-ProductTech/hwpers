#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
IMAGE_NAME="hwp-converter"
IMAGE_TAG="${1:-latest}"
OUTPUT_FILE="${SCRIPT_DIR}/${IMAGE_NAME}.tar.gz"

echo "=== HWP Converter 이미지 빌드 ==="
echo "프로젝트: ${PROJECT_DIR}"
echo "이미지: ${IMAGE_NAME}:${IMAGE_TAG}"
echo ""

# 빌드
echo "[1/2] 이미지 빌드 중..."
docker build --platform linux/amd64 -t "${IMAGE_NAME}:${IMAGE_TAG}" "${PROJECT_DIR}"

# 저장
echo "[2/2] 이미지 저장 중... → ${OUTPUT_FILE}"
docker save "${IMAGE_NAME}:${IMAGE_TAG}" | gzip > "${OUTPUT_FILE}"

SIZE=$(du -h "${OUTPUT_FILE}" | cut -f1)
echo ""
echo "=== 완료 ==="
echo "파일: ${OUTPUT_FILE}"
echo "크기: ${SIZE}"
echo ""
echo "폐쇄망 전달 파일:"
echo "  1. ${OUTPUT_FILE}"
echo "  2. ${SCRIPT_DIR}/docker-compose.yml"
echo "  3. ${SCRIPT_DIR}/load-and-run.sh"
