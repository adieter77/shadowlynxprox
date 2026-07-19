// ============================================================
// Shadowlynx ProX — Rust Orchestrator
// ============================================================
// gRPC server that:
//   1. Routes chat/execute requests to the Python AI Core
//   2. Manages WASM plugins with sandboxed execution
//   3. Handles health checks and service discovery
// ============================================================

mod service;
mod plugin;
mod plugin_service;

// The original tonic-build output uses these module names.
// We rename via the `compile_well_known_types` and use the proto file basenames.
pub mod orchestrator_proto {
    tonic::include_proto!("orchestrator");
}

pub mod ai_core_proto {
    tonic::include_proto!("ai_core");
}

pub mod plugin_proto {
    tonic::include_proto!("plugin");
}

use tonic::transport::Server;
use std::sync::Arc;
use crate::plugin::runtime::PluginRuntime;
use crate::plugin_service::PluginServiceImpl;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::]:50053".parse()?;
    let ai_core_addr = "http://127.0.0.1:50051".to_string();

    // Initialize the WASM plugin runtime
    let plugin_runtime = Arc::new(PluginRuntime::new());

    let orchestrator_service = service::OrchestratorService::new(ai_core_addr);
    let plugin_service = PluginServiceImpl::new(plugin_runtime);

    println!("========================================");
    println!(" Shadowlynx ProX — Orchestrator");
    println!("========================================");
    println!(" gRPC Server:  0.0.0.0:50053");
    println!(" AI Core:      http://127.0.0.1:50051");
    println!(" Plugins:      WASM runtime (wasmtime)");
    println!("========================================");

    // Build the server with both services
    let orchestrator_svc = orchestrator_proto::orchestrator_server::OrchestratorServer::new(orchestrator_service);
    let plugin_svc = plugin_proto::plugin_service_server::PluginServiceServer::new(plugin_service);

    Server::builder()
        .add_service(orchestrator_svc)
        .add_service(plugin_svc)
        .serve(addr)
        .await?;

    Ok(())
}
