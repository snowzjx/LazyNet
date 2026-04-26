use crate::collectors::{rdma::collect_rdma_counters, read_iface_counters};
use crate::data::{IfaceCounters, Inventory, NodeType, RdmaCounters};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
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
use std::time::Instant;

pub mod interfaces;
pub mod pci;
pub mod rdma;
pub mod raw;

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
    
    pub fn all() -> Vec<Tab> {
        vec![Tab::Interfaces, Tab::Rdma, Tab::Pci, Tab::Raw]
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
                rx_bytes:   e.rx_bytes.saturating_sub(s.rx_bytes),
                tx_bytes:   e.tx_bytes.saturating_sub(s.tx_bytes),
                rx_packets: e.rx_packets.saturating_sub(s.rx_packets),
                tx_packets: e.tx_packets.saturating_sub(s.tx_packets),
                rx_errors:  e.rx_errors.saturating_sub(s.rx_errors),
                tx_errors:  e.tx_errors.saturating_sub(s.tx_errors),
                rx_dropped: e.rx_dropped.saturating_sub(s.rx_dropped),
                tx_dropped: e.tx_dropped.saturating_sub(s.tx_dropped),
                rx_missed:  e.rx_missed.saturating_sub(s.rx_missed),
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
                port_rcv_data:                  e.port_rcv_data.saturating_sub(s.port_rcv_data),
                port_xmit_data:                 e.port_xmit_data.saturating_sub(s.port_xmit_data),
                port_rcv_packets:               e.port_rcv_packets.saturating_sub(s.port_rcv_packets),
                port_xmit_packets:              e.port_xmit_packets.saturating_sub(s.port_xmit_packets),
                port_rcv_errors:                e.port_rcv_errors.saturating_sub(s.port_rcv_errors),
                port_xmit_discards:             e.port_xmit_discards.saturating_sub(s.port_xmit_discards),
                port_xmit_wait:                 e.port_xmit_wait.saturating_sub(s.port_xmit_wait),
                np_cnp_sent:                    e.np_cnp_sent.saturating_sub(s.np_cnp_sent),
                np_ecn_marked_roce_packets:     e.np_ecn_marked_roce_packets.saturating_sub(s.np_ecn_marked_roce_packets),
                rp_cnp_handled:                 e.rp_cnp_handled.saturating_sub(s.rp_cnp_handled),
                rp_cnp_ignored:                 e.rp_cnp_ignored.saturating_sub(s.rp_cnp_ignored),
                out_of_buffer:                  e.out_of_buffer.saturating_sub(s.out_of_buffer),
                out_of_sequence:                e.out_of_sequence.saturating_sub(s.out_of_sequence),
                packet_seq_err:                 e.packet_seq_err.saturating_sub(s.packet_seq_err),
                rnr_nak_retry_err:              e.rnr_nak_retry_err.saturating_sub(s.rnr_nak_retry_err),
                req_transport_retries_exceeded: e.req_transport_retries_exceeded.saturating_sub(s.req_transport_retries_exceeded),
                local_ack_timeout_err:          e.local_ack_timeout_err.saturating_sub(s.local_ack_timeout_err),
                rx_icrc_encapsulated:           e.rx_icrc_encapsulated.saturating_sub(s.rx_icrc_encapsulated),
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
    pub recording: Option<CounterRecording>,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            current_tab: Tab::Interfaces,
            search_query: String::new(),
            show_help: false,
            search_mode: false,
            selected_index: 0,
            recording: None,
        }
    }

    pub async fn run(&mut self, inventory: &Inventory) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal, inventory).await;

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
        inventory: &Inventory,
    ) -> Result<()> {
        loop {
            terminal.draw(|f| self.draw(f, inventory))?;

            if let Event::Key(key) = event::read()? {
                if self.search_mode {
                    match key.code {
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
                    }
                } else {
                    match key.code {
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
                        KeyCode::Char('4') => {
                            self.current_tab = Tab::Raw;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('/') => {
                            self.search_mode = true;
                        }
                        KeyCode::Char('[') => {
                            self.recording = Some(CounterRecording::Started(live_snapshot(inventory)));
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
                            if self.selected_index > 0 {
                                self.selected_index -= 1;
                            }
                        }
                        KeyCode::Down => {
                            let len = self.current_list_len(inventory);
                            if len > 0 && self.selected_index + 1 < len {
                                self.selected_index += 1;
                            }
                        }
                        KeyCode::Esc => {
                            self.search_query.clear();
                            self.show_help = false;
                            self.recording = None;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn draw(&self, f: &mut Frame, inventory: &Inventory) {
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Main content area
                Constraint::Length(1), // Status bar
            ])
            .split(f.size());

        // Split main content area
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                if self.search_mode { Constraint::Length(3) } else { Constraint::Length(0) }, // Search
            ])
            .split(main_chunks[0]);

        self.draw_tabs(f, content_chunks[0]);
        self.draw_content(f, content_chunks[1], inventory);

        if self.search_mode {
            self.draw_search_input(f, content_chunks[2]);
        }

        // Always draw status bar at the bottom
        self.draw_status_bar(f, main_chunks[1]);

        if self.show_help {
            self.draw_help(f);
        }
    }

    fn draw_tabs(&self, f: &mut Frame, area: Rect) {
        let titles: Vec<Line> = Tab::all()
            .iter()
            .map(|tab| Line::from(tab.as_str()))
            .collect();

        let selected_index = Tab::all()
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
            Tab::Interfaces => interfaces::draw(f, area, inventory, &self.search_query, self.selected_index, self.recording.as_ref()),
            Tab::Rdma => rdma::draw(f, area, inventory, &self.search_query, self.selected_index, self.recording.as_ref()),
            Tab::Pci => pci::draw(f, area, inventory, &self.search_query, self.selected_index),
            Tab::Raw => raw::draw(f, area, inventory, &self.search_query),
        }
    }

    fn current_list_len(&self, inventory: &Inventory) -> usize {
        let q = self.search_query.to_lowercase();
        match self.current_tab {
            Tab::Interfaces => inventory.get_nodes_by_type(&NodeType::NetworkInterface)
                .iter().filter(|n| q.is_empty() || n.get_property("name").map(|v| v.to_lowercase().contains(&q)).unwrap_or(false)).count(),
            Tab::Rdma => inventory.get_nodes_by_type(&NodeType::RdmaDevice)
                .iter().filter(|n| q.is_empty() || n.get_property("name").map(|v| v.to_lowercase().contains(&q)).unwrap_or(false)).count(),
            Tab::Pci => inventory.get_nodes_by_type(&NodeType::PciDevice)
                .iter().filter(|n| q.is_empty() || ["pci_id","vendor","device","class","driver"].iter()
                    .any(|k| n.get_property(k).map(|v| v.to_lowercase().contains(&q)).unwrap_or(false))).count(),
            Tab::Raw => 0,
        }
    }

    fn draw_help(&self, f: &mut Frame) {
        let area = centered_rect(60, 50, f.size());
        f.render_widget(Clear, area);

        let help_text = vec![
            Line::from("LazyNet - Network Device Inspector"),
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
                Span::styled("Tab/Shift+Tab", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Switch tabs"),
            ]),
            Line::from(vec![
                Span::styled("1-4", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Jump to tab"),
            ]),
            Line::from(vec![
                Span::styled("↑/↓", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Navigate items"),
            ]),
            Line::from(vec![
                Span::styled("/", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Search"),
            ]),
            Line::from(vec![
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Clear search/Close help"),
            ]),
            Line::from(vec![
                Span::styled("[", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Start counter recording"),
            ]),
            Line::from(vec![
                Span::styled("]", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" - Finish recording / show delta"),
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
            ("Enter: Apply search | Esc: Cancel | Type to search".into(),
             Style::default().bg(Color::DarkGray).fg(Color::White))
        } else {
            match &self.recording {
                Some(CounterRecording::Started(_)) => (
                    "● REC  ] Finish recording  |  [ Restart  |  q: Quit | Tab: Switch tabs | ↑↓: Navigate".into(),
                    Style::default().bg(Color::Red).fg(Color::White),
                ),
                Some(CounterRecording::Finished { start, end }) => {
                    let secs = end.taken_at.duration_since(start.taken_at).as_secs_f64();
                    (format!("■ DELTA ({:.1}s)  [ New recording  |  Esc: Clear  |  q: Quit | Tab: Switch tabs | ↑↓: Navigate", secs),
                     Style::default().bg(Color::Green).fg(Color::Black))
                }
                None => (
                    "q: Quit | h: Help | Tab: Switch tabs | ↑↓: Navigate | /: Search | [: Start recording".into(),
                    Style::default().bg(Color::DarkGray).fg(Color::White),
                ),
            }
        };
        f.render_widget(Paragraph::new(text).style(style), area);
    }

    fn next_tab(&mut self) {
        let tabs = Tab::all();
        let current_index = tabs.iter().position(|&tab| tab == self.current_tab).unwrap_or(0);
        let next_index = (current_index + 1) % tabs.len();
        self.current_tab = tabs[next_index];
    }

    fn prev_tab(&mut self) {
        let tabs = Tab::all();
        let current_index = tabs.iter().position(|&tab| tab == self.current_tab).unwrap_or(0);
        let prev_index = if current_index == 0 {
            tabs.len() - 1
        } else {
            current_index - 1
        };
        self.current_tab = tabs[prev_index];
    }
}

/// Read current counters live from sysfs for all known devices.
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
        .filter_map(|n| n.get_property("name"))
        .map(|name| (name.clone(), collect_rdma_counters(name)))
        .collect();

    CounterSnapshot { counters, rdma_counters, taken_at: Instant::now() }
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