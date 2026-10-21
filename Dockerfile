FROM node:24-alpine AS web
WORKDIR /build
COPY package.json package-lock.json ./
RUN npm ci
COPY index.html tsconfig*.json vite.config.ts ./
COPY public public
COPY src src
RUN npm run build

FROM rust:1.90-bookworm AS server
WORKDIR /build
COPY Cargo.toml ./
COPY server server
RUN cargo test --workspace
RUN cargo build --workspace --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl libssl3 libcurl4 && rm -rf /var/lib/apt/lists/*
COPY --from=server /build/target/release/job-tracker-api /usr/local/bin/job-tracker-api
COPY --from=server /build/target/release/job-tracker-worker /usr/local/bin/job-tracker-worker
COPY --from=web /build/dist /app/dist
WORKDIR /app
ENV STATIC_DIR=/app/dist BIND_ADDRESS=0.0.0.0:8080
EXPOSE 8080
CMD ["job-tracker-api"]
