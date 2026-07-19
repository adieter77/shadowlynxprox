#!/bin/bash
# Check all required files exist

REQUIRED_FILES=(
    "src/__init__.py"
    "src/config.py"
    "src/main.py"
    "src/ai_core_service.py"
    "src/llm/__init__.py"
    "src/llm/types.py"
    "src/llm/router.py"
    "src/llm/providers/__init__.py"
    "src/llm/providers/anthropic_provider.py"
    "src/llm/providers/openai_provider.py"
    "src/llm/providers/deepseek_provider.py"
    "src/llm/providers/ollama_provider.py"
    "src/llm/prompt_templates/__init__.py"
    "src/llm/prompt_templates/default.py"
    "src/proto/__init__.py"
    "src/proto/ai_core_pb2.py"
    "src/proto/ai_core_pb2_grpc.py"
    "pyproject.toml"
    ".env"
)

MISSING=0
for f in "${REQUIRED_FILES[@]}"; do
    if [ -f "$f" ]; then
        echo "  ✓ $f"
    else
        echo "  ✗ MISSING: $f"
        MISSING=$((MISSING + 1))
    fi
done

echo ""
if [ $MISSING -eq 0 ]; then
    echo "=== ALL FILES PRESENT ==="
else
    echo "=== $MISSING FILE(S) MISSING ==="
fi
