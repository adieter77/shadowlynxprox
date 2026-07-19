// ============================================================
// Plugin Sandbox Configuration
// ============================================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SandboxMode {
    WasmNative,
    Docker,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkMode {
    None,
    Loopback,
    Outbound,
    Full,
    Custom(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsAccess {
    pub read_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub allow_list: bool,
}

impl Default for FsAccess {
    fn default() -> Self {
        FsAccess {
            read_paths: vec!["/tmp/slpx/*".into()],
            write_paths: vec!["/tmp/slpx/*".into()],
            allow_list: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub mode: SandboxMode,
    pub network: NetworkMode,
    pub filesystem: FsAccess,
    pub fuel_budget: u64,
    pub timeout_ms: u64,
    pub memory_mb: u64,
    pub allow_exec: bool,
    pub env_vars: Vec<(String, String)>,
    pub docker_image: String,
    pub seccomp_profile: String,
    pub read_only_rootfs: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        SandboxConfig {
            mode: SandboxMode::WasmNative,
            network: NetworkMode::None,
            filesystem: FsAccess::default(),
            fuel_budget: 100_000_000,
            timeout_ms: 30_000,
            memory_mb: 64,
            allow_exec: false,
            env_vars: Vec::new(),
            docker_image: "alpine:latest".into(),
            seccomp_profile: String::new(),
            read_only_rootfs: true,
        }
    }
}
