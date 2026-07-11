#!/bin/bash
# Start all Shadowlynx ProX services
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Shadowlynx ProX — Starting All Services ==="
echo ""

# 1. Start Redis (if not running)
if redis-cli ping > /dev/null 2>&1; then
    echo "✓ Redis already running"
else
    echo "Starting Redis..."
    redis-server --port 6379 --daemonize yes --dir "$PROJECT_DIR/data/redis"
    sleep 1
    echo "✓ Redis started"
fi

# 2. Start AI Core (Python gRPC server)
echo ""
echo "Starting AI Core..."
cd "$PROJECT_DIR/ai-core"
if [ ! -d ".venv" ]; then
    echo "  Creating virtual environment..."
    python3.12 -m venv .venv
fi
source .venv/bin/activate

# Check if already running
if lsof -i :50051 > /dev/null 2>&1; then
    echo "✓ AI Core already running on port 50051"
else
    python -m src.main &
    AI_CORE_PID=$!
    sleep 2
    if kill -0 $AI_CORE_PID 2>/dev/null; then
        echo "✓ AI Core started (PID: $AI_CORE_PID)"
    else
        echo "✗ AI Core failed to start"
        exit 1
    fi
fi

# 3. Start Orchestrator (Rust gRPC server)
echo ""
echo "Starting Orchestrator..."
cd "$PROJECT_DIR/orchestrator"
if lsof -i :50052 > /dev/null 2>&1; then
    echo "✓ Orchestrator already running on port 50052"
else
    cargo run --release &
    ORCH_PID=$!
    sleep 2
    if kill -0 $ORCH_PID 2>/dev/null; then
        echo "✓ Orchestrator started (PID: $ORCH_PID)"
    else
        echo "✗ Orchestrator failed to start"
        exit 1
    fi
fi

echo ""
echo "=== All services running ==="
echo ""
echo "  AI Core:       http://127.0.0.1:50051"
echo "  Orchestrator:  http://127.0.0.1:50052"
echo "  Redis:         redis://127.0.0.1:6379"
echo ""
echo "Run: ./bin/slpx chat"
echo "Stop all: pkill -f 'src.main' && pkill -f orchestrator"
