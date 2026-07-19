// ============================================================
// Host Functions — What Plugins Can Call
// ============================================================

use once_cell::sync::Lazy;
use wasmtime::*;

use super::manifest::CapabilityLevel;
use super::sandbox::SandboxConfig;

// Global, process-lifetime HTTP client.
// We use a Lazy static (not a per-Store client) because
// `reqwest::blocking::Client` lazily builds an internal tokio
// runtime on first use; if that runtime gets dropped from within
// an async context it panics. By keeping the client in a static
// we ensure the runtime lives for the entire program.
static HTTP_CLIENT: Lazy<reqwest::blocking::Client> = Lazy::new(|| {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Shadowlynx-ProX-Plugin/1.0")
        .build()
        .unwrap_or_default()
});

pub struct HostState {
    pub sandbox_config: SandboxConfig,
    pub plugin_id: String,
    pub capabilities: std::collections::HashMap<String, CapabilityLevel>,
}

impl HostState {
    pub fn new(
        plugin_id: &str,
        sandbox_config: &SandboxConfig,
        capabilities: Vec<(String, CapabilityLevel)>,
    ) -> Self {
        HostState {
            sandbox_config: sandbox_config.clone(),
            plugin_id: plugin_id.to_string(),
            capabilities: capabilities.into_iter().collect(),
        }
    }

    pub fn check_capability(&self, cap: &str, required: CapabilityLevel) -> Result<(), String> {
        match self.capabilities.get(cap) {
            Some(granted) => {
                let ok = match (granted, &required) {
                    (CapabilityLevel::Full, _) => true,
                    (CapabilityLevel::ReadWrite, CapabilityLevel::ReadWrite) => true,
                    (CapabilityLevel::ReadWrite, CapabilityLevel::ReadOnly) => true,
                    (CapabilityLevel::ReadOnly, CapabilityLevel::ReadOnly) => true,
                    _ => false,
                };
                if ok { Ok(()) }
                else { Err(format!("Plugin '{}' lacks capability '{}'", self.plugin_id, cap)) }
            }
            None => Err(format!("Plugin '{}' has no capability '{}'", self.plugin_id, cap)),
        }
    }
}

pub fn register_host_functions(
    linker: &mut Linker<HostState>,
) -> Result<(), Error> {
    // slpx_log(level: i32, ptr: i32, len: i32)
    linker.func_wrap("slpx", "log",
        |mut caller: Caller<'_, HostState>, level: i32, ptr: i32, len: i32| -> Result<(), Error> {
            let mem = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| Error::msg("No memory export"))?;
            let data = mem.data(&caller);
            let slice = &data[ptr as usize..(ptr + len) as usize];
            let msg = String::from_utf8_lossy(slice);
            let level_str = match level {
                0 => "DEBUG", 1 => "INFO", 2 => "WARN", 3 => "ERROR", _ => "UNKNOWN",
            };
            let plugin_id = &caller.data().plugin_id;
            eprintln!("[plugin:{}][{}] {}", plugin_id, level_str, msg);
            Ok(())
        }
    )?;

    // slpx_read_file(path_ptr, path_len, out_ptr) -> i32
    linker.func_wrap("slpx", "read_file",
        |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32, out_ptr: i32| -> Result<i32, Error> {
            caller.data().check_capability("fs:read", CapabilityLevel::ReadOnly)
                .map_err(|e| Error::msg(e))?;
            let mem = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| Error::msg("No memory export"))?;
            let data = mem.data(&caller);
            let path_bytes = &data[path_ptr as usize..(path_ptr + path_len) as usize];
            let path = String::from_utf8_lossy(path_bytes);
            let allowed = caller.data().sandbox_config.filesystem.read_paths.iter()
                .any(|p| path.starts_with(p.trim_end_matches('*')) || p == "*");
            if !allowed {
                return Err(Error::msg(format!("Read access denied: {}", path)));
            }
            match std::fs::read_to_string(path.as_ref()) {
                Ok(content) => {
                    let content_bytes = content.as_bytes();
                    let data_mut = mem.data_mut(&mut caller);
                    let dest = out_ptr as usize;
                    if dest + content_bytes.len() > data_mut.len() { return Ok(-1); }
                    data_mut[dest..dest + content_bytes.len()].copy_from_slice(content_bytes);
                    Ok(content_bytes.len() as i32)
                }
                Err(_) => Ok(-1),
            }
        }
    )?;

    // slpx_write_file(path_ptr, path_len, content_ptr, content_len) -> i32
    linker.func_wrap("slpx", "write_file",
        |mut caller: Caller<'_, HostState>,
         path_ptr: i32, path_len: i32,
         content_ptr: i32, content_len: i32| -> Result<i32, Error> {
            caller.data().check_capability("fs:write", CapabilityLevel::ReadWrite)
                .map_err(|e| Error::msg(e))?;
            let mem = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| Error::msg("No memory export"))?;
            let data = mem.data(&caller);
            let path = {
                let bytes = &data[path_ptr as usize..(path_ptr + path_len) as usize];
                String::from_utf8_lossy(bytes).to_string()
            };
            let content = {
                let bytes = &data[content_ptr as usize..(content_ptr + content_len) as usize];
                String::from_utf8_lossy(bytes).to_string()
            };
            let allowed = caller.data().sandbox_config.filesystem.write_paths.iter()
                .any(|p| path.starts_with(p.trim_end_matches('*')) || p == "*");
            if !allowed {
                return Err(Error::msg(format!("Write access denied: {}", path)));
            }
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&path, content) {
                Ok(_) => Ok(0),
                Err(_) => Ok(-1),
            }
        }
    )?;

    // slpx_http_get(url_ptr, url_len, out_ptr) -> i32
    linker.func_wrap("slpx", "http_get",
        |mut caller: Caller<'_, HostState>, url_ptr: i32, url_len: i32, out_ptr: i32| -> Result<i32, Error> {
            caller.data().check_capability("network:outbound", CapabilityLevel::ReadOnly)
                .map_err(|e| Error::msg(e))?;
            let mem = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| Error::msg("No memory export"))?;
            let data = mem.data(&caller);
            let url_bytes = &data[url_ptr as usize..(url_ptr + url_len) as usize];
            let url = String::from_utf8_lossy(url_bytes).to_string();
            match HTTP_CLIENT.get(&url).send() {
                Ok(resp) => {
                    match resp.text() {
                        Ok(body) => {
                            let body_bytes = body.as_bytes();
                            let data_mut = mem.data_mut(&mut caller);
                            let dest = out_ptr as usize;
                            if dest + body_bytes.len() > data_mut.len() { return Ok(-1); }
                            data_mut[dest..dest + body_bytes.len()].copy_from_slice(body_bytes);
                            Ok(body_bytes.len() as i32)
                        }
                        Err(_) => Ok(-1),
                    }
                }
                Err(_) => Ok(-2),
            }
        }
    )?;

    // slpx_get_time() -> i64
    linker.func_wrap("slpx", "get_time",
        |_caller: Caller<'_, HostState>| -> i64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        }
    )?;

    // slpx_random_bytes(out_ptr: i32, len: i32)
    linker.func_wrap("slpx", "random_bytes",
        |mut caller: Caller<'_, HostState>, out_ptr: i32, len: i32| -> Result<(), Error> {
            let mem = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| Error::msg("No memory export"))?;
            let data = mem.data_mut(&mut caller);
            let out_slice = &mut data[out_ptr as usize..(out_ptr + len) as usize];
            use rand::RngCore;
            rand::thread_rng().fill_bytes(out_slice);
            Ok(())
        }
    )?;

    Ok(())
}

