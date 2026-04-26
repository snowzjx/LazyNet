use crate::collectors::{pci::PciCollector, rdma::RdmaCollector, Collector, NetworkCollector};
use crate::config::Config;
use crate::data::Inventory;
use crate::ui::Ui;
use anyhow::Result;

pub struct App {
    config: Config,
    inventory: Inventory,
    ui: Ui,
}

impl App {
    pub async fn new(config_path: &str) -> Result<Self> {
        let config = Config::load_from_file(config_path)?;
        let mut inventory = Inventory::new();
        
        // Collect data from all enabled collectors
        if config.collectors.enable_network {
            let collector = NetworkCollector::new();
            collector.collect(&mut inventory).await?;
        }
        
        if config.collectors.enable_pci {
            let collector = PciCollector::new();
            collector.collect(&mut inventory).await?;
        }
        
        if config.collectors.enable_rdma {
            let collector = RdmaCollector::new();
            collector.collect(&mut inventory).await?;
        }
        
        let ui = Ui::new();
        
        Ok(Self {
            config,
            inventory,
            ui,
        })
    }
    
    pub async fn run(&mut self) -> Result<()> {
        self.ui.run(&self.inventory).await
    }
    
    pub async fn export_json(&self) -> Result<()> {
        let json_str = if self.config.export.pretty_json {
            serde_json::to_string_pretty(&self.inventory)?
        } else {
            serde_json::to_string(&self.inventory)?
        };
        
        println!("{}", json_str);
        Ok(())
    }
    
    pub async fn refresh_inventory(&mut self) -> Result<()> {
        self.inventory = Inventory::new();
        
        // Re-collect data from all enabled collectors
        if self.config.collectors.enable_network {
            let collector = NetworkCollector::new();
            collector.collect(&mut self.inventory).await?;
        }
        
        if self.config.collectors.enable_pci {
            let collector = PciCollector::new();
            collector.collect(&mut self.inventory).await?;
        }
        
        if self.config.collectors.enable_rdma {
            let collector = RdmaCollector::new();
            collector.collect(&mut self.inventory).await?;
        }
        
        Ok(())
    }
}