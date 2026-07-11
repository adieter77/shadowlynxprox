fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile(&["../proto/orchestrator.proto"], &["../proto"])?;

    println!("cargo:rerun-if-changed=../proto/orchestrator.proto");
    Ok(())
}
