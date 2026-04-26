use crate::data::Inventory;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use serde_json::{Map, Value};

pub fn draw(
    f: &mut Frame,
    area: Rect,
    inventory: &Inventory,
    _search_query: &str,
    scroll_offset: u16,
) {
    let json_str = match serde_json::to_string_pretty(&stable_inventory_value(inventory)) {
        Ok(json) => json,
        Err(e) => format!("Error serializing inventory: {}", e),
    };
    let visible_lines = area.height.saturating_sub(2);
    let max_scroll = (json_str.lines().count() as u16).saturating_sub(visible_lines);
    let scroll_offset = scroll_offset.min(max_scroll);

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
        .scroll((scroll_offset, 0))
        .style(Style::default().fg(Color::Gray));

    f.render_widget(paragraph, area);
}

fn stable_inventory_value(inventory: &Inventory) -> Value {
    let mut root = Map::new();

    let mut nodes = inventory.nodes.clone();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    root.insert(
        "nodes".into(),
        Value::Array(
            nodes
                .iter()
                .map(|node| stable_value(serde_json::to_value(node).unwrap_or(Value::Null)))
                .collect(),
        ),
    );

    let mut edges = inventory.edges.clone();
    edges.sort_by(|a, b| {
        (&a.from, &a.to, format!("{:?}", a.edge_type)).cmp(&(
            &b.from,
            &b.to,
            format!("{:?}", b.edge_type),
        ))
    });
    root.insert(
        "edges".into(),
        Value::Array(
            edges
                .iter()
                .map(|edge| stable_value(serde_json::to_value(edge).unwrap_or(Value::Null)))
                .collect(),
        ),
    );

    root.insert(
        "pfc_info".into(),
        stable_value(serde_json::to_value(&inventory.pfc_info).unwrap_or(Value::Null)),
    );
    root.insert(
        "iface_counters".into(),
        stable_value(serde_json::to_value(&inventory.iface_counters).unwrap_or(Value::Null)),
    );
    root.insert(
        "rdma_counters".into(),
        stable_value(serde_json::to_value(&inventory.rdma_counters).unwrap_or(Value::Null)),
    );

    Value::Object(root)
}

fn stable_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(stable_value).collect()),
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();

            let mut sorted = Map::new();
            for key in keys {
                if let Some(value) = map.get(&key) {
                    sorted.insert(key, stable_value(value.clone()));
                }
            }
            Value::Object(sorted)
        }
        other => other,
    }
}
