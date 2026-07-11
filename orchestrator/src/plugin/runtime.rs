// ============================================================
// Plugin Runtime — The Core WASM Execution Engine
// ============================================================

use wasmtime::*;
use std::collections::HashMap;
use tokio::sync::RwLock;

use super::manifest::{PluginManifest, CapabilityLevel};
use super::sandbox::SandboxConfig;
use super::host_functions::{HostState, register_host_functions};

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub json_schema: String,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub fuel_consumed: u64,
}

struct LoadedPlugin {
    manifest: PluginManifest,
    sandbox_config: SandboxConfig,
    module_bytes: Vec<u8>,
    tools: Vec<ToolInfo>,
}

pub struct PluginRuntime {
    plugins: RwLock<HashMap<String, LoadedPlugin>>,
}

impl PluginRuntime {
    pub fn new() -> Self {
        PluginRuntime {
            plugins: RwLock::new(HashMap::new()),
        }
    }

    pub async fn load_plugin(
        &self,
        wasm_bytes: &[u8],
        manifest: &PluginManifest,
        sandbox_config: Option<SandboxConfig>,
    ) -> Result<(), String> {
        manifest.validate()?;

        {
            let plugins = self.plugins.read().await;
            if plugins.contains_key(&manifest.id) {
                return Err(format!("Plugin '{}' is already loaded", manifest.id));
            }
        }

        let computed_hash = PluginManifest::compute_hash(wasm_bytes);
        if !manifest.wasm_hash.is_empty() && computed_hash != manifest.wasm_hash {
            return Err(format!("WASM hash mismatch: expected {}, got {}", manifest.wasm_hash, computed_hash));
        }

        let sandbox = sandbox_config.unwrap_or_else(|| {
            let mut cfg = SandboxConfig::default();
            cfg.fuel_budget = manifest.fuel_budget;
            cfg.timeout_ms = manifest.timeout_ms;
            cfg.memory_mb = manifest.memory_mb;
            cfg
        });

        // Validate the WASM compiles
        let mut config = Config::new();
        config.consume_fuel(true);
        config.wasm_multi_memory(true);
        config.wasm_bulk_memory(true);
        config.wasm_reference_types(true);

        let engine = Engine::new(&config).map_err(|e| format!("Engine: {}", e))?;
        let module = Module::from_binary(&engine, wasm_bytes)
            .map_err(|e| format!("WASM compile: {}", e))?;

        // Discover tools by instantiating a temporary instance
        let tools = Self::discover_tools(&engine, &module, &manifest.id)?;

        let loaded = LoadedPlugin {
            manifest: manifest.clone(),
            sandbox_config: sandbox,
            module_bytes: wasm_bytes.to_vec(),
            tools,
        };

        let mut plugins = self.plugins.write().await;
        plugins.insert(manifest.id.clone(), loaded);

        eprintln!("[plugin_runtime] Loaded plugin: {} v{}", manifest.id, manifest.version);
        Ok(())
    }

    fn discover_tools(engine: &Engine, module: &Module, plugin_id: &str) -> Result<Vec<ToolInfo>, String> {
        let host_state = HostState::new(plugin_id, &SandboxConfig::default(), Vec::new());
        let mut store = Store::new(engine, host_state);
        store.set_fuel(1_000_000).ok();

        let mut linker: Linker<HostState> = Linker::new(engine);
        register_host_functions(&mut linker)
            .map_err(|e| format!("Host funcs: {}", e))?;

        let instance = linker.instantiate(&mut store, module)
            .map_err(|e| format!("Instantiate: {}", e))?;

        let mut tools = Vec::new();
        for export in instance.exports(&mut store) {
            let name = export.name();
            if name.starts_with("tool_") {
                let tool_name = name[5..].to_string();
                tools.push(ToolInfo {
                    name: tool_name,
                    description: format!("Tool from plugin {}", plugin_id),
                    json_schema: r#"{"type": "object", "properties": {}}"#.to_string(),
                });
            }
        }
        if tools.is_empty() && instance.get_export(&mut store, "run").is_some() {
            tools.push(ToolInfo {
                name: "run".to_string(),
                description: format!("Default entry for plugin {}", plugin_id),
                json_schema: r#"{"type": "object", "properties": {"input": {"type": "string"}}}"#.to_string(),
            });
        }
        Ok(tools)
    }

    pub async fn execute_tool(
        &self,
        plugin_id: &str,
        tool_name: &str,
        arguments_json: &str,
    ) -> Result<ToolExecutionResult, String> {
        let start = std::time::Instant::now();

        let (module_bytes, sandbox, manifest, tool_exists) = {
            let plugins = self.plugins.read().await;
            let plugin = plugins.get(plugin_id)
                .ok_or_else(|| format!("Plugin '{}' not found", plugin_id))?;
            let tool_exists = plugin.tools.iter().any(|t| t.name == tool_name);
            (plugin.module_bytes.clone(), plugin.sandbox_config.clone(),
             plugin.manifest.clone(), tool_exists)
        };

        if !tool_exists {
            return Err(format!("Tool '{}' not found in plugin '{}'", tool_name, plugin_id));
        }

        // Set up a fresh engine + store for execution
        let mut config = Config::new();
        config.consume_fuel(true);
        config.wasm_multi_memory(true);
        config.wasm_bulk_memory(true);
        config.wasm_reference_types(true);

        let engine = Engine::new(&config).map_err(|e| format!("Engine: {}", e))?;
        let module = Module::from_binary(&engine, &module_bytes)
            .map_err(|e| format!("Module: {}", e))?;

        let capabilities: Vec<(String, CapabilityLevel)> = manifest.capabilities.iter()
            .map(|c| (c.name.clone(), c.level.clone())).collect();
        let host_state = HostState::new(plugin_id, &sandbox, capabilities);

        let mut store = Store::new(&engine, host_state);
        store.set_fuel(sandbox.fuel_budget).map_err(|e| format!("Fuel: {}", e))?;

        let mut linker: Linker<HostState> = Linker::new(&engine);
        register_host_functions(&mut linker).map_err(|e| format!("Host funcs: {}", e))?;

        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| format!("Instance: {}", e))?;

        let func_name = if tool_name == "run" { "run".to_string() }
                        else { format!("tool_{}", tool_name) };

        let func = instance
            .get_export(&mut store, &func_name)
            .and_then(|e| e.into_func())
            .ok_or_else(|| format!("Function '{}' not exported", func_name))?;

        // Allocate memory for arguments via the plugin's "alloc" function
        let arg_bytes = arguments_json.as_bytes();
        let alloc = instance
            .get_export(&mut store, "alloc")
            .and_then(|e| e.into_func())
            .ok_or_else(|| "Plugin must export 'alloc(i32) -> i32'".to_string())?;

        let mut alloc_results = [Val::I32(0)];
        alloc.call(&mut store, &[Val::I32(arg_bytes.len() as i32)], &mut alloc_results)
            .map_err(|e| format!("alloc call: {}", e))?;

        let arg_ptr = match alloc_results[0] {
            Val::I32(p) => p,
            _ => return Err("alloc returned invalid value".to_string()),
        };

        // Write arguments into WASM memory
        {
            let mem = instance.get_export(&mut store, "memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| "No memory export".to_string())?;
            let data = mem.data_mut(&mut store);
            let dest = arg_ptr as usize;
            if dest + arg_bytes.len() > data.len() {
                return Err("Argument buffer out of bounds".to_string());
            }
            data[dest..dest + arg_bytes.len()].copy_from_slice(arg_bytes);
        }

        // Call the tool function
        let mut call_results = [Val::I64(0)];
        let call_result = func.call(
            &mut store,
            &[Val::I32(arg_ptr), Val::I32(arg_bytes.len() as i32)],
            &mut call_results,
        );

        let result_json = match call_result {
            Ok(()) => {
                let packed = match call_results[0] {
                    Val::I64(v) => v,
                    Val::I32(v) => v as i64,
                    _ => -1,
                };
                let result_ptr = (packed >> 32) as i32;
                let result_len = (packed & 0xFFFF_FFFF) as i32;
                if result_ptr >= 0 && result_len > 0 {
                    let mem = instance.get_export(&mut store, "memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| "No memory export".to_string())?;
                    let data = mem.data(&store);
                    let s = result_ptr as usize;
                    let e = s + result_len as usize;
                    if e <= data.len() {
                        String::from_utf8_lossy(&data[s..e]).to_string()
                    } else {
                        format!(r#"{{"error":"invalid memory range"}}"#)
                    }
                } else {
                    format!(r#"{{"return_code":{}}}"#, packed)
                }
            }
            Err(e) => format!(r#"{{"error":"{}"}}"#, e.to_string().replace('"', "'")),
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let fuel_consumed = sandbox.fuel_budget.saturating_sub(store.get_fuel().unwrap_or(0));
        let success = !result_json.contains(r#""error""#);

        Ok(ToolExecutionResult {
            success,
            output: result_json,
            error: if success { None } else { Some("Tool execution reported an error".to_string()) },
            duration_ms,
            fuel_consumed,
        })
    }

    pub async fn list_plugins(&self) -> Vec<(String, PluginManifest, Vec<ToolInfo>)> {
        let plugins = self.plugins.read().await;
        plugins.iter().map(|(id, p)| {
            (id.clone(), p.manifest.clone(), p.tools.clone())
        }).collect()
    }

    pub async fn get_plugin(&self, plugin_id: &str) -> Option<(PluginManifest, Vec<ToolInfo>)> {
        let plugins = self.plugins.read().await;
        plugins.get(plugin_id).map(|p| (p.manifest.clone(), p.tools.clone()))
    }

    pub async fn unload_plugin(&self, plugin_id: &str) -> Result<(), String> {
        let mut plugins = self.plugins.write().await;
        if plugins.remove(plugin_id).is_some() {
            eprintln!("[plugin_runtime] Unloaded plugin: {}", plugin_id);
            Ok(())
        } else {
            Err(format!("Plugin '{}' not found", plugin_id))
        }
    }

    pub fn validate_wasm(wasm_bytes: &[u8]) -> Result<(PluginManifest, Vec<ToolInfo>), String> {
        let manifest = PluginManifest::from_wasm_bytes(wasm_bytes)?;
        manifest.validate()?;

        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|e| format!("Engine: {}", e))?;
        let module = Module::from_binary(&engine, wasm_bytes)
            .map_err(|e| format!("WASM compile: {}", e))?;
        let tools = PluginRuntime::discover_tools(&engine, &module, &manifest.id)?;
        Ok((manifest, tools))
    }
}
