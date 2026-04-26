use crate::collectors::Collector;
use crate::data::{Edge, EdgeType, Inventory, Node, NodeType};
use anyhow::Result;
use std::process::Command;

pub struct PciCollector;

impl PciCollector {
    pub fn new() -> Self {
        Self
    }

    fn parse_lspci_output(&self, output: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        
        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }
            
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() < 2 {
                continue;
            }
            
            let pci_id = parts[0];
            let description = parts[1];
            
            let node = Node::new(
                format!("pci:{}", pci_id),
                NodeType::PciDevice,
            )
            .with_property("pci_id", pci_id)
            .with_property("description", description)
            .with_property("type", "pci_device");
            
            nodes.push(node);
        }
        
        nodes
    }

    async fn get_network_pci_mappings(&self) -> Result<Vec<Edge>> {
        let edges = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            // Try to find network device to PCI mappings via sysfs
            let output = Command::new("find")
                .args(&["/sys/class/net", "-type", "l", "-exec", "readlink", "-f", "{}", ";"])
                .output();
                
            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("/pci") {
                        // Extract network interface name and PCI ID
                        if let Some(net_pos) = line.rfind("/net/") {
                            let interface_name = &line[net_pos + 5..];
                            
                            // Extract PCI ID from path
                            if let Some(pci_start) = line.find("/pci") {
                                if let Some(pci_end) = line[pci_start..].find("/net") {
                                    let pci_path = &line[pci_start..pci_start + pci_end];
                                    if let Some(pci_id_start) = pci_path.rfind('/') {
                                        let pci_id = &pci_path[pci_id_start + 1..];
                                        
                                        let edge = Edge::new(
                                            format!("netdev:{}", interface_name),
                                            format!("pci:{}", pci_id),
                                            EdgeType::PciBinding,
                                        );
                                        edges.push(edge);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(edges)
    }
}

impl Collector for PciCollector {
    async fn collect(&self, inventory: &mut Inventory) -> Result<()> {
        // Check if lspci is available
        let lspci_check = Command::new("which")
            .arg("lspci")
            .output();
            
        if lspci_check.is_err() || !lspci_check.unwrap().status.success() {
            // lspci not available, skip PCI collection
            return Ok(());
        }
        
        // Collect PCI devices
        let output = Command::new("lspci")
            .output()?;
            
        let stdout = String::from_utf8_lossy(&output.stdout);
        let nodes = self.parse_lspci_output(&stdout);
        
        for node in nodes {
            inventory.add_node(node);
        }
        
        // Collect PCI to network device mappings
        let edges = self.get_network_pci_mappings().await?;
        for edge in edges {
            inventory.add_edge(edge);
        }
        
        Ok(())
    }
}