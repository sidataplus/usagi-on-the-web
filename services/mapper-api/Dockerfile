FROM rust:1.94-slim AS build
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release -p mapper-api

FROM debian:trixie-slim
COPY --from=build /app/target/release/mapper-api /usr/local/bin/mapper-api
EXPOSE 8790
CMD ["mapper-api"]
