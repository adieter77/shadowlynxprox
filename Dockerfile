# Stage 1: Build
FROM rust:1.79 as builder
WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y protobuf-compiler
RUN cd orchestrator && cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/orchestrator/target/release/orchestrator /usr/local/bin/orchestrator
EXPOSE 50053
CMD ["orchestrator"]
