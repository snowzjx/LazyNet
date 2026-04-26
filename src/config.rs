use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ui: UiConfig,
    pub collectors: CollectorConfig,
    pub export: ExportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub refresh_interval_ms: u64,
    pub default_tab: String,
    pub show_raw_tab: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorConfig {
    pub enable_network: bool,
    pub enable_pci: bool,
    pub enable_rdma: bool,
    pub enable_dpdk: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    pub pretty_json: bool,
    pub include_metadata: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UiConfig {
                refresh_interval_ms: 1000,
                default_tab: "interfaces".to_string(),
                show_raw_tab: true,
            },
            collectors: CollectorConfig {
                enable_network: true,
                enable_pci: true,
                enable_rdma: true,
                enable_dpdk: false, // DPDK not implemented yet
            },
            export: ExportConfig {
                pretty_json: true,
                include_metadata: true,
            },
        }
    }
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        // Expand ~ to home directory
        let expanded_path = if path.to_string_lossy().starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(path.strip_prefix("~/").unwrap())
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        };
        
        if !expanded_path.exists() {
            // Create default config file
            let config = Config::default();
            config.save_to_file(&expanded_path)?;
            return Ok(config);
        }
        
        let content = std::fs::read_to_string(&expanded_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
    
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}