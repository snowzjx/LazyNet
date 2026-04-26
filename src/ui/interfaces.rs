use crate::data::{Inventory, NodeType};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, inventory: &Inventory, search_query: &str, selected_index: usize) {
    let interfaces = inventory.get_nodes_by_type(&NodeType::NetworkInterface);
    
    let filtered_interfaces = if search_query.is_empty() {
        interfaces
    } else {
        interfaces
            .into_iter()
            .filter(|node| {
                node.get_property("name")
                    .map(|name| name.to_lowercase().contains(&search_query.to_lowercase()))
                    .unwrap_or(false)
            })
            .collect()
    };

    // Split the area into list and details
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Clamp selected_index to valid range
    let selected_index = selected_index.min(filtered_interfaces.len().saturating_sub(1));

    // Draw interface list
    let items: Vec<ListItem> = filtered_interfaces
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let name = node.get_property("name").unwrap_or(&"N/A".to_string()).clone();
            let state = node.get_property("state").unwrap_or(&"unknown".to_string()).clone();
            let mac = node.get_property("mac").unwrap_or(&"N/A".to_string()).clone();
            
            let content = format!("{} ({}) - {}", name, state, mac);
            let style = if i == selected_index {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                match state.as_str() {
                    "up" => Style::default().fg(Color::Green),
                    "down" => Style::default().fg(Color::Red),
                    _ => Style::default().fg(Color::Yellow),
                }
            };
            
            ListItem::new(content).style(style)
        })
        .collect();

    let total_interfaces = inventory.get_nodes_by_type(&NodeType::NetworkInterface).len();
    let title = if search_query.is_empty() {
        format!("Network Interfaces ({})", filtered_interfaces.len())
    } else {
        format!(
            "Network Interfaces ({}/{}) - Search: '{}'",
            filtered_interfaces.len(),
            total_interfaces,
            search_query
        )
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));

    f.render_widget(list, chunks[0]);

    // Draw interface details
    if !filtered_interfaces.is_empty() && selected_index < filtered_interfaces.len() {
        let selected_node = filtered_interfaces[selected_index];
        
        let details = vec![
            format!("Name: {}", selected_node.get_property("name").unwrap_or(&"N/A".to_string())),
            format!("State: {}", selected_node.get_property("state").unwrap_or(&"unknown".to_string())),
            format!("MAC: {}", selected_node.get_property("mac").unwrap_or(&"N/A".to_string())),
            format!("MTU: {}", selected_node.get_property("mtu").unwrap_or(&"N/A".to_string())),
            format!("Flags: {}", selected_node.get_property("flags").unwrap_or(&"N/A".to_string())),
            format!("Type: {}", selected_node.get_property("type").unwrap_or(&"N/A".to_string())),
            format!("ID: {}", selected_node.id),
        ].join("\n");

        let details_paragraph = Paragraph::new(details)
            .block(Block::default().borders(Borders::ALL).title("Interface Details"))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));

        f.render_widget(details_paragraph, chunks[1]);
    } else {
        let no_details = Paragraph::new("No interface selected")
            .block(Block::default().borders(Borders::ALL).title("Interface Details"))
            .style(Style::default().fg(Color::Gray));

        f.render_widget(no_details, chunks[1]);
    }
}