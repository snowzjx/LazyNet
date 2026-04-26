use crate::data::{Inventory, NodeType};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, inventory: &Inventory, search_query: &str) {
    let rdma_devices = inventory.get_nodes_by_type(&NodeType::RdmaDevice);
    
    let filtered_devices = if search_query.is_empty() {
        rdma_devices
    } else {
        rdma_devices
            .into_iter()
            .filter(|node| {
                node.get_property("name")
                    .map(|name| name.to_lowercase().contains(&search_query.to_lowercase()))
                    .unwrap_or(false)
            })
            .collect()
    };

    let header_cells = ["Device", "Transport", "Connected NetDev", "Status"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).style(Style::default().bg(Color::Blue));

    let rows: Vec<Row> = filtered_devices
        .iter()
        .map(|node| {
            let name = node.get_property("name").unwrap_or(&"N/A".to_string()).clone();
            let transport = node.get_property("transport").unwrap_or(&"Unknown".to_string()).clone();
            
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

            let transport_style = match transport.as_str() {
                "InfiniBand" => Style::default().fg(Color::Cyan),
                "RoCE" => Style::default().fg(Color::Green),
                _ => Style::default().fg(Color::Yellow),
            };

            let status = if connected_netdevs.is_empty() {
                "Disconnected"
            } else {
                "Connected"
            };

            let status_style = if connected_netdevs.is_empty() {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            };

            Row::new(vec![
                Cell::from(name),
                Cell::from(transport).style(transport_style),
                Cell::from(netdev_str),
                Cell::from(status).style(status_style),
            ])
        })
        .collect();

    let total_devices = inventory.get_nodes_by_type(&NodeType::RdmaDevice).len();
    let title = if search_query.is_empty() {
        format!("RDMA Devices ({})", filtered_devices.len())
    } else {
        format!(
            "RDMA Devices ({}/{}) - Search: '{}'",
            filtered_devices.len(),
            total_devices,
            search_query
        )
    };

    let table = Table::new(rows, [
            Constraint::Length(15),
            Constraint::Length(12),
            Constraint::Length(20),
            Constraint::Min(12),
        ])
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(table, area);
}