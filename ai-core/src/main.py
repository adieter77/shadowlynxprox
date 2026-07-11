"""
Shadowlynx ProX — AI Core Server Entry Point.

Starts the gRPC server that the Rust orchestrator connects to.
"""

import asyncio
import logging
import signal
import sys
from pathlib import Path

import grpc
from grpc import aio

from .proto import ai_core_pb2, ai_core_pb2_grpc
from .ai_core_service import AICoreService
from .config import settings

# Set up logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    datefmt="%H:%M:%S",
)
logger = logging.getLogger("ai-core")


async def serve():
    """Start the gRPC server and wait for shutdown."""
    server = aio.server(
        options=[
            ("grpc.max_send_message_length", 50 * 1024 * 1024),  # 50MB
            ("grpc.max_receive_message_length", 50 * 1024 * 1024),
        ]
    )
    
    # Register our service
    service = AICoreService()
    ai_core_pb2_grpc.add_AICoreServicer_to_server(service, server)
    
    # Bind to port
    address = f"{settings.orchestrator_host}:{settings.orchestrator_port}"
    server.add_insecure_port(address)
    
    logger.info(f"Starting AI Core gRPC server on {address}")
    logger.info(f"Default provider: {settings.default_provider}")
    
    # Check which providers are available
    status = await service.router.check_all_providers()
    available = [name for name, ok in status.items() if ok]
    
    if not available:
        logger.error("NO LLM PROVIDERS CONFIGURED!")
        logger.error("Set at least one in your .env file:")
        logger.error("  ANTHROPIC_API_KEY=sk-ant-...")
        logger.error("  OPENAI_API_KEY=sk-...")
        logger.error("  DEEPSEEK_API_KEY=sk-...")
        logger.error("Or install Ollama for free local models:")
        logger.error("  curl -fsSL https://ollama.com/install.sh | sh")
        logger.error("  ollama pull llama3.1:8b")
    else:
        logger.info(f"Available providers: {', '.join(available)}")
    
    await server.start()
    logger.info("Server started. Waiting for connections...")
    
    # Wait for shutdown signal
    stop_event = asyncio.Event()
    
    def signal_handler():
        logger.info("Shutdown signal received")
        stop_event.set()
    
    for sig in (signal.SIGINT, signal.SIGTERM):
        asyncio.get_event_loop().add_signal_handler(sig, signal_handler)
    
    await stop_event.wait()
    
    logger.info("Shutting down...")
    await server.stop(grace=5)
    logger.info("Server stopped")


def main():
    """Entry point."""
    try:
        asyncio.run(serve())
    except KeyboardInterrupt:
        logger.info("Interrupted")
    except Exception as e:
        logger.error(f"Fatal error: {e}", exc_info=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
