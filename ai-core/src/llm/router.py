"""
LLM Router — Multi-Provider AI Gateway.

This is the central switchboard. It:
1. Accepts a unified LLMRequest
2. Picks the right provider (Anthropic, OpenAI, DeepSeek, or Ollama)
3. Calls the provider's API
4. Returns a unified LLMResponse

Key design principle: the rest of the system never needs to know
which AI model is being used. It just sends a request and gets a response.
"""

import time
import asyncio
from typing import AsyncIterator, Optional

from ..config import settings
from .types import (
    LLMRequest, 
    LLMResponse, 
    ProviderType, 
    Message, 
    MessageRole,
    ToolCall,
)
from .providers.anthropic_provider import AnthropicProvider
from .providers.openai_provider import OpenAIProvider
from .providers.deepseek_provider import DeepSeekProvider
from .providers.ollama_provider import OllamaProvider


class LLMRouter:
    """
    Routes LLM requests to the appropriate provider.
    
    Usage:
        router = LLMRouter()
        
        # Non-streaming
        response = await router.complete(request)
        print(response.content)
        
        # Streaming
        async for chunk in router.stream(request):
            print(chunk.content, end="")
    """
    
    # Provider classes ordered by priority
    PROVIDERS = {
        ProviderType.ANTHROPIC: AnthropicProvider,
        ProviderType.OPENAI: OpenAIProvider,
        ProviderType.DEEPSEEK: DeepSeekProvider,
        ProviderType.OLLAMA: OllamaProvider,
    }
    
    # Fallback order: if the preferred provider fails, try these
    FALLBACK_ORDER = [
        ProviderType.ANTHROPIC,
        ProviderType.OPENAI,
        ProviderType.DEEPSEEK,
        ProviderType.OLLAMA,
    ]
    
    def __init__(self):
        self._provider_instances = {}
        self._available = None
    
    def _get_provider(self, provider_type: ProviderType):
        """Get or create a provider instance."""
        if provider_type not in self._provider_instances:
            provider_class = self.PROVIDERS[provider_type]
            config = settings.get_provider_config(provider_type.value)
            self._provider_instances[provider_type] = provider_class(config)
        return self._provider_instances[provider_type]
    
    def _is_available(self, provider_type: ProviderType) -> bool:
        """Check if a provider is configured and available."""
        return settings.is_provider_configured(provider_type.value)
    
    async def _check_ollama_health(self) -> bool:
        """Quick health check for Ollama."""
        try:
            import httpx
            async with httpx.AsyncClient() as client:
                resp = await client.get(
                    f"{settings.ollama_endpoint}/api/tags",
                    timeout=5.0
                )
                return resp.status_code == 200
        except Exception:
            return False
    
    async def check_all_providers(self) -> dict[str, bool]:
        """Check which providers are available right now."""
        status = {}
        for provider in ProviderType:
            if not self._is_available(provider):
                status[provider.value] = False
                continue
            if provider == ProviderType.OLLAMA:
                status[provider.value] = await self._check_ollama_health()
            else:
                status[provider.value] = True
        self._available = status
        return status
    
    def _resolve_provider(self, request: LLMRequest) -> ProviderType:
        """
        Decide which provider to use.
        
        Priority:
        1. Request's explicit provider choice
        2. Default provider from settings
        3. First available provider in fallback order
        """
        if request.provider:
            if self._is_available(request.provider):
                return request.provider
            raise ValueError(f"Requested provider '{request.provider}' is not configured")
        
        # Try the default
        default = ProviderType(settings.default_provider)
        if self._is_available(default):
            return default
        
        # Fall back to any available provider
        for provider in self.FALLBACK_ORDER:
            if self._is_available(provider):
                return provider
        
        raise RuntimeError(
            "No LLM provider configured. Set at least one API key in .env:\n"
            "  ANTHROPIC_API_KEY=...\n"
            "  OPENAI_API_KEY=...\n"
            "  DEEPSEEK_API_KEY=...\n"
            "Or install Ollama for free local models."
        )
    
    async def complete(self, request: LLMRequest) -> LLMResponse:
        """
        Send a non-streaming request and get a complete response.
        
        This waits for the entire response before returning.
        Use stream() if you want to display text as it arrives.
        """
        provider_type = self._resolve_provider(request)
        provider = self._get_provider(provider_type)
        
        start = time.monotonic()
        response = await provider.complete(request)
        response.latency_ms = (time.monotonic() - start) * 1000
        response.provider = provider_type
        
        return response
    
    async def stream(self, request: LLMRequest) -> AsyncIterator[LLMResponse]:
        """
        Send a streaming request and yield chunks as they arrive.
        
        Each chunk contains incremental text. The final chunk has
        finish_reason set and token counts populated.
        """
        provider_type = self._resolve_provider(request)
        provider = self._get_provider(provider_type)
        
        start = time.monotonic()
        async for chunk in provider.stream(request):
            chunk.provider = provider_type
            yield chunk
        
        # Note: latency is only meaningful for the full response
        # Individual chunks don't track their own latency
    
    async def complete_with_fallback(self, request: LLMRequest) -> LLMResponse:
        """
        Try the preferred provider, fall back to others on failure.
        
        This is more resilient — if one API is down, it automatically
        tries the next one.
        """
        last_error = None
        
        # Start with the resolved provider and try fallbacks
        try:
            preferred = self._resolve_provider(request)
        except RuntimeError:
            preferred = None
        
        providers_to_try = []
        if preferred:
            providers_to_try.append(preferred)
        providers_to_try.extend([
            p for p in self.FALLBACK_ORDER 
            if p != preferred and self._is_available(p)
        ])
        
        for provider_type in providers_to_try:
            try:
                provider = self._get_provider(provider_type)
                start = time.monotonic()
                response = await provider.complete(request)
                response.latency_ms = (time.monotonic() - start) * 1000
                response.provider = provider_type
                return response
            except Exception as e:
                last_error = e
                continue
        
        raise RuntimeError(
            f"All providers failed. Last error: {last_error}\n"
            f"Providers tried: {[p.value for p in providers_to_try]}"
        )
