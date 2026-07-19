"""
Quick test to verify the LLM router works.
Run from the ai-core directory with: python test_llm.py
"""
import asyncio
import sys
import os

# Add the project root to path so src is importable as a package
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from src.llm.router import LLMRouter
from src.llm.types import LLMRequest, Message, MessageRole, ProviderType

async def main():
    router = LLMRouter()

    # Check providers
    print("Checking providers...")
    status = await router.check_all_providers()
    for name, ok in status.items():
        print(f"  {name}: {'✓ available' if ok else '✗ not configured'}")

    available = [name for name, ok in status.items() if ok]
    if not available:
        print("\nNo providers available. Configure one in .env and try again.")
        print("For free local AI: install Ollama and run 'ollama pull llama3.1:8b'")
        return

    print(f"\nTesting with: {available[0]}")

    request = LLMRequest(
        messages=[
            Message(role=MessageRole.USER, content="Hello! What is 2+2? Reply in one sentence.")
        ],
        max_tokens=100,
        stream=True,
    )

    print("\nStreaming response:")
    async for chunk in router.stream(request):
        print(chunk.content, end="", flush=True)
    print("\n")

    print("Done! The AI Core is working.")

asyncio.run(main())
