use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame,
};
use ssmgr_shared::{PlaybackMode, Sample, DEFAULT_CATEGORIES};
use std::time::Duration;
use tracing::info;

use crate::api::ApiClient;
use crate::player::AudioPlayer;
use crate::state::ClientState;

pub enum InputMode {
    Normal,
    Search,
    Category,
    Message,
}

pub struct App {
    pub state: ClientState,
    pub api: ApiClient,
    pub player: Option<AudioPlayer>,
    pub input_mode: InputMode,
    pub search_input: String,
    pub category_input: String,
    pub message: String,
    pub message_timer: u64,
    pub selected_index: usize,
    pub filtered_samples: Vec<Sample>,
    pub categories: Vec<String>,
    pub should_quit: bool,
    pub show_help: bool,
    pub selected_tab: usize,
    pub loading: bool,
    pub server_connected: bool,
}

impl App {
    pub fn new(state: ClientState, api: ApiClient, player: Option<AudioPlayer>) -> Self {
        let mut app = Self {
            state,
            api,
            player,
            input_mode: InputMode::Normal,
            search_input: String::new(),
            category_input: String::new(),
            message: String::new(),
            message_timer: 0,
            selected_index: 0,
            filtered_samples: Vec::new(),
            categories: Vec::new(),
            should_quit: false,
            show_help: false,
            selected_tab: 0,
            loading: false,
            server_connected: false,
        };
        app
    }

    pub async fn sync_samples(&mut self) {
        self.loading = true;
        let connected = self.api.health_check().await;
        self.server_connected = connected;
        {
            let mut conn = self.state.server_connected.write().await;
            *conn = connected;
        }

        if connected {
            match self.api.get_samples(None, None, None).await {
                Ok(samples) => {
                    self.state.set_samples(samples).await;
                    self.set_message("Synced samples from server".to_string());
                }
                Err(e) => self.set_message(format!("Sync failed: {}", e)),
            }
        } else {
            self.set_message("Cannot connect to server".to_string());
        }

        self.categories = self.state.get_all_categories().await;
        self.update_filtered_samples().await;
        self.loading = false;
    }

    pub async fn update_filtered_samples(&mut self) {
        self.filtered_samples = self.state.get_filtered_samples().await;

        let selected = self.selected_index;
        if selected >= self.filtered_samples.len() {
            self.selected_index = if self.filtered_samples.is_empty() {
                0
            } else {
                self.filtered_samples.len() - 1
            };
        }
    }

    pub fn set_message(&mut self, msg: String) {
        self.message = msg;
        self.message_timer = 3;
    }

    pub async fn handle_key(&mut self, key: KeyEvent) {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key).await,
            InputMode::Search => self.handle_search_key(key).await,
            InputMode::Category => self.handle_category_key(key).await,
            InputMode::Message => {
                self.input_mode = InputMode::Normal;
            }
        }
    }

    async fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search;
                self.search_input.clear();
            }
            KeyCode::Char('c') => {
                self.input_mode = InputMode::Category;
                self.category_input.clear();
            }
            KeyCode::Char('r') => {
                self.sync_samples().await;
            }
            KeyCode::Char('e') => {
                if let Some(sample) = self.get_selected_sample() {
                    let id = sample.id.to_string();
                    match self.api.toggle_sample(&id).await {
                        Ok(enabled) => {
                            self.set_message(format!(
                                "{} {}",
                                sample.name,
                                if enabled { "enabled" } else { "disabled" }
                            ));
                            self.sync_samples().await;
                        }
                        Err(e) => self.set_message(format!("Toggle failed: {}", e)),
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(sample) = self.get_selected_sample() {
                    self.play_sample(&sample).await;
                }
            }
            KeyCode::Char('l') => {
                if let Some(player) = &mut self.player {
                    player.toggle_loop().await;
                }
                if let Some(player) = &self.player {
                    let mode_str = match player.get_playback_mode() {
                        PlaybackMode::Once => "OFF",
                        PlaybackMode::Loop => "ON",
                    };
                    self.set_message(format!("Loop: {}", mode_str));
                }
            }
            KeyCode::Esc => {
                self.state.set_category(None).await;
                self.state.set_search(String::new()).await;
                self.update_filtered_samples().await;
            }
            KeyCode::Tab => {
                self.selected_tab = (self.selected_tab + 1) % 2;
            }
            KeyCode::Down => self.move_selection(1),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Char('1') => self.selected_tab = 0,
            KeyCode::Char('2') => self.selected_tab = 1,
            KeyCode::Char('?') => self.show_help = !self.show_help,
            _ => {}
        }
    }

    async fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.state.set_search(self.search_input.clone()).await;
                self.update_filtered_samples().await;
                self.input_mode = InputMode::Normal;
                if !self.search_input.is_empty() {
                    self.set_message(format!("Search: {}", self.search_input));
                }
            }
            KeyCode::Char(c) => self.search_input.push(c),
            KeyCode::Backspace => {
                self.search_input.pop();
            }
            _ => {}
        }
    }

    async fn handle_category_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if !self.category_input.is_empty() {
                    if let Some(sample) = self.get_selected_sample() {
                        let id = sample.id.to_string();
                        let cat = self.category_input.clone();
                        match self.api.add_category(&id, &cat).await {
                            Ok(_) => {
                                self.set_message(format!("Added category '{}' to {}", cat, sample.name));
                                self.sync_samples().await;
                            }
                            Err(e) => self.set_message(format!("Failed: {}", e)),
                        }
                    }
                }
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => self.category_input.push(c),
            KeyCode::Backspace => {
                self.category_input.pop();
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered_samples.is_empty() {
            return;
        }
        let current = self.selected_index as isize;
        let max = self.filtered_samples.len() as isize - 1;
        let next = (current + delta).clamp(0, max);
        self.selected_index = next as usize;
    }

    fn get_selected_sample(&self) -> Option<Sample> {
        self.filtered_samples.get(self.selected_index).cloned()
    }

    async fn play_sample(&mut self, sample: &Sample) {
        if let Some(player) = &mut self.player {
            let url = self.api.get_audio_url(&sample.path);
            match reqwest::get(&url).await {
                Ok(resp) => match resp.bytes().await {
                    Ok(data) => {
                        if let Err(e) = player.play_bytes(data.to_vec(), &sample.name).await {
                            self.set_message(format!("Playback error: {}", e));
                        }
                    }
                    Err(e) => self.set_message(format!("Download error: {}", e)),
                },
                Err(e) => self.set_message(format!("Fetch error: {}", e)),
            }
        } else {
            self.set_message("No audio player available".to_string());
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(frame.area());

        self.render_header(frame, chunks[0]);
        self.render_main(frame, chunks[1]);
        self.render_status(frame, chunks[2]);
        self.render_footer(frame, chunks[3]);

        if self.show_help {
            self.render_help(frame);
        }

        match self.input_mode {
            InputMode::Search => self.render_search_popup(frame),
            InputMode::Category => self.render_category_popup(frame),
            _ => {}
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let tabs = Tabs::new(vec!["Samples", "Preview"])
            .block(Block::default().borders(Borders::ALL).title("ssmgr"))
            .select(self.selected_tab)
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(tabs, area);
    }

    fn render_main(&self, frame: &mut Frame, area: Rect) {
        match self.selected_tab {
            0 => self.render_sample_list(frame, area),
            1 => self.render_preview(frame, area),
            _ => {}
        }
    }

    fn render_sample_list(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .filtered_samples
            .iter()
            .map(|s| {
                let indicator = if s.enabled {
                    Span::styled(" ●", Style::default().fg(Color::Green))
                } else {
                    Span::styled(" ○", Style::default().fg(Color::DarkGray))
                };

                let bpm = s
                    .bpm
                    .map(|b| format!("{:.0}bpm", b))
                    .unwrap_or_else(|| "--".to_string());

                let cats = if s.categories.is_empty() {
                    "".to_string()
                } else {
                    format!(" [{}]", s.categories.join(", "))
                };

                let dur = s
                    .duration_secs
                    .map(|d| format!("{:.1}s", d))
                    .unwrap_or_default();

                let text = format!(
                    "{} {:<30} {:>6} {:>5}{}",
                    indicator,
                    s.name,
                    dur,
                    bpm,
                    cats
                );

                ListItem::new(text)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        "Samples ({}) [j/k:nav e:toggle space:play l:loop]",
                        self.filtered_samples.len()
                    )),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut list_state = ListState::default();
        list_state.select(Some(self.selected_index));
        frame.render_stateful_widget(list, area, &mut list_state);
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        if let Some(sample) = self.get_selected_sample() {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(Color::Yellow)),
                    Span::raw(&sample.name),
                ]),
                Line::from(vec![
                    Span::styled("Path: ", Style::default().fg(Color::Yellow)),
                    Span::raw(&sample.path),
                ]),
                Line::from(vec![
                    Span::styled("Format: ", Style::default().fg(Color::Yellow)),
                    Span::raw(&sample.metadata.format),
                ]),
                Line::from(vec![
                    Span::styled("Duration: ", Style::default().fg(Color::Yellow)),
                    Span::raw(
                        sample
                            .duration_secs
                            .map(|d| format!("{:.2}s", d))
                            .unwrap_or_else(|| "Unknown".to_string()),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Sample Rate: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{} Hz", sample.metadata.sample_rate)),
                ]),
                Line::from(vec![
                    Span::styled("Channels: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{}", sample.metadata.channels)),
                ]),
                Line::from(vec![
                    Span::styled("Bit Depth: ", Style::default().fg(Color::Yellow)),
                    Span::raw(
                        sample
                            .metadata
                            .bit_depth
                            .map(|b| b.to_string())
                            .unwrap_or_else(|| "Unknown".to_string()),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("BPM: ", Style::default().fg(Color::Yellow)),
                    Span::raw(
                        sample
                            .bpm
                            .map(|b| format!("{:.1}", b))
                            .unwrap_or_else(|| "Not analyzed".to_string()),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Enabled: ", Style::default().fg(Color::Yellow)),
                    Span::raw(if sample.enabled { "Yes" } else { "No" }),
                ]),
                Line::from(vec![
                    Span::styled("Categories: ", Style::default().fg(Color::Yellow)),
                    Span::raw(if sample.categories.is_empty() {
                        "None".to_string()
                    } else {
                        sample.categories.join(", ")
                    }),
                ]),
            ];

            if !sample.metadata.tags.is_empty() {
                lines.push(Line::from(Span::styled(
                    "--- Metadata Tags ---",
                    Style::default().fg(Color::DarkGray),
                )));
                for (key, value) in &sample.metadata.tags {
                    lines.push(Line::from(vec![
                        Span::styled(format!("{}: ", key), Style::default().fg(Color::Yellow)),
                        Span::raw(value),
                    ]));
                }
            }

            let paragraph = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Sample Details"),
                )
                .wrap(Wrap { trim: true });

            frame.render_widget(paragraph, area);
        } else {
            let paragraph = Paragraph::new("No sample selected")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Sample Details"),
                );
            frame.render_widget(paragraph, area);
        }
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let total = self.filtered_samples.len();
        let enabled = self.filtered_samples.iter().filter(|s| s.enabled).count();

        let conn_status = if self.loading {
            Span::styled("syncing...", Style::default().fg(Color::Yellow))
        } else if self.server_connected {
            Span::styled("connected", Style::default().fg(Color::Green))
        } else {
            Span::styled("disconnected", Style::default().fg(Color::Red))
        };

        let player_status = if let Some(player) = &self.player {
            if player.is_playing() {
                let name = player
                    .currently_playing()
                    .unwrap_or("unknown")
                    .to_string();
                let mode = match player.get_playback_mode() {
                    PlaybackMode::Once => "",
                    PlaybackMode::Loop => " [LOOP]",
                };
                Span::styled(
                    format!("Playing: {}{}", name, mode),
                    Style::default().fg(Color::Cyan),
                )
            } else {
                Span::styled(
                    format!(
                        "Player ready [{}]",
                        match player.get_playback_mode() {
                            PlaybackMode::Once => "ONCE",
                            PlaybackMode::Loop => "LOOP",
                        }
                    ),
                    Style::default().fg(Color::DarkGray),
                )
            }
        } else {
            Span::styled("No player", Style::default().fg(Color::DarkGray))
        };

        let lines = vec![
            Line::from(vec![
                Span::raw(format!("{} samples | {} enabled", total, enabled)),
                Span::raw(" | "),
                conn_status,
            ]),
            Line::from(player_status),
        ];

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Status"),
        );
        frame.render_widget(paragraph, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let message = if self.message_timer > 0 {
            &self.message
        } else {
            "[/]search [c]category [r]resync [e]enable [space]play [l]loop [?]help [q]quit"
        };

        let paragraph = Paragraph::new(message).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
    }

    fn render_help(&self, frame: &mut Frame) {
        let area = centered_rect(60, 50, frame.area());
        frame.render_widget(Clear, area);

        let lines = vec![
            Line::from(Span::styled(
                "Keyboard Shortcuts",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("j/k or ↑/↓ - Navigate samples"),
            Line::from("Tab or 1/2 - Switch tabs"),
            Line::from("Space - Play selected sample"),
            Line::from("e - Toggle enabled/disabled"),
            Line::from("l - Toggle loop mode"),
            Line::from("/ - Search by name"),
            Line::from("c - Add category to sample"),
            Line::from("r - Resync from server"),
            Line::from("Esc - Clear filters"),
            Line::from("? - Toggle this help"),
            Line::from("q - Quit"),
            Line::from(""),
            Line::from("Press any key to close"),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Help");
        let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn render_search_popup(&self, frame: &mut Frame) {
        let area = centered_rect(50, 3, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Search (Enter to apply, Esc to cancel)");
        let input = Paragraph::new(format!("{}_", self.search_input)).block(block);
        frame.render_widget(input, area);
    }

    fn render_category_popup(&self, frame: &mut Frame) {
        let area = centered_rect(50, 5, frame.area());
        frame.render_widget(Clear, area);

        let mut lines = vec![Line::from("Add category (Enter to confirm, Esc to cancel):")];
        lines.push(Line::from(format!("> {}_", self.category_input)));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Common categories:",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            DEFAULT_CATEGORIES.join(", "),
            Style::default().fg(Color::DarkGray),
        )));

        let block = Block::default().borders(Borders::ALL).title("Category");
        let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
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

pub async fn run(mut app: App) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::execute;
    use crossterm::terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    };
    use ratatui::backend::CrosstermBackend;
    use ratatui::Terminal;

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    app.sync_samples().await;

    while !app.should_quit {
        terminal.draw(|f| app.render(f))?;

        if app.message_timer > 0 {
            app.message_timer -= 1;
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    app.should_quit = true;
                } else {
                    app.handle_key(key).await;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
