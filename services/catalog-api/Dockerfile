FROM rust:1.94-slim AS build
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release -p catalog-api

FROM debian:trixie-slim
COPY --from=build /app/target/release/catalog-api /usr/local/bin/catalog-api
EXPOSE 8788
CMD ["catalog-api"]
