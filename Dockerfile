# Stage 1: Build
FROM rust:1.79 as builder
WORKDIR /app
COPY . .
# Install protoc for gRPC/protobuf builds
RUN apt-get update && apt-get install -y protobuf-compiler
RUN cd orchestrator && cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim
WORKDIR /app
# Copy the compiled binary from builder stage
COPY --from=builder /app/orchestrator/target/release/orchestrator /usr/local/bin/orchestrator
# Expose gRPC port
EXPOSE 50053
# Run the orchestrator
CMD ["orchestrator"]
