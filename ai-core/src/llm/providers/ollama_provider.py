"""Ollama Provider for local models — no API key needed."""

from typing import AsyncIterator
from openai import AsyncOpenAI
from ..types import LLMRequest, LLMResponse, MessageRole


class OllamaProvider:
    def __init__(self, config: dict):
        self.endpoint = config.get("endpoint", "http://localhost:11434")
        self.default_model = config.get("default_model", "llama3.1:8b")
        self._client = None

    @property
    def client(self):
        if self._client is None:
            self._client = AsyncOpenAI(
                api_key="ollama",
                base_url=f"{self.endpoint}/v1",
            )
        return self._client

    def _convert_messages(self, request: LLMRequest) -> list[dict]:
        converted = []
        system = request.system_prompt or "You are Shadowlynx ProX, an expert AI assistant."
        converted.append({"role": "system", "content": system})
        for msg in request.messages:
            converted.append({"role": msg.role.value, "content": msg.content})
        return converted

    async def complete(self, request: LLMRequest) -> LLMResponse:
        messages = self._convert_messages(request)
        try:
            response = await self.client.chat.completions.create(
                model=request.model or self.default_model,
                messages=messages,
                max_tokens=request.max_tokens,
                temperature=request.temperature,
            )
        except Exception as e:
            raise RuntimeError(
                f"Ollama connection failed. Is Ollama running?\n"
                f"  Install: curl -fsSL https://ollama.com/install.sh | sh\n"
                f"  Pull model: ollama pull {self.default_model}\n"
                f"  Start: ollama serve\n  Original error: {e}"
            )
        choice = response.choices[0]
        return LLMResponse(
            content=choice.message.content or "",
            model=response.model,
            provider=None,
            input_tokens=response.usage.prompt_tokens if response.usage else 0,
            output_tokens=response.usage.completion_tokens if response.usage else 0,
            finish_reason=choice.finish_reason or "stop",
        )

    async def stream(self, request: LLMRequest) -> AsyncIterator[LLMResponse]:
        messages = self._convert_messages(request)
        try:
            stream = await self.client.chat.completions.create(
                model=request.model or self.default_model,
                messages=messages,
                max_tokens=request.max_tokens,
                temperature=request.temperature,
                stream=True,
            )
        except Exception as e:
            raise RuntimeError(f"Ollama streaming failed: {e}\nIs Ollama running? Run: ollama serve")
        async for chunk in stream:
            if chunk.choices and chunk.choices[0].delta.content:
                yield LLMResponse(
                    content=chunk.choices[0].delta.content,
                    model=chunk.model,
                    provider=None,
                    finish_reason="",
                )
        yield LLMResponse(content="", model=request.model or self.default_model, provider=None, finish_reason="stop")
