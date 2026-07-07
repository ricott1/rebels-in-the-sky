pub(crate) mod big_numbers;
pub(crate) mod button;
pub(crate) mod checkbox;
pub(crate) mod clickable_list;
pub(crate) mod clickable_table;
pub(crate) mod dropdown;
pub(crate) mod hover_text_line;
pub(crate) mod hover_text_span;
pub(crate) mod popup_message;

use crate::ui::constants::UiStyle;
use ratatui::crossterm::event::KeyCode;
use ratatui::text::{Line, Span};

pub(crate) fn underline_hotkey(text: &str, hotkey: Option<KeyCode>) -> Line<'static> {
    if let Some(key) = hotkey {
        let key_str = key.to_string();
        if let Some((before, after)) = text.split_once(&key_str) {
            return Line::from(vec![
                Span::raw(before.to_owned()),
                Span::styled(key_str, UiStyle::DEFAULT.underlined()),
                Span::raw(after.to_owned()),
            ]);
        }
    }
    Line::from(text.to_owned())
}
