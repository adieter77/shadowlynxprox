use std::pin::Pin;
use std::time::Instant;
use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{info, debug, warn, error};

use crate::orchestrator_proto::{
    orchestrator_server::Orchestrator,
    ChatRequest, ChatResponse,
    ExecuteRequest, ExecuteResponse,
    HealthCheckRequest, HealthCheckResponse,
    GetInfoRequest, GetInfoResponse,
    TokenUsage, Finding,
};

use crate::ai_core_proto::{
    ai_core_client::AiCoreClient,
    ChatCompletionRequest, ChatMessage,
};

/// The main orchestrator service
pub struct OrchestratorService {
    start_time: Instant,
    ai_core_addr: String,
}

impl OrchestratorService {
    pub fn new(ai_core_addr: String) -> Self {
        Self {
            start_time: Instant::now(),
            ai_core_addr,
        }
    }

    pub fn with_default() -> Self {
        Self::new("http://127.0.0.1:50051".to_string())
    }

    /// Process a chat message by calling the Python AI Core
    async fn process_chat_with_ai(
        &self,
        request: &ChatRequest,
    ) -> Result<Vec<ChatResponse>, String> {
        // Connect to AI Core
        let mut client = AiCoreClient::connect(self.ai_core_addr.clone())
            .await
            .map_err(|e| format!("Failed to connect to AI Core: {}", e))?;
        
        // Build the request
        let ai_request = ChatCompletionRequest {
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: request.message.clone(),
                }
            ],
            system_prompt: String::new(),
            model: if request.model.is_empty() { String::new() } else { request.model.clone() },
            provider: String::new(), // Use default
            max_tokens: 4096,
            temperature: 0.7,
            conversation_id: if request.conversation_id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                request.conversation_id.clone()
            },
        };
        
        let conv_id = ai_request.conversation_id.clone();
        
        // Stream from AI Core
        let mut stream = client
            .chat_completion(ai_request)
            .await
            .map_err(|e| format!("AI Core call failed: {}", e))?
            .into_inner();
        
        let mut responses = Vec::new();
        let mut accumulated = String::new();
        let mut first = true;
        
        while let Some(chunk) = stream.message().await
            .map_err(|e| format!("Stream error: {}", e))?
        {
            if !chunk.error.is_empty() {
                // If there's an error but we have accumulated text, use that
                if !accumulated.is_empty() {
                    break;
                }
                return Err(chunk.error);
            }
            
            accumulated.push_str(&chunk.text_chunk);
            
            let response = ChatResponse {
                text_chunk: chunk.text_chunk.clone(),
                is_final: chunk.is_final,
                conversation_id: if first {
                    first = false;
                    conv_id.clone()
                } else {
                    String::new()
                },
                token_usage: if chunk.is_final {
                    Some(TokenUsage {
                        input_tokens: chunk.input_tokens as i64,
                        output_tokens: chunk.output_tokens as i64,
                        cost_usd: 0.0,
                    })
                } else {
                    None
                },
                tool_calls: vec![],
                error: String::new(),
            };
            
            responses.push(response);
            
            if chunk.is_final {
                break;
            }
        }
        
        Ok(responses)
    }
    
    /// Fallback: process locally when AI Core is unavailable
    fn process_chat_local(&self, request: &ChatRequest) -> Vec<ChatResponse> {
        let message = &request.message;
        let response_text = match message.to_lowercase().as_str() {
            m if m.contains("hello") || m.contains("hi") => {
                format!(
                    "Shadowlynx ProX is online.\n\n🟡 AI Core not connected.\n\nSet up the Python AI Core for full capabilities:\n1. cd ai-core && source .venv/bin/activate\n2. python -m src.main\n\nThen restart the orchestrator.\n\nConversation ID: {}\n\nWhat would you like me to do?",
                    request.conversation_id,
                )
            }
            m if m.contains("version") => {
                format!(
                    "Shadowlynx ProX v{}\n\nComponents:\n- CLI: Go\n- Orchestrator: Rust v{}\n- AI Core: Python (checking...)\n\nBuild: development",
                    env!("CARGO_PKG_VERSION"),
                    env!("CARGO_PKG_VERSION"),
                )
            }
            _ => {
                format!(
                    "🟡 AI Core not connected.\n\nReceived: \"{}\"\n\nTo enable full AI capabilities:\n1. cd ai-core && source .venv/bin/activate\n2. Configure an LLM provider in .env\n3. python -m src.main\n4. Restart the orchestrator",
                    message
                )
            }
        };
        
        let words: Vec<&str> = response_text.split(' ').collect();
        let mut responses = Vec::new();
        let cid = request.conversation_id.clone();
        
        for (i, chunk) in words.chunks(3).enumerate() {
            let text = chunk.join(" ") + " ";
            let is_last = (i + 1) * 3 >= words.len();
            
            responses.push(ChatResponse {
                text_chunk: text,
                is_final: is_last,
                conversation_id: if i == 0 { cid.clone() } else { String::new() },
                token_usage: if is_last {
                    Some(TokenUsage {
                        input_tokens: 10,
                        output_tokens: words.len() as i64,
                        cost_usd: 0.0,
                    })
                } else {
                    None
                },
                tool_calls: vec![],
                error: String::new(),
            });
        }
        
        responses
    }
}

#[async_trait]
impl Orchestrator for OrchestratorService {
    type ChatStream = Pin<Box<dyn tokio_stream::Stream<Item = Result<ChatResponse, Status>> + Send>>;

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

        let (tx, rx) = mpsc::channel(16);
        let service = OrchestratorService {
            start_time: self.start_time,
            ai_core_addr: self.ai_core_addr.clone(),
        };
        let req_clone = req.clone();

        tokio::spawn(async move {
            // Try AI Core first, fall back to local processing
            let responses = match service.process_chat_with_ai(&req_clone).await {
                Ok(responses) => responses,
                Err(e) => {
                    warn!("AI Core failed: {}. Falling back to local processing.", e);
                    service.process_chat_local(&req_clone)
                }
            };

            for response in responses {
                if tx.send(Ok(response)).await.is_err() {
                    break; // Client disconnected
                }
                // Small delay for streaming effect
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

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

        // Build a chat request from the execute request
        let chat_req = ChatRequest {
            conversation_id: String::new(),
            message: req.prompt.clone(),
            workspace_path: req.workspace_path,
            model: req.model,
            ..Default::default()
        };

        // Process via AI Core
        let result = match self.process_chat_with_ai(&chat_req).await {
            Ok(responses) => {
                let mut text = String::new();
                for r in &responses {
                    text.push_str(&r.text_chunk);
                }
                text
            }
            Err(e) => {
                warn!("AI Core failed for execute: {}. Using local.", e);
                let local = self.process_chat_local(&chat_req);
                let mut text = String::new();
                for r in &local {
                    text.push_str(&r.text_chunk);
                }
                text
            }
        };

        let execution_time_ms = start.elapsed().as_millis() as i64;

        Ok(Response::new(ExecuteResponse {
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
        }))
    }

    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let uptime = self.start_time.elapsed().as_secs() as i64;
        let mut checks = HashMap::new();
        checks.insert("orchestrator".to_string(), "ok".to_string());
        checks.insert("gRPC".to_string(), "serving".to_string());
        
        // Check AI Core availability
        match AiCoreClient::connect(self.ai_core_addr.clone()).await {
            Ok(mut client) => {
                let health_req = crate::ai_core_proto::HealthRequest {};
                match client.health(health_req).await {
                    Ok(resp) => {
                        let h = resp.into_inner();
                        checks.insert("ai_core".to_string(), h.status);
                    }
                    Err(_) => {
                        checks.insert("ai_core".to_string(), "unreachable".to_string());
                    }
                }
            }
            Err(_) => {
                checks.insert("ai_core".to_string(), "unreachable".to_string());
            }
        }
        
        Ok(Response::new(HealthCheckResponse {
            healthy: true,
            status: "serving".to_string(),
            uptime_seconds: uptime,
            checks,
        }))
    }

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
                "deepseek-chat".to_string(),
                "ollama:llama3.1".to_string(),
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
