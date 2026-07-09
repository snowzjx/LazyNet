use crate::collectors::{
    pci::PciCollector,
    rdma::{collect_rdma_counters, RdmaPort},
    read_iface_counters, Collector, NetworkCollector,
};
use crate::config::{CollectorConfig, UiConfig};
use crate::data::{IfaceCounters, Inventory, Node, NodeType, RdmaCounters};
use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEvent,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

pub mod interfaces;
pub mod pci;
pub mod raw;
pub mod rdma;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Interfaces,
    Rdma,
    Pci,
    Raw,
}

impl Tab {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tab::Interfaces => "Interfaces",
            Tab::Rdma => "RDMA",
            Tab::Pci => "PCI",
            Tab::Raw => "Raw",
        }
    }

    pub fn all(show_raw: bool) -> Vec<Tab> {
        let mut tabs = vec![Tab::Interfaces, Tab::Rdma, Tab::Pci];
        if show_raw {
            tabs.push(Tab::Raw);
        }
        tabs
    }

    pub fn from_config(value: &str, show_raw: bool) -> Self {
        match value.to_lowercase().as_str() {
            "rdma" => Tab::Rdma,
            "pci" => Tab::Pci,
            "raw" if show_raw => Tab::Raw,
            _ => Tab::Interfaces,
        }
    }
}

/// Holds a counter snapshot taken at a point in time.
pub struct CounterSnapshot {
    pub counters: HashMap<String, IfaceCounters>,
    pub rdma_counters: HashMap<String, RdmaCounters>,
    pub taken_at: Instant,
}

/// State for the [ ] counter recording feature.
pub enum CounterRecording {
    /// [ pressed — start snapshot captured
    Started(CounterSnapshot),
    /// ] pressed — both snapshots captured, delta ready
    Finished {
        start: CounterSnapshot,
        end: CounterSnapshot,
    },
}

impl CounterRecording {
    pub fn delta_secs(&self) -> f64 {
        match self {
            CounterRecording::Finished { start, end } => {
                end.taken_at.duration_since(start.taken_at).as_secs_f64()
            }
            _ => 0.0,
        }
    }

    pub fn delta(&self, dev: &str) -> Option<IfaceCounters> {
        if let CounterRecording::Finished { start, end } = self {
            let s = start.counters.get(dev)?;
            let e = end.counters.get(dev)?;
            Some(IfaceCounters {
                rx_bytes: e.rx_bytes.saturating_sub(s.rx_bytes),
                tx_bytes: e.tx_bytes.saturating_sub(s.tx_bytes),
                rx_packets: e.rx_packets.saturating_sub(s.rx_packets),
                tx_packets: e.tx_packets.saturating_sub(s.tx_packets),
                rx_errors: e.rx_errors.saturating_sub(s.rx_errors),
                tx_errors: e.tx_errors.saturating_sub(s.tx_errors),
                rx_dropped: e.rx_dropped.saturating_sub(s.rx_dropped),
                tx_dropped: e.tx_dropped.saturating_sub(s.tx_dropped),
                rx_missed: e.rx_missed.saturating_sub(s.rx_missed),
                collisions: e.collisions.saturating_sub(s.collisions),
            })
        } else {
            None
        }
    }

    pub fn rdma_delta(&self, dev: &str) -> Option<RdmaCounters> {
        if let CounterRecording::Finished { start, end } = self {
            let s = start.rdma_counters.get(dev)?;
            let e = end.rdma_counters.get(dev)?;
            Some(RdmaCounters {
                port_rcv_data: e.port_rcv_data.saturating_sub(s.port_rcv_data),
                port_xmit_data: e.port_xmit_data.saturating_sub(s.port_xmit_data),
                port_rcv_packets: e.port_rcv_packets.saturating_sub(s.port_rcv_packets),
                port_xmit_packets: e.port_xmit_packets.saturating_sub(s.port_xmit_packets),
                port_rcv_errors: e.port_rcv_errors.saturating_sub(s.port_rcv_errors),
                port_xmit_discards: e.port_xmit_discards.saturating_sub(s.port_xmit_discards),
                port_xmit_wait: e.port_xmit_wait.saturating_sub(s.port_xmit_wait),
                np_cnp_sent: e.np_cnp_sent.saturating_sub(s.np_cnp_sent),
                np_ecn_marked_roce_packets: e
                    .np_ecn_marked_roce_packets
                    .saturating_sub(s.np_ecn_marked_roce_packets),
                rp_cnp_handled: e.rp_cnp_handled.saturating_sub(s.rp_cnp_handled),
                rp_cnp_ignored: e.rp_cnp_ignored.saturating_sub(s.rp_cnp_ignored),
                out_of_buffer: e.out_of_buffer.saturating_sub(s.out_of_buffer),
                out_of_sequence: e.out_of_sequence.saturating_sub(s.out_of_sequence),
                packet_seq_err: e.packet_seq_err.saturating_sub(s.packet_seq_err),
                rnr_nak_retry_err: e.rnr_nak_retry_err.saturating_sub(s.rnr_nak_retry_err),
                req_transport_retries_exceeded: e
                    .req_transport_retries_exceeded
                    .saturating_sub(s.req_transport_retries_exceeded),
                local_ack_timeout_err: e
                    .local_ack_timeout_err
                    .saturating_sub(s.local_ack_timeout_err),
                rx_icrc_encapsulated: e
                    .rx_icrc_encapsulated
                    .saturating_sub(s.rx_icrc_encapsulated),
            })
        } else {
            None
        }
    }
}

pub struct Ui {
    current_tab: Tab,
    search_query: String,
    show_help: bool,
    search_mode: bool,
    selected_index: usize,
    raw_scroll_offset: u16,
    pub recording: Option<CounterRecording>,
    refresh_interval: Duration,
    show_raw_tab: bool,
}

impl Ui {
    pub fn new(config: &UiConfig) -> Self {
        let show_raw_tab = config.show_raw_tab;
        Self {
            current_tab: Tab::from_config(&config.default_tab, show_raw_tab),
            search_query: String::new(),
            show_help: false,
            search_mode: false,
            selected_index: 0,
            raw_scroll_offset: 0,
            recording: None,
            refresh_interval: Duration::from_millis(config.refresh_interval_ms.max(100)),
            show_raw_tab,
        }
    }

    pub async fn run(
        &mut self,
        inventory: &mut Inventory,
        collectors: CollectorConfig,
    ) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal, inventory, &collectors).await;

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn run_app<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        inventory: &mut Inventory,
        collectors: &CollectorConfig,
    ) -> Result<()> {
        let mut last_refresh = Instant::now();
        loop {
            terminal.draw(|f| self.draw(f, inventory))?;

            let elapsed = last_refresh.elapsed();
            let timeout = self.refresh_interval.saturating_sub(elapsed);

            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) if self.search_mode => match key.code {
                        KeyCode::Enter => {
                            self.search_mode = false;
                        }
                        KeyCode::Esc => {
                            self.search_mode = false;
                            self.search_query.clear();
                            self.selected_index = 0;
                        }
                        KeyCode::Backspace => {
                            self.search_query.pop();
                            self.selected_index = 0;
                        }
                        KeyCode::Char(c) => {
                            self.search_query.push(c);
                            self.selected_index = 0;
                        }
                        _ => {}
                    },
                    Event::Key(key) => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('h') | KeyCode::F(1) => {
                            self.show_help = !self.show_help;
                        }
                        KeyCode::Tab => {
                            self.next_tab();
                            self.selected_index = 0;
                        }
                        KeyCode::BackTab => {
                            self.prev_tab();
                            self.selected_index = 0;
                        }
                        KeyCode::Char('1') => {
                            self.current_tab = Tab::Interfaces;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('2') => {
                            self.current_tab = Tab::Rdma;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('3') => {
                            self.current_tab = Tab::Pci;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('4') if self.show_raw_tab => {
                            self.current_tab = Tab::Raw;
                            self.selected_index = 0;
                            self.raw_scroll_offset = 0;
                        }
                        KeyCode::Char('/') => {
                            self.search_mode = true;
                        }
                        KeyCode::Char('[') => {
                            self.recording =
                                Some(CounterRecording::Started(live_snapshot(inventory)));
                        }
                        KeyCode::Char(']') => {
                            if let Some(CounterRecording::Started(start)) = self.recording.take() {
                                self.recording = Some(CounterRecording::Finished {
                                    start,
                                    end: live_snapshot(inventory),
                                });
                            }
                        }
                        KeyCode::Up => {
                            if self.current_tab == Tab::Raw {
                                self.raw_scroll_offset = self.raw_scroll_offset.saturating_sub(1);
                            } else if self.selected_index > 0 {
                                self.selected_index -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if self.current_tab == Tab::Raw {
                                self.raw_scroll_offset = self.raw_scroll_offset.saturating_add(1);
                            } else {
                                let len = self.current_list_len(inventory);
                                if len > 0 && self.selected_index + 1 < len {
                                    self.selected_index += 1;
                                }
                            }
                        }
                        KeyCode::PageUp if self.current_tab == Tab::Raw => {
                            self.raw_scroll_offset = self
                                .raw_scroll_offset
                                .saturating_sub(raw_page_step(terminal));
                        }
                        KeyCode::PageDown => {
                            if self.current_tab == Tab::Raw {
                                self.raw_scroll_offset = self
                                    .raw_scroll_offset
                                    .saturating_add(raw_page_step(terminal));
                            } else {
                                let len = self.current_list_len(inventory);
                                let step = raw_page_step(terminal) as usize;
                                if len > 0 {
                                    self.selected_index =
                                        (self.selected_index + step).min(len.saturating_sub(1));
                                }
                            }
                        }
                        KeyCode::Home => {
                            if self.current_tab == Tab::Raw {
                                self.raw_scroll_offset = 0;
                            } else {
                                self.selected_index = 0;
                            }
                        }
                        KeyCode::End => {
                            if self.current_tab == Tab::Raw {
                                self.raw_scroll_offset = u16::MAX;
                            } else {
                                let len = self.current_list_len(inventory);
                                self.selected_index = len.saturating_sub(1);
                            }
                        }
                        KeyCode::Char('g') if self.current_tab == Tab::Raw => {
                            self.raw_scroll_offset = 0;
                        }
                        KeyCode::Char('G') if self.current_tab == Tab::Raw => {
                            self.raw_scroll_offset = u16::MAX;
                        }
                        KeyCode::Left => {
                            self.navigate_connected(inventory, DirectionHint::Left);
                        }
                        KeyCode::Right => {
                            self.navigate_connected(inventory, DirectionHint::Right);
                        }
                        KeyCode::Esc => {
                            self.search_query.clear();
                            self.show_help = false;
                            self.recording = None;
                        }
                        _ => {}
                    },
                    Event::Mouse(mouse) => {
                        self.handle_mouse(mouse, terminal, inventory);
                    }
                    _ => {}
                }
                if last_refresh.elapsed() >= self.refresh_interval {
                    refresh_inventory(inventory, collectors).await?;
                    last_refresh = Instant::now();
                }
            } else {
                refresh_inventory(inventory, collectors).await?;
                last_refresh = Instant::now();
            }
        }
    }

    fn draw(&self, f: &mut Frame, inventory: &Inventory) {
        let layout = ui_layout(f.size(), self.search_mode);

        self.draw_tabs(f, layout.tabs);
        self.draw_content(f, layout.content, inventory);

        if let Some(search) = layout.search {
            self.draw_search_input(f, search);
        }

        // Always draw status bar at the bottom
        self.draw_status_bar(f, layout.status);

        if self.show_help {
            self.draw_help(f);
        }
    }

    fn draw_tabs(&self, f: &mut Frame, area: Rect) {
        let tabs = Tab::all(self.show_raw_tab);
        let titles: Vec<Line> = tabs.iter().map(|tab| Line::from(tab.as_str())).collect();

        let selected_index = tabs
            .iter()
            .position(|&tab| tab == self.current_tab)
            .unwrap_or(0);

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("LazyNet"))
            .select(selected_index)
            .style(Style::default().fg(Color::Cyan))
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Black),
            );

        f.render_widget(tabs, area);
    }

    fn draw_content(&self, f: &mut Frame, area: Rect, inventory: &Inventory) {
        match self.current_tab {
            Tab::Interfaces => interfaces::draw(
                f,
                area,
                inventory,
                &self.search_query,
                self.selected_index,
                self.recording.as_ref(),
            ),
            Tab::Rdma => rdma::draw(
                f,
                area,
                inventory,
                &self.search_query,
                self.selected_index,
                self.recording.as_ref(),
            ),
            Tab::Pci => pci::draw(f, area, inventory, &self.search_query, self.selected_index),
            Tab::Raw => raw::draw(
                f,
                area,
                inventory,
                &self.search_query,
                self.raw_scroll_offset,
            ),
        }
    }

    fn current_list_len(&self, inventory: &Inventory) -> usize {
        self.filtered_nodes(inventory, self.current_tab).len()
    }

    fn handle_mouse<B: Backend>(
        &mut self,
        mouse: MouseEvent,
        terminal: &Terminal<B>,
        inventory: &Inventory,
    ) {
        if self.search_mode {
            return;
        }

        let Ok(size) = terminal.size() else {
            return;
        };
        let layout = ui_layout(size, self.search_mode);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.show_help {
                    self.show_help = false;
                    return;
                }

                if rect_contains(layout.tabs, mouse.column, mouse.row) {
                    self.select_tab_at(mouse.column, layout.tabs);
                } else if rect_contains(layout.content, mouse.column, mouse.row) {
                    self.select_content_at(mouse.column, mouse.row, layout.content, inventory);
                }
            }
            MouseEventKind::ScrollUp => {
                self.scroll_current(-3, terminal, inventory);
            }
            MouseEventKind::ScrollDown => {
                self.scroll_current(3, terminal, inventory);
            }
            _ => {}
        }
    }

    fn select_tab_at(&mut self, column: u16, area: Rect) {
        let tabs = Tab::all(self.show_raw_tab);
        let mut start = area.x.saturating_add(1);

        for tab in tabs {
            let width = tab.as_str().len() as u16;
            let end = start.saturating_add(width);
            if column >= start && column < end {
                self.current_tab = tab;
                self.selected_index = 0;
                self.raw_scroll_offset = 0;
                return;
            }
            start = end.saturating_add(3);
        }
    }

    fn select_content_at(&mut self, column: u16, row: u16, area: Rect, inventory: &Inventory) {
        if self.current_tab == Tab::Raw {
            return;
        }

        let Some(list_area) = current_list_area(self.current_tab, area) else {
            return;
        };
        if !rect_contains(list_area, column, row) {
            return;
        }

        let row_offset = match self.current_tab {
            Tab::Interfaces => row.saturating_sub(list_area.y.saturating_add(1)),
            Tab::Rdma | Tab::Pci => row.saturating_sub(list_area.y.saturating_add(2)),
            Tab::Raw => 0,
        };
        let len = self.current_list_len(inventory);
        if len > 0 && row_offset < len as u16 {
            self.selected_index = row_offset as usize;
        }
    }

    fn scroll_current<B: Backend>(
        &mut self,
        amount: i16,
        terminal: &Terminal<B>,
        inventory: &Inventory,
    ) {
        if self.current_tab == Tab::Raw {
            self.raw_scroll_offset = if amount.is_negative() {
                self.raw_scroll_offset.saturating_sub(amount.unsigned_abs())
            } else {
                self.raw_scroll_offset.saturating_add(amount as u16)
            };
            return;
        }

        let len = self.current_list_len(inventory);
        if len == 0 {
            self.selected_index = 0;
            return;
        }

        let step = if amount == 0 {
            raw_page_step(terminal) as isize
        } else {
            amount as isize
        };
        let next = (self.selected_index as isize + step).clamp(0, len.saturating_sub(1) as isize);
        self.selected_index = next as usize;
    }

    fn filtered_nodes<'a>(&self, inventory: &'a Inventory, tab: Tab) -> Vec<&'a Node> {
        let q = self.search_query.to_lowercase();
        filtered_nodes_for_query(inventory, tab, &q)
    }

    fn selected_node_id(&self, inventory: &Inventory) -> Option<String> {
        let nodes = self.filtered_nodes(inventory, self.current_tab);
        nodes
            .get(self.selected_index.min(nodes.len().saturating_sub(1)))
            .map(|node| node.id.clone())
    }

    fn select_node(&mut self, inventory: &Inventory, tab: Tab, node_id: &str) -> bool {
        let nodes = filtered_nodes_for_query(inventory, tab, "");
        let Some(index) = nodes.iter().position(|node| node.id == node_id) else {
            return false;
        };

        self.search_query.clear();
        self.current_tab = tab;
        self.selected_index = index;
        true
    }

    fn navigate_connected(&mut self, inventory: &Inventory, direction: DirectionHint) {
        let Some(node_id) = self.selected_node_id(inventory) else {
            return;
        };

        match self.current_tab {
            Tab::Interfaces => {
                let primary = match direction {
                    DirectionHint::Left => NodeType::PciDevice,
                    DirectionHint::Right => NodeType::RdmaDevice,
                };
                let fallback = match direction {
                    DirectionHint::Left => NodeType::RdmaDevice,
                    DirectionHint::Right => NodeType::PciDevice,
                };

                if let Some(target) = first_connected_node(inventory, &node_id, &primary) {
                    let tab = tab_for_node_type(&target.node_type);
                    self.select_node(inventory, tab, &target.id);
                } else if let Some(target) = first_connected_node(inventory, &node_id, &fallback) {
                    let tab = tab_for_node_type(&target.node_type);
                    self.select_node(inventory, tab, &target.id);
                }
            }
            Tab::Rdma | Tab::Pci => {
                if let Some(target) =
                    first_connected_node(inventory, &node_id, &NodeType::NetworkInterface)
                {
                    self.select_node(inventory, Tab::Interfaces, &target.id);
                }
            }
            Tab::Raw => {}
        }
    }

    fn draw_help(&self, f: &mut Frame) {
        let area = centered_rect(74, 68, f.size());
        f.render_widget(Clear, area);

        let help_text = vec![
            Line::from(vec![Span::styled(
                " _                    _   _      _   ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "| |    __ _ _____   _| \\ | | ___| |_ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "| |   / _` |_  / | | |  \\| |/ _ \\ __|",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "| |__| (_| |/ /| |_| | |\\  |  __/ |_ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "|_____\\__,_/___|\\__, |_| \\_|\\___|\\__|",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "                 |___/                 ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::raw("Network Device Inspector"),
                Span::raw("  |  "),
                Span::styled(
                    "Junxue ZHANG",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Recording workflow",
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled(
                    "[",
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" start snapshot  "),
                Span::styled(
                    "]",
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" finish snapshot  "),
                Span::styled(
                    "Delta view",
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" shows traffic, errors, RDMA/RoCE changes"),
            ]),
            Line::from(""),
            Line::from("Controls"),
            Line::from(""),
            Line::from(vec![
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Quit"),
            ]),
            Line::from(vec![
                Span::styled("h/F1", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Toggle help"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Tab/Shift+Tab",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" - Switch tabs"),
            ]),
            Line::from(vec![
                Span::styled("1-4", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Jump to tab"),
            ]),
            Line::from(vec![
                Span::styled("←/→", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Jump to connected device"),
            ]),
            Line::from(vec![
                Span::styled("↑/↓", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Navigate items / scroll Raw"),
            ]),
            Line::from(vec![
                Span::styled("Mouse", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Click tabs/items, wheel to navigate/scroll"),
            ]),
            Line::from(vec![
                Span::styled(
                    "PageUp/PageDown",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" - Scroll Raw by page"),
            ]),
            Line::from(vec![
                Span::styled("/", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Search"),
            ]),
            Line::from(vec![
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Clear search/Close help"),
            ]),
        ];

        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::White));

        f.render_widget(help, area);
    }

    fn draw_search_input(&self, f: &mut Frame, area: Rect) {
        let search_text = format!("Search: {}", self.search_query);
        let search_input = Paragraph::new(search_text)
            .block(
                Block::default()
                    .title("Search (Enter to apply, Esc to cancel)")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Yellow)),
            )
            .style(Style::default().fg(Color::White));

        f.render_widget(search_input, area);
    }

    fn draw_status_bar(&self, f: &mut Frame, area: Rect) {
        let (text, style) = if self.search_mode {
            (
                "Enter: Apply search | Esc: Cancel | Type to search".into(),
                Style::default().bg(Color::DarkGray).fg(Color::White),
            )
        } else {
            match &self.recording {
                Some(CounterRecording::Started(_)) => (
                    "● REC  ] Finish recording  |  [ Restart  |  ←→ Connected | q: Quit | Tab: Switch tabs | ↑↓: Navigate".into(),
                    Style::default().bg(Color::Red).fg(Color::White),
                ),
                Some(CounterRecording::Finished { start, end }) => {
                    let secs = end.taken_at.duration_since(start.taken_at).as_secs_f64();
                    (format!("■ DELTA ({:.1}s)  [ New recording  |  Esc: Clear  |  ←→ Connected | q: Quit | Tab: Switch tabs | ↑↓: Navigate", secs),
                     Style::default().bg(Color::Green).fg(Color::Black))
                }
                None => (
                    "q: Quit | h: Help | Click tabs/items | Wheel/↑↓: Navigate/Scroll | PgUp/PgDn: Page Raw | ←→: Connected | /: Search | [: Start recording".into(),
                    Style::default().bg(Color::DarkGray).fg(Color::White),
                ),
            }
        };
        f.render_widget(Paragraph::new(text).style(style), area);
    }

    fn next_tab(&mut self) {
        let tabs = Tab::all(self.show_raw_tab);
        let current_index = tabs
            .iter()
            .position(|&tab| tab == self.current_tab)
            .unwrap_or(0);
        let next_index = (current_index + 1) % tabs.len();
        self.current_tab = tabs[next_index];
    }

    fn prev_tab(&mut self) {
        let tabs = Tab::all(self.show_raw_tab);
        let current_index = tabs
            .iter()
            .position(|&tab| tab == self.current_tab)
            .unwrap_or(0);
        let prev_index = if current_index == 0 {
            tabs.len() - 1
        } else {
            current_index - 1
        };
        self.current_tab = tabs[prev_index];
    }
}

struct UiLayout {
    tabs: Rect,
    content: Rect,
    search: Option<Rect>,
    status: Rect,
}

fn ui_layout(area: Rect, search_mode: bool) -> UiLayout {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Main content area
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tabs
            Constraint::Min(0),    // Content
            if search_mode {
                Constraint::Length(3)
            } else {
                Constraint::Length(0)
            }, // Search
        ])
        .split(main_chunks[0]);

    UiLayout {
        tabs: content_chunks[0],
        content: content_chunks[1],
        search: search_mode.then_some(content_chunks[2]),
        status: main_chunks[1],
    }
}

fn current_list_area(tab: Tab, area: Rect) -> Option<Rect> {
    match tab {
        Tab::Interfaces => Some(
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(area)[0],
        ),
        Tab::Rdma => Some(
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(area)[0],
        ),
        Tab::Pci => Some(
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area)[0],
        ),
        Tab::Raw => None,
    }
}

fn rect_contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

#[derive(Debug, Clone, Copy)]
enum DirectionHint {
    Left,
    Right,
}

fn filtered_nodes_for_query<'a>(inventory: &'a Inventory, tab: Tab, q: &str) -> Vec<&'a Node> {
    match tab {
        Tab::Interfaces => inventory
            .get_nodes_by_type(&NodeType::NetworkInterface)
            .iter()
            .filter(|n| {
                q.is_empty()
                    || n.get_property("name")
                        .map(|v| v.to_lowercase().contains(q))
                        .unwrap_or(false)
            })
            .copied()
            .collect(),
        Tab::Rdma => inventory
            .get_nodes_by_type(&NodeType::RdmaDevice)
            .iter()
            .filter(|n| {
                q.is_empty()
                    || ["name", "display_name", "port"].iter().any(|key| {
                        n.get_property(key)
                            .map(|v| v.to_lowercase().contains(q))
                            .unwrap_or(false)
                    })
            })
            .copied()
            .collect(),
        Tab::Pci => inventory
            .get_nodes_by_type(&NodeType::PciDevice)
            .iter()
            .filter(|n| {
                q.is_empty()
                    || ["pci_id", "vendor", "device", "class", "driver"]
                        .iter()
                        .any(|k| {
                            n.get_property(k)
                                .map(|v| v.to_lowercase().contains(q))
                                .unwrap_or(false)
                        })
            })
            .copied()
            .collect(),
        Tab::Raw => Vec::new(),
    }
}

fn first_connected_node<'a>(
    inventory: &'a Inventory,
    node_id: &str,
    node_type: &NodeType,
) -> Option<&'a Node> {
    inventory
        .find_connected_nodes(node_id)
        .into_iter()
        .find(|node| same_node_type(&node.node_type, node_type))
}

fn same_node_type(a: &NodeType, b: &NodeType) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn tab_for_node_type(node_type: &NodeType) -> Tab {
    match node_type {
        NodeType::RdmaDevice => Tab::Rdma,
        NodeType::PciDevice => Tab::Pci,
        _ => Tab::Interfaces,
    }
}

fn raw_page_step<B: Backend>(terminal: &Terminal<B>) -> u16 {
    terminal
        .size()
        .map(|area| area.height.saturating_sub(5).max(1))
        .unwrap_or(10)
}

/// Read current counters live for all known devices.
fn live_snapshot(inventory: &Inventory) -> CounterSnapshot {
    let counters = inventory
        .get_nodes_by_type(&NodeType::NetworkInterface)
        .iter()
        .filter_map(|n| n.get_property("name"))
        .map(|name| (name.clone(), read_iface_counters(name)))
        .collect();

    let rdma_counters = inventory
        .get_nodes_by_type(&NodeType::RdmaDevice)
        .iter()
        .filter_map(|n| RdmaPort::from_node(n))
        .map(|port| (port.key(), collect_rdma_counters(&port.device, &port.port)))
        .collect();

    CounterSnapshot {
        counters,
        rdma_counters,
        taken_at: Instant::now(),
    }
}

async fn refresh_inventory(inventory: &mut Inventory, config: &CollectorConfig) -> Result<()> {
    let mut next = Inventory::new();

    if config.enable_network {
        NetworkCollector::new().collect(&mut next).await?;
    }
    if config.enable_pci {
        PciCollector::new().collect(&mut next).await?;
    }
    if config.enable_rdma {
        crate::collectors::rdma::RdmaCollector::new()
            .collect(&mut next)
            .await?;
    }

    *inventory = next;
    Ok(())
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
