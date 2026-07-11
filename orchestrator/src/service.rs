// OrchestratorService — implements the gRPC Orchestrator trait
//
// This handles incoming requests from the CLI and coordinates
// with the AI core and plugins.

use std::pin::Pin;
use std::time::Instant;

use async_trait::async_trait;
use futures_core::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::info;

use crate::orchestrator_proto::{
    orchestrator_server::Orchestrator, ChatRequest, ChatResponse, ExecuteRequest, ExecuteResponse,
    GetInfoRequest, GetInfoResponse, HealthCheckRequest, HealthCheckResponse, TokenUsage,
};

/// The main orchestrator service
#[derive(Debug)]
pub struct OrchestratorService {
    start_time: Instant,
}

impl OrchestratorService {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    /// Process a single chat message and return a response
    /// This will eventually call out to the Python AI core
    async fn process_chat(&self, request: &ChatRequest) -> Result<String, String> {
        let message = &request.message;

        // For now, we handle basic commands locally
        // Later this will route to the appropriate AI model
        let response = match message.to_lowercase().as_str() {
            m if m.contains("hello") || m.contains("hi") => {
                format!(
                    "Shadowlynx ProX is online and ready.\n\n\
                    Orchestrator v{}\n\
                    Conversation ID: {}\n\n\
                    What would you like me to do?",
                    env!("CARGO_PKG_VERSION"),
                    request.conversation_id,
                )
            }
            m if m.contains("version") => {
                format!(
                    "Shadowlynx ProX v{}\n\n\
                    Components:\n\
                    - CLI: Go 1.22+\n\
                    - Orchestrator: Rust {}\n\
                    - AI Core: Python (coming soon)\n\
                    \n\
                    Build: development",
                    env!("CARGO_PKG_VERSION"),
                    env!("CARGO_PKG_VERSION"),
                )
            }
            m if m.contains("scan") => {
                format!(
                    "[PLACEHOLDER] Scan request received.\n\n\
                    The security scanning plugin will handle this.\n\
                    Target: {}\n\
                    \n\
                    This feature is being built. Currently available:\n\
                    - Port scanning (SYN, connect, UDP)\n\
                    - Service fingerprinting\n\
                    - Web fuzzing\n\
                    \n\
                    Full plugin integration coming in Phase 3.",
                    request.message
                )
            }
            m if m.contains("payload") || m.contains("exploit") => {
                "[PLACEHOLDER] Payload generation request received.\n\n\
                    The offensive security plugin will generate:\n\
                    - Reverse shells (Python, Bash, PowerShell, Go, Rust)\n\
                    - Staged and stageless payloads\n\
                    - Custom shellcode\n\n\
                    Full integration coming in Phase 3."
                    .to_string()
            }
            m if m.contains("help") => "Shadowlynx ProX capabilities:\n\n\
                1. Security Operations\n\
                   - Port scanning & service enumeration\n\
                   - Vulnerability assessment\n\
                   - Exploit generation & execution\n\
                   - Payload crafting\n\n\
                2. Software Engineering\n\
                   - Multi-language code generation\n\
                   - Code review & refactoring\n\
                   - CI/CD pipeline generation\n\n\
                3. Blockchain\n\
                   - Smart contract auditing\n\
                   - Wallet management\n\
                   - On-chain analysis\n\n\
                4. General\n\
                   - Natural language understanding\n\
                   - File & code analysis\n\
                   - Research & documentation\n\n\
                Type any request and I'll handle it."
                .to_string(),
            _ => {
                format!(
                    "I received: \"{}\"\n\n\
                    [Note: The AI reasoning core is not connected yet.\n\
                    Full natural language responses with AI will be available\n\
                    once the Python AI core integration is complete.\n\
                    This is the Rust orchestrator responding directly.]\n\n\
                    In the meantime, try these commands:\n\
                    - 'help' for capabilities\n\
                    - 'version' for version info\n\
                    - 'scan ...' for security operations\n\
                    - 'payload ...' for exploit generation",
                    request.message
                )
            }
        };

        Ok(response)
    }
}

#[async_trait]
impl Orchestrator for OrchestratorService {
    /// Streaming chat — the AI responds token by token
    type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatResponse, Status>> + Send>>;

    async fn chat(
        &self,
        request: Request<ChatRequest>,
    ) -> Result<Response<Self::ChatStream>, Status> {
        let req = request.into_inner();
        let conversation_id = if req.conversation_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            req.conversation_id.clone()
        };

        info!(
            conversation_id = %conversation_id,
            message = %req.message,
            "Chat request received"
        );

        // Process the message
        let response_text = self
            .process_chat(&req)
            .await
            .unwrap_or_else(|e| format!("Error: {}", e));

        // Create a channel to simulate streaming
        let (tx, rx) = mpsc::channel(4);
        let cid = conversation_id.clone();

        tokio::spawn(async move {
            // Split the response into chunks to simulate streaming
            // In production, these would come from the LLM token by token
            let words: Vec<&str> = response_text.split(' ').collect();
            let chunk_size = 3; // Send 3 words at a time
            let total_words = words.len();

            for (i, chunk) in words.chunks(chunk_size).enumerate() {
                let text = chunk.join(" ") + " ";
                let is_last = (i + 1) * chunk_size >= total_words;

                let response = ChatResponse {
                    text_chunk: text,
                    is_final: is_last,
                    conversation_id: if i == 0 { cid.clone() } else { String::new() },
                    token_usage: if is_last {
                        Some(TokenUsage {
                            input_tokens: 10,
                            output_tokens: total_words as i64,
                            cost_usd: 0.0,
                        })
                    } else {
                        None
                    },
                    tool_calls: vec![],
                    error: String::new(),
                };

                if tx.send(Ok(response)).await.is_err() {
                    break; // Client disconnected
                }

                // Small delay to simulate streaming
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        });

        // Convert mpsc receiver to a stream
        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    /// One-shot execution
    async fn execute(
        &self,
        request: Request<ExecuteRequest>,
    ) -> Result<Response<ExecuteResponse>, Status> {
        let req = request.into_inner();
        let start = Instant::now();

        info!(
            prompt = %req.prompt,
            exec_type = ?req.execution_type,
            "Execute request received"
        );

        // Process the command (same logic as chat for now)
        let chat_req = ChatRequest {
            conversation_id: String::new(),
            message: req.prompt.clone(),
            workspace_path: req.workspace_path,
            ..Default::default()
        };

        let result = self
            .process_chat(&chat_req)
            .await
            .unwrap_or_else(|e| format!("Error: {}", e));

        let execution_time_ms = start.elapsed().as_millis() as i64;

        let response = ExecuteResponse {
            result,
            findings: vec![],
            token_usage: Some(TokenUsage {
                input_tokens: 10,
                output_tokens: 50,
                cost_usd: 0.0,
            }),
            execution_time_ms,
            error: String::new(),
            success: true,
        };

        Ok(Response::new(response))
    }

    /// Health check
    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let uptime = self.start_time.elapsed().as_secs() as i64;

        let mut checks = std::collections::HashMap::new();
        checks.insert("orchestrator".to_string(), "ok".to_string());
        checks.insert("gRPC".to_string(), "serving".to_string());

        Ok(Response::new(HealthCheckResponse {
            healthy: true,
            status: "serving".to_string(),
            uptime_seconds: uptime,
            checks,
        }))
    }

    /// Get orchestrator info
    async fn get_info(
        &self,
        _request: Request<GetInfoRequest>,
    ) -> Result<Response<GetInfoResponse>, Status> {
        Ok(Response::new(GetInfoResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_commit: "development".to_string(),
            build_date: chrono::Utc::now().to_rfc3339(),
            available_models: vec![
                "claude-sonnet-4".to_string(),
                "gpt-5".to_string(),
                "deepseek-v4".to_string(),
                "ollama:llama3".to_string(),
            ],
            available_plugins: vec![
                "port_scanner".to_string(),
                "payload_generator".to_string(),
                "smart_contract_auditor".to_string(),
            ],
            max_context_tokens: 200000,
        }))
    }
}
