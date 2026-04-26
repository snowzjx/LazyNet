use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RDMA port counters from sysfs (ports/1/counters + ports/1/hw_counters).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RdmaCounters {
    // Standard port counters
    pub port_rcv_data: u64,
    pub port_xmit_data: u64,
    pub port_rcv_packets: u64,
    pub port_xmit_packets: u64,
    pub port_rcv_errors: u64,
    pub port_xmit_discards: u64,
    pub port_xmit_wait: u64,
    // RoCE/congestion hw_counters
    pub np_cnp_sent: u64,
    pub np_ecn_marked_roce_packets: u64,
    pub rp_cnp_handled: u64,
    pub rp_cnp_ignored: u64,
    pub out_of_buffer: u64,
    pub out_of_sequence: u64,
    pub packet_seq_err: u64,
    pub rnr_nak_retry_err: u64,
    pub req_transport_retries_exceeded: u64,
    pub local_ack_timeout_err: u64,
    pub rx_icrc_encapsulated: u64,
}

/// Traffic counters for a network interface (from /sys/class/net/<dev>/statistics/).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IfaceCounters {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
    pub rx_missed: u64,
    pub collisions: u64,
}

/// PFC (Priority Flow Control) settings and counters for a network device.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PfcInfo {
    /// Per-priority PFC enabled state (index 0-7)
    pub prio_enabled: [bool; 8],
    /// PFC capability (max priorities)
    pub pfc_cap: u8,
    /// Per-priority RX PFC pause frames received (index 0-7)
    pub rx_pfc: [u64; 8],
    /// Per-priority TX PFC pause frames sent (index 0-7)
    pub tx_pfc: [u64; 8],
    /// Global RX pause frames
    pub rx_pause: u64,
    /// Global TX pause frames
    pub tx_pause: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// PFC info keyed by netdev name
    pub pfc_info: HashMap<String, PfcInfo>,
    /// Interface counters keyed by netdev name
    pub iface_counters: HashMap<String, IfaceCounters>,
    /// RDMA port counters keyed by RDMA device name
    pub rdma_counters: HashMap<String, RdmaCounters>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    NetworkInterface,
    PciDevice,
    RdmaDevice,
    DpdkDevice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeType {
    PciBinding,
    RdmaMapping,
    DpdkBinding,
    PhysicalConnection,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            pfc_info: HashMap::new(),
            iface_counters: HashMap::new(),
            rdma_counters: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    pub fn get_nodes_by_type(&self, node_type: &NodeType) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| std::mem::discriminant(&node.node_type) == std::mem::discriminant(node_type))
            .collect()
    }

    pub fn find_connected_nodes(&self, node_id: &str) -> Vec<&Node> {
        let connected_ids: Vec<&String> = self
            .edges
            .iter()
            .filter_map(|edge| {
                if edge.from == node_id {
                    Some(&edge.to)
                } else if edge.to == node_id {
                    Some(&edge.from)
                } else {
                    None
                }
            })
            .collect();

        self.nodes
            .iter()
            .filter(|node| connected_ids.contains(&&node.id))
            .collect()
    }
}

impl Node {
    pub fn new(id: String, node_type: NodeType) -> Self {
        Self {
            id,
            node_type,
            properties: HashMap::new(),
        }
    }

    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.properties.insert(key.to_string(), value.to_string());
        self
    }

    pub fn get_property(&self, key: &str) -> Option<&String> {
        self.properties.get(key)
    }
}

impl Edge {
    pub fn new(from: String, to: String, edge_type: EdgeType) -> Self {
        Self {
            from,
            to,
            edge_type,
            properties: HashMap::new(),
        }
    }

    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.properties.insert(key.to_string(), value.to_string());
        self
    }
}