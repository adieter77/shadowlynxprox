fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = &[
        "proto/orchestrator.proto",
        "proto/ai_core.proto",
        "proto/plugin.proto",
    ];
    let include_dirs = &["proto"];

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(proto_files, include_dirs)?;

    for file in proto_files {
        println!("cargo:rerun-if-changed={}", file);
    }
    Ok(())
}
