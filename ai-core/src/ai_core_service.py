"""
AI Core gRPC Service — implements the AICore service defined in ai_core.proto.

This is the server that the Rust orchestrator connects to.
It wraps the LLM Router and exposes it over gRPC.
"""

import asyncio
import logging
from typing import AsyncIterator

import grpc
from grpc import aio

from .proto import ai_core_pb2, ai_core_pb2_grpc
from .llm.router import LLMRouter
from .llm.types import (
    LLMRequest, 
    LLMResponse, 
    Message, 
    MessageRole,
    ProviderType,
)
from .config import settings

logger = logging.getLogger(__name__)


class AICoreService(ai_core_pb2_grpc.AICoreServicer):
    """
    gRPC service implementing the AICore interface.
    
    The Rust orchestrator calls these methods to get AI-generated responses.
    """
    
    def __init__(self):
        self.router = LLMRouter()
        self._startup_time = asyncio.get_event_loop().time()
    
    async def ChatCompletion(
        self, 
        request: ai_core_pb2.ChatCompletionRequest,
        context: grpc.aio.ServicerContext,
    ) -> AsyncIterator[ai_core_pb2.ChatCompletionResponse]:
        """
        Streaming chat completion.
        
        The Rust orchestrator opens this stream and receives AI-generated
        text chunks as they're produced.
        """
        logger.info(
            f"ChatCompletion request: conv={request.conversation_id}, "
            f"provider={request.provider or 'default'}"
        )
        
        try:
            # Convert proto messages to our internal types
            llm_request = self._proto_to_llm_request(request)
            
            # Stream from the LLM
            async for chunk in self.router.stream(llm_request):
                yield ai_core_pb2.ChatCompletionResponse(
                    text_chunk=chunk.content,
                    is_final=(chunk.finish_reason != ""),
                    input_tokens=chunk.input_tokens,
                    output_tokens=chunk.output_tokens,
                    model=chunk.model,
                    provider=chunk.provider.value if chunk.provider else "",
                )
        
        except Exception as e:
            logger.error(f"ChatCompletion error: {e}", exc_info=True)
            yield ai_core_pb2.ChatCompletionResponse(
                text_chunk="",
                is_final=True,
                error=str(e),
            )
    
    async def Completion(
        self,
        request: ai_core_pb2.CompletionRequest,
        context: grpc.aio.ServicerContext,
    ) -> ai_core_pb2.CompletionResponse:
        """
        Non-streaming completion.
        
        Waits for the full response before returning.
        """
        logger.info(f"Completion request: provider={request.provider or 'default'}")
        
        try:
            llm_request = LLMRequest(
                messages=self._proto_messages_to_internal(request.messages),
                system_prompt=request.system_prompt or None,
                model=request.model or None,
                provider=ProviderType(request.provider) if request.provider else None,
                max_tokens=request.max_tokens or 4096,
                temperature=request.temperature or 0.7,
                stream=False,
            )
            
            response = await self.router.complete(llm_request)
            
            return ai_core_pb2.CompletionResponse(
                content=response.content,
                model=response.model,
                provider=response.provider.value if response.provider else "",
                input_tokens=response.input_tokens,
                output_tokens=response.output_tokens,
            )
        
        except Exception as e:
            logger.error(f"Completion error: {e}", exc_info=True)
            return ai_core_pb2.CompletionResponse(
                content="",
                error=str(e),
            )
    
    async def GetProviders(
        self,
        request: ai_core_pb2.GetProvidersRequest,
        context: grpc.aio.ServicerContext,
    ) -> ai_core_pb2.GetProvidersResponse:
        """Return which LLM providers are configured and available."""
        status = await self.router.check_all_providers()
        
        providers = []
        any_available = False
        
        for name, available in status.items():
            config = settings.get_provider_config(name)
            providers.append(ai_core_pb2.ProviderInfo(
                name=name,
                available=available,
                default_model=config.get("default_model", ""),
            ))
            if available:
                any_available = True
        
        return ai_core_pb2.GetProvidersResponse(
            providers=providers,
            any_available=any_available,
        )
    
    async def Health(
        self,
        request: ai_core_pb2.HealthRequest,
        context: grpc.aio.ServicerContext,
    ) -> ai_core_pb2.HealthResponse:
        """Health check."""
        status = await self.router.check_all_providers()
        
        providers = []
        any_available = False
        for name, available in status.items():
            config = settings.get_provider_config(name)
            providers.append(ai_core_pb2.ProviderInfo(
                name=name,
                available=available,
                default_model=config.get("default_model", ""),
            ))
            if available:
                any_available = True
        
        return ai_core_pb2.HealthResponse(
            healthy=any_available,
            status="serving" if any_available else "no_providers",
            providers=providers,
        )
    
    # ---- Helper methods ----
    
    def _proto_to_llm_request(
        self, 
        request: ai_core_pb2.ChatCompletionRequest
    ) -> LLMRequest:
        """Convert a proto ChatCompletionRequest to our internal LLMRequest."""
        return LLMRequest(
            messages=self._proto_messages_to_internal(request.messages),
            system_prompt=request.system_prompt or None,
            model=request.model or None,
            provider=ProviderType(request.provider) if request.provider else None,
            max_tokens=request.max_tokens or 4096,
            temperature=request.temperature or 0.7,
            stream=True,
        )
    
    def _proto_messages_to_internal(
        self, 
        proto_messages: list[ai_core_pb2.ChatMessage]
    ) -> list[Message]:
        """Convert proto ChatMessage list to internal Message list."""
        messages = []
        for pm in proto_messages:
            role = MessageRole.USER
            if pm.role == "system":
                role = MessageRole.SYSTEM
            elif pm.role == "assistant":
                role = MessageRole.ASSISTANT
            
            messages.append(Message(role=role, content=pm.content))
        return messages
