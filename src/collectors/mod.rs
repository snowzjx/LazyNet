use crate::data::{IfaceCounters, Inventory, Node, NodeType};
use anyhow::Result;
use std::process::Command;

pub mod network;
pub mod pci;
pub mod rdma;

pub trait Collector {
    async fn collect(&self, inventory: &mut Inventory) -> Result<()>;
}

pub struct NetworkCollector;

impl NetworkCollector {
    pub fn new() -> Self {
        Self
    }

    fn parse_ifconfig_output(&self, output: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut current_interface: Option<String> = None;
        let mut current_flags: Option<String> = None;
        let mut current_mtu: Option<String> = None;
        
        for line in output.lines() {
            if !line.starts_with('\t') && !line.starts_with(' ') && line.contains(':') {
                // Save previous interface if exists
                if let Some(interface_name) = current_interface.take() {
                    let mut node = Node::new(
                        format!("netdev:{}", interface_name),
                        NodeType::NetworkInterface,
                    )
                    .with_property("name", &interface_name)
                    .with_property("type", "network_interface");

                    if let Some(flags) = current_flags.take() {
                        node = node.with_property("flags", &flags);
                        let state = if flags.contains("UP") { "up" } else { "down" };
                        node = node.with_property("state", state);
                    }

                    if let Some(mtu) = current_mtu.take() {
                        node = node.with_property("mtu", &mtu);
                    }

                    nodes.push(node);
                }
                
                // Parse new interface line
                if let Some(colon_pos) = line.find(':') {
                    current_interface = Some(line[..colon_pos].to_string());
                    
                    // Parse flags
                    if let Some(flags_start) = line.find('<') {
                        if let Some(flags_end) = line.find('>') {
                            current_flags = Some(line[flags_start + 1..flags_end].to_string());
                        }
                    }
                    
                    // Parse MTU
                    if let Some(mtu_pos) = line.find("mtu ") {
                        let mtu_part = &line[mtu_pos + 4..];
                        if let Some(mtu_end) = mtu_part.find(' ') {
                            current_mtu = Some(mtu_part[..mtu_end].to_string());
                        } else {
                            current_mtu = Some(mtu_part.trim().to_string());
                        }
                    }
                }
            }
        }
        
        // Don't forget the last interface
        if let Some(interface_name) = current_interface {
            let mut node = Node::new(
                format!("netdev:{}", interface_name),
                NodeType::NetworkInterface,
            )
            .with_property("name", &interface_name)
            .with_property("type", "network_interface");

            if let Some(flags) = current_flags {
                node = node.with_property("flags", &flags);
                let state = if flags.contains("UP") { "up" } else { "down" };
                node = node.with_property("state", state);
            }

            if let Some(mtu) = current_mtu {
                node = node.with_property("mtu", &mtu);
            }

            nodes.push(node);
        }
        
        nodes
    }

    fn parse_ip_output(&self, output: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        
        for line in output.lines() {
            if line.trim().is_empty() || line.starts_with(' ') {
                continue;
            }
            
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            
            let interface_name = parts[1].trim_end_matches(':');
            let mut node = Node::new(
                format!("netdev:{}", interface_name),
                NodeType::NetworkInterface,
            )
            .with_property("name", interface_name)
            .with_property("type", "network_interface");

            // Parse flags and state
            if let Some(flags_start) = line.find('<') {
                if let Some(flags_end) = line.find('>') {
                    let flags = &line[flags_start + 1..flags_end];
                    node = node.with_property("flags", flags);
                    
                    let state = if flags.contains("UP") { "up" } else { "down" };
                    node = node.with_property("state", state);
                }
            }

            // Parse MTU
            if let Some(mtu_pos) = line.find("mtu ") {
                let mtu_part = &line[mtu_pos + 4..];
                if let Some(mtu_end) = mtu_part.find(' ') {
                    let mtu = &mtu_part[..mtu_end];
                    node = node.with_property("mtu", mtu);
                } else {
                    node = node.with_property("mtu", mtu_part.trim());
                }
            }

            nodes.push(node);
        }
        
        nodes
    }

    async fn get_mac_addresses(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut mac_map = std::collections::HashMap::new();
        
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("ip")
                .args(&["link", "show"])
                .output()?;
            
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_interface = None;
            
            for line in stdout.lines() {
                if !line.starts_with(' ') {
                    // New interface line
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        current_interface = Some(parts[1].trim_end_matches(':').to_string());
                    }
                } else if line.contains("link/ether") {
                    // MAC address line
                    if let Some(interface) = &current_interface {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            mac_map.insert(interface.clone(), parts[1].to_string());
                        }
                    }
                }
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            let output = Command::new("ifconfig")
                .output()?;
            
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_interface = None;
            
            for line in stdout.lines() {
                if !line.starts_with('\t') && !line.starts_with(' ') && line.contains(':') {
                    // New interface line
                    if let Some(colon_pos) = line.find(':') {
                        current_interface = Some(line[..colon_pos].to_string());
                    }
                } else if line.contains("ether ") {
                    // MAC address line
                    if let Some(interface) = &current_interface {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if let Some(ether_pos) = parts.iter().position(|&x| x == "ether") {
                            if ether_pos + 1 < parts.len() {
                                mac_map.insert(interface.clone(), parts[ether_pos + 1].to_string());
                            }
                        }
                    }
                }
            }
        }
        
        Ok(mac_map)
    }
}

impl Collector for NetworkCollector {
    async fn collect(&self, inventory: &mut Inventory) -> Result<()> {
        #[cfg(target_os = "linux")]
        let output = Command::new("ip")
            .args(&["addr", "show"])
            .output()?;
            
        #[cfg(target_os = "macos")]
        let output = Command::new("ifconfig")
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        
        #[cfg(target_os = "linux")]
        let mut nodes = self.parse_ip_output(&stdout);
        
        #[cfg(target_os = "macos")]
        let mut nodes = self.parse_ifconfig_output(&stdout);
        
        // Get MAC addresses
        let mac_addresses = self.get_mac_addresses().await?;
        
        // Add MAC addresses to nodes
        for node in &mut nodes {
            if let Some(name) = node.get_property("name") {
                if let Some(mac) = mac_addresses.get(name) {
                    node.properties.insert("mac".to_string(), mac.clone());
                }
            }
        }
        
        for node in nodes {
            if let Some(name) = node.get_property("name") {
                inventory.iface_counters.insert(name.clone(), read_iface_counters(name));
            }
            inventory.add_node(node);
        }
        
        Ok(())
    }
}

fn read_stat(dev: &str, stat: &str) -> u64 {
    std::fs::read_to_string(format!("/sys/class/net/{}/statistics/{}", dev, stat))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

pub fn read_iface_counters(dev: &str) -> IfaceCounters {
    IfaceCounters {
        rx_bytes:   read_stat(dev, "rx_bytes"),
        tx_bytes:   read_stat(dev, "tx_bytes"),
        rx_packets: read_stat(dev, "rx_packets"),
        tx_packets: read_stat(dev, "tx_packets"),
        rx_errors:  read_stat(dev, "rx_errors"),
        tx_errors:  read_stat(dev, "tx_errors"),
        rx_dropped: read_stat(dev, "rx_dropped"),
        tx_dropped: read_stat(dev, "tx_dropped"),
        rx_missed:  read_stat(dev, "rx_missed_errors"),
        collisions: read_stat(dev, "collisions"),
    }
}