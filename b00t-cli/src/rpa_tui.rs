//! RPA TUI — fzf-like ratatui menu for curating browser automation commands.
//!
//! Provides a lightweight terminal interface for:
//! - Selecting CDP targets (open pages)
//! - Fuzzy-searching through available commands
//! - Executing RPA scripts (click, type, evaluate, screenshot)
//! - Curation: saving/loading command sequences as scripts

use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use nucleo::Matcher;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};
use std::io;

/// A curated RPA command — one step in a browser automation script.
#[derive(Debug, Clone)]
pub struct RpaCommand {
    pub action: &'static str,
    pub selector: &'static str,
    pub args: &'static str,
    pub description: &'static str,
}

/// Built-in RPA command palette.
pub static COMMAND_PALETTE: &[RpaCommand] = &[
    RpaCommand {
        action: "navigate",
        selector: "",
        args: "<url>",
        description: "Navigate to a URL",
    },
    RpaCommand {
        action: "click",
        selector: "css",
        args: "<selector>",
        description: "Click an element",
    },
    RpaCommand {
        action: "type",
        selector: "css",
        args: "<selector> <text>",
        description: "Type text into an input",
    },
    RpaCommand {
        action: "evaluate",
        selector: "",
        args: "<js>",
        description: "Execute JavaScript",
    },
    RpaCommand {
        action: "wait_for",
        selector: "css",
        args: "<selector>",
        description: "Wait for element to appear",
    },
    RpaCommand {
        action: "get_text",
        selector: "",
        args: "",
        description: "Get page text content",
    },
    RpaCommand {
        action: "screenshot",
        selector: "",
        args: "<file.png>",
        description: "Take a screenshot",
    },
    RpaCommand {
        action: "screenshot",
        selector: "",
        args: "<file.png>",
        description: "Take a screenshot of current page",
    },
    RpaCommand {
        action: "list_pages",
        selector: "",
        args: "",
        description: "List all open tabs",
    },
    RpaCommand {
        action: "close",
        selector: "",
        args: "",
        description: "Close current page",
    },
    RpaCommand {
        action: "save_script",
        selector: "",
        args: "<name>",
        description: "Save current sequence as script",
    },
    RpaCommand {
        action: "load_script",
        selector: "",
        args: "<name>",
        description: "Load a saved script",
    },
    RpaCommand {
        action: "run_script",
        selector: "",
        args: "<name>",
        description: "Run a saved script",
    },
];

/// Curation entry — a user-selected sequence of commands.
#[derive(Debug, Clone)]
pub struct ScriptStep {
    pub action: String,
    pub selector: String,
    pub args: String,
}

#[derive(Debug)]
pub struct Script {
    pub name: String,
    pub steps: Vec<ScriptStep>,
}

/// TUI state for the RPA command menu.
struct RpaTuiState {
    /// Available commands (filtered by search)
    filtered_commands: Vec<(usize, &'static RpaCommand)>,
    /// Current selection index (into filtered_commands)
    selected: usize,
    /// Search/filter string
    search: String,
    /// Cursor position in search
    cursor: usize,
    /// List state for ratatui
    list_state: ListState,
    /// Status message
    status: String,
    /// Command sequence being curated (our "shopping cart")
    curated: Vec<ScriptStep>,
    /// Matcher for fuzzy search
    matcher: Matcher,
}

impl RpaTuiState {
    fn new() -> Self {
        let all: Vec<(usize, &'static RpaCommand)> = COMMAND_PALETTE.iter().enumerate().collect();
        let mut state = RpaTuiState {
            filtered_commands: all,
            selected: 0,
            search: String::new(),
            cursor: 0,
            list_state: ListState::default(),
            status: String::new(),
            curated: Vec::new(),
            matcher: Matcher::new(nucleo::Config::DEFAULT),
        };
        state.list_state.select(Some(0));
        state
    }

    /// Re-filter the command list based on current search string.
    fn update_filter(&mut self) {
        let all: Vec<(usize, &'static RpaCommand)> = COMMAND_PALETTE.iter().enumerate().collect();
        if self.search.is_empty() {
            self.filtered_commands = all;
            self.selected = self
                .selected
                .min(self.filtered_commands.len().saturating_sub(1));
            self.list_state.select(Some(self.selected));
            return;
        }
        let mut scored: Vec<(u32, usize, &'static RpaCommand)> = Vec::new();
        let search_lower = self.search.to_lowercase();
        for (i, cmd) in &all {
            let text = format!(
                "{} {} {} {}",
                cmd.action, cmd.selector, cmd.args, cmd.description
            );
            let text_lower = text.to_lowercase();
            // Prefix/substring match as simple fuzzy
            if text_lower.contains(&search_lower) {
                let score = if text_lower.starts_with(&search_lower) {
                    100
                } else {
                    50
                };
                scored.push((score, *i, cmd));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        self.filtered_commands = scored.into_iter().map(|(_, i, c)| (i, c)).collect();
        self.selected = if self.filtered_commands.is_empty() {
            0
        } else {
            self.selected
                .min(self.filtered_commands.len().saturating_sub(1))
        };
        self.list_state.select(Some(self.selected));
    }

    fn selected_cmd(&self) -> Option<&RpaCommand> {
        self.filtered_commands.get(self.selected).map(|(_, c)| *c)
    }
}

/// Run the RPA command curation TUI.
/// Returns the curated script steps if user exits cleanly.
pub fn run_curation_menu() -> Result<Vec<ScriptStep>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;

    let terminal = Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
    let mut state = RpaTuiState::new();
    let res = run_tui_loop(terminal, &mut state);

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    res?;
    Ok(state.curated.clone())
}

fn run_tui_loop(
    mut terminal: Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    state: &mut RpaTuiState,
) -> Result<()> {
    loop {
        terminal.draw(|f| render_ui(f, state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Enter => {
                    let action = state
                        .selected_cmd()
                        .map(|c| c.action.to_string())
                        .unwrap_or_default();
                    if let Some(cmd) = state.selected_cmd() {
                        let step = ScriptStep {
                            action: cmd.action.to_string(),
                            selector: cmd.selector.to_string(),
                            args: cmd.args.to_string(),
                        };
                        state.curated.push(step);
                        state.status = format!("➕ Added: {}", action);
                    }
                }
                KeyCode::Backspace => {
                    if !state.curated.is_empty() {
                        state.curated.pop();
                        state.status = "🗑️ Removed last command".to_string();
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.selected > 0 {
                        state.selected -= 1;
                        state.list_state.select(Some(state.selected));
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if state.selected + 1 < state.filtered_commands.len() {
                        state.selected += 1;
                        state.list_state.select(Some(state.selected));
                    }
                }
                KeyCode::Char(c) => {
                    state.search.push(c);
                    state.cursor = state.search.len();
                    state.update_filter();
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn render_ui(frame: &mut Frame, state: &RpaTuiState) {
    let area = frame.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search bar
            Constraint::Min(1),    // command list
            Constraint::Length(3), // curated list
            Constraint::Length(1), // status bar
        ])
        .split(area);

    // Search bar
    let search_text = if state.search.is_empty() {
        "🔍 Type to filter commands... (Enter=add, Backspace=remove, Esc=exit, j/k=navigate)"
            .to_string()
    } else {
        format!("🔍 {}", state.search)
    };
    let search = Paragraph::new(Span::styled(search_text, Style::default().fg(Color::Cyan))).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Search Commands "),
    );
    frame.render_widget(search, chunks[0]);

    // Command list
    let items: Vec<ListItem> = state
        .filtered_commands
        .iter()
        .map(|(_, cmd)| {
            let text = format!(
                " {} {} {}  {}",
                cmd.action,
                if cmd.selector.is_empty() {
                    "".to_string()
                } else {
                    format!("[{}]", cmd.selector)
                },
                cmd.args,
                cmd.description,
            );
            ListItem::new(text)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Commands ({} filtered) ",
            state.filtered_commands.len()
        )))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, chunks[1], &mut state.list_state.clone());

    // Curated sequence
    let curated_text = if state.curated.is_empty() {
        "📋 No commands selected yet. Press Enter on a command to add it.".to_string()
    } else {
        let steps: Vec<String> = state
            .curated
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {} {} {}", i + 1, s.action, s.selector, s.args))
            .collect();
        steps.join("  →  ")
    };
    let curated = Paragraph::new(Span::styled(
        curated_text,
        Style::default().fg(Color::Green),
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Script ({} steps) ", state.curated.len())),
    );
    frame.render_widget(curated, chunks[2]);

    // Status bar
    let status = Paragraph::new(Span::styled(
        &state.status,
        Style::default().fg(Color::Yellow),
    ));
    frame.render_widget(status, chunks[3]);
}

/// Print a saved script as a sequence of commands.
pub fn print_script(script: &[ScriptStep]) {
    println!("📋 RPA Script ({} steps):", script.len());
    for (i, step) in script.iter().enumerate() {
        println!(
            "  {}. {} {} {}",
            i + 1,
            step.action,
            step.selector,
            step.args
        );
    }
}
