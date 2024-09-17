use super::button::Button;
use super::constants::{UiKey, UiStyle, UiText};
use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::utils::{hover_text_target, input_from_key_event, validate_textarea_input};
use super::widgets::default_block;
use crate::types::{AppResult, SystemTimeTick, Tick};
use core::fmt::Debug;
use ratatui::layout::{Margin, Rect};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::{
    layout::{Constraint, Layout},
    Frame,
};
use std::sync::{Arc, Mutex};
use strum_macros::Display;
use tui_textarea::TextArea;

#[derive(Debug, Display, Clone, PartialEq)]
pub enum PopupMessage {
    Error(String, Tick),
    Ok(String, Tick),
    Dialog(String, Vec<(String, UiCallbackPreset)>, Tick),
    AsteroidNameDialog(Tick),
}

impl PopupMessage {
    pub fn is_skippable(&self) -> bool {
        match self {
            PopupMessage::Error { .. } => true,
            PopupMessage::Ok { .. } => true,
            PopupMessage::Dialog { .. } => false,
            PopupMessage::AsteroidNameDialog { .. } => false,
        }
    }
    pub fn consumes_input(
        &self,
        popup_input: &mut TextArea<'static>,
        key_event: crossterm::event::KeyEvent,
    ) -> Option<UiCallbackPreset> {
        match self {
            PopupMessage::AsteroidNameDialog { .. } => {
                if key_event.code == UiKey::YES_TO_DIALOG {
                    let mut name = popup_input.lines()[0].clone();
                    name = name
                        .chars()
                        .enumerate()
                        .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                        .collect();
                    if validate_textarea_input(popup_input, "Asteroid name".into()) {
                        return Some(UiCallbackPreset::NameAndAcceptAsteroid { name });
                    }
                } else if key_event.code == UiKey::NO_TO_DIALOG {
                    if popup_input.lines()[0].len() == 0 {
                        return Some(UiCallbackPreset::CloseUiPopup);
                    }
                    popup_input.input(input_from_key_event(key_event));
                } else {
                    popup_input.input(input_from_key_event(key_event));
                }
            }
            _ => {
                if key_event.code == UiKey::YES_TO_DIALOG {
                    return Some(UiCallbackPreset::CloseUiPopup);
                } else if key_event.code == UiKey::NO_TO_DIALOG {
                    return Some(UiCallbackPreset::CloseUiPopup);
                }
            }
        }
        None
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        popup_rect: Rect,
        popup_input: &mut TextArea<'static>,
        callback_registry: &Arc<Mutex<CallbackRegistry>>,
    ) -> AppResult<()> {
        let split = Layout::vertical([
            Constraint::Length(3), //header
            Constraint::Min(3),    //message
            Constraint::Length(3), //button
        ])
        .split(popup_rect.inner(Margin {
            vertical: 1,
            horizontal: 1,
        }));

        frame.render_widget(Clear, popup_rect);
        frame.render_widget(default_block(), popup_rect);
        let hover_text_target = hover_text_target(frame);
        match self {
            PopupMessage::Ok(message, tick) => {
                frame.render_widget(
                    Paragraph::new(format!("Message: {}", tick.formatted_as_date()))
                        .block(default_block().border_style(UiStyle::OK))
                        .centered(),
                    split[0],
                );
                frame.render_widget(
                    Paragraph::new(message.clone())
                        .centered()
                        .wrap(Wrap { trim: true }),
                    split[1].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );
                let button = Button::new(
                    UiText::YES.into(),
                    UiCallbackPreset::CloseUiPopup,
                    Arc::clone(&callback_registry),
                )
                .set_hover_text("Close the popup".into(), hover_text_target)
                .set_hotkey(UiKey::YES_TO_DIALOG)
                .set_box_style(UiStyle::OK)
                .set_layer(1);

                frame.render_widget(
                    button,
                    split[2].inner(Margin {
                        vertical: 0,
                        horizontal: 8,
                    }),
                );
            }
            PopupMessage::Error(message, tick) => {
                frame.render_widget(
                    Paragraph::new(format!("Error: {}", tick.formatted_as_date()))
                        .block(default_block().border_style(UiStyle::ERROR))
                        .centered(),
                    split[0],
                );
                frame.render_widget(
                    Paragraph::new(message.clone())
                        .centered()
                        .wrap(Wrap { trim: true }),
                    split[1].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );
                let button = Button::new(
                    UiText::YES.into(),
                    UiCallbackPreset::CloseUiPopup,
                    Arc::clone(&callback_registry),
                )
                .set_hover_text("Close the popup".into(), hover_text_target)
                .set_hotkey(UiKey::YES_TO_DIALOG)
                .set_box_style(UiStyle::OK)
                .set_layer(1);

                frame.render_widget(
                    button,
                    split[2].inner(Margin {
                        vertical: 0,
                        horizontal: 8,
                    }),
                );
            }
            PopupMessage::Dialog(message, options, tick) => {
                frame.render_widget(
                    Paragraph::new(format!("Conundrum! {}", tick.formatted_as_date()))
                        .block(default_block().border_style(UiStyle::NETWORK))
                        .centered(),
                    split[0],
                );
                frame.render_widget(
                    Paragraph::new(message.clone())
                        .centered()
                        .wrap(Wrap { trim: true }),
                    split[1].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );

                let buttons_split = Layout::horizontal(
                    [Constraint::Ratio(1, options.len() as u32)].repeat(options.len()),
                )
                .split(split[2]);
                for idx in 0..options.len() {
                    let (text, callback) = options[idx].clone();
                    let button = Button::new(text, callback, Arc::clone(&callback_registry))
                        .set_hover_text(
                            "Choose this option and close the popup".into(),
                            hover_text_target,
                        )
                        .set_layer(1);

                    frame.render_widget(button, buttons_split[idx]);
                }
            }

            PopupMessage::AsteroidNameDialog(tick) => {
                frame.render_widget(
                    Paragraph::new(format!("Asteorid discovered! {}", tick.formatted_as_date()))
                        .block(default_block().border_style(UiStyle::NETWORK))
                        .centered(),
                    split[0],
                );

                let message_split = Layout::vertical([
                    Constraint::Length(4), //message
                    Constraint::Length(3), //input
                ])
                .split(split[1]);

                frame.render_widget(
                    Paragraph::new("Do you want to set up base on this asteroid?\nYou will need a proper name for it!")
                        .centered()
                        .wrap(Wrap { trim: true }),
                        message_split[0].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );

                popup_input.set_cursor_style(UiStyle::SELECTED);
                popup_input.set_block(
                    default_block()
                        .border_style(UiStyle::DEFAULT)
                        .title("Asteroid name"),
                );

                frame.render_widget(
                    &popup_input.clone(),
                    message_split[1].inner(Margin {
                        horizontal: 1,
                        vertical: 0,
                    }),
                );

                let buttons_split =
                    Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                        .split(split[2]);

                let mut name = popup_input.lines()[0].clone();
                name = name
                    .chars()
                    .enumerate()
                    .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                    .collect();

                let mut ok_button = Button::new(
                    UiText::YES.into(),
                    UiCallbackPreset::NameAndAcceptAsteroid { name },
                    Arc::clone(&callback_registry),
                )
                .set_hover_text(
                    "Name and set the asteroid as home planet".into(),
                    hover_text_target,
                )
                .set_hotkey(UiKey::YES_TO_DIALOG)
                .set_box_style(UiStyle::OK)
                .set_layer(1);

                if !validate_textarea_input(popup_input, "Asteroid name".into()) {
                    ok_button.disable(None);
                }

                frame.render_widget(ok_button, buttons_split[0]);

                let no_button = Button::new(
                    UiText::NO.into(),
                    UiCallbackPreset::CloseUiPopup,
                    Arc::clone(&callback_registry),
                )
                .set_hover_text("Leave the asteroid alone!".into(), hover_text_target)
                .set_hotkey(UiKey::NO_TO_DIALOG)
                .set_box_style(UiStyle::ERROR)
                .set_layer(1);

                frame.render_widget(no_button, buttons_split[1]);
            }
        }
        Ok(())
    }
}
