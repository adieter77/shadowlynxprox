// ============================================================
// Plugin gRPC Service
// ============================================================

use tonic::{Request, Response, Status};
use std::sync::Arc;
use crate::plugin::runtime::PluginRuntime;
use crate::plugin_proto as pb;
use crate::plugin_proto::plugin_service_server::PluginService;
use pb::plugin_service_server::PluginServiceServer;

pub struct PluginServiceImpl {
    runtime: Arc<PluginRuntime>,
}

impl PluginServiceImpl {
    pub fn new(runtime: Arc<PluginRuntime>) -> Self {
        PluginServiceImpl { runtime }
    }
}

fn capability_level_to_int(level: &crate::plugin::manifest::CapabilityLevel) -> i32 {
    use crate::plugin::manifest::CapabilityLevel;
    match level {
        CapabilityLevel::None => 0,
        CapabilityLevel::ReadOnly => 1,
        CapabilityLevel::ReadWrite => 2,
        CapabilityLevel::Full => 3,
    }
}

fn int_to_capability_level(v: i32) -> crate::plugin::manifest::CapabilityLevel {
    use crate::plugin::manifest::CapabilityLevel;
    match v {
        1 => CapabilityLevel::ReadOnly,
        2 => CapabilityLevel::ReadWrite,
        3 => CapabilityLevel::Full,
        _ => CapabilityLevel::None,
    }
}

fn manifest_to_proto(m: &crate::plugin::manifest::PluginManifest) -> pb::PluginManifest {
    pb::PluginManifest {
        id: m.id.clone(),
        name: m.name.clone(),
        version: m.version.clone(),
        description: m.description.clone(),
        author: m.author.clone(),
        capabilities: m.capabilities.iter().map(|c| pb::Capability {
            name: c.name.clone(),
            level: capability_level_to_int(&c.level),
            scope: c.scope.clone(),
            reason: c.reason.clone(),
        }).collect(),
        fuel_budget: m.fuel_budget as i32,
        timeout_ms: m.timeout_ms as i32,
        memory_mb: m.memory_mb as i32,
        wasm_hash: m.wasm_hash.clone(),
    }
}

#[tonic::async_trait]
impl PluginService for PluginServiceImpl {
    async fn load_plugin(
        &self,
        request: Request<pb::LoadPluginRequest>,
    ) -> Result<Response<pb::LoadPluginResponse>, Status> {
        let req = request.into_inner();
        let manifest_msg = req.manifest
            .ok_or_else(|| Status::invalid_argument("manifest is required"))?;

        let capabilities = manifest_msg.capabilities.iter().map(|c| {
            crate::plugin::manifest::Capability {
                name: c.name.clone(),
                level: int_to_capability_level(c.level),
                scope: c.scope.clone(),
                reason: c.reason.clone(),
            }
        }).collect();

        let manifest = crate::plugin::manifest::PluginManifest {
            id: manifest_msg.id,
            name: manifest_msg.name,
            version: manifest_msg.version,
            description: manifest_msg.description,
            author: manifest_msg.author,
            capabilities,
            fuel_budget: manifest_msg.fuel_budget as u64,
            timeout_ms: manifest_msg.timeout_ms as u64,
            memory_mb: manifest_msg.memory_mb as u64,
            wasm_hash: manifest_msg.wasm_hash,
        };

        let wasm_bytes = match req.source {
            Some(pb::load_plugin_request::Source::FilePath(file_path)) => {
                std::fs::read(&file_path)
                    .map_err(|e| Status::not_found(format!("Cannot read plugin file: {}", e)))?
            }
            Some(pb::load_plugin_request::Source::WasmBytes(bytes)) => bytes,
            None => return Err(Status::invalid_argument("source is required")),
        };

        match self.runtime.load_plugin(&wasm_bytes, &manifest, None).await {
            Ok(()) => Ok(Response::new(pb::LoadPluginResponse {
                success: true,
                plugin_id: manifest.id,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(pb::LoadPluginResponse {
                success: false,
                plugin_id: manifest.id,
                error: e,
            })),
        }
    }

    async fn execute_tool(
        &self,
        request: Request<pb::ExecuteToolRequest>,
    ) -> Result<Response<pb::ExecuteToolResponse>, Status> {
        let req = request.into_inner();
        match self.runtime.execute_tool(&req.plugin_id, &req.tool_name, &req.arguments_json).await {
            Ok(r) => Ok(Response::new(pb::ExecuteToolResponse {
                result: Some(pb::ToolResult {
                    success: r.success,
                    output: r.output,
                    error: r.error.unwrap_or_default(),
                    duration_ms: r.duration_ms as i64,
                    fuel_consumed: r.fuel_consumed as i64,
                }),
            })),
            Err(e) => Ok(Response::new(pb::ExecuteToolResponse {
                result: Some(pb::ToolResult {
                    success: false,
                    output: String::new(),
                    error: e,
                    duration_ms: 0,
                    fuel_consumed: 0,
                }),
            })),
        }
    }

    async fn list_plugins(
        &self,
        _request: Request<pb::ListPluginsRequest>,
    ) -> Result<Response<pb::ListPluginsResponse>, Status> {
        let plugins = self.runtime.list_plugins().await;
        let mut infos = Vec::new();
        for (id, manifest, tools) in plugins {
            let tools_proto = tools.iter().map(|t| pb::PluginTool {
                name: t.name.clone(),
                description: t.description.clone(),
                json_schema: t.json_schema.clone(),
                plugin_id: id.clone(),
            }).collect();
            infos.push(pb::PluginInfo {
                plugin_id: id.clone(),
                manifest: Some(manifest_to_proto(&manifest)),
                state: 1, // LOADED
                tools: tools_proto,
            });
        }
        Ok(Response::new(pb::ListPluginsResponse { plugins: infos }))
    }

    async fn get_plugin(
        &self,
        request: Request<pb::GetPluginRequest>,
    ) -> Result<Response<pb::GetPluginResponse>, Status> {
        let req = request.into_inner();
        match self.runtime.get_plugin(&req.plugin_id).await {
            Some((manifest, tools)) => {
                let tools_proto = tools.iter().map(|t| pb::PluginTool {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    json_schema: t.json_schema.clone(),
                    plugin_id: req.plugin_id.clone(),
                }).collect();
                Ok(Response::new(pb::GetPluginResponse {
                    plugin: Some(pb::PluginInfo {
                        plugin_id: req.plugin_id.clone(),
                        manifest: Some(manifest_to_proto(&manifest)),
                        state: 1,
                        tools: tools_proto,
                    }),
                    found: true,
                }))
            }
            None => Ok(Response::new(pb::GetPluginResponse {
                plugin: None,
                found: false,
            })),
        }
    }

    async fn unload_plugin(
        &self,
        request: Request<pb::UnloadPluginRequest>,
    ) -> Result<Response<pb::UnloadPluginResponse>, Status> {
        let req = request.into_inner();
        match self.runtime.unload_plugin(&req.plugin_id).await {
            Ok(()) => Ok(Response::new(pb::UnloadPluginResponse {
                success: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(pb::UnloadPluginResponse {
                success: false,
                error: e,
            })),
        }
    }

    async fn validate_plugin(
        &self,
        request: Request<pb::ValidatePluginRequest>,
    ) -> Result<Response<pb::ValidatePluginResponse>, Status> {
        let req = request.into_inner();
        match PluginRuntime::validate_wasm(&req.wasm_bytes) {
            Ok((manifest, tools)) => {
                let tools_proto = tools.iter().map(|t| pb::PluginTool {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    json_schema: t.json_schema.clone(),
                    plugin_id: manifest.id.clone(),
                }).collect();
                Ok(Response::new(pb::ValidatePluginResponse {
                    valid: true,
                    error: String::new(),
                    tools: tools_proto,
                    manifest: Some(manifest_to_proto(&manifest)),
                }))
            }
            Err(e) => Ok(Response::new(pb::ValidatePluginResponse {
                valid: false,
                error: e,
                tools: Vec::new(),
                manifest: None,
            })),
        }
    }

    type WatchPluginsStream = tokio_stream::wrappers::ReceiverStream<Result<pb::PluginEvent, Status>>;

    async fn watch_plugins(
        &self,
        _request: Request<pb::ListPluginsRequest>,
    ) -> Result<Response<Self::WatchPluginsStream>, Status> {
        Err(Status::unimplemented("watch_plugins streaming not yet implemented"))
    }
}
