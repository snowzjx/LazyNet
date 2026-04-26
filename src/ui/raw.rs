use crate::data::Inventory;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, inventory: &Inventory, _search_query: &str) {
    let json_str = match serde_json::to_string_pretty(inventory) {
        Ok(json) => json,
        Err(e) => format!("Error serializing inventory: {}", e),
    };

    let paragraph = Paragraph::new(json_str)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    "Raw Inventory Data ({} nodes, {} edges)",
                    inventory.nodes.len(),
                    inventory.edges.len()
                ))
                .style(Style::default().fg(Color::White)),
        )
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Gray));

    f.render_widget(paragraph, area);
}