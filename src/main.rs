mod chats;
mod config;
mod gets;

use std::env;
use std::io::{self, Write};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use config::KeyConfig;
use gets::fetch_chat_list;
use grammers_client::{Client, Config, SignInError};
use grammers_session::Session;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

const SESSION_FILE: &str = "tg_session.bin";

#[derive(PartialEq, Eq, Debug)]
enum ActivePane {
    Chats,
    Messages,
    SendBox,
}

impl ActivePane {
    fn next(&self) -> Self {
        match self {
            ActivePane::Chats => ActivePane::Messages,
            ActivePane::Messages => ActivePane::SendBox,
            ActivePane::SendBox => ActivePane::Chats,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read API credentials from Environment Variables
    let api_id: i32 = env::var("API_ID")
        .expect("API_ID env var missing! Set it via: export API_ID=123456")
        .parse()
        .expect("API_ID must be an integer");

    let api_hash: String = env::var("API_HASH")
        .expect("API_HASH env var missing! Set it via: export API_HASH=your_hash");

    let keys = KeyConfig::default();

    let client = Client::connect(Config {
        session: Session::load_file_or_create(SESSION_FILE)?,
        api_id,
        api_hash,
        params: Default::default(),
    })
    .await?;

    // --- Terminal Authentication (if not logged in) ---
    if !client.is_authorized().await? {
        print!("Enter phone number (+XXXXXXXXXXX): ");
        io::stdout().flush()?;
        let mut phone = String::new();
        io::stdin().read_line(&mut phone)?;

        let token = client.request_login_code(phone.trim()).await?;
        print!("Enter code sent by Telegram: ");
        io::stdout().flush()?;
        let mut code = String::new();
        io::stdin().read_line(&mut code)?;

        match client.sign_in(&token, code.trim()).await {
            Ok(_) => println!("Login successful!"),
            Err(SignInError::PasswordRequired(password_token)) => {
                print!("2FA Password required: ");
                io::stdout().flush()?;
                let mut password = String::new();
                io::stdin().read_line(&mut password)?;
                client
                    .check_password(password_token, password.trim())
                    .await?;
                println!("2FA Authentication successful!");
            }
            Err(e) => return Err(e.into()),
        }
        client.session().save_to_file(SESSION_FILE)?;
    }

    // --- Terminal Setup ---
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    // --- State Initialization ---
    let chat_list = fetch_chat_list(&client, 30).await.unwrap_or_default();
    let mut selected_chat_idx: Option<usize> = None;
    let mut active_messages: Vec<chats::MessageItem> = Vec::new();
    let mut loaded_offset: usize = 0;
    let mut input_buffer = String::new();
    let mut active_pane = ActivePane::Chats;

    // --- Main Rendering & Event Loop ---
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(f.size());

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(4)])
                .split(chunks[1]);

            // Border styles dependent on active pane focus
            let chat_border_style = if active_pane == ActivePane::Chats {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let msg_border_style = if active_pane == ActivePane::Messages {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let send_border_style = if active_pane == ActivePane::SendBox {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            // 1. Chats Panel
            let items: Vec<ListItem> = chat_list
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let is_selected = Some(i) == selected_chat_idx;
                    let prefix = if is_selected { " > " } else { "   " };
                    let folder_icon = if c.is_archived { "📁 [Archived] " } else { "💬 " };

                    let text = format!("{}{}{}", prefix, folder_icon, c.name);
                    let style = if is_selected {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    ListItem::new(Text::styled(text, style))
                })
                .collect();

            f.render_widget(
                List::new(items).block(
                    Block::default()
                        .title(" [1] Chats (TAB to Switch) ")
                        .borders(Borders::ALL)
                        .border_style(chat_border_style),
                ),
                chunks[0],
            );

            // 2. Messages Panel (Sender color != Message color + Colored Dividers)
            let mut msg_items: Vec<ListItem> = Vec::new();
            for m in active_messages.iter() {
                let header = Line::from(vec![
                    Span::styled(
                        format!("[{}] ", m.date),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{}: ", m.sender),
                        Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&m.text, Style::default().fg(Color::White)),
                ]);

                let divider = Line::from(Span::styled(
                    "──────────────────────────────────────────────────────",
                    Style::default().fg(Color::Blue),
                ));

                msg_items.push(ListItem::new(vec![header, divider]));
            }

            f.render_widget(
                List::new(msg_items).block(
                    Block::default()
                        .title(" [2] Messages (Scroll Up to Lazy Load) ")
                        .borders(Borders::ALL)
                        .border_style(msg_border_style),
                ),
                right_chunks[0],
            );

            // 3. Send Box Panel (Supports Shift+Enter line jumps)
            let input_widget = Paragraph::new(input_buffer.as_str()).block(
                Block::default()
                    .title(" [3] Send Box (Enter = Send, Shift+Enter = Line Jump) ")
                    .borders(Borders::ALL)
                    .border_style(send_border_style),
            );
            f.render_widget(input_widget, right_chunks[1]);
        })?;

        // --- Handle Input Events ---
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.code == keys.quit.0 && key.modifiers.contains(keys.quit.1) {
                    break;
                }

                // TAB Key: Switch focused panel
                if key.code == KeyCode::Tab {
                    active_pane = active_pane.next();
                    continue;
                }

                match active_pane {
                    ActivePane::Chats => match key.code {
                        KeyCode::Up => {
                            selected_chat_idx = match selected_chat_idx {
                                Some(idx) if idx > 0 => Some(idx - 1),
                                _ => Some(0),
                            };
                        }
                        KeyCode::Down => {
                            selected_chat_idx = match selected_chat_idx {
                                Some(idx) if idx + 1 < chat_list.len() => Some(idx + 1),
                                _ => Some(0),
                            };
                        }
                        KeyCode::Enter => {
                            if let Some(idx) = selected_chat_idx {
                                loaded_offset = 0;
                                active_messages = chats::fetch_latest_messages(
                                    &client,
                                    &chat_list[idx].packed,
                                    20,
                                    loaded_offset,
                                )
                                .await?;
                                active_pane = ActivePane::Messages;
                            }
                        }
                        _ => {}
                    },

                    ActivePane::Messages => match key.code {
                        KeyCode::Up | KeyCode::PageUp => {
                            // Lazy load older messages when scrolling up
                            if let Some(idx) = selected_chat_idx {
                                loaded_offset += 15;
                                let older_msgs = chats::fetch_latest_messages(
                                    &client,
                                    &chat_list[idx].packed,
                                    15,
                                    loaded_offset,
                                )
                                .await?;

                                if !older_msgs.is_empty() {
                                    let mut updated = older_msgs;
                                    updated.extend(active_messages);
                                    active_messages = updated;
                                }
                            }
                        }
                        _ => {}
                    },

                    ActivePane::SendBox => {
                        // Shift + Enter = Insert Newline
                        if key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::SHIFT) {
                            input_buffer.push('\n');
                        } else if key.code == KeyCode::Enter {
                            // Plain Enter = Send Message
                            if let Some(idx) = selected_chat_idx {
                                if !input_buffer.trim().is_empty() {
                                    chats::send_text_message(
                                        &client,
                                        &chat_list[idx].packed,
                                        &input_buffer,
                                    )
                                    .await?;
                                    input_buffer.clear();

                                    // Refresh view with new message
                                    loaded_offset = 0;
                                    active_messages = chats::fetch_latest_messages(
                                        &client,
                                        &chat_list[idx].packed,
                                        20,
                                        0,
                                    )
                                    .await?;
                                }
                            }
                        } else if let KeyCode::Char(c) = key.code {
                            input_buffer.push(c);
                        } else if key.code == KeyCode::Backspace {
                            input_buffer.pop();
                        }
                    }
                }
            }
        }
    }

    // --- Cleanup Terminal ---
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
