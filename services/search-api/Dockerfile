FROM rust:1.94-slim AS build
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release -p search-api

FROM debian:trixie-slim
COPY --from=build /app/target/release/search-api /usr/local/bin/search-api
EXPOSE 8789
CMD ["search-api"]
