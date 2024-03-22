use super::{
    button::Button,
    clickable_list::{ClickableList, ClickableListItem},
    constants::{PrintableKeyCode, UiKey, UiStyle},
    gif_map::GifMap,
    hover_text_line::HoverTextLine,
    hover_text_span::HoverTextSpan,
    traits::StyledRating,
    ui_callback::{CallbackRegistry, UiCallbackPreset},
    utils::hover_text_target,
};
use crate::{
    engine::constants::MAX_TIREDNESS,
    image::{player::PLAYER_IMAGE_WIDTH, spaceship::SPACESHIP_IMAGE_WIDTH},
    types::{AppResult, SystemTimeTick, Tick, AU, HOURS, SECONDS},
    world::{
        constants::CURRENCY_SYMBOL,
        player::Player,
        position::{GamePosition, Position, MAX_POSITION},
        resources::Resource,
        skill::{GameSkill, Rated, SKILL_NAMES},
        team::Team,
        types::TeamLocation,
        world::World,
    },
};
use once_cell::sync::Lazy;
use ratatui::{
    prelude::*,
    text::Span,
    widgets::{block::Block, BorderType, Borders, List, Paragraph},
    Frame,
};
use std::{sync::Arc, sync::Mutex};

const POPUP_WIDTH: u16 = 48;
const POPUP_HEIGHT: u16 = 20;
pub const UP_ARROW_SPAN: Lazy<Span<'static>> = Lazy::new(|| Span::styled("↑", UiStyle::OK));
pub const DOWN_ARROW_SPAN: Lazy<Span<'static>> = Lazy::new(|| Span::styled("↓", UiStyle::ERROR));
pub const SWITCH_ARROW_SPAN: Lazy<Span<'static>> =
    Lazy::new(|| Span::styled("⇆", Style::default().fg(Color::Yellow)));

pub fn default_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
}

pub fn default_list() -> List<'static> {
    List::new::<Vec<String>>(vec![])
}

pub fn popup_rect(area: Rect) -> Rect {
    let x = if area.width < POPUP_WIDTH {
        0
    } else {
        (area.width - POPUP_WIDTH) / 2
    };
    let y = if area.height < POPUP_HEIGHT {
        0
    } else {
        (area.height - POPUP_HEIGHT) / 2
    };

    let width = if area.width < x + POPUP_WIDTH {
        area.width
    } else {
        POPUP_WIDTH
    };

    let height = if area.height < y + POPUP_HEIGHT {
        area.height
    } else {
        POPUP_HEIGHT
    };

    Rect::new(x, y, width, height)
}

pub fn selectable_list<'a>(
    options: Vec<(String, Style)>,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
) -> ClickableList<'a> {
    let items: Vec<ClickableListItem> = options
        .iter()
        .enumerate()
        .map(|(_, content)| {
            ClickableListItem::new(Span::styled(format!(" {}", content.0), content.1))
        })
        .collect();

    ClickableList::new(items, Arc::clone(&callback_registry))
        .highlight_style(UiStyle::SELECTED)
        .hovering_style(UiStyle::HIGHLIGHT)
}

pub fn go_to_team_planet_button<'a>(
    world: &World,
    team: &Team,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
) -> AppResult<Button<'a>> {
    let go_to_team_planet_button = match team.current_location {
        TeamLocation::OnPlanet { planet_id } => Button::new(
            format!(
                "{}: On planet {}",
                UiKey::GO_TO_PLANET.to_string(),
                world.get_planet_or_err(planet_id)?.name
            ),
            UiCallbackPreset::GoToCurrentTeamPlanet { team_id: team.id },
            Arc::clone(&callback_registry),
        )
        .set_hover_text(
            format!("Go to planet {}", world.get_planet_or_err(planet_id)?.name),
            hover_text_target,
        ),

        TeamLocation::Travelling {
            from: _from,
            to,
            started,
            duration,
        } => {
            let to = world.get_planet_or_err(to)?.name.to_string();
            let text = if started + duration > world.last_tick_short_interval + 3 * SECONDS {
                format!("Travelling to {}", to)
            } else {
                "Landing".into()
            };
            let countdown = if started + duration > world.last_tick_short_interval {
                (started + duration - world.last_tick_short_interval).formatted()
            } else {
                (0 as Tick).formatted()
            };
            let mut button = Button::new(
                format!("{} {}", text, countdown),
                UiCallbackPreset::None,
                Arc::clone(&callback_registry),
            );
            button.disable(None);
            button.set_hover_text(format!("Travelling to planet {}", to), hover_text_target)
        }
        TeamLocation::Exploring {
            around,
            started,
            duration,
        } => {
            let around_planet = world.get_planet_or_err(around)?.name.to_string();
            let text = if started + duration > world.last_tick_short_interval + 3 * SECONDS {
                format!("Around {}", around_planet)
            } else {
                "Landing".into()
            };
            let countdown = if started + duration > world.last_tick_short_interval {
                (started + duration - world.last_tick_short_interval).formatted()
            } else {
                (0 as Tick).formatted()
            };
            let mut button = Button::new(
                format!("{} {}", text, countdown),
                UiCallbackPreset::None,
                Arc::clone(&callback_registry),
            );
            button.disable(None);
            button.set_hover_text(
                format!("Exploring around planet {}", around_planet),
                hover_text_target,
            )
        }
    };

    Ok(go_to_team_planet_button)
}

pub fn trade_button<'a>(
    world: &World,
    resource: Resource,
    amount: i32,
    unit_cost: u32,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
) -> AppResult<Button<'a>> {
    let style = if amount > 0 {
        UiStyle::OK
    } else if amount < 0 {
        UiStyle::ERROR
    } else {
        UiStyle::DEFAULT
    };
    let mut button = Button::new(
        format!("{amount:^+}"),
        UiCallbackPreset::TradeResource {
            resource,
            amount,
            unit_cost,
        },
        Arc::clone(&callback_registry),
    )
    .set_box_style(style);

    if world
        .get_own_team()?
        .can_trade_resource(resource, amount, unit_cost)
        .is_err()
    {
        button = button.set_box_style(UiStyle::UNSELECTABLE);
        button.disable(None);
    }

    let button = button.set_hover_text(
        format!(
            "{} {} {} for {} {}.",
            if amount > 0 { "Buy" } else { "Sell" },
            amount.abs(),
            resource,
            amount.abs() as u32 * unit_cost,
            CURRENCY_SYMBOL
        ),
        hover_text_target,
    );

    Ok(button)
}

pub fn explore_button<'a>(
    world: &World,
    team: &Team,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
) -> AppResult<Button<'a>> {
    let explore_button = match team.current_location {
        TeamLocation::OnPlanet { planet_id } => {
            let planet = world.get_planet_or_err(planet_id)?;
            team.can_explore_around_planet(&planet)?;
            let explore_time = 10 * SECONDS;
            Button::new(
                format!(
                    "{}: Explore ({})",
                    UiKey::EXPLORE.to_string(),
                    explore_time.formatted()
                ),
                UiCallbackPreset::ExploreAroundPlanet,
                Arc::clone(&callback_registry),
            )
            .set_hover_text(
                format!(
                    "Explore the space around {}. Hope to find resources, free agents or more...",
                    planet.name
                ),
                hover_text_target,
            )
        }
        TeamLocation::Travelling {
            from: _from,
            to,
            started,
            duration,
        } => {
            let to = world.get_planet_or_err(to)?.name.to_string();
            let text = if started + duration > world.last_tick_short_interval + 3 * SECONDS {
                format!("Travelling to {}", to)
            } else {
                "Landing".into()
            };
            let countdown = if started + duration > world.last_tick_short_interval {
                (started + duration - world.last_tick_short_interval).formatted()
            } else {
                (0 as Tick).formatted()
            };
            let mut button = Button::new(
                format!("{} {}", text, countdown,),
                UiCallbackPreset::None,
                Arc::clone(&callback_registry),
            );
            button.disable(None);
            button.set_hover_text(format!("Travelling to planet {}", to), hover_text_target)
        }
        TeamLocation::Exploring {
            around,
            started,
            duration,
        } => {
            let around_planet = world.get_planet_or_err(around)?.name.to_string();
            let text = if started + duration > world.last_tick_short_interval + 3 * SECONDS {
                format!("Around {}", around_planet)
            } else {
                "Landing".into()
            };
            let countdown = if started + duration > world.last_tick_short_interval {
                (started + duration - world.last_tick_short_interval).formatted()
            } else {
                (0 as Tick).formatted()
            };
            let mut button = Button::new(
                format!("{} {}", text, countdown),
                UiCallbackPreset::None,
                Arc::clone(&callback_registry),
            );
            button.disable(None);
            button.set_hover_text(
                format!("Exploring around planet {}", around_planet),
                hover_text_target,
            )
        }
    };

    Ok(explore_button)
}
pub fn render_spaceship_description(
    team: &Team,
    gif_map: &Arc<Mutex<GifMap>>,
    tick: usize,
    world: &World,
    frame: &mut Frame,
    area: Rect,
) {
    let spaceship_split = Layout::horizontal([
        Constraint::Length(SPACESHIP_IMAGE_WIDTH as u16 + 2),
        Constraint::Min(1),
    ])
    .split(area.inner(&Margin {
        horizontal: 1,
        vertical: 1,
    }));

    if let Ok(lines) = gif_map
        .lock()
        .unwrap()
        .spaceship_lines(team.id, tick, world)
    {
        let paragraph = Paragraph::new(lines);
        frame.render_widget(
            paragraph.centered(),
            spaceship_split[0].inner(&Margin {
                horizontal: 1,
                vertical: 0,
            }),
        );
    }

    let spaceship_info = if team.id == world.own_team_id {
        Paragraph::new(vec![
            Line::from(format!("Reputation {}", team.reputation.stars())),
            Line::from(format!("Treasury {} {}", team.balance(), CURRENCY_SYMBOL)),
            Line::from(format!(
                "Food {}",
                team.resources.get(&Resource::FOOD).unwrap_or(&0)
            )),
            Line::from(format!(
                "Gold {}",
                team.resources.get(&Resource::GOLD).unwrap_or(&0)
            )),
            Line::from(format!("Ship name: {}", team.spaceship.name.to_string())),
            Line::from(format!(
                "Speed: {:.3} AU/h",
                team.spaceship.speed() * HOURS as f32 / AU as f32
            )),
            Line::from(format!(
                "Crew: {}/{}",
                team.player_ids.len(),
                team.spaceship.crew_capacity()
            )),
            Line::from(format!(
                "Storage: {}/{}",
                team.used_storage_capacity(),
                team.max_storage_capacity()
            )),
            Line::from(format!(
                "Tank: {}/{} t",
                team.fuel(),
                team.spaceship.fuel_capacity()
            )),
            Line::from(format!(
                "Consumption: {:.2} t/h",
                team.spaceship.fuel_consumption() * HOURS as f32
            )),
            Line::from(format!(
                "Max distance: {:.0} AU",
                team.spaceship.max_distance(team.fuel()) / AU as f32
            )),
            Line::from(format!(
                "Value: {} {}",
                team.spaceship.cost(),
                CURRENCY_SYMBOL
            )),
        ])
    } else {
        Paragraph::new(vec![
            Line::from(format!("Reputation {}", team.reputation.stars())),
            Line::from(format!("Treasury {} {}", team.balance(), CURRENCY_SYMBOL)),
            Line::from(format!("Ship name: {}", team.spaceship.name.to_string())),
            Line::from(format!(
                "Crew: {}/{}",
                team.player_ids.len(),
                team.spaceship.crew_capacity()
            )),
        ])
    };

    frame.render_widget(
        spaceship_info,
        spaceship_split[1].inner(&Margin {
            horizontal: 1,
            vertical: 1,
        }),
    );

    // Render main block
    let block = default_block()
        .title("Spaceship")
        .title_alignment(Alignment::Left);
    frame.render_widget(block, area);
}

pub fn render_player_description(
    player: &Player,
    gif_map: &Arc<Mutex<GifMap>>,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    tick: usize,
    frame: &mut Frame,
    world: &World,
    area: Rect,
) {
    let h_split = Layout::horizontal([
        Constraint::Length(PLAYER_IMAGE_WIDTH as u16 + 4),
        Constraint::Min(2),
    ])
    .split(area);

    let header_body_img = Layout::vertical([Constraint::Length(2), Constraint::Min(2)]).split(
        h_split[0].inner(&Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );

    let header_body_stats = Layout::vertical([
        Constraint::Length(2),  //margin
        Constraint::Length(1),  //header
        Constraint::Length(1),  //header
        Constraint::Length(1),  //header
        Constraint::Length(1),  //margin
        Constraint::Length(20), //stats
    ])
    .split(h_split[1]);

    if let Ok(lines) = gif_map.lock().unwrap().player_frame_lines(&player, tick) {
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, header_body_img[1]);
    }

    let mut tiredness = player.tiredness;

    // Check if player is currently playing.
    // In this case, read current tiredness from game.
    if let Some(team_id) = player.team {
        if let Ok(team) = world.get_team_or_err(team_id) {
            if let Some(game_id) = team.current_game {
                if let Ok(game) = world.get_game_or_err(game_id) {
                    if let Some(p) = if game.home_team_in_game.team_id == team_id {
                        game.home_team_in_game.players.get(&player.id)
                    } else {
                        game.away_team_in_game.players.get(&player.id)
                    } {
                        tiredness = p.tiredness;
                    }
                }
            }
        }
    }

    let max_tiredness_length = 25;
    let tiredness_length =
        (tiredness / MAX_TIREDNESS * max_tiredness_length as f32).round() as usize;
    let energy_string = format!(
        "{}{}",
        "▰".repeat(max_tiredness_length - tiredness_length),
        "▱".repeat(tiredness_length),
    );
    let energy_style = match tiredness {
        x if x < MAX_TIREDNESS / 4.0 => UiStyle::OK,
        x if x < MAX_TIREDNESS / 2.0 => UiStyle::WARNING,
        x if x < MAX_TIREDNESS => UiStyle::ERROR,
        _ => UiStyle::UNSELECTABLE,
    };

    let hover_text_target = hover_text_target(frame);

    let line = HoverTextLine::from(vec![
        HoverTextSpan::new(
            Span::raw(format!(
                "Reputation {}  ",
                player.reputation.stars()
            )),
            "Reputation indicates how much the player is known and respected in the galaxy. It influences special trait bonuses and the player's hiring cost.".to_string(),
            hover_text_target,
            Arc::clone(&callback_registry),
        ),
        HoverTextSpan::new(
            Span::raw(format!(
                "Trait {}",
                if player.special_trait.is_some() {
                    format!("{}", player.special_trait.as_ref().unwrap())
                } else {
                    "None".to_string()
                }
            )),
            if player.special_trait.is_some() {
                player.special_trait.as_ref().unwrap().description(&player)}else{"".to_string()},
            hover_text_target,
            Arc::clone(&callback_registry),
        )
    ]);
    frame.render_widget(line, header_body_stats[1]);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("Energy ".to_string()),
            Span::styled(format!("{}", energy_string), energy_style),
        ])),
        header_body_stats[2],
    );

    frame.render_widget(
        Paragraph::new(format!(
            "{} yo, {} cm, {} kg, {}",
            player.info.age as u8,
            player.info.height as u8,
            player.info.weight as u8,
            player.info.population,
        )),
        header_body_stats[3],
    );

    frame.render_widget(
        Paragraph::new(format_player_data(player)),
        header_body_stats[5],
    );

    // Render main block
    let block = default_block()
        .title(format!(
            "{} {}",
            player.info.first_name, player.info.last_name
        ))
        .title_alignment(Alignment::Left);
    frame.render_widget(block, area);
}

fn improvement_indicator<'a>(skill: u8, previous: u8) -> Span<'a> {
    if skill > previous {
        UP_ARROW_SPAN.clone()
    } else if skill < previous {
        DOWN_ARROW_SPAN.clone()
    } else {
        Span::styled(" ", UiStyle::DEFAULT)
    }
}

fn format_player_data(player: &Player) -> Vec<Line> {
    let skills = player.current_skill_array();
    let mut text = vec![];
    let mut roles = (0..MAX_POSITION)
        .map(|i: Position| {
            (
                i.as_str().to_string(),
                i.player_rating(player.current_skill_array()),
            )
        })
        .collect::<Vec<(String, f32)>>();
    roles.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut spans = vec![];
    spans.push(Span::styled(
        format!("{:<2} {:<5}          ", roles[0].0, roles[0].1.stars()),
        roles[0].1.style(),
    ));
    spans.push(Span::raw(format!(
        "Athleticism {:<5}",
        player.athleticism.stars()
    )));
    text.push(Line::from(spans));

    for i in 0..4 {
        let mut spans = vec![];
        spans.push(Span::styled(
            format!("{:<2} {:<5}       ", roles[i + 1].0, roles[i + 1].1.stars()),
            roles[i + 1].1.style(),
        ));

        spans.push(Span::styled(
            format!("   {:<12}{:02} ", SKILL_NAMES[i], skills[i].value(),),
            skills[i].style(),
        ));
        spans.push(improvement_indicator(
            skills[i].value(),
            player.previous_skills[i].value(),
        ));

        text.push(Line::from(spans));
    }
    text.push(Line::from(""));

    text.push(Line::from(Span::raw(format!(
        "{:<8}{:<5}     {} {}",
        "Offense",
        player.offense.stars(),
        "Defense",
        player.defense.stars()
    ))));
    for i in 0..4 {
        let mut spans = vec![];
        spans.push(Span::styled(
            format!("{:<10}{:02} ", SKILL_NAMES[i + 4], skills[i + 4].value(),),
            skills[i + 4].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 4].value(),
            player.previous_skills[i + 4].value(),
        ));

        spans.push(Span::styled(
            format!(
                "    {:<12}{:02} ",
                SKILL_NAMES[i + 8],
                skills[i + 8].value(),
            ),
            skills[i + 8].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 8].value(),
            player.previous_skills[i + 8].value(),
        ));

        text.push(Line::from(spans));
    }
    text.push(Line::from(""));
    text.push(Line::from(Span::raw(format!(
        "{} {}   {} {}",
        "Technical",
        player.technical.stars(),
        "Mental",
        player.mental.stars()
    ))));

    for i in 0..4 {
        let mut spans = vec![];
        spans.push(Span::styled(
            format!("{:<10}{:02} ", SKILL_NAMES[i + 12], skills[i + 12].value(),),
            skills[i + 12].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 12].value(),
            player.previous_skills[i + 12].value(),
        ));

        spans.push(Span::styled(
            format!(
                "    {:<12}{:02} ",
                SKILL_NAMES[i + 16],
                skills[i + 16].value(),
            ),
            skills[i + 16].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 16].value(),
            player.previous_skills[i + 16].value(),
        ));

        text.push(Line::from(spans));
    }

    text
}
