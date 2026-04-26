use crate::data::Inventory;
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
use std::io;

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

pub struct Ui {
    current_tab: Tab,
    search_query: String,
    show_help: bool,
    search_mode: bool,
    selected_index: usize,
}

impl Ui {
    pub fn new() -> Self {
        Self {
            current_tab: Tab::Interfaces,
            search_query: String::new(),
            show_help: false,
            search_mode: false,
            selected_index: 0,
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
                        }
                        KeyCode::Backspace => {
                            self.search_query.pop();
                        }
                        KeyCode::Char(c) => {
                            self.search_query.push(c);
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
                        KeyCode::Up => {
                            if self.selected_index > 0 {
                                self.selected_index -= 1;
                            }
                        }
                        KeyCode::Down => {
                            self.selected_index += 1;
                        }
                        KeyCode::Esc => {
                            self.search_query.clear();
                            self.show_help = false;
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
            Tab::Interfaces => interfaces::draw(f, area, inventory, &self.search_query, self.selected_index),
            Tab::Rdma => rdma::draw(f, area, inventory, &self.search_query),
            Tab::Pci => pci::draw(f, area, inventory, &self.search_query),
            Tab::Raw => raw::draw(f, area, inventory, &self.search_query),
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
        let status_text = if self.search_mode {
            "Enter: Apply search | Esc: Cancel | Type to search"
        } else {
            "q: Quit | h: Help | Tab: Switch tabs | ↑↓: Navigate | /: Search"
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White));

        f.render_widget(status, area);
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