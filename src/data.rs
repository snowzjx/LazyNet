use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
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