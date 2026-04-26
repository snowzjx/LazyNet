use crate::collectors::Collector;
use crate::data::{Edge, EdgeType, Inventory, Node, NodeType, PfcInfo, RdmaCounters};
use anyhow::Result;
use std::process::Command;

pub struct RdmaCollector;

impl RdmaCollector {
    pub fn new() -> Self {
        Self
    }

    /// Enumerate devices from sysfs and enrich with ibv_devinfo if available.
    fn collect_devices(&self) -> Vec<Node> {
        let mut nodes = Vec::new();

        let entries = match std::fs::read_dir("/sys/class/infiniband") {
            Ok(e) => e,
            Err(_) => return nodes,
        };

        // Parse ibv_devinfo output once for all devices
        let devinfo = Command::new("ibv_devinfo")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();

        for entry in entries.flatten() {
            let device_name = entry.file_name().to_string_lossy().to_string();

            let transport = std::fs::read_to_string(
                format!("/sys/class/infiniband/{}/ports/1/link_layer", device_name),
            )
            .map(|s| match s.trim() {
                "InfiniBand" => "InfiniBand",
                "Ethernet" => "RoCE",
                _ => "Unknown",
            })
            .unwrap_or("Unknown");

            let mut node = Node::new(format!("rdma:{}", device_name), NodeType::RdmaDevice)
                .with_property("name", &device_name)
                .with_property("transport", transport);

            // Enrich from ibv_devinfo block for this device
            if !devinfo.is_empty() {
                enrich_from_devinfo(&mut node, &devinfo, &device_name);
            }

            nodes.push(node);
        }

        nodes
    }

    /// Build RDMA→netdev edges using `rdma link` (most reliable) with sysfs fallback.
    fn collect_edges(&self) -> Vec<Edge> {
        let mut edges = Vec::new();

        // Primary: `rdma link` output — "link mlx5_0/1 state ACTIVE ... netdev ens24np0"
        if let Ok(out) = Command::new("rdma").args(["link"]).output() {
            if out.status.success() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    // e.g. "link mlx5_0/1 state ACTIVE physical_state LINK_UP netdev ens24np0"
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 2 {
                        continue;
                    }
                    // parts[1] is "mlx5_0/1"
                    let rdma_dev = parts[1].split('/').next().unwrap_or("");
                    if rdma_dev.is_empty() {
                        continue;
                    }
                    if let Some(nd_pos) = parts.iter().position(|&p| p == "netdev") {
                        if let Some(netdev) = parts.get(nd_pos + 1) {
                            edges.push(Edge::new(
                                format!("rdma:{}", rdma_dev),
                                format!("netdev:{}", netdev),
                                EdgeType::RdmaMapping,
                            ));
                        }
                    }
                }
                return edges;
            }
        }

        // Fallback: sysfs gid_attrs
        #[cfg(target_os = "linux")]
        if let Ok(ib_entries) = std::fs::read_dir("/sys/class/infiniband") {
            for ib_entry in ib_entries.flatten() {
                let rdma_dev = ib_entry.file_name().to_string_lossy().to_string();
                let ports_path = ib_entry.path().join("ports");
                if let Ok(ports) = std::fs::read_dir(&ports_path) {
                    for port in ports.flatten() {
                        let ndev_path = port.path().join("gid_attrs/ndevs/0");
                        if let Ok(netdev) = std::fs::read_to_string(&ndev_path) {
                            let netdev = netdev.trim();
                            if !netdev.is_empty() {
                                edges.push(Edge::new(
                                    format!("rdma:{}", rdma_dev),
                                    format!("netdev:{}", netdev),
                                    EdgeType::RdmaMapping,
                                ));
                            }
                        }
                    }
                }
            }
        }

        edges
    }
}

/// Extract properties for `device_name` from a full `ibv_devinfo` output block.
fn enrich_from_devinfo(node: &mut Node, devinfo: &str, device_name: &str) {
    // Find the block starting with "hca_id: <device_name>"
    let mut in_block = false;
    for line in devinfo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("hca_id:") {
            in_block = trimmed.split_whitespace().nth(1) == Some(device_name);
            continue;
        }
        if !in_block {
            continue;
        }
        // Stop at next device block
        if trimmed.starts_with("hca_id:") {
            break;
        }
        // Parse key: value pairs (top-level, not port-indented)
        if !line.starts_with('\t') && !line.starts_with(' ') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            match key {
                "fw_ver" => node.properties.insert("fw_ver".into(), val.into()),
                "node_guid" => node.properties.insert("node_guid".into(), val.into()),
                "phys_port_cnt" => node.properties.insert("port_count".into(), val.into()),
                _ => None,
            };
        }
    }
}

/// Read a single sysfs counter file, returning 0 on failure.
fn read_rdma_stat(dev: &str, subdir: &str, name: &str) -> u64 {
    std::fs::read_to_string(format!(
        "/sys/class/infiniband/{}/ports/1/{}/{}",
        dev, subdir, name
    ))
    .ok()
    .and_then(|s| s.trim().parse().ok())
    .unwrap_or(0)
}

pub fn collect_rdma_counters(dev: &str) -> RdmaCounters {
    let c = |name: &str| read_rdma_stat(dev, "counters", name);
    let h = |name: &str| read_rdma_stat(dev, "hw_counters", name);
    RdmaCounters {
        port_rcv_data:                  c("port_rcv_data"),
        port_xmit_data:                 c("port_xmit_data"),
        port_rcv_packets:               c("port_rcv_packets"),
        port_xmit_packets:              c("port_xmit_packets"),
        port_rcv_errors:                c("port_rcv_errors"),
        port_xmit_discards:             c("port_xmit_discards"),
        port_xmit_wait:                 c("port_xmit_wait"),
        np_cnp_sent:                    h("np_cnp_sent"),
        np_ecn_marked_roce_packets:     h("np_ecn_marked_roce_packets"),
        rp_cnp_handled:                 h("rp_cnp_handled"),
        rp_cnp_ignored:                 h("rp_cnp_ignored"),
        out_of_buffer:                  h("out_of_buffer"),
        out_of_sequence:                h("out_of_sequence"),
        packet_seq_err:                 h("packet_seq_err"),
        rnr_nak_retry_err:              h("rnr_nak_retry_err"),
        req_transport_retries_exceeded: h("req_transport_retries_exceeded"),
        local_ack_timeout_err:          h("local_ack_timeout_err"),
        rx_icrc_encapsulated:           h("rx_icrc_encapsulated"),
    }
}

/// Collect PFC settings and counters for a netdev.
fn collect_pfc(netdev: &str) -> PfcInfo {
    let mut info = PfcInfo::default();

    // --- dcb pfc show dev <netdev> ---
    // Output: "pfc-cap 8 macsec-bypass off delay 7"
    //         "prio-pfc 0:off 1:off 2:on ..."
    if let Ok(out) = Command::new("dcb").args(["pfc", "show", "dev", netdev]).output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let line = line.trim();
            if line.starts_with("pfc-cap") {
                if let Some(v) = line.split_whitespace().nth(1) {
                    info.pfc_cap = v.parse().unwrap_or(0);
                }
            } else if line.starts_with("prio-pfc") {
                // "prio-pfc 0:off 1:off 2:on ..."
                for token in line.split_whitespace().skip(1) {
                    if let Some((prio, state)) = token.split_once(':') {
                        if let Ok(p) = prio.parse::<usize>() {
                            if p < 8 {
                                info.prio_enabled[p] = state == "on";
                            }
                        }
                    }
                }
            }
        }
    }

    // --- ethtool -S <netdev> ---
    if let Ok(out) = Command::new("ethtool").args(["-S", netdev]).output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let line = line.trim();
            // rx_prioN_packets / tx_prioN_packets
            for (prefix, arr) in [("rx_prio", &mut info.rx_pfc as &mut [u64; 8]),
                                   ("tx_prio", &mut info.tx_pfc as &mut [u64; 8])] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    // rest = "0_packets: 12345"
                    if let Some((prio_s, val_s)) = rest.split_once("_packets:") {
                        if let (Ok(p), Ok(v)) = (prio_s.parse::<usize>(), val_s.trim().parse::<u64>()) {
                            if p < 8 { arr[p] = v; }
                        }
                    }
                }
            }
            // global pause
            if let Some(v) = line.strip_prefix("rx_global_pause:") {
                info.rx_pause = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("tx_global_pause:") {
                info.tx_pause = v.trim().parse().unwrap_or(0);
            }
        }
    }

    info
}

impl Collector for RdmaCollector {
    async fn collect(&self, inventory: &mut Inventory) -> Result<()> {
        for node in self.collect_devices() {
            if let Some(name) = node.properties.get("name") {
                inventory.rdma_counters.insert(name.clone(), collect_rdma_counters(name));
            }
            inventory.add_node(node);
        }
        for edge in self.collect_edges() {
            // Collect PFC for the netdev side of each RDMA→netdev edge
            if let Some(netdev) = edge.to.strip_prefix("netdev:") {
                inventory.pfc_info.entry(netdev.to_string())
                    .or_insert_with(|| collect_pfc(netdev));
            }
            inventory.add_edge(edge);
        }
        Ok(())
    }
}