# Stage 1: Build
FROM rust:latest as builder
WORKDIR /app
COPY . .

# Install protoc for gRPC/protobuf builds
RUN apt-get update && apt-get install -y protobuf-compiler

# Install nightly toolchain automatically
RUN rustup install nightly && rustup default nightly

# Build orchestrator and gateway
RUN cd orchestrator && cargo build --release
RUN cd gateway && cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/orchestrator/target/release/orchestrator /usr/local/bin/orchestrator
COPY --from=builder /app/gateway/target/release/gateway /usr/local/bin/gateway
EXPOSE 50053 8080
CMD ["gateway"]
