use crate::data::{Inventory, NodeType};
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
) {
    let pci_devices = inventory.get_nodes_by_type(&NodeType::PciDevice);

    let filtered: Vec<_> = if search_query.is_empty() {
        pci_devices
    } else {
        let q = search_query.to_lowercase();
        pci_devices
            .into_iter()
            .filter(|n| {
                ["pci_id", "vendor", "device", "class", "driver"]
                    .iter()
                    .any(|k| {
                        n.get_property(k)
                            .map(|v| v.to_lowercase().contains(&q))
                            .unwrap_or(false)
                    })
            })
            .collect()
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_list(
        f,
        chunks[0],
        inventory,
        &filtered,
        search_query,
        selected_index,
    );

    let selected = filtered.get(selected_index.min(filtered.len().saturating_sub(1)));
    draw_detail(f, chunks[1], inventory, selected);
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
        [
            "PCI ID", "Class", "Vendor", "Device", "Driver", "Link", "NUMA", "NetDev",
        ]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD))),
    )
    .style(Style::default().bg(Color::Blue));

    let rows: Vec<Row> = devices
        .iter()
        .map(|node| {
            let pci_id = node
                .get_property("pci_id")
                .cloned()
                .unwrap_or_else(|| "-".into());
            let class = node
                .get_property("class")
                .cloned()
                .unwrap_or_else(|| "-".into());
            let vendor = node
                .get_property("vendor")
                .cloned()
                .unwrap_or_else(|| "-".into());
            let device = node
                .get_property("device")
                .cloned()
                .unwrap_or_else(|| "-".into());
            let driver = node
                .get_property("driver")
                .cloned()
                .unwrap_or_else(|| "-".into());
            let numa = node
                .get_property("numa_node")
                .cloned()
                .unwrap_or_else(|| "-".into());

            // Link speed summary "16GT/s x16"
            let speed = node
                .get_property("link_speed")
                .cloned()
                .unwrap_or_else(|| "-".into());
            let width = node
                .get_property("link_width")
                .cloned()
                .unwrap_or_else(|| "-".into());
            let link = if speed == "-" {
                "-".into()
            } else {
                format!("{} x{}", speed.replace(" PCIe", ""), width)
            };

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
            let netdev_str = if netdevs.is_empty() {
                "-".into()
            } else {
                netdevs.join(", ")
            };

            let is_net = !netdevs.is_empty()
                || class.to_lowercase().contains("ethernet")
                || class.to_lowercase().contains("network")
                || class.to_lowercase().contains("infiniband");

            let row_style = if is_net {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(pci_id),
                Cell::from(class),
                Cell::from(vendor),
                Cell::from(device),
                Cell::from(driver),
                Cell::from(link),
                Cell::from(numa),
                Cell::from(netdev_str),
            ])
            .style(row_style)
        })
        .collect();

    let total = inventory.get_nodes_by_type(&NodeType::PciDevice).len();
    let title = if search_query.is_empty() {
        format!("PCI Devices ({}) ↑↓ to select", devices.len())
    } else {
        format!(
            "PCI Devices ({}/{}) - Search: '{}'",
            devices.len(),
            total,
            search_query
        )
    };

    let mut state = TableState::default();
    if !devices.is_empty() {
        state.select(Some(selected_index));
    }

    f.render_stateful_widget(
        Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(20),
                Constraint::Length(20),
                Constraint::Length(28),
                Constraint::Length(16),
                Constraint::Length(18),
                Constraint::Length(6),
                Constraint::Min(10),
            ],
        )
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
) {
    let Some(node) = node else {
        f.render_widget(
            Paragraph::new("No device selected")
                .block(Block::default().borders(Borders::ALL).title("Detail")),
            area,
        );
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: identity
    let p = |key: &str| {
        node.get_property(key)
            .cloned()
            .unwrap_or_else(|| "-".into())
    };

    let left_lines = vec![
        kv("PCI ID", &p("pci_id")),
        kv("Class", &p("class")),
        kv("Vendor", &p("vendor")),
        kv("Device", &p("device")),
        kv("Sub-Vendor", &p("svendor")),
        kv("Sub-Device", &p("sdevice")),
        kv("Rev", &p("rev")),
        kv("Phys Slot", &p("physlot")),
        kv("IOMMU Group", &p("iommugroup")),
        kv("NUMA Node", &p("numa_node")),
        kv("Driver", &p("driver")),
        Line::from(""),
        // Connected netdevs
        Line::from(Span::styled(
            "Connected NetDevs",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];

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

    let mut all_left = left_lines;
    if netdevs.is_empty() {
        all_left.push(Line::from("  -"));
    } else {
        for nd in &netdevs {
            all_left.push(Line::from(format!("  {}", nd)));
        }
    }

    f.render_widget(
        Paragraph::new(all_left).block(Block::default().borders(Borders::ALL).title("Device Info")),
        chunks[0],
    );

    // Right: PCIe link
    let speed = p("link_speed");
    let width = p("link_width");
    let max_speed = p("max_link_speed");
    let max_width = p("max_link_width");

    let at_max_speed = speed == max_speed;
    let at_max_width = width == max_width;

    let link_style = |ok: bool| {
        if ok {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        }
    };

    let right_lines = vec![
        Line::from(Span::styled(
            "PCIe Link",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Current Speed: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(speed.clone(), link_style(at_max_speed)),
            if !at_max_speed {
                Span::styled(" (degraded)", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::styled(
                "Max Speed:     ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(max_speed.clone()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Current Width: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("x{}", width), link_style(at_max_width)),
            if !at_max_width {
                Span::styled(" (degraded)", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::styled(
                "Max Width:     ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("x{}", max_width)),
        ]),
    ];

    f.render_widget(
        Paragraph::new(right_lines)
            .block(Block::default().borders(Borders::ALL).title("PCIe Link")),
        chunks[1],
    );
}

fn kv(key: &'static str, val: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:<14}", key),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(val.to_string()),
    ])
}
