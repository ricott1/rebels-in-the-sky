use super::button::Button;
use super::constants::{UiKey, UiStyle, UiText};
use super::gif_map::{self, TREASURE_GIF};
use super::ui_callback::{CallbackRegistry, UiCallback};
use super::utils::{
    hover_text_target, img_to_lines, input_from_key_event, validate_textarea_input,
};
use super::widgets::default_block;
use crate::image::types::{Gif, PrintableGif};
use crate::types::*;
use crate::ui::gif_map::PORTAL_GIFS;
use crate::world::{player::Player, resources::Resource, skill::Rated};
use anyhow::anyhow;
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

const FRAME_DURATION_MILLIS: Tick = 150;
const TREASURE_GIF_ANIMATION_DELAY: Tick = 450;

#[derive(Debug, Display, Clone, PartialEq)]
pub enum PopupMessage {
    Error {
        message: String,
        tick: Tick,
    },
    Ok {
        message: String,
        is_skippable: bool,
        tick: Tick,
    },
    PromptQuit {
        during_space_adventure: bool,
        tick: Tick,
    },
    ReleasePlayer {
        player_name: String,
        player_id: PlayerId,
        tick: Tick,
    },
    AsteroidNameDialog {
        tick: Tick,
    },
    PortalFound {
        player_name: String,
        portal_target: String,
        tick: Tick,
    },
    ExplorationResult {
        resources: ResourceMap,
        players: Vec<Player>,
        tick: Tick,
    },
    TeamLanded {
        team_name: String,
        planet_name: String,
        planet_filename: String,
        tick: Tick,
    },
    Tutorial {
        index: usize,
        tick: Tick,
    },
}

impl PopupMessage {
    const MAX_TUTORIAL_PAGE: usize = 2;
    fn rect(&self, area: Rect) -> Rect {
        let (width, height) = match self {
            PopupMessage::AsteroidNameDialog { .. } => (54, 28),
            PopupMessage::PortalFound { .. } => (54, 44),
            PopupMessage::ExplorationResult { resources, .. } => {
                if resources.value(&&Resource::GOLD) > 0 {
                    (54, 26)
                } else {
                    (54, 16)
                }
            }
            PopupMessage::TeamLanded { .. } => (54, 26),
            _ => (48, 16),
        };

        let x = if area.width < width {
            0
        } else {
            (area.width - width) / 2
        };

        let y = if area.height < height {
            0
        } else {
            (area.height - height) / 2
        };

        let rect_width = if area.width < x + width {
            area.width
        } else {
            width
        };

        let rect_height = if area.height < y + height {
            area.height
        } else {
            height
        };

        Rect::new(x, y, rect_width, rect_height)
    }

    pub fn is_skippable(&self) -> bool {
        match self {
            PopupMessage::Error { .. } => true,
            PopupMessage::Ok { is_skippable, .. } => *is_skippable,
            _ => false,
        }
    }

    // This function is necessary because we want to consume some inputs on the textarea (like backspaces).
    // COuldn't find a better way at the moment.
    pub fn consumes_input(
        &self,
        popup_input: &mut TextArea<'static>,
        key_event: crossterm::event::KeyEvent,
    ) -> Option<UiCallback> {
        match self {
            PopupMessage::AsteroidNameDialog { tick } => {
                if key_event.code == UiKey::YES_TO_DIALOG {
                    let mut name = popup_input.lines()[0].clone();
                    name = name
                        .chars()
                        .enumerate()
                        .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                        .collect();
                    if validate_textarea_input(popup_input, "Asteroid name".into()) {
                        let filename = format!("asteroid{}", tick % 30);
                        return Some(UiCallback::NameAndAcceptAsteroid { name, filename });
                    }
                } else if key_event.code == UiKey::NO_TO_DIALOG {
                    if popup_input.lines()[0].len() == 0 {
                        return Some(UiCallback::CloseUiPopup);
                    }
                    popup_input.input(input_from_key_event(key_event));
                } else {
                    popup_input.input(input_from_key_event(key_event));
                }
            }

            PopupMessage::ReleasePlayer { player_id, .. } => {
                if key_event.code == UiKey::YES_TO_DIALOG {
                    return Some(UiCallback::ConfirmReleasePlayer {
                        player_id: player_id.clone(),
                    });
                } else if key_event.code == UiKey::NO_TO_DIALOG {
                    return Some(UiCallback::CloseUiPopup);
                }
            }

            PopupMessage::PromptQuit { .. } => {
                if key_event.code == UiKey::YES_TO_DIALOG {
                    return Some(UiCallback::QuitGame);
                } else if key_event.code == UiKey::NO_TO_DIALOG {
                    return Some(UiCallback::CloseUiPopup);
                }
            }

            PopupMessage::Tutorial { index, .. } => {
                if key_event.code == UiKey::YES_TO_DIALOG
                    && *index == PopupMessage::MAX_TUTORIAL_PAGE
                {
                    return Some(UiCallback::CloseUiPopup);
                } else if key_event.code == UiKey::YES_TO_DIALOG {
                    return Some(UiCallback::PushTutorialPage { index: index + 1 });
                } else if key_event.code == UiKey::NO_TO_DIALOG {
                    return Some(UiCallback::CloseUiPopup);
                }
            }
            _ => {
                if key_event.code == UiKey::YES_TO_DIALOG || key_event.code == UiKey::NO_TO_DIALOG {
                    return Some(UiCallback::CloseUiPopup);
                }
            }
        }
        None
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        popup_input: &mut TextArea<'static>,
        callback_registry: &Arc<Mutex<CallbackRegistry>>,
    ) -> AppResult<()> {
        let rect = self.rect(area);

        let split = Layout::vertical([
            Constraint::Length(3), //header
            Constraint::Min(3),    //message
            Constraint::Length(3), //button
        ])
        .split(rect.inner(Margin {
            vertical: 1,
            horizontal: 1,
        }));

        frame.render_widget(Clear, rect);
        frame.render_widget(default_block(), rect);
        let hover_text_target = hover_text_target(frame);
        match self {
            PopupMessage::Ok { message, tick, .. } => {
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
                    UiCallback::CloseUiPopup,
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

            PopupMessage::Error { message, tick } => {
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
                    UiCallback::CloseUiPopup,
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

            PopupMessage::ReleasePlayer {
                player_name,
                player_id,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new(format!("Attention!"))
                        .block(default_block().border_style(UiStyle::NETWORK))
                        .centered(),
                    split[0],
                );
                frame.render_widget(
                    Paragraph::new(format!(
                        "Are you sure you want to release {} from the crew?",
                        player_name
                    ))
                    .centered()
                    .wrap(Wrap { trim: true }),
                    split[1].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );

                let buttons_split =
                    Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                        .split(split[2]);

                let confirm_button = Button::new(
                    UiText::YES.into(),
                    UiCallback::ConfirmReleasePlayer {
                        player_id: player_id.clone(),
                    },
                    Arc::clone(&callback_registry),
                )
                .set_hover_text(
                    format!("Confirm releasing {}.", player_name),
                    hover_text_target,
                )
                .set_hotkey(UiKey::YES_TO_DIALOG)
                .set_box_style(UiStyle::OK)
                .set_layer(1);

                frame.render_widget(confirm_button, buttons_split[0]);

                let no_button = Button::new(
                    UiText::NO.into(),
                    UiCallback::CloseUiPopup,
                    Arc::clone(&callback_registry),
                )
                .set_hover_text(format!("Don't release {}", player_name), hover_text_target)
                .set_hotkey(UiKey::NO_TO_DIALOG)
                .set_box_style(UiStyle::ERROR)
                .set_layer(1);

                frame.render_widget(no_button, buttons_split[1]);
            }

            PopupMessage::PromptQuit {
                during_space_adventure,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new(format!("Attention!"))
                        .block(default_block().border_style(UiStyle::NETWORK))
                        .centered(),
                    split[0],
                );

                let text = if *during_space_adventure {
                    format!("Are you sure you want to quit?\nYou will lose the whole cargo! Go back to the base first\n(Press '{}')", UiKey::SPACE_BACK_TO_BASE)
                } else {
                    "Are you sure you want to quit?".to_string()
                };
                frame.render_widget(
                    Paragraph::new(text).centered().wrap(Wrap { trim: true }),
                    split[1].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );

                let buttons_split =
                    Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                        .split(split[2]);

                let confirm_button = Button::new(
                    UiText::YES.into(),
                    UiCallback::QuitGame,
                    Arc::clone(&callback_registry),
                )
                .set_hover_text(format!("Confirm quitting."), hover_text_target)
                .set_hotkey(UiKey::YES_TO_DIALOG)
                .set_box_style(UiStyle::OK)
                .set_layer(1);

                frame.render_widget(confirm_button, buttons_split[0]);

                let no_button = Button::new(
                    UiText::NO.into(),
                    UiCallback::CloseUiPopup,
                    Arc::clone(&callback_registry),
                )
                .set_hover_text(
                    format!("Please don't go, don't goooooo..."),
                    hover_text_target,
                )
                .set_hotkey(UiKey::NO_TO_DIALOG)
                .set_box_style(UiStyle::ERROR)
                .set_layer(1);

                frame.render_widget(no_button, buttons_split[1]);
            }

            PopupMessage::AsteroidNameDialog { tick } => {
                frame.render_widget(
                    Paragraph::new(format!("Asteroid discovered: {}", tick.formatted_as_date()))
                        .block(default_block().border_style(UiStyle::NETWORK))
                        .centered(),
                    split[0],
                );

                let filename = format!("asteroid{}", tick % 30);
                let asteroid_img = img_to_lines(&gif_map::GifMap::asteroid_zoom_out(&filename)?[0]);

                if asteroid_img.len() == 0 {
                    return Err(anyhow!("Invalid asteroid image"));
                }

                let asteroid_image_height = asteroid_img.len() as u16;

                let m_split = Layout::vertical([
                    Constraint::Length(4), //message
                    Constraint::Length(asteroid_image_height),
                    Constraint::Min(0),
                    Constraint::Length(3), //input
                ])
                .split(split[1]);

                frame.render_widget(
                    Paragraph::new("Do you want to set up base on this asteroid?\nYou will need a proper name for it!")
                        .centered()
                        .wrap(Wrap { trim: true }),
                        m_split[0].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );

                frame.render_widget(Paragraph::new(asteroid_img).centered(), m_split[1]);

                popup_input.set_cursor_style(UiStyle::SELECTED);
                popup_input.set_block(
                    default_block()
                        .border_style(UiStyle::DEFAULT)
                        .title("Asteroid name"),
                );

                frame.render_widget(
                    &popup_input.clone(),
                    m_split[3].inner(Margin {
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
                    UiCallback::NameAndAcceptAsteroid { name, filename },
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
                    UiCallback::CloseUiPopup,
                    Arc::clone(&callback_registry),
                )
                .set_hover_text("Leave the asteroid alone!".into(), hover_text_target)
                .set_hotkey(UiKey::NO_TO_DIALOG)
                .set_box_style(UiStyle::ERROR)
                .set_layer(1);

                frame.render_widget(no_button, buttons_split[1]);
            }

            PopupMessage::PortalFound {
                player_name,
                portal_target,
                tick,
            } => {
                frame.render_widget(
                    Paragraph::new(format!("Portal: {}", tick.formatted_as_date()))
                        .block(default_block().border_style(UiStyle::HIGHLIGHT))
                        .centered(),
                    split[0],
                );

                // Select a portal pseudorandomly.
                let portal = &PORTAL_GIFS[*tick as usize % PORTAL_GIFS.len()];

                if portal.len() == 0 {
                    return Err(anyhow!("Invalid portal gif"));
                }

                let portal_image_height = portal[0].len() as u16;

                let m_split = Layout::vertical([
                    Constraint::Length(5),
                    Constraint::Length(portal_image_height),
                    Constraint::Min(0),
                ])
                .split(split[1]);

                let text = format!(
                    "{} got drunk while driving and accidentally found a portal to {}!",
                    player_name, portal_target
                );
                frame.render_widget(
                    Paragraph::new(text).centered().wrap(Wrap { trim: true }),
                    m_split[0].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );

                // Tick::now() returns time as milliseconds. To implement the wanted framerate,
                // we need to divide by the frame duration in milliseconds
                let current_frame =
                    ((Tick::now() - tick) / FRAME_DURATION_MILLIS) as usize % portal.len();

                frame.render_widget(
                    Paragraph::new(portal[current_frame].clone()).centered(),
                    m_split[1],
                );

                let button = Button::new(
                    UiText::YES.into(),
                    UiCallback::CloseUiPopup,
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

            PopupMessage::ExplorationResult {
                resources,
                players,
                tick,
            } => {
                frame.render_widget(
                    Paragraph::new(format!("Exploration result: {}", tick.formatted_as_date()))
                        .block(default_block().border_style(UiStyle::HIGHLIGHT))
                        .centered(),
                    split[0],
                );

                let treasure = &TREASURE_GIF;

                if treasure.len() == 0 {
                    return Err(anyhow!("Invalid treasure gif"));
                }

                let treasure_image_height = if resources.value(&Resource::GOLD) > 0 {
                    treasure[0].len() as u16
                } else {
                    0
                };

                let m_split = Layout::vertical([
                    Constraint::Min(3),
                    Constraint::Length(treasure_image_height),
                ])
                .split(split[1]);

                let mut text = "".to_string();
                for (resource, &amount) in resources.iter() {
                    if amount > 0 {
                        text.push_str(
                            format!("  {} {}\n", amount, resource.to_string().to_lowercase())
                                .as_str(),
                        );
                    }
                }

                if players.len() > 0 {
                    text.push_str(
                    format! {"\nFound {} stranded pirate{}:\n", players.len(), if players.len() > 1 {
                        "s"
                    }else{""}}.as_str(),
                );
                    for player in players.iter() {
                        let p_text = format!(
                            "  {:<16} {}\n",
                            player.info.shortened_name(),
                            player.stars()
                        );
                        text.push_str(p_text.as_str());
                    }
                }

                if text.len() == 0 {
                    text.push_str("Nothing found!")
                }

                frame.render_widget(
                    Paragraph::new(text).centered().wrap(Wrap { trim: true }),
                    m_split[0].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );

                if resources.value(&Resource::GOLD) > 0 {
                    // Tick::now() returns time as milliseconds. To implement the wanted framerate,
                    // we need to divide by the frame duration in milliseconds. After the last frame,
                    // we just leave the treasure open rather than looping.
                    let current_frame = if Tick::now() - tick > TREASURE_GIF_ANIMATION_DELAY {
                        (((Tick::now() - tick - TREASURE_GIF_ANIMATION_DELAY)
                            / FRAME_DURATION_MILLIS) as usize)
                            .min(treasure.len() - 1)
                    } else {
                        0
                    };

                    frame.render_widget(
                        Paragraph::new(treasure[current_frame].clone()).centered(),
                        m_split[1],
                    );
                }

                let button = Button::new(
                    UiText::YES.into(),
                    UiCallback::CloseUiPopup,
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

            PopupMessage::TeamLanded {
                team_name,
                planet_name,
                planet_filename,
                tick,
            } => {
                frame.render_widget(
                    Paragraph::new(format!("Team landed: {}", tick.formatted_as_date()))
                        .block(default_block().border_style(UiStyle::HIGHLIGHT))
                        .centered(),
                    split[0],
                );

                let planet_gif =
                    Gif::open(format!("planets/{}_zoomout.gif", planet_filename))?.to_lines();

                if planet_gif.len() == 0 {
                    return Err(anyhow!("Invalid planet gif"));
                }

                let planet_image_height = planet_gif[0].len() as u16;

                let m_split = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Length(planet_image_height),
                    Constraint::Min(0),
                ])
                .split(split[1]);

                let text = format!("{} landed on planet {}.", team_name, planet_name);
                frame.render_widget(
                    Paragraph::new(text).centered().wrap(Wrap { trim: true }),
                    m_split[0].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );

                // Tick::now() returns time as milliseconds. To implement the wanted framerate,
                // we need to divide by the frame duration in milliseconds
                let current_frame =
                    ((Tick::now() - tick) / FRAME_DURATION_MILLIS) as usize % planet_gif.len();

                frame.render_widget(
                    Paragraph::new(planet_gif[current_frame].clone()).centered(),
                    m_split[1],
                );

                let button = Button::new(
                    UiText::YES.into(),
                    UiCallback::CloseUiPopup,
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

            PopupMessage::Tutorial { index, .. } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Tutorial {}/{}",
                        index + 1,
                        PopupMessage::MAX_TUTORIAL_PAGE + 1
                    ))
                    .block(default_block().border_style(UiStyle::NETWORK))
                    .centered(),
                    split[0],
                );

                let message = match index {
                    0 => "Hello pirate! This is your team page.\nHere you can check your pirates and ship and interact with the market.",
                    1 => "To start, you can try to challenge another team to a game,\nor maybe explore around your planet to gather resources.",
                    _ => "Have fun!"
                };

                frame.render_widget(
                    Paragraph::new(message).centered().wrap(Wrap { trim: true }),
                    split[1].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );

                let next_button = Button::new(
                    "Next >>".into(),
                    UiCallback::PushTutorialPage { index: index + 1 },
                    Arc::clone(&callback_registry),
                )
                .set_hover_text("Next tutorial".into(), hover_text_target)
                .set_hotkey(UiKey::YES_TO_DIALOG)
                .set_box_style(UiStyle::OK)
                .set_layer(1);

                let close_button = Button::new(
                    "Close".into(),
                    UiCallback::CloseUiPopup,
                    Arc::clone(&callback_registry),
                )
                .set_hover_text("Skip the tutorial".into(), hover_text_target)
                .set_hotkey(UiKey::NO_TO_DIALOG)
                .set_box_style(UiStyle::ERROR)
                .set_layer(1);

                match index {
                    x if *x < PopupMessage::MAX_TUTORIAL_PAGE => {
                        let buttons_split =
                            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                                .split(split[2]);
                        frame.render_widget(next_button, buttons_split[0]);
                        frame.render_widget(close_button, buttons_split[1]);
                    }
                    _ => {
                        frame.render_widget(close_button, split[2]);
                    }
                }
            }
        }
        Ok(())
    }
}
