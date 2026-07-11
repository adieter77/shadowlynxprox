"""
Type definitions for the LLM layer.

These are the data structures that flow through the system.
Every provider must accept LLMRequest and return LLMResponse.
"""

from dataclasses import dataclass, field
from typing import Optional, Literal, Any
from enum import Enum


class ProviderType(str, Enum):
    ANTHROPIC = "anthropic"
    OPENAI = "openai"
    DEEPSEEK = "deepseek"
    OLLAMA = "ollama"


class MessageRole(str, Enum):
    SYSTEM = "system"
    USER = "user"
    ASSISTANT = "assistant"


@dataclass
class Message:
    """A single message in a conversation."""
    role: MessageRole
    content: str


@dataclass
class ToolDefinition:
    """Definition of a tool/function the AI can call."""
    name: str
    description: str
    parameters: dict  # JSON Schema for the parameters


@dataclass
class ToolCall:
    """A tool call made by the AI."""
    id: str
    name: str
    arguments: dict


@dataclass
class LLMRequest:
    """Unified request format for all LLM providers."""
    messages: list[Message]
    model: Optional[str] = None
    provider: Optional[ProviderType] = None
    system_prompt: Optional[str] = None
    max_tokens: int = 4096
    temperature: float = 0.7
    tools: list[ToolDefinition] = field(default_factory=list)
    stream: bool = True
    # Extra parameters passed directly to the provider
    extra: dict[str, Any] = field(default_factory=dict)


@dataclass
class LLMResponse:
    """Unified response format from all LLM providers."""
    content: str
    model: str
    provider: ProviderType
    input_tokens: int = 0
    output_tokens: int = 0
    tool_calls: list[ToolCall] = field(default_factory=list)
    finish_reason: str = "stop"
    # How long the API call took
    latency_ms: float = 0.0
