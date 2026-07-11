// Shadowlynx ProX Orchestrator — Main Entry Point
//
// This is the gRPC server that the CLI connects to.
// It handles chat requests, command execution, health checks,
// and eventually will coordinate plugins and the AI core.

mod service;

use tonic::transport::Server;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Shadowlynx ProX Orchestrator starting...");

    // Our gRPC service
    let orchestrator = service::OrchestratorService::new();

    // Start the gRPC server
    let addr = "0.0.0.0:50052".parse()?;
    tracing::info!("gRPC server listening on {}", addr);

    Server::builder()
        .add_service(
            // The generated OrchestratorServer from proto
            orchestrator_proto::orchestrator_server::OrchestratorServer::new(orchestrator),
        )
        .serve(addr)
        .await?;

    Ok(())
}

// Include the generated protobuf code
mod orchestrator_proto {
    tonic::include_proto!("orchestrator");
}
