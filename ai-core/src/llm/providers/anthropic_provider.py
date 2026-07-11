"""Anthropic Claude Provider — uses the official anthropic Python SDK."""

from typing import AsyncIterator
import anthropic
from ..types import LLMRequest, LLMResponse, Message, MessageRole, ToolCall


class AnthropicProvider:
    def __init__(self, config: dict):
        self.api_key = config.get("api_key", "")
        self.default_model = config.get("default_model", "claude-sonnet-4-20250514")
        self._client = None

    @property
    def client(self):
        if self._client is None:
            if not self.api_key:
                raise ValueError("ANTHROPIC_API_KEY not set in .env")
            self._client = anthropic.AsyncAnthropic(api_key=self.api_key)
        return self._client

    def _build_system_prompt(self, request: LLMRequest) -> str:
        return request.system_prompt or "You are Shadowlynx ProX, an expert AI assistant."

    def _convert_messages(self, messages: list[Message]) -> list[dict]:
        converted = []
        for msg in messages:
            if msg.role == MessageRole.SYSTEM:
                continue
            converted.append({"role": msg.role.value, "content": msg.content})
        return converted

    async def complete(self, request: LLMRequest) -> LLMResponse:
        system = self._build_system_prompt(request)
        messages = self._convert_messages(request.messages)
        response = await self.client.messages.create(
            model=request.model or self.default_model,
            system=system,
            messages=messages,
            max_tokens=request.max_tokens,
            temperature=request.temperature,
        )
        text_blocks = [block.text for block in response.content if block.type == "text"]
        content = "\n".join(text_blocks)
        return LLMResponse(
            content=content,
            model=response.model,
            provider=None,
            input_tokens=response.usage.input_tokens,
            output_tokens=response.usage.output_tokens,
            finish_reason=response.stop_reason or "stop",
        )

    async def stream(self, request: LLMRequest) -> AsyncIterator[LLMResponse]:
        system = self._build_system_prompt(request)
        messages = self._convert_messages(request.messages)
        accumulated = ""
        async with self.client.messages.stream(
            model=request.model or self.default_model,
            system=system,
            messages=messages,
            max_tokens=request.max_tokens,
            temperature=request.temperature,
        ) as stream:
            async for event in stream:
                if event.type == "content_block_delta":
                    if event.delta.type == "text_delta":
                        text = event.delta.text
                        accumulated += text
                        yield LLMResponse(
                            content=text,
                            model=request.model or self.default_model,
                            provider=None,
                            finish_reason="",
                        )
            final = await stream.get_final_message()
            yield LLMResponse(
                content="",
                model=final.model,
                provider=None,
                input_tokens=final.usage.input_tokens,
                output_tokens=final.usage.output_tokens,
                finish_reason=final.stop_reason or "stop",
            )
