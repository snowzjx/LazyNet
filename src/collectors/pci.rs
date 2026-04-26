use crate::collectors::Collector;
use crate::data::{Edge, EdgeType, Inventory, Node, NodeType};
use anyhow::Result;
use std::process::Command;

pub struct PciCollector;

impl PciCollector {
    pub fn new() -> Self {
        Self
    }
}

/// Parse `lspci -vmm` output into nodes, enriched with sysfs data.
fn parse_lspci_vmm(output: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut props: Vec<(&str, String)> = Vec::new();

    for line in output.lines() {
        if line.trim().is_empty() {
            if !props.is_empty() {
                if let Some(slot) = props.iter().find(|(k, _)| *k == "Slot").map(|(_, v)| v.clone()) {
                    let mut node = Node::new(format!("pci:{}", slot), NodeType::PciDevice)
                        .with_property("pci_id", &slot);
                    for (k, v) in &props {
                        node = node.with_property(&k.to_lowercase(), v);
                    }
                    // Enrich from sysfs
                    let addr = format!("0000:{}", slot);
                    let sysfs = format!("/sys/bus/pci/devices/{}", addr);
                    for (file, prop) in &[
                        ("current_link_speed", "link_speed"),
                        ("current_link_width", "link_width"),
                        ("max_link_speed",     "max_link_speed"),
                        ("max_link_width",     "max_link_width"),
                        ("numa_node",          "numa_node"),
                    ] {
                        if let Ok(val) = std::fs::read_to_string(format!("{}/{}", sysfs, file)) {
                            node = node.with_property(prop, val.trim());
                        }
                    }
                    // Driver from symlink
                    if let Ok(link) = std::fs::read_link(format!("{}/driver", sysfs)) {
                        if let Some(drv) = link.file_name() {
                            node = node.with_property("driver", &drv.to_string_lossy());
                        }
                    }
                    nodes.push(node);
                }
                props.clear();
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            props.push((k.trim(), v.trim().to_string()));
        }
    }
    // Handle last block (no trailing blank line)
    if !props.is_empty() {
        if let Some(slot) = props.iter().find(|(k, _)| *k == "Slot").map(|(_, v)| v.clone()) {
            let mut node = Node::new(format!("pci:{}", slot), NodeType::PciDevice)
                .with_property("pci_id", &slot);
            for (k, v) in &props {
                node = node.with_property(&k.to_lowercase(), v);
            }
            let addr = format!("0000:{}", slot);
            let sysfs = format!("/sys/bus/pci/devices/{}", addr);
            for (file, prop) in &[
                ("current_link_speed", "link_speed"),
                ("current_link_width", "link_width"),
                ("max_link_speed",     "max_link_speed"),
                ("max_link_width",     "max_link_width"),
                ("numa_node",          "numa_node"),
            ] {
                if let Ok(val) = std::fs::read_to_string(format!("{}/{}", sysfs, file)) {
                    node = node.with_property(prop, val.trim());
                }
            }
            if let Ok(link) = std::fs::read_link(format!("{}/driver", sysfs)) {
                if let Some(drv) = link.file_name() {
                    node = node.with_property("driver", &drv.to_string_lossy());
                }
            }
            nodes.push(node);
        }
    }
    nodes
}

/// Build netdev→pci edges via sysfs symlinks.
fn collect_pci_edges() -> Vec<Edge> {
    let mut edges = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/net") else { return edges };
    for entry in entries.flatten() {
        let iface = entry.file_name().to_string_lossy().to_string();
        let Ok(link) = std::fs::read_link(entry.path()) else { continue };
        let path = link.to_string_lossy();
        // path looks like ../../devices/pci0000:00/.../0000:99:00.0/net/ens24np0
        if let Some(net_pos) = path.find("/net/") {
            let before_net = &path[..net_pos];
            if let Some(last_slash) = before_net.rfind('/') {
                let pci_addr = &before_net[last_slash + 1..];
                // strip domain prefix "0000:" to match our node IDs
                let slot = pci_addr.trim_start_matches("0000:");
                if !slot.is_empty() && slot.contains(':') {
                    edges.push(Edge::new(
                        format!("netdev:{}", iface),
                        format!("pci:{}", slot),
                        EdgeType::PciBinding,
                    ));
                }
            }
        }
    }
    edges
}

impl Collector for PciCollector {
    async fn collect(&self, inventory: &mut Inventory) -> Result<()> {
        let Ok(out) = Command::new("lspci").args(["-vmm"]).output() else { return Ok(()) };
        if !out.status.success() { return Ok(()); }

        let stdout = String::from_utf8_lossy(&out.stdout);
        for node in parse_lspci_vmm(&stdout) {
            inventory.add_node(node);
        }
        for edge in collect_pci_edges() {
            inventory.add_edge(edge);
        }
        Ok(())
    }
}
