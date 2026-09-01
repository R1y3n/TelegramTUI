use crossterm::event::{KeyCode, KeyModifiers};

#[derive(Clone, Debug)]
pub struct KeyConfig {
    pub switch_focus: (KeyCode, KeyModifiers),
    pub select_chat: (KeyCode, KeyModifiers),
    pub send_message: (KeyCode, KeyModifiers),
    pub newline: (KeyCode, KeyModifiers),
    pub quit: (KeyCode, KeyModifiers),
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            switch_focus: (KeyCode::Tab, KeyModifiers::NONE),
            select_chat: (KeyCode::Enter, KeyModifiers::NONE),
            send_message: (KeyCode::Enter, KeyModifiers::NONE),
            newline: (KeyCode::Enter, KeyModifiers::SHIFT),
            quit: (KeyCode::Char('q'), KeyModifiers::CONTROL),
        }
    }
}
