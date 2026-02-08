use super::button::Button;
use super::constants::{UiStyle, UiText};
use super::gif_map::{self, GifMap, TREASURE_GIF};
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::utils::{img_to_lines, input_from_key_event, validate_textarea_input};
use super::widgets::{default_block, thick_block};
use crate::core::planet::PlanetType;
use crate::core::MAX_SKILL;
use crate::core::{player::Player, resources::Resource, skill::Rated};
use crate::image::utils::open_gif;
use crate::types::*;
use crate::ui::constants::MAX_NAME_LENGTH;
use crate::ui::gif_map::PORTAL_GIFS;
use crate::ui::traits::PrintableGif;
use crate::ui::ui_key;
use anyhow::anyhow;
use core::fmt::Debug;
use itertools::Itertools;
use ratatui::crossterm;
use ratatui::layout::{Constraint, Layout};
use ratatui::layout::{Margin, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph, Wrap};
use strum_macros::Display;
use tui_textarea::TextArea;

const FRAME_DURATION_MILLIS: Tick = 150;
const TREASURE_GIF_ANIMATION_DELAY: Tick = 450;

#[derive(Debug, Display, Clone, PartialEq)]
pub enum PopupMessage {
    Error {
        message: String,
        timestamp: Tick,
    },
    Warning {
        message: String,
        timestamp: Tick,
    },
    Ok {
        message: String,
        is_skippable: bool,
        timestamp: Tick,
    },
    PromptQuit {
        during_space_adventure: bool,
        timestamp: Tick,
    },
    ReleasePlayer {
        player_name: String,
        player_id: PlayerId,
        not_enough_players_for_game: bool,
        timestamp: Tick,
    },
    ConfirmSpaceAdventure {
        has_shooter: bool,
        average_tiredness: f32,
        timestamp: Tick,
    },
    AbandonAsteroid {
        asteroid_name: String,
        asteroid_id: PlanetId,
        timestamp: Tick,
    },
    AsteroidNameDialog {
        timestamp: Tick,
        asteroid_type: usize,
    },
    BuildSpaceCove {
        asteroid_name: String,
        asteroid_id: PlanetId,
        timestamp: Tick,
    },
    PortalFound {
        player_name: String,
        portal_target: String,
        timestamp: Tick,
    },
    ExplorationResult {
        planet_name: String,
        resources: ResourceMap,
        players: Vec<Player>,
        timestamp: Tick,
    },
    TeamLanded {
        team_name: String,
        planet_name: String,
        planet_filename: String,
        planet_type: PlanetType,
        timestamp: Tick,
    },
    Tutorial {
        index: usize,
        timestamp: Tick,
    },
}

impl PopupMessage {
    const MAX_TUTORIAL_PAGE: usize = 7;
    fn rect(&self, area: Rect) -> Rect {
        let (width, height) = match self {
            Self::AsteroidNameDialog { .. } => (54, 28),
            Self::PortalFound { .. } => (54, 44),
            Self::ExplorationResult { resources, .. } => {
                if resources.value(&Resource::GOLD) > 0 {
                    (54, 26)
                } else {
                    (54, 16)
                }
            }
            Self::TeamLanded { .. } => (54, 26),
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

    pub const fn is_skippable(&self) -> bool {
        match self {
            Self::Error { .. } | Self::Warning { .. } => true,
            Self::Ok { is_skippable, .. } => *is_skippable,
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
            Self::AsteroidNameDialog { timestamp, .. } => {
                if key_event.code == ui_key::YES_TO_DIALOG {
                    let mut name = popup_input.lines()[0].clone();
                    name = name
                        .chars()
                        .enumerate()
                        .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                        .take(MAX_NAME_LENGTH)
                        .collect();
                    if validate_textarea_input(popup_input, "Asteroid name") {
                        let filename = format!("asteroid{}", timestamp % 30);
                        return Some(UiCallback::NameAndAcceptAsteroid { name, filename });
                    }
                } else if key_event.code == ui_key::NO_TO_DIALOG {
                    if popup_input.lines()[0].is_empty() {
                        return Some(UiCallback::CloseUiPopup);
                    }
                    popup_input.input(input_from_key_event(key_event));
                } else {
                    popup_input.input(input_from_key_event(key_event));
                }
            }

            Self::ReleasePlayer { player_id, .. } => {
                if key_event.code == ui_key::YES_TO_DIALOG {
                    return Some(UiCallback::ReleasePlayer {
                        player_id: *player_id,
                    });
                } else if key_event.code == ui_key::NO_TO_DIALOG {
                    return Some(UiCallback::CloseUiPopup);
                }
            }

            Self::ConfirmSpaceAdventure { .. } => {
                if key_event.code == ui_key::YES_TO_DIALOG {
                    return Some(UiCallback::StartSpaceAdventure);
                } else if key_event.code == ui_key::NO_TO_DIALOG {
                    return Some(UiCallback::CloseUiPopup);
                }
            }

            Self::AbandonAsteroid { asteroid_id, .. } => {
                if key_event.code == ui_key::YES_TO_DIALOG {
                    return Some(UiCallback::AbandonAsteroid {
                        asteroid_id: *asteroid_id,
                    });
                } else if key_event.code == ui_key::NO_TO_DIALOG {
                    return Some(UiCallback::CloseUiPopup);
                }
            }

            Self::BuildSpaceCove { asteroid_id, .. } => {
                if key_event.code == ui_key::YES_TO_DIALOG {
                    return Some(UiCallback::BuildSpaceCove {
                        asteroid_id: *asteroid_id,
                    });
                } else if key_event.code == ui_key::NO_TO_DIALOG {
                    return Some(UiCallback::CloseUiPopup);
                }
            }

            Self::PromptQuit { .. } => {
                if key_event.code == ui_key::YES_TO_DIALOG {
                    return Some(UiCallback::QuitGame);
                } else if key_event.code == ui_key::NO_TO_DIALOG {
                    return Some(UiCallback::CloseUiPopup);
                }
            }

            Self::Tutorial { index, .. } => match key_event.code {
                ui_key::YES_TO_DIALOG => {
                    if *index == Self::MAX_TUTORIAL_PAGE {
                        return Some(UiCallback::CloseUiPopup);
                    }
                    return Some(UiCallback::PushTutorialPage { index: index + 1 });
                }

                ui_key::NO_TO_DIALOG => {
                    return Some(UiCallback::CloseUiPopup);
                }

                code => match index {
                    2 if code == ui_key::GO_TO_CHALLENGES => {
                        return Some(UiCallback::TutorialGoToChallenges)
                    }
                    3 if code == ui_key::GO_TO_MARKET => {
                        return Some(UiCallback::TutorialGoToMarket)
                    }
                    4 if code == ui_key::GO_TO_SHIPYARD => {
                        return Some(UiCallback::TutorialGoToShipyard)
                    }
                    5 if code == ui_key::GO_TO_FREE_PIRATES => {
                        return Some(UiCallback::TutorialGoToFreePirates)
                    }
                    6 if code == ui_key::GO_TO_SPACE_ADVENTURE => {
                        return Some(UiCallback::TutorialGoToSpaceAdventure)
                    }
                    7 if code == ui_key::GO_TO_CHAT => return Some(UiCallback::TutorialGoToChat),
                    _ => {}
                },
            },
            _ => {
                if key_event.code == ui_key::YES_TO_DIALOG || key_event.code == ui_key::NO_TO_DIALOG
                {
                    return Some(UiCallback::CloseUiPopup);
                }
            }
        }
        None
    }

    pub fn render(
        &self,
        frame: &mut UiFrame,
        area: Rect,
        popup_input: &mut TextArea<'static>,
    ) -> AppResult<()> {
        let rect = frame.to_screen_rect(self.rect(area));
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
        frame.render_widget(thick_block(), rect);
        match self {
            Self::Ok {
                message, timestamp, ..
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Message: {} {}",
                        timestamp.formatted_as_date(),
                        timestamp.formatted_as_time()
                    ))
                    .bold()
                    .block(default_block().border_style(UiStyle::OK))
                    .centered(),
                    split[0],
                );

                let lines = message.split("\n").map(Line::from).collect_vec();
                frame.render_widget(
                    Paragraph::new(lines).centered().wrap(Wrap { trim: true }),
                    split[1].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );
                let button = Button::new(UiText::YES, UiCallback::CloseUiPopup)
                    .set_hover_text("Close the popup")
                    .set_hotkey(ui_key::YES_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::OK))
                    .set_layer(1);

                frame.render_interactive_widget(
                    button,
                    split[2].inner(Margin {
                        vertical: 0,
                        horizontal: 8,
                    }),
                );
            }

            Self::Error { message, timestamp } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Error: {} {}",
                        timestamp.formatted_as_date(),
                        timestamp.formatted_as_time()
                    ))
                    .bold()
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
                let button = Button::new(UiText::YES, UiCallback::CloseUiPopup)
                    .set_hover_text("Close the popup")
                    .set_hotkey(ui_key::YES_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::OK))
                    .set_layer(1);

                frame.render_interactive_widget(
                    button,
                    split[2].inner(Margin {
                        vertical: 0,
                        horizontal: 8,
                    }),
                );
            }

            Self::Warning { message, timestamp } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Warning: {} {}",
                        timestamp.formatted_as_date(),
                        timestamp.formatted_as_time()
                    ))
                    .bold()
                    .block(default_block().border_style(UiStyle::WARNING))
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
                let button = Button::new(UiText::YES, UiCallback::CloseUiPopup)
                    .set_hover_text("Close the popup")
                    .set_hotkey(ui_key::YES_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::OK))
                    .set_layer(1);

                frame.render_interactive_widget(
                    button,
                    split[2].inner(Margin {
                        vertical: 0,
                        horizontal: 8,
                    }),
                );
            }

            Self::ReleasePlayer {
                player_name,
                player_id,
                not_enough_players_for_game,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new("Attention!")
                        .bold()
                        .block(default_block().border_style(UiStyle::WARNING))
                        .centered(),
                    split[0],
                );

                let mut text =
                    format!("Are you sure you want to release {player_name} from the crew?");
                if *not_enough_players_for_game {
                    text.push_str("\nThere will be not enough players for games!");
                }
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
                    UiText::YES,
                    UiCallback::ReleasePlayer {
                        player_id: *player_id,
                    },
                )
                .set_hover_text(format!("Confirm releasing {player_name}"))
                .set_hotkey(ui_key::YES_TO_DIALOG)
                .block(default_block().border_style(UiStyle::OK))
                .set_layer(1);

                frame.render_interactive_widget(confirm_button, buttons_split[0]);

                let no_button = Button::new(UiText::NO, UiCallback::CloseUiPopup)
                    .set_hover_text(format!("Don't release {player_name}"))
                    .set_hotkey(ui_key::NO_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::ERROR))
                    .set_layer(1);

                frame.render_interactive_widget(no_button, buttons_split[1]);
            }

            Self::ConfirmSpaceAdventure {
                has_shooter,
                average_tiredness,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new("Attention!")
                        .bold()
                        .block(default_block().border_style(UiStyle::WARNING))
                        .centered(),
                    split[0],
                );

                let mut text = format!(
                    "Go on a Space Adventure? It will spend 25% of your pirates' energy{}.",
                    if *average_tiredness > MAX_SKILL / 2.0 {
                        " and they are already quite tired"
                    } else {
                        ""
                    }
                );
                if !has_shooter {
                    text.push_str("Your spaceship has no shooters, it will be very dangerous!");
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

                let confirm_button = Button::new(UiText::YES, UiCallback::StartSpaceAdventure)
                    .set_hover_text("Start space adventure")
                    .set_hotkey(ui_key::YES_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::OK))
                    .set_layer(1);

                frame.render_interactive_widget(confirm_button, buttons_split[0]);

                let no_button = Button::new(UiText::NO, UiCallback::CloseUiPopup)
                    .set_hover_text("Don't start space adventure")
                    .set_hotkey(ui_key::NO_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::ERROR))
                    .set_layer(1);

                frame.render_interactive_widget(no_button, buttons_split[1]);
            }

            Self::AbandonAsteroid {
                asteroid_name,
                asteroid_id,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new("Attention!")
                        .bold()
                        .block(default_block().border_style(UiStyle::WARNING))
                        .centered(),
                    split[0],
                );
                frame.render_widget(
                    Paragraph::new(format!(
                        "Are you sure you want to abandon {asteroid_name}?\nYou will not be able to come back!"
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
                    UiText::YES,
                    UiCallback::AbandonAsteroid {
                        asteroid_id: *asteroid_id,
                    },
                )
                .set_hover_text(format!("Confirm abandoning {asteroid_name}"))
                .set_hotkey(ui_key::YES_TO_DIALOG)
                .block(default_block().border_style(UiStyle::OK))
                .set_layer(1);

                frame.render_interactive_widget(confirm_button, buttons_split[0]);

                let no_button = Button::new(UiText::NO, UiCallback::CloseUiPopup)
                    .set_hover_text(format!("Don't abandon {asteroid_name}"))
                    .set_hotkey(ui_key::NO_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::ERROR))
                    .set_layer(1);

                frame.render_interactive_widget(no_button, buttons_split[1]);
            }

            Self::BuildSpaceCove {
                asteroid_name,
                asteroid_id,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new("Attention!")
                        .bold()
                        .block(default_block().border_style(UiStyle::WARNING))
                        .centered(),
                    split[0],
                );
                frame.render_widget(
                    Paragraph::new(format!(
                        "Are you sure you want to build your space cove on {asteroid_name}?\nYou can only have one space cove!"
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
                    UiText::YES,
                    UiCallback::BuildSpaceCove {
                        asteroid_id: *asteroid_id,
                    },
                )
                .set_hover_text(format!("Confirm building space cove on {asteroid_name}"))
                .set_hotkey(ui_key::YES_TO_DIALOG)
                .block(default_block().border_style(UiStyle::OK))
                .set_layer(1);

                frame.render_interactive_widget(confirm_button, buttons_split[0]);

                let no_button = Button::new(UiText::NO, UiCallback::CloseUiPopup)
                    .set_hover_text(format!("Don't build space cove on {asteroid_name}"))
                    .set_hotkey(ui_key::NO_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::ERROR))
                    .set_layer(1);

                frame.render_interactive_widget(no_button, buttons_split[1]);
            }

            Self::PromptQuit {
                during_space_adventure,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new("Attention!")
                        .bold()
                        .block(default_block().border_style(UiStyle::WARNING))
                        .centered(),
                    split[0],
                );

                let text = if *during_space_adventure {
                    format!(
                        "Are you sure you want to quit?\nTo go back to the base press '{}'",
                        ui_key::space::BACK_TO_BASE
                    )
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

                let confirm_button = Button::new(UiText::YES, UiCallback::QuitGame)
                    .set_hover_text("Confirm quitting.".to_string())
                    .set_hotkey(ui_key::YES_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::OK))
                    .set_layer(1);

                frame.render_interactive_widget(confirm_button, buttons_split[0]);

                let no_button = Button::new(UiText::NO, UiCallback::CloseUiPopup)
                    .set_hover_text("Please don't go, don't goooooo...".to_string())
                    .set_hotkey(ui_key::NO_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::ERROR))
                    .set_layer(1);

                frame.render_interactive_widget(no_button, buttons_split[1]);
            }

            Self::AsteroidNameDialog {
                timestamp,
                asteroid_type,
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Asteroid discovered: {} {}",
                        timestamp.formatted_as_date(),
                        timestamp.formatted_as_time()
                    ))
                    .bold()
                    .block(default_block().border_style(UiStyle::HIGHLIGHT))
                    .centered(),
                    split[0],
                );

                let filename = format!("asteroid{asteroid_type}");
                let asteroid_img = img_to_lines(&gif_map::GifMap::asteroid_zoom_out(&filename)?[0]);

                if asteroid_img.is_empty() {
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
                    UiText::YES,
                    UiCallback::NameAndAcceptAsteroid { name, filename },
                )
                .set_hover_text("Name and set the asteroid as home planet")
                .set_hotkey(ui_key::YES_TO_DIALOG)
                .block(default_block().border_style(UiStyle::OK))
                .set_layer(1);

                if !validate_textarea_input(popup_input, "Asteroid name") {
                    ok_button.disable(Some("Invalid asteroid name"));
                }

                frame.render_interactive_widget(ok_button, buttons_split[0]);

                let no_button = Button::new(UiText::NO, UiCallback::CloseUiPopup)
                    .set_hover_text("Leave the asteroid alone!")
                    .set_hotkey(ui_key::NO_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::ERROR))
                    .set_layer(1);

                frame.render_interactive_widget(no_button, buttons_split[1]);
            }

            Self::PortalFound {
                player_name,
                portal_target,
                timestamp,
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Portal: {} {}",
                        timestamp.formatted_as_date(),
                        timestamp.formatted_as_time()
                    ))
                    .bold()
                    .block(default_block().border_style(UiStyle::HIGHLIGHT))
                    .centered(),
                    split[0],
                );

                // Select a portal pseudorandomly.
                let portal = &PORTAL_GIFS[*timestamp as usize % PORTAL_GIFS.len()];

                if portal.is_empty() {
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
                    "{player_name} got drunk while driving and accidentally found a portal to {portal_target}!"
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
                    ((Tick::now() - timestamp) / FRAME_DURATION_MILLIS) as usize % portal.len();

                frame.render_widget(
                    Paragraph::new(portal[current_frame].clone()).centered(),
                    m_split[1],
                );

                let button = Button::new(UiText::YES, UiCallback::CloseUiPopup)
                    .set_hover_text("Close the popup")
                    .set_hotkey(ui_key::YES_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::OK))
                    .set_layer(1);

                frame.render_interactive_widget(
                    button,
                    split[2].inner(Margin {
                        vertical: 0,
                        horizontal: 8,
                    }),
                );
            }

            Self::ExplorationResult {
                planet_name,
                resources,
                players,
                timestamp,
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Exploration result: {} {}",
                        timestamp.formatted_as_date(),
                        timestamp.formatted_as_time()
                    ))
                    .bold()
                    .block(default_block().border_style(UiStyle::HIGHLIGHT))
                    .centered(),
                    split[0],
                );

                let treasure = &TREASURE_GIF;

                if treasure.is_empty() {
                    return Err(anyhow!("Invalid treasure gif"));
                }

                let treasure_image_height = if resources.value(&Resource::GOLD) > 0 {
                    treasure[0].len() as u16
                } else {
                    0
                };

                let m_split = Layout::vertical([
                    Constraint::Fill(1),
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

                if !players.is_empty() {
                    text.push_str(
                    format! {"\nFound {} stranded pirate{}:\n", players.len(), if players.len() > 1 {
                        "s"
                    }else{""}}.as_str(),
                );
                    for player in players.iter() {
                        let p_text =
                            format!("  {:<16} {}\n", player.info.short_name(), player.stars());
                        text.push_str(p_text.as_str());
                    }

                    text.push_str(format!("You can hire them on {planet_name}").as_str());
                }

                if text.is_empty() {
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
                    let current_frame = if Tick::now() - timestamp > TREASURE_GIF_ANIMATION_DELAY {
                        (((Tick::now() - timestamp - TREASURE_GIF_ANIMATION_DELAY)
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

                let button = Button::new(UiText::YES, UiCallback::CloseUiPopup)
                    .set_hover_text("Close the popup")
                    .set_hotkey(ui_key::YES_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::OK))
                    .set_layer(1);

                frame.render_interactive_widget(
                    button,
                    split[2].inner(Margin {
                        vertical: 0,
                        horizontal: 8,
                    }),
                );
            }

            Self::TeamLanded {
                team_name,
                planet_name,
                planet_filename,
                planet_type,
                timestamp,
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Team landed: {} {}",
                        timestamp.formatted_as_date(),
                        timestamp.formatted_as_time()
                    ))
                    .bold()
                    .block(default_block().border_style(UiStyle::HIGHLIGHT))
                    .centered(),
                    split[0],
                );

                let planet_gif = if *planet_type == PlanetType::Asteroid {
                    GifMap::asteroid_zoom_out(planet_filename)?
                } else {
                    open_gif(format!("planets/{planet_filename}_zoomout.gif"))?
                };

                let planet_gif_lines = planet_gif.to_lines();

                if planet_gif_lines.is_empty() {
                    return Err(anyhow!("Invalid planet gif"));
                }

                let planet_image_height = planet_gif[0].len() as u16;

                let m_split = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Length(planet_image_height),
                    Constraint::Min(0),
                ])
                .split(split[1]);

                let text = format!("{team_name} landed on planet {planet_name}.");
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
                    ((Tick::now() - timestamp) / FRAME_DURATION_MILLIS) as usize % planet_gif.len();

                frame.render_widget(
                    Paragraph::new(planet_gif_lines[current_frame].clone()).centered(),
                    m_split[1],
                );

                let button = Button::new(UiText::YES, UiCallback::CloseUiPopup)
                    .set_hover_text("Close the popup")
                    .set_hotkey(ui_key::YES_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::OK))
                    .set_layer(1);

                frame.render_interactive_widget(
                    button,
                    split[2].inner(Margin {
                        vertical: 0,
                        horizontal: 8,
                    }),
                );
            }

            Self::Tutorial { index, .. } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "Tutorial {}/{}",
                        index + 1,
                        Self::MAX_TUTORIAL_PAGE + 1
                    ))
                    .bold()
                    .block(default_block().border_style(UiStyle::HIGHLIGHT))
                    .centered(),
                    split[0],
                );

                let messages = [
                     "Hello pirate! This is a brief tutorial to get you started. Check the wiki at wiki.rebels.frittura.org",
                     "You can navigate around by clicking on the tabs at the top or using the arrow keys.",
                     "To start, you can try to challenge another team to a game.",
                     "You can also explore around your planet to gather resources which you can then sell at the market.",
                     "Once you have enough resources, you can upgrade your spaceship in the Shipyard.",
                     "You can hire free pirates from the Pirates panel in exchange for satoshi.",
                     "After you add shooters to your spaceship, you can go in a Space Adventure and try to find Asteroids.",
                     "Be sure to check out the Chat in the Swarm panel from time to time.\nHave fun!"
                ];

                let message = messages.get(*index).copied().unwrap_or_default();

                let central_split =
                    Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).split(split[1]);

                frame.render_widget(
                    Paragraph::new(message).centered().wrap(Wrap { trim: true }),
                    central_split[0].inner(Margin::new(1, 1)),
                );

                let close_button = Button::new("Close", UiCallback::CloseUiPopup)
                    .set_hover_text("Skip the tutorial")
                    .set_hotkey(ui_key::NO_TO_DIALOG)
                    .block(default_block().border_style(UiStyle::ERROR))
                    .set_layer(1);

                let buttons_split =
                    Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                        .split(split[2]);

                let next_button =
                    Button::new("Next >>", UiCallback::PushTutorialPage { index: index + 1 })
                        .set_hover_text("Next tutorial")
                        .set_hotkey(ui_key::YES_TO_DIALOG)
                        .block(default_block().border_style(UiStyle::OK))
                        .set_layer(1);

                match index {
                    2 => {
                        let games_button =
                            Button::new("Challenges", UiCallback::TutorialGoToChallenges)
                                .set_hover_text("Go to available challenges")
                                .set_hotkey(ui_key::GO_TO_CHALLENGES)
                                .block(default_block())
                                .set_layer(1);
                        frame.render_interactive_widget(games_button, central_split[1]);

                        frame.render_interactive_widget(next_button, buttons_split[0]);
                        frame.render_interactive_widget(close_button, buttons_split[1]);
                    }
                    3 => {
                        let market_button = Button::new("Market", UiCallback::TutorialGoToMarket)
                            .set_hover_text("Go to market")
                            .set_hotkey(ui_key::GO_TO_MARKET)
                            .block(default_block())
                            .set_layer(1);
                        frame.render_interactive_widget(market_button, central_split[1]);

                        frame.render_interactive_widget(next_button, buttons_split[0]);
                        frame.render_interactive_widget(close_button, buttons_split[1]);
                    }
                    4 => {
                        let market_button =
                            Button::new("Shipyard", UiCallback::TutorialGoToShipyard)
                                .set_hover_text("Go to shipyard")
                                .set_hotkey(ui_key::GO_TO_SHIPYARD)
                                .block(default_block())
                                .set_layer(1);
                        frame.render_interactive_widget(market_button, central_split[1]);

                        frame.render_interactive_widget(next_button, buttons_split[0]);
                        frame.render_interactive_widget(close_button, buttons_split[1]);
                    }
                    5 => {
                        let market_button =
                            Button::new("Free Pirates", UiCallback::TutorialGoToFreePirates)
                                .set_hover_text("Go to free pirates")
                                .set_hotkey(ui_key::GO_TO_FREE_PIRATES)
                                .block(default_block())
                                .set_layer(1);
                        frame.render_interactive_widget(market_button, central_split[1]);

                        frame.render_interactive_widget(next_button, buttons_split[0]);
                        frame.render_interactive_widget(close_button, buttons_split[1]);
                    }
                    6 => {
                        let market_button =
                            Button::new("Space Adventure", UiCallback::TutorialGoToSpaceAdventure)
                                .set_hover_text("Go to space adventure")
                                .set_hotkey(ui_key::GO_TO_SPACE_ADVENTURE)
                                .block(default_block())
                                .set_layer(1);
                        frame.render_interactive_widget(market_button, central_split[1]);

                        frame.render_interactive_widget(next_button, buttons_split[0]);
                        frame.render_interactive_widget(close_button, buttons_split[1]);
                    }
                    7 => {
                        let chat_button = Button::new("Chat", UiCallback::TutorialGoToChat)
                            .set_hover_text("Go to Chat")
                            .set_hotkey(ui_key::GO_TO_CHAT)
                            .block(default_block().border_style(UiStyle::NETWORK))
                            .set_layer(1);
                        frame.render_interactive_widget(chat_button, central_split[1]);
                        frame.render_interactive_widget(close_button, split[2]);
                    }

                    _ => {
                        frame.render_interactive_widget(next_button, buttons_split[0]);
                        frame.render_interactive_widget(close_button, buttons_split[1]);
                    }
                }
            }
        }
        Ok(())
    }
}
