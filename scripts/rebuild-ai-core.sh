#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
AI_CORE_DIR="$PROJECT_DIR/ai-core"

echo "============================================"
echo " Shadowlynx ProX — AI Core Rebuild Script"
echo "============================================"
echo ""

# Step 1: Activate venv
echo "[1/6] Activating virtual environment..."
cd "$AI_CORE_DIR"
if [ ! -d ".venv" ]; then
    echo "  Creating venv..."
    python3.12 -m venv .venv
fi
source .venv/bin/activate
echo "  ✓ .venv activated"

# Step 2: Install dependencies
echo "[2/6] Installing Python packages..."
pip install --quiet grpcio grpcio-tools protobuf
pip install --quiet anthropic openai httpx
pip install --quiet pydantic pydantic-settings python-dotenv
pip install --quiet tenacity msgpack orjson
pip install --quiet chromadb redis
pip install --quiet rich
echo "  ✓ Packages installed"

# Step 3: Generate protobuf
echo "[3/6] Generating gRPC code from proto..."
mkdir -p src/proto
python -m grpc_tools.protoc \
    -I../proto \
    --python_out=src/proto \
    --grpc_python_out=src/proto \
    ../proto/ai_core.proto

# Fix import
sed -i 's/^import ai_core_pb2 as ai__core__pb2/from . import ai_core_pb2 as ai__core__pb2/' \
    src/proto/ai_core_pb2_grpc.py 2>/dev/null || true

echo "  ✓ Proto code generated"
echo "  Files:"
ls -la src/proto/ai_core_pb2*.py

# Step 4: Create all __init__.py files
echo "[4/6] Ensuring all __init__.py files exist..."
find src -type d -exec touch {}/__init__.py \; 2>/dev/null || true
echo "  ✓ __init__.py files created"

# Step 5: Create .env if missing
echo "[5/6] Checking .env file..."
if [ ! -f ".env" ]; then
    cp .env.example .env 2>/dev/null || cat > .env << 'ENVEOF'
DEFAULT_PROVIDER=ollama
OLLAMA_ENDPOINT=http://localhost:11434
OLLAMA_DEFAULT_MODEL=llama3.1:8b
ANTHROPIC_API_KEY=
OPENAI_API_KEY=
DEEPSEEK_API_KEY=
ENVEOF
    echo "  ✓ .env created (edit with your API keys)"
else
    echo "  ✓ .env already exists"
fi

# Step 6: Import test
echo "[6/6] Testing imports..."
python -c "
import sys
print('  Testing imports...')

# Test config
from src.config import settings
print(f'  ✓ config (provider={settings.default_provider})')

# Test proto
from src.proto import ai_core_pb2, ai_core_pb2_grpc
print('  ✓ proto')

# Test LLM types
from src.llm.types import LLMRequest, LLMResponse, ProviderType
print('  ✓ llm.types')

# Test providers
from src.llm.providers.anthropic_provider import AnthropicProvider
from src.llm.providers.openai_provider import OpenAIProvider
from src.llm.providers.deepseek_provider import DeepSeekProvider
from src.llm.providers.ollama_provider import OllamaProvider
print('  ✓ llm.providers')

# Test router
from src.llm.router import LLMRouter
print('  ✓ llm.router')

# Test prompt templates
from src.llm.prompt_templates.default import get_system_prompt
print('  ✓ prompt_templates')

# Test AI core service
from src.ai_core_service import AICoreService
print('  ✓ ai_core_service')

print('')
print('ALL IMPORTS PASSED ✓')
"

echo ""
echo "============================================"
echo " REBUILD COMPLETE"
echo "============================================"
echo ""
echo "To start the AI Core server:"
echo "  cd ai-core && source .venv/bin/activate"
echo "  python -m src.main"
echo ""
