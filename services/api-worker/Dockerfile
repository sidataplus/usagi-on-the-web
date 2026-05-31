FROM rust:1.94-slim AS build
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release -p api-worker --bins

FROM debian:trixie-slim
COPY --from=build /app/target/release/api-worker /usr/local/bin/api-worker
COPY --from=build /app/target/release/usagi-worker /usr/local/bin/usagi-worker
CMD ["usagi-worker", "--queues", "catalog,index,embed,map"]
