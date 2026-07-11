.PHONY: all build clean test run-chat dev help

# Default target
all: build

# ---- Build Commands ----

build: build-cli ## Build everything
	@echo "Build complete. Binary at bin/slpx"

build-cli: ## Build the Go CLI
	@echo "Building CLI..."
	cd cli && go build -o ../bin/slpx -ldflags="-s -w -X github.com/shadowlynx/prox-cli/cmd.Version=$$(git describe --tags --always --dirty 2>/dev/null || echo 'dev')" .

build-orchestrator: ## Build the Rust orchestrator
	@echo "Building orchestrator..."
	cd orchestrator && cargo build --release

build-ai-core: ## Set up Python venv
	@echo "Setting up AI core..."
	cd ai-core && python3 -m venv .venv && . .venv/bin/activate && pip install -e ".[dev]"

build-all: build-cli build-orchestrator build-ai-core ## Build everything

# ---- Run Commands ----

run-chat: build-cli ## Build and run interactive chat
	./bin/slpx chat

run-server: build-orchestrator ## Run the orchestrator
	cd orchestrator && cargo run --release

# ---- Development ----

dev: ## Start development environment
	@echo "Starting development services..."
	redis-server --port 6379 --daemonize yes --dir ./data/redis 2>/dev/null || echo "Redis already running"
	@echo "Redis: OK"
	@echo "Run 'make run-chat' to start chatting"

# ---- Test Commands ----

test: ## Run all tests
	@echo "Running Go tests..."
	cd cli && go test ./...
	@echo "Running Rust tests..."
	cd orchestrator && cargo test

test-cli: ## Run CLI tests only
	cd cli && go test ./...

# ---- Cleanup ----

clean: ## Remove build artifacts
	@echo "Cleaning..."
	rm -rf bin/
	cd cli && go clean
	cd orchestrator && cargo clean
	rm -rf ai-core/.venv ai-core/__pycache__ ai-core/src/__pycache__
	@echo "Clean complete"

# ---- Docker ----

docker-build: ## Build Docker images
	docker compose build

docker-up: ## Start all services with Docker
	docker compose up -d

docker-down: ## Stop all services
	docker compose down

# ---- Help ----

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
	awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'
