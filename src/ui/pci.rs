use crate::data::{Inventory, NodeType};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, inventory: &Inventory, search_query: &str) {
    let pci_devices = inventory.get_nodes_by_type(&NodeType::PciDevice);
    
    let filtered_devices = if search_query.is_empty() {
        pci_devices
    } else {
        pci_devices
            .into_iter()
            .filter(|node| {
                let pci_id_match = node
                    .get_property("pci_id")
                    .map(|id| id.to_lowercase().contains(&search_query.to_lowercase()))
                    .unwrap_or(false);
                let desc_match = node
                    .get_property("description")
                    .map(|desc| desc.to_lowercase().contains(&search_query.to_lowercase()))
                    .unwrap_or(false);
                pci_id_match || desc_match
            })
            .collect()
    };

    let header_cells = ["PCI ID", "Description", "Connected NetDev"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).style(Style::default().bg(Color::Blue));

    let rows: Vec<Row> = filtered_devices
        .iter()
        .map(|node| {
            let pci_id = node.get_property("pci_id").unwrap_or(&"N/A".to_string()).clone();
            let description = node.get_property("description").unwrap_or(&"N/A".to_string()).clone();
            
            // Find connected network devices
            let connected_netdevs: Vec<String> = inventory
                .find_connected_nodes(&node.id)
                .iter()
                .filter_map(|connected_node| {
                    if matches!(connected_node.node_type, NodeType::NetworkInterface) {
                        connected_node.get_property("name").cloned()
                    } else {
                        None
                    }
                })
                .collect();
            
            let netdev_str = if connected_netdevs.is_empty() {
                "None".to_string()
            } else {
                connected_netdevs.join(", ")
            };

            // Highlight network-related PCI devices
            let desc_style = if description.to_lowercase().contains("network") 
                || description.to_lowercase().contains("ethernet") 
                || description.to_lowercase().contains("infiniband") {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(pci_id),
                Cell::from(description).style(desc_style),
                Cell::from(netdev_str),
            ])
        })
        .collect();

    let total_devices = inventory.get_nodes_by_type(&NodeType::PciDevice).len();
    let title = if search_query.is_empty() {
        format!("PCI Devices ({})", filtered_devices.len())
    } else {
        format!(
            "PCI Devices ({}/{}) - Search: '{}'",
            filtered_devices.len(),
            total_devices,
            search_query
        )
    };

    let table = Table::new(rows, [
            Constraint::Length(12),
            Constraint::Min(40),
            Constraint::Length(20),
        ])
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(table, area);
}