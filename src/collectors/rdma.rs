use crate::collectors::Collector;
use crate::data::{Edge, EdgeType, Inventory, Node, NodeType};
use anyhow::Result;
use std::process::Command;

pub struct RdmaCollector;

impl RdmaCollector {
    pub fn new() -> Self {
        Self
    }

    fn parse_rdma_devices(&self, output: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        
        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }
            
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            
            let device_name = parts[0];
            let mut node = Node::new(
                format!("rdma:{}", device_name),
                NodeType::RdmaDevice,
            )
            .with_property("name", device_name)
            .with_property("type", "rdma_device");
            
            // Determine if it's IB or RoCE based on device name patterns
            let transport_type = if device_name.starts_with("mlx") {
                if device_name.contains("ib") {
                    "InfiniBand"
                } else {
                    "RoCE"
                }
            } else if device_name.starts_with("hfi") {
                "InfiniBand"
            } else {
                "Unknown"
            };
            
            node = node.with_property("transport", transport_type);
            
            nodes.push(node);
        }
        
        nodes
    }

    async fn get_rdma_netdev_mappings(&self) -> Result<Vec<Edge>> {
        let edges = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            // Try to find RDMA to network device mappings
            let output = Command::new("find")
                .args(&["/sys/class/infiniband", "-name", "ports", "-type", "d"])
                .output();
                
            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for ports_dir in stdout.lines() {
                    if let Ok(entries) = std::fs::read_dir(ports_dir) {
                        for entry in entries.flatten() {
                            let port_path = entry.path();
                            let netdev_path = port_path.join("gid_attrs/ndevs/0");
                            
                            if let Ok(netdev_name) = std::fs::read_to_string(&netdev_path) {
                                let netdev_name = netdev_name.trim();
                                if !netdev_name.is_empty() {
                                    // Extract RDMA device name from path
                                    if let Some(ib_pos) = ports_dir.rfind("/infiniband/") {
                                        let remaining = &ports_dir[ib_pos + 12..];
                                        if let Some(slash_pos) = remaining.find('/') {
                                            let rdma_device = &remaining[..slash_pos];
                                            
                                            let edge = Edge::new(
                                                format!("rdma:{}", rdma_device),
                                                format!("netdev:{}", netdev_name),
                                                EdgeType::RdmaMapping,
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
        }
        
        Ok(edges)
    }
}

impl Collector for RdmaCollector {
    async fn collect(&self, inventory: &mut Inventory) -> Result<()> {
        // Check if RDMA tools are available
        let output = Command::new("which")
            .arg("ibstat")
            .output();
            
        if output.is_err() || !output.unwrap().status.success() {
            // Try alternative method using sysfs (Linux only)
            #[cfg(target_os = "linux")]
            {
                if let Ok(entries) = std::fs::read_dir("/sys/class/infiniband") {
                    for entry in entries.flatten() {
                        let device_name = entry.file_name().to_string_lossy().to_string();
                        let node = Node::new(
                            format!("rdma:{}", device_name),
                            NodeType::RdmaDevice,
                        )
                        .with_property("name", &device_name)
                        .with_property("type", "rdma_device")
                        .with_property("transport", "Unknown");
                        
                        inventory.add_node(node);
                    }
                }
            }
            
            // On macOS or if no sysfs, just return without error
            return Ok(());
        } else {
            // Use ibstat if available
            let output = Command::new("ibstat")
                .arg("-l")
                .output()?;
                
            let stdout = String::from_utf8_lossy(&output.stdout);
            let nodes = self.parse_rdma_devices(&stdout);
            
            for node in nodes {
                inventory.add_node(node);
            }
        }
        
        // Collect RDMA to network device mappings
        let edges = self.get_rdma_netdev_mappings().await?;
        for edge in edges {
            inventory.add_edge(edge);
        }
        
        Ok(())
    }
}