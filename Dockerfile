# Build stage
FROM rust:1.85 AS builder

WORKDIR /app

# Cargo.toml/lock 및 vendor 복사
COPY Cargo.toml Cargo.lock ./
COPY vendor vendor

# 벤더 소스 사용 설정
RUN mkdir -p .cargo && \
    echo '[source.crates-io]' > .cargo/config.toml && \
    echo 'replace-with = "vendored-sources"' >> .cargo/config.toml && \
    echo '[source.vendored-sources]' >> .cargo/config.toml && \
    echo 'directory = "vendor"' >> .cargo/config.toml

# 소스 복사 및 빌드
COPY src ./src
RUN cargo build --release --bin jsontohwpx-api

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

# 출력 디렉토리 생성
RUN mkdir -p /tmp/hwp-converter

# 바이너리 복사
COPY --from=builder /app/target/release/jsontohwpx-api /usr/local/bin/hwp-converter

EXPOSE 9040

ENV RUST_LOG=info \
    HOST=0.0.0.0 \
    PORT=9040 \
    OUTPUT_DIR=/tmp/hwp-converter

ENTRYPOINT ["hwp-converter"]
