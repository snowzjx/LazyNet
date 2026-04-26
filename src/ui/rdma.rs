use crate::data::{Inventory, NodeType, RdmaCounters};
use crate::ui::CounterRecording;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
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
    let rdma_devices = inventory.get_nodes_by_type(&NodeType::RdmaDevice);

    let filtered: Vec<_> = if search_query.is_empty() {
        rdma_devices
    } else {
        rdma_devices
            .into_iter()
            .filter(|n| {
                n.get_property("name")
                    .map(|name| name.to_lowercase().contains(&search_query.to_lowercase()))
                    .unwrap_or(false)
            })
            .collect()
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_list(f, chunks[0], inventory, &filtered, search_query, selected_index);

    let selected = filtered.get(selected_index.min(filtered.len().saturating_sub(1)));
    draw_detail(f, chunks[1], inventory, selected, recording);
}

fn draw_list(
    f: &mut Frame,
    area: Rect,
    inventory: &Inventory,
    devices: &[&crate::data::Node],
    search_query: &str,
    selected_index: usize,
) {
    let header = Row::new(
        ["Device", "Transport", "NetDev", "FW Ver", "GUID", "Status"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD))),
    )
    .style(Style::default().bg(Color::Blue));

    let rows: Vec<Row> = devices
        .iter()
        .map(|node| {
            let name = node.get_property("name").cloned().unwrap_or_else(|| "N/A".into());
            let transport = node.get_property("transport").cloned().unwrap_or_else(|| "Unknown".into());
            let fw_ver = node.get_property("fw_ver").cloned().unwrap_or_else(|| "-".into());
            let guid = node.get_property("node_guid").cloned().unwrap_or_else(|| "-".into());

            let netdevs: Vec<String> = inventory
                .find_connected_nodes(&node.id)
                .iter()
                .filter_map(|n| {
                    if matches!(n.node_type, NodeType::NetworkInterface) {
                        n.get_property("name").cloned()
                    } else {
                        None
                    }
                })
                .collect();

            let netdev_str = if netdevs.is_empty() { "None".into() } else { netdevs.join(", ") };
            let connected = !netdevs.is_empty();

            let transport_style = match transport.as_str() {
                "InfiniBand" => Style::default().fg(Color::Cyan),
                "RoCE" => Style::default().fg(Color::Green),
                _ => Style::default().fg(Color::Yellow),
            };
            let status_style = if connected { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Red) };

            Row::new(vec![
                Cell::from(name),
                Cell::from(transport).style(transport_style),
                Cell::from(netdev_str),
                Cell::from(fw_ver),
                Cell::from(guid),
                Cell::from(if connected { "Connected" } else { "Disconnected" }).style(status_style),
            ])
        })
        .collect();

    let total = inventory.get_nodes_by_type(&NodeType::RdmaDevice).len();
    let title = if search_query.is_empty() {
        format!("RDMA Devices ({}) ↑↓ to select", devices.len())
    } else {
        format!("RDMA Devices ({}/{}) - Search: '{}'", devices.len(), total, search_query)
    };

    let mut state = TableState::default();
    if !devices.is_empty() {
        state.select(Some(selected_index));
    }

    f.render_stateful_widget(
        Table::new(rows, [
            Constraint::Length(12), Constraint::Length(12), Constraint::Length(16),
            Constraint::Length(14), Constraint::Length(22), Constraint::Min(12),
        ])
        .header(header)
        .highlight_style(Style::default().bg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title(title)),
        area,
        &mut state,
    );
}

fn draw_detail(
    f: &mut Frame,
    area: Rect,
    inventory: &Inventory,
    node: Option<&&crate::data::Node>,
    recording: Option<&CounterRecording>,
) {
    let Some(node) = node else {
        f.render_widget(
            Paragraph::new("No device selected")
                .block(Block::default().borders(Borders::ALL).title("Detail")),
            area,
        );
        return;
    };

    let name = node.get_property("name").cloned().unwrap_or_default();
    let netdev = inventory
        .find_connected_nodes(&node.id)
        .into_iter()
        .find(|n| matches!(n.node_type, NodeType::NetworkInterface))
        .and_then(|n| n.get_property("name").cloned());

    // Three columns: PFC config | port counters | RoCE/congestion counters
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(37), Constraint::Percentage(38)])
        .split(area);

    // --- PFC Config ---
    let pfc = netdev.as_deref().and_then(|nd| inventory.pfc_info.get(nd));
    let mut pfc_lines = vec![
        Line::from(vec![Span::styled("Device:  ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(name.clone())]),
        Line::from(vec![Span::styled("NetDev:  ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(netdev.clone().unwrap_or_else(|| "-".into()))]),
        Line::from(""),
    ];
    if let Some(pfc) = pfc {
        pfc_lines.push(Line::from(vec![Span::styled("PFC Cap: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(pfc.pfc_cap.to_string())]));
        pfc_lines.push(Line::from(""));
        pfc_lines.push(Line::from(Span::styled("Prio  PFC", Style::default().add_modifier(Modifier::BOLD))));
        pfc_lines.push(Line::from("─────────"));
        for p in 0..8 {
            let (state, style) = if pfc.prio_enabled[p] {
                ("on ", Style::default().fg(Color::Green))
            } else {
                ("off", Style::default().fg(Color::DarkGray))
            };
            pfc_lines.push(Line::from(vec![Span::raw(format!("  {}     ", p)), Span::styled(state, style)]));
        }
        pfc_lines.push(Line::from(""));
        pfc_lines.push(Line::from(Span::styled("Global Pause", Style::default().add_modifier(Modifier::BOLD))));
        pfc_lines.push(Line::from(format!("  RX: {}  TX: {}", pfc.rx_pause, pfc.tx_pause)));
    } else {
        pfc_lines.push(Line::from(Span::styled("PFC data unavailable", Style::default().fg(Color::DarkGray))));
    }
    f.render_widget(
        Paragraph::new(pfc_lines).block(Block::default().borders(Borders::ALL).title("PFC Config")),
        chunks[0],
    );

    // --- Port Counters + RoCE counters ---
    let (rdma_c, counter_title) = match recording {
        Some(CounterRecording::Started(_)) => (
            None,
            format!("Port Counters [RECORDING] — {}", name),
        ),
        Some(CounterRecording::Finished { .. }) => (
            recording.unwrap().rdma_delta(&name),
            format!("Port Counters [DELTA {:.1}s] — {}", recording.unwrap().delta_secs(), name),
        ),
        None => (
            inventory.rdma_counters.get(&name).cloned(),
            format!("Port Counters — {}", name),
        ),
    };

    let port_lines = build_port_lines(rdma_c.as_ref(), recording);
    f.render_widget(
        Paragraph::new(port_lines).block(Block::default().borders(Borders::ALL).title(counter_title)),
        chunks[1],
    );

    let roce_title = match recording {
        Some(CounterRecording::Started(_)) => "RoCE / Congestion [RECORDING]".into(),
        Some(CounterRecording::Finished { .. }) => format!("RoCE / Congestion [DELTA]"),
        None => "RoCE / Congestion".into(),
    };
    let roce_lines = build_roce_lines(rdma_c.as_ref(), recording);
    f.render_widget(
        Paragraph::new(roce_lines).block(Block::default().borders(Borders::ALL).title(roce_title)),
        chunks[2],
    );
}

fn fmt_bytes(n: u64) -> String {
    if n >= 1_000_000_000 { format!("{:.2}G", n as f64 / 1e9) }
    else if n >= 1_000_000 { format!("{:.2}M", n as f64 / 1e6) }
    else if n >= 1_000     { format!("{:.2}K", n as f64 / 1e3) }
    else                   { n.to_string() }
}

fn warn(n: u64, is_delta: bool) -> Span<'static> {
    if is_delta && n > 0 { Span::styled(" (!)", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)) }
    else                 { Span::raw("    ") }
}

fn stat_line(label: &'static str, val: u64, is_error: bool, is_delta: bool) -> Line<'static> {
    let style = if is_error && is_delta && val > 0 { Style::default().fg(Color::Red) } else { Style::default() };
    Line::from(vec![
        Span::styled(label, Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(format!("{:>12}", fmt_bytes(val)), style),
        if is_error { warn(val, is_delta) } else { Span::raw("") },
    ])
}

fn build_port_lines(c: Option<&RdmaCounters>, recording: Option<&CounterRecording>) -> Vec<Line<'static>> {
    if matches!(recording, Some(CounterRecording::Started(_))) {
        return vec![Line::from(Span::styled("● Recording — press ] to finish", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)))];
    }
    let Some(c) = c else {
        return vec![Line::from("No counter data")];
    };

    let mut lines = Vec::new();
    if let Some(secs) = recording.map(|r| r.delta_secs()).filter(|&s| s > 0.0) {
        let rx_bps = c.port_rcv_data as f64 * 4.0 / secs; // IB data is in 4-byte words
        let tx_bps = c.port_xmit_data as f64 * 4.0 / secs;
        let fmt_rate = |bps: f64| {
            if bps >= 1e9 { format!("{:.2} Gbps", bps * 8.0 / 1e9) }
            else if bps >= 1e6 { format!("{:.2} Mbps", bps * 8.0 / 1e6) }
            else { format!("{:.2} Kbps", bps * 8.0 / 1e3) }
        };
        lines.push(Line::from(vec![
            Span::styled("RX rate: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(fmt_rate(rx_bps), Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("TX rate: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(fmt_rate(tx_bps), Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(""));
    }

    let is_delta = recording.map(|r| matches!(r, CounterRecording::Finished { .. })).unwrap_or(false);

    lines.push(stat_line("RCV data:      ", c.port_rcv_data,      false, is_delta));
    lines.push(stat_line("XMT data:      ", c.port_xmit_data,     false, is_delta));
    lines.push(stat_line("RCV packets:   ", c.port_rcv_packets,   false, is_delta));
    lines.push(stat_line("XMT packets:   ", c.port_xmit_packets,  false, is_delta));
    lines.push(Line::from(""));
    lines.push(stat_line("RCV errors:    ", c.port_rcv_errors,    true, is_delta));
    lines.push(stat_line("XMT discards:  ", c.port_xmit_discards, true, is_delta));
    lines.push(stat_line("XMT wait:      ", c.port_xmit_wait,     true, is_delta));
    lines
}

fn build_roce_lines(c: Option<&RdmaCounters>, recording: Option<&CounterRecording>) -> Vec<Line<'static>> {
    if matches!(recording, Some(CounterRecording::Started(_))) {
        return vec![Line::from(Span::styled("● Recording — press ] to finish", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)))];
    }
    let Some(c) = c else {
        return vec![Line::from("No counter data")];
    };

    let is_delta = recording.map(|r| matches!(r, CounterRecording::Finished { .. })).unwrap_or(false);

    vec![
        Line::from(Span::styled("── Congestion (ECN/CNP) ──", Style::default().fg(Color::Yellow))),
        stat_line("NP CNP sent:       ", c.np_cnp_sent,                false, is_delta),
        stat_line("NP ECN marked:     ", c.np_ecn_marked_roce_packets, false, is_delta),
        stat_line("RP CNP handled:    ", c.rp_cnp_handled,             false, is_delta),
        stat_line("RP CNP ignored:    ", c.rp_cnp_ignored,             true,  is_delta),
        Line::from(""),
        Line::from(Span::styled("── Errors ──", Style::default().fg(Color::Yellow))),
        stat_line("Out of buffer:     ", c.out_of_buffer,                  true, is_delta),
        stat_line("Out of sequence:   ", c.out_of_sequence,                true, is_delta),
        stat_line("Packet seq err:    ", c.packet_seq_err,                 true, is_delta),
        stat_line("RNR NAK retry:     ", c.rnr_nak_retry_err,              true, is_delta),
        stat_line("Transport retries: ", c.req_transport_retries_exceeded, true, is_delta),
        stat_line("Local ACK timeout: ", c.local_ack_timeout_err,          true, is_delta),
        stat_line("ICRC errors:       ", c.rx_icrc_encapsulated,           true, is_delta),
    ]
}
