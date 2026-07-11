"""
Configuration system for Shadowlynx ProX AI Core.

Reads settings from:
1. Environment variables (.env file)
2. Default values

Uses pydantic-settings for validation and type safety.
"""

import os
from pathlib import Path
from typing import Optional, Literal
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Root configuration for the AI Core."""

    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        case_sensitive=False,
        extra="ignore",
    )

    # ---- LLM Provider API Keys ----
    anthropic_api_key: str = ""
    anthropic_default_model: str = "claude-sonnet-4-20250514"

    openai_api_key: str = ""
    openai_default_model: str = "gpt-5"

    deepseek_api_key: str = ""
    deepseek_default_model: str = "deepseek-chat"

    # Local models via Ollama (free, runs on your machine)
    ollama_endpoint: str = "http://localhost:11434"
    ollama_default_model: str = "llama3.1:8b"

    # ---- Default Provider ----
    default_provider: Literal["anthropic", "openai", "deepseek", "ollama"] = "ollama"

    # ---- Server ----
    orchestrator_host: str = "0.0.0.0"
    orchestrator_port: int = 50051

    # ---- Memory ----
    redis_url: str = "redis://localhost:6379"
    chroma_persist_dir: str = "../data/chromadb"

    # ---- Security ----
    vault_path: str = "../data/vault"
    encryption_key: str = ""

    # ---- Performance ----
    max_context_tokens: int = 200000
    request_timeout_seconds: int = 120
    max_retries: int = 3

    def get_provider_config(self, provider: str) -> dict:
        """Get the full configuration for a specific provider."""
        providers = {
            "anthropic": {
                "api_key": self.anthropic_api_key,
                "default_model": self.anthropic_default_model,
            },
            "openai": {
                "api_key": self.openai_api_key,
                "default_model": self.openai_default_model,
            },
            "deepseek": {
                "api_key": self.deepseek_api_key,
                "default_model": self.deepseek_default_model,
                "endpoint": "https://api.deepseek.com",
            },
            "ollama": {
                "endpoint": self.ollama_endpoint,
                "default_model": self.ollama_default_model,
            },
        }
        return providers.get(provider, {})

    def is_provider_configured(self, provider: str) -> bool:
        """Check if a provider has the necessary credentials."""
        config = self.get_provider_config(provider)
        if provider == "ollama":
            return bool(config.get("endpoint"))
        return bool(config.get("api_key"))


# Global settings instance
settings = Settings()
