use crate::data::{IfaceCounters, Inventory, NodeType};
use crate::ui::CounterRecording;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

pub fn draw(
    f: &mut Frame,
    area: Rect,
    inventory: &Inventory,
    search_query: &str,
    selected_index: usize,
    recording: Option<&CounterRecording>,
) {
    let interfaces = inventory.get_nodes_by_type(&NodeType::NetworkInterface);

    let filtered: Vec<_> = if search_query.is_empty() {
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

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let selected_index = selected_index.min(filtered.len().saturating_sub(1));

    // List
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|node| {
            let name = node.get_property("name").cloned().unwrap_or_else(|| "N/A".into());
            let state = node.get_property("state").cloned().unwrap_or_else(|| "unknown".into());
            let mac = node.get_property("mac").cloned().unwrap_or_else(|| "N/A".into());
            let content = format!("{} ({}) - {}", name, state, mac);
            let style = match state.as_str() {
                "up" => Style::default().fg(Color::Green),
                "down" => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::Yellow),
            };
            ListItem::new(content).style(style)
        })
        .collect();

    let total = inventory.get_nodes_by_type(&NodeType::NetworkInterface).len();
    let title = if search_query.is_empty() {
        format!("Network Interfaces ({})", filtered.len())
    } else {
        format!("Network Interfaces ({}/{}) - Search: '{}'", filtered.len(), total, search_query)
    };

    let mut list_state = ListState::default();
    if !filtered.is_empty() {
        list_state.select(Some(selected_index));
    }

    f.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)),
        chunks[0],
        &mut list_state,
    );

    // Detail panel
    if filtered.is_empty() {
        f.render_widget(
            Paragraph::new("No interface selected")
                .block(Block::default().borders(Borders::ALL).title("Details"))
                .style(Style::default().fg(Color::Gray)),
            chunks[1],
        );
        return;
    }

    let node = filtered[selected_index];
    let name = node.get_property("name").map(|s| s.as_str()).unwrap_or("N/A");

    let detail_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(chunks[1]);

    // Properties
    let props = [
        format!("Name:   {}", name),
        format!("State:  {}", node.get_property("state").map(|s| s.as_str()).unwrap_or("unknown")),
        format!("MAC:    {}", node.get_property("mac").map(|s| s.as_str()).unwrap_or("N/A")),
        format!("MTU:    {}", node.get_property("mtu").map(|s| s.as_str()).unwrap_or("N/A")),
        format!("Flags:  {}", node.get_property("flags").map(|s| s.as_str()).unwrap_or("N/A")),
    ]
    .join("\n");

    f.render_widget(
        Paragraph::new(props)
            .block(Block::default().borders(Borders::ALL).title("Interface Details"))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White)),
        detail_chunks[0],
    );

    // Counters / Delta
    let (counter_title, counter_lines, counter_style) = match recording {
        Some(CounterRecording::Started(_)) => {
            let lines = vec![Line::from(Span::styled(
                "● Recording started — press ] to finish",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ))];
            ("Counters [RECORDING]", lines, Style::default().fg(Color::White))
        }
        Some(CounterRecording::Finished { .. }) => {
            let secs = recording.unwrap().delta_secs();
            let delta = recording.unwrap().delta(name);
            let lines = build_counter_lines(delta.as_ref(), Some(secs));
            (
                "Counters [DELTA]",
                lines,
                Style::default().fg(Color::White),
            )
        }
        None => {
            let current = inventory.iface_counters.get(name);
            let lines = build_counter_lines(current, None);
            ("Counters", lines, Style::default().fg(Color::White))
        }
    };

    f.render_widget(
        Paragraph::new(counter_lines)
            .block(Block::default().borders(Borders::ALL).title(counter_title))
            .wrap(Wrap { trim: true })
            .style(counter_style),
        detail_chunks[1],
    );
}

fn build_counter_lines(c: Option<&IfaceCounters>, delta_secs: Option<f64>) -> Vec<Line<'static>> {
    let Some(c) = c else {
        return vec![Line::from("No counter data available")];
    };

    let fmt = |n: u64| -> String {
        if n >= 1_000_000_000 { format!("{:.2}G", n as f64 / 1e9) }
        else if n >= 1_000_000 { format!("{:.2}M", n as f64 / 1e6) }
        else if n >= 1_000     { format!("{:.2}K", n as f64 / 1e3) }
        else                   { n.to_string() }
    };
    let warn = |n: u64| -> Span<'static> {
        if n > 0 {
            Span::styled(" (!)", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            Span::raw("    ")
        }
    };

    let mut lines: Vec<Line<'static>> = Vec::new();

    if let Some(secs) = delta_secs {
        let rx_bps = c.rx_bytes as f64 / secs;
        let tx_bps = c.tx_bytes as f64 / secs;
        let fmt_rate = |bps: f64| -> String {
            if bps >= 1e9 { format!("{:.2} Gbps", bps * 8.0 / 1e9) }
            else if bps >= 1e6 { format!("{:.2} Mbps", bps * 8.0 / 1e6) }
            else if bps >= 1e3 { format!("{:.2} Kbps", bps * 8.0 / 1e3) }
            else { format!("{:.0} bps", bps * 8.0) }
        };
        lines.push(Line::from(vec![
            Span::styled("Duration: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:.3}s", secs)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("RX rate:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(fmt_rate(rx_bps), Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled("TX rate:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(fmt_rate(tx_bps), Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(""));
    }

    let row = |label_rx: &'static str, rx: u64, label_tx: &'static str, tx: u64| -> Line<'static> {
        Line::from(vec![
            Span::styled(label_rx, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:>10}", fmt(rx))),
            warn(rx),
            Span::raw("  "),
            Span::styled(label_tx, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:>10}", fmt(tx))),
            warn(tx),
        ])
    };

    lines.push(row("RX bytes:   ", c.rx_bytes,   "TX bytes:   ", c.tx_bytes));
    lines.push(row("RX packets: ", c.rx_packets, "TX packets: ", c.tx_packets));
    lines.push(row("RX errors:  ", c.rx_errors,  "TX errors:  ", c.tx_errors));
    lines.push(row("RX dropped: ", c.rx_dropped, "TX dropped: ", c.tx_dropped));
    lines.push(row("RX missed:  ", c.rx_missed,  "Collisions: ", c.collisions));

    lines
}
