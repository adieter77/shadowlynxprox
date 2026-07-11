// ============================================================
// Plugin Manifest
// ============================================================
// Each WASM plugin embeds a manifest in a custom section
// called "slpx_manifest". This module parses it.
// ============================================================

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityLevel {
    None,
    ReadOnly,
    ReadWrite,
    Full,
}

impl Default for CapabilityLevel {
    fn default() -> Self { CapabilityLevel::None }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    #[serde(default)]
    pub level: CapabilityLevel,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default = "default_fuel_budget")]
    pub fuel_budget: u64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u64,
    #[serde(default)]
    pub wasm_hash: String,
}

fn default_fuel_budget() -> u64 { 100_000_000 }
fn default_timeout_ms() -> u64 { 30_000 }
fn default_memory_mb() -> u64 { 64 }

impl Default for PluginManifest {
    fn default() -> Self {
        PluginManifest {
            id: String::new(),
            name: String::new(),
            version: String::new(),
            description: String::new(),
            author: String::new(),
            capabilities: Vec::new(),
            fuel_budget: default_fuel_budget(),
            timeout_ms: default_timeout_ms(),
            memory_mb: default_memory_mb(),
            wasm_hash: String::new(),
        }
    }
}

impl PluginManifest {
    pub fn compute_hash(wasm_bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(wasm_bytes);
        hex::encode(hasher.finalize())
    }

    pub fn from_wasm_bytes(wasm_bytes: &[u8]) -> Result<Self, String> {
        if wasm_bytes.len() < 8 {
            return Err("WASM binary too short".to_string());
        }
        if &wasm_bytes[0..4] != b"\0asm" {
            return Err("Invalid WASM magic number".to_string());
        }

        let mut offset = 8;
        while offset < wasm_bytes.len() {
            if offset >= wasm_bytes.len() { break; }
            let section_id = wasm_bytes[offset];
            offset += 1;

            let (size, bytes_read) = read_leb128_u32(&wasm_bytes[offset..])
                .map_err(|e| format!("Failed to read section size: {}", e))?;
            offset += bytes_read;
            let section_end = offset + size as usize;
            if section_end > wasm_bytes.len() {
                return Err("Section extends beyond WASM binary".to_string());
            }

            if section_id == 0 {
                let (name_len, name_len_bytes) = read_leb128_u32(&wasm_bytes[offset..])
                    .map_err(|e| format!("Failed to read name length: {}", e))?;
                offset += name_len_bytes;
                let name_end = offset + name_len as usize;
                if name_end > section_end {
                    return Err("Name extends beyond section".to_string());
                }
                let name = std::str::from_utf8(&wasm_bytes[offset..name_end])
                    .map_err(|e| format!("Invalid UTF-8: {}", e))?;

                if name == "slpx_manifest" {
                    let payload = &wasm_bytes[name_end..section_end];
                    let manifest: PluginManifest = serde_json::from_slice(payload)
                        .map_err(|e| format!("Failed to parse manifest JSON: {}", e))?;

                    if !manifest.wasm_hash.is_empty() {
                        let computed = Self::compute_hash(wasm_bytes);
                        if computed != manifest.wasm_hash {
                            return Err(format!("WASM hash mismatch: expected {}, got {}", manifest.wasm_hash, computed));
                        }
                    }
                    return Ok(manifest);
                }
            }
            offset = section_end;
        }
        Err("No 'slpx_manifest' custom section found".to_string())
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse manifest JSON: {}", e))
    }

    pub fn has_capability(&self, capability_name: &str, required: &CapabilityLevel) -> bool {
        for cap in &self.capabilities {
            if cap.name == capability_name {
                return match (&cap.level, required) {
                    (CapabilityLevel::Full, _) => true,
                    (CapabilityLevel::ReadWrite, CapabilityLevel::ReadWrite) => true,
                    (CapabilityLevel::ReadWrite, CapabilityLevel::ReadOnly) => true,
                    (CapabilityLevel::ReadOnly, CapabilityLevel::ReadOnly) => true,
                    _ => false,
                };
            }
        }
        false
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() { return Err("Plugin ID is required".to_string()); }
        if self.name.is_empty() { return Err("Plugin name is required".to_string()); }
        if self.version.is_empty() { return Err("Plugin version is required".to_string()); }
        if self.version.split('.').count() != 3 {
            return Err("Version must be semver (X.Y.Z)".to_string());
        }
        Ok(())
    }
}

fn read_leb128_u32(bytes: &[u8]) -> Result<(u32, usize), String> {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;
    let mut bytes_read = 0;
    for &byte in bytes.iter() {
        bytes_read += 1;
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 { return Ok((result, bytes_read)); }
        shift += 7;
        if shift >= 35 { return Err("LEB128 too long for u32".to_string()); }
    }
    Err("Unexpected end of LEB128".to_string())
}
