use std::collections::HashSet;
use std::iter::zip;

use super::ui_frame::UiFrame;
use super::{
    button::Button,
    clickable_list::{ClickableList, ClickableListItem},
    constants::*,
    gif_map::GifMap,
    hover_text_line::HoverTextLine,
    hover_text_span::HoverTextSpan,
    traits::UiStyled,
    ui_callback::UiCallback,
    utils::format_satoshi,
};
use crate::core::skill::{Skill, MAX_SKILL, MIN_SKILL};
use crate::core::types::TeamBonus;
use crate::core::{ChargeUnit, Honour, Shield, Shooter, SpaceshipComponent, UpgradeableElement};
use crate::ui::ui_key;
use crate::ui::utils::format_au;
use crate::{
    core::{
        constants::*,
        player::Player,
        position::{GamePosition, GamePositionUtils, MAX_GAME_POSITION},
        resources::Resource,
        skill::{GameSkill, Rated, SKILL_NAMES},
        spaceship_upgrades::SpaceshipUpgradeTarget,
        team::Team,
        types::TeamLocation,
        world::World,
    },
    image::{player::PLAYER_IMAGE_WIDTH, spaceship::SPACESHIP_IMAGE_WIDTH},
    types::*,
};
use anyhow::anyhow;
use crossterm::event::KeyCode;
use once_cell::sync::Lazy;
use ratatui::{
    prelude::*,
    text::Span,
    widgets::{Block, BorderType, Borders, Paragraph},
};
use strum::Display;

// This is used as a convenience value so that the bars are colored green if at max value.
pub const GREEN_STYLE_SKILL: f32 = 16.0;

pub static UP_ARROW_SPAN: Lazy<Span<'static>> = Lazy::new(|| Span::styled("↑", UiStyle::HEADER));
pub static UP_RIGHT_ARROW_SPAN: Lazy<Span<'static>> = Lazy::new(|| Span::styled("↗", UiStyle::OK));
pub static DOWN_ARROW_SPAN: Lazy<Span<'static>> = Lazy::new(|| Span::styled("↓", UiStyle::ERROR));
pub static DOWN_RIGHT_ARROW_SPAN: Lazy<Span<'static>> =
    Lazy::new(|| Span::styled("↘", UiStyle::WARNING));

pub static SWITCH_ARROW_SPAN: Lazy<Span<'static>> =
    Lazy::new(|| Span::styled("⇆", Style::default().fg(Color::Yellow)));

#[derive(Debug, Default, Display, Clone, Copy, PartialEq)]
pub enum PlayerWidgetView {
    #[default]
    Skills,
    Stats,
}

pub fn default_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
}

pub fn thick_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
}

pub fn selectable_list<'a>(options: Vec<(String, Style)>) -> ClickableList<'a> {
    let items: Vec<ClickableListItem> = options
        .iter()
        .map(|content| ClickableListItem::new(Span::styled(format!(" {}", content.0), content.1)))
        .collect();

    ClickableList::new(items)
}

pub fn go_to_planet_button<'a>(world: &World, planet_id: PlanetId) -> AppResult<Button<'a>> {
    let planet_name = world.planets.get_or_err(&planet_id)?.name.as_str();
    Ok(Button::new(
        format!("Go to planet: {planet_name}"),
        UiCallback::GoToPlanet { planet_id },
    )
    .set_hover_text(format!("Go to planet {planet_name}"))
    .set_hotkey(ui_key::GO_TO_PLANET))
}

pub fn go_to_space_cove_button<'a>() -> AppResult<Button<'a>> {
    Ok(Button::new("Go to space cove", UiCallback::GoToSpaceCove)
        .set_hover_text("Go to space cove panel".to_string())
        .set_hotkey(ui_key::GO_TO_SPACE_COVE))
}

pub fn teleport_button<'a>(world: &World, planet_id: PlanetId) -> AppResult<Button<'a>> {
    let own_team = world.get_own_team()?;
    let planet = world.planets.get_or_err(&planet_id)?;

    let mut teleport_button = Button::new(
        format!("Teleport (-{} Rum)", own_team.player_ids.len()),
        UiCallback::TravelToPlanet { planet_id },
    )
    .set_hover_text(format!(
        "Travel instantaneously to {}{}",
        planet.name,
        if planet_id == own_team.home_planet_id {
            "".to_string()
        } else {
            format!(" for {} Rum", own_team.player_ids.len()) // FIXME: don't hardcode this, but get it somehow from the teleport action.)
        }
    ))
    .set_hotkey(ui_key::TRAVEL);

    if let Err(e) = own_team.can_travel_to_planet(planet, 0) {
        teleport_button.disable(Some(e.to_string()));
    }

    Ok(teleport_button)
}

pub fn go_to_team_home_planet_button<'a>(world: &World, team_id: &TeamId) -> AppResult<Button<'a>> {
    let team = world.teams.get_or_err(team_id)?;
    let planet_name = &world.planets.get_or_err(&team.home_planet_id)?.name;
    Ok(Button::new(
        format!("Home planet {planet_name}"),
        UiCallback::GoToHomePlanet { team_id: team.id },
    )
    .set_hover_text(format!("Go to team home planet {planet_name}",))
    .set_hotkey(ui_key::GO_TO_HOME_PLANET))
}

pub fn go_to_team_current_planet_button<'a>(
    world: &World,
    team_id: &TeamId,
) -> AppResult<Button<'a>> {
    let team = world.teams.get_or_err(team_id)?;
    let go_to_team_current_planet_button = match team.current_location {
        TeamLocation::OnPlanet { planet_id } => Button::new(
            format!("On planet {}", world.planets.get_or_err(&planet_id)?.name),
            UiCallback::GoToCurrentTeamPlanet { team_id: team.id },
        )
        .set_hover_text(format!(
            "Go to planet {}",
            world.planets.get_or_err(&planet_id)?.name
        ))
        .set_hotkey(ui_key::ON_PLANET),

        TeamLocation::Travelling {
            from: _from,
            to,
            started,
            duration,
            ..
        } => {
            let to = world.planets.get_or_err(&to)?.name.to_string();
            let text = if started + duration > world.last_tick_short_interval + 3 * SECONDS {
                format!("Travelling to {to}")
            } else {
                "Landing".to_string()
            };

            Button::new(text, UiCallback::None).disabled(Some(format!("Travelling to planet {to}")))
        }
        TeamLocation::Exploring {
            around,
            started,
            duration,
        } => {
            let around_planet = world.planets.get_or_err(&around)?.name.to_string();
            let text = if started + duration > world.last_tick_short_interval + 3 * SECONDS {
                format!("Around {around_planet}")
            } else {
                "Landing".to_string()
            };
            let countdown = if started + duration > world.last_tick_short_interval {
                (started + duration - world.last_tick_short_interval).formatted()
            } else {
                (0 as Tick).formatted()
            };
            Button::new(format!("{text} {countdown}"), UiCallback::None)
                .disabled(Some(format!("Exploring around planet {around_planet}")))
        }
        TeamLocation::OnSpaceAdventure { .. } => {
            return Err(anyhow!("Team is on a space adventure"))
        }
    };

    Ok(go_to_team_current_planet_button)
}

pub fn drink_button<'a>(world: &World, player_id: &PlayerId) -> AppResult<Button<'a>> {
    let player = world.players.get_or_err(player_id)?;
    let can_drink = player.can_drink(world);

    let mut button = Button::new(
        "Drink! (-1 Rum)",
        UiCallback::Drink {
            player_id: *player_id,
        },
    )
    .set_hotkey(ui_key::player::DRINK)
    .set_hover_text("Drink a liter of rum, increasing morale and decreasing energy.");

    if let Err(err) = can_drink {
        button.disable(Some(err.to_string()));
    }

    Ok(button)
}

pub fn render_challenge_button(
    world: &World,
    team: &Team,
    hotkey: bool,
    frame: &mut UiFrame,
    area: Rect,
) -> AppResult<()> {
    let own_team = world.get_own_team()?;
    let can_challenge = if team.peer_id.is_some() {
        own_team.can_challenge_network_team(team)
    } else if let Err(e) = own_team.can_challenge_local_team(team) {
        Err(e)
    } else {
        // If other team is local, reject challenge if team is too tired
        let average_tiredness = team.average_tiredness(world);
        if average_tiredness > MAX_AVG_TIREDNESS_PER_CHALLENGED_GAME {
            Err(anyhow!("{} is too tired", team.name))
        } else {
            Ok(())
        }
    };

    // If we received a challenge from that team, display the accept/decline buttons
    if let Some(challenge) = own_team.received_challenges.get(&team.id) {
        let c_split = Layout::horizontal([
            Constraint::Min(10),
            Constraint::Length(6),
            Constraint::Length(6),
        ])
        .split(area);

        let mut accept_button = Button::new(
            format!("{:6^}", UiText::YES),
            UiCallback::AcceptChallenge {
                challenge: challenge.clone(),
            },
        )
        .block(default_block().border_style(UiStyle::OK))
        .set_hover_text(format!(
            "Accept the challenge from {} and start a game.",
            team.name
        ));
        if own_team.current_game.is_some() {
            accept_button.disable(Some(format!("{} is already playing", own_team.name)));
        } else if let Err(e) = own_team.can_accept_network_challenge(team) {
            accept_button.disable(Some(e.to_string()));
        }

        let decline_button = Button::new(
            format!("{:6^}", UiText::NO),
            UiCallback::DeclineChallenge {
                challenge: challenge.clone(),
            },
        )
        .block(default_block().border_style(UiStyle::ERROR))
        .set_hover_text(format!("Decline the challenge from {}.", team.name));

        frame.render_widget(
            Paragraph::new("Challenged!")
                .centered()
                .block(default_block()),
            c_split[0],
        );

        frame.render_interactive_widget(accept_button, c_split[1]);
        frame.render_interactive_widget(decline_button, c_split[2]);
        return Ok(());
    }

    let challenge_button = if let Some(game_id) = team.current_game {
        // FIXME: The game is not necessarily part of the world if it's a network game.
        let game_text = if let Ok(game) = world.games.get_or_err(&game_id) {
            if let Some(action) = game.action_results.last() {
                format!(
                    "{} {:>3}-{:<3} {}",
                    game.home_team_in_game.name,
                    action.home_score,
                    action.away_score,
                    game.away_team_in_game.name,
                )
            } else {
                format!(
                    "{}   0-0   {}",
                    game.home_team_in_game.name, game.away_team_in_game.name,
                )
            }
        } else {
            "Local game".to_string()
        };
        let mut b = Button::new(
            format!("Playing - {game_text}"),
            UiCallback::GoToGame { game_id },
        )
        .set_hover_text("Go to team's game")
        .set_hotkey(ui_key::GO_TO_GAME);
        if world.games.get_or_err(&game_id).is_err() {
            b.disable(Some("Game is not visible"));
        }
        b
    } else {
        let mut button = Button::new("Challenge", UiCallback::ChallengeTeam { team_id: team.id })
            .set_hover_text(format!("Challenge {} to a game", team.name));

        if hotkey {
            button = button.set_hotkey(ui_key::game::CHALLENGE_TEAM)
        }

        if let Err(err) = can_challenge {
            button.disable(Some(err.to_string()));
        } else {
            button = if team.peer_id.is_some() {
                button.block(default_block().border_style(UiStyle::NETWORK))
            } else {
                button.block(default_block().border_style(UiStyle::OK))
            };
        }
        button
    };
    frame.render_interactive_widget(challenge_button, area);

    Ok(())
}

pub fn trade_resource_button<'a>(
    world: &World,
    resource: Resource,
    amount: i32,
    unit_cost: u32,

    hotkey: Option<KeyCode>,
    box_style: Style,
) -> AppResult<Button<'a>> {
    let mut button = Button::new(
        format!("{amount:^+}"),
        UiCallback::TradeResource {
            resource,
            amount,
            unit_cost,
        },
    )
    .block(default_block().border_style(box_style));

    let can_trade_resource = world
        .get_own_team()?
        .can_trade_resource(resource, amount, unit_cost);
    if let Err(e) = can_trade_resource {
        button.disable(Some(e.to_string()));
    }

    if amount == 0 {
        button.set_text("");
        let disabled_text: Option<&str> = None;
        button.disable(disabled_text);
    }

    let mut button = button.set_hover_text(format!(
        "{} {} {} for {}.",
        if amount > 0 { "Buy" } else { "Sell" },
        amount.abs(),
        resource,
        format_satoshi(amount.unsigned_abs() * unit_cost),
    ));
    if let Some(key) = hotkey {
        button = button.set_hotkey(key);
    }

    Ok(button)
}

pub fn explore_button<'a>(world: &World, team: &Team) -> AppResult<Button<'a>> {
    let duration = EXPLORATION_DURATION;
    let mut button = Button::new(
        format!("Explore ({})", duration.formatted()),
        UiCallback::ExploreAroundPlanet { duration },
    )
    .set_hotkey(ui_key::EXPLORE);

    match team.current_location {
        TeamLocation::OnPlanet { planet_id } => {
            let planet = world.planets.get_or_err(&planet_id)?;
            let needed_fuel = (duration as f32 * team.spaceship_fuel_consumption_per_tick()) as u32;
            button = button.set_hover_text(
                format!(
                    "Explore the space around {} on autopilot (need {} t of fuel). Hope to find resources, free pirates or more...",
                    planet.name,
                    needed_fuel
                ),
            );

            if let Err(msg) = team.can_explore_around_planet(planet, duration) {
                button.disable(Some(msg.to_string()));
            }
        }
        TeamLocation::Travelling {
            from: _from, to, ..
        } => {
            button = button.set_hover_text(
                "Explore the space on autopilot. Hope to find resources, free pirates or more..."
                    .to_string(),
            );
            let to = world.planets.get_or_err(&to)?.name.to_string();
            button.disable(Some(format!("Travelling to planet {to}")));
        }
        TeamLocation::Exploring { around, .. } => {
            button = button.set_hover_text(
                "Explore the space on autopilot. Hope to find resources, free pirates or more..."
                    .to_string(),
            );
            let around_planet = world.planets.get_or_err(&around)?.name.to_string();
            button.disable(Some(format!("Exploring around planet {around_planet}")));
        }
        TeamLocation::OnSpaceAdventure { .. } => {
            return Err(anyhow!("Team is on a space adventure"))
        }
    };

    Ok(button)
}

pub fn space_adventure_button<'a>(world: &World, team: &Team) -> AppResult<Button<'a>> {
    let mut button = Button::new("Space Adventure", UiCallback::StartSpaceAdventure)
        .set_hotkey(ui_key::SPACE_ADVENTURE);

    match team.current_location {
        TeamLocation::OnPlanet { planet_id } => {
            let planet = world.planets.get_or_err(&planet_id)?;
            button = button.set_hover_text(format!(
                "Start a space adventure around {} to collect resources and more...",
                planet.name,
            ));

            if let Err(msg) = team.can_start_space_adventure() {
                button.disable(Some(msg.to_string()));
            }
        }
        TeamLocation::Travelling {
            from: _from, to, ..
        } => {
            button = button.set_hover_text(
                "Start a space adventure to manually collect resources and more...".to_string(),
            );
            let to = world.planets.get_or_err(&to)?.name.to_string();
            button.disable(Some(format!("Travelling to planet {to}")));
        }
        TeamLocation::Exploring { around, .. } => {
            button = button.set_hover_text(
                "Start a space adventure to manually collect resources and more...".to_string(),
            );
            let around_planet = world.planets.get_or_err(&around)?.name.to_string();
            button.disable(Some(format!("Exploring around planet {around_planet}")));
        }
        TeamLocation::OnSpaceAdventure { .. } => {
            return Err(anyhow!("Already on a space adventure"))
        }
    };

    Ok(button)
}

pub(crate) fn get_storage_lengths(
    resources: &ResourceMap,
    storage_capacity: u32,
    bars_length: usize,
) -> Vec<usize> {
    let gold = resources.value(&Resource::GOLD);
    let scraps = resources.value(&Resource::SCRAPS);
    let rum = resources.value(&Resource::RUM);

    // Calculate temptative length
    let mut gold_length = ((Resource::GOLD.to_storing_space() * gold) as f32
        / storage_capacity as f32
        * bars_length as f32)
        .round() as usize;
    let mut scraps_length = ((Resource::SCRAPS.to_storing_space() * scraps) as f32
        / storage_capacity as f32
        * bars_length as f32)
        .round() as usize;
    let mut rum_length = ((Resource::RUM.to_storing_space() * rum) as f32 / storage_capacity as f32
        * bars_length as f32)
        .round() as usize;

    // If the quantity is larger than 0, we should display it with at least 1 bar.
    if gold > 0 {
        gold_length = gold_length.max(1);
    }
    if scraps > 0 {
        scraps_length = scraps_length.max(1);
    }
    if rum > 0 {
        rum_length = rum_length.max(1);
    }

    // free_bars can be negative because of the previous rule.
    let mut free_bars: isize =
        bars_length as isize - (gold_length + scraps_length + rum_length) as isize;

    // If free_bars is negative, remove enough bars from the largest length.
    if free_bars < 0 {
        if gold_length > scraps_length && gold_length > rum_length {
            gold_length -= (-free_bars) as usize;
        } else if rum_length > scraps_length {
            rum_length -= (-free_bars) as usize;
        } else {
            scraps_length -= (-free_bars) as usize;
        }
        free_bars = 0;
    } else if free_bars > 0 {
        // Round up to eliminate free bars when storage is full
        let free_space = storage_capacity - resources.used_storage_capacity();
        if free_space == 0 {
            if gold_length >= scraps_length && gold_length >= rum_length {
                gold_length += free_bars as usize;
            } else if rum_length >= scraps_length {
                rum_length += free_bars as usize;
            } else {
                scraps_length += free_bars as usize;
            }
            free_bars = 0
        }
    }

    vec![gold_length, scraps_length, rum_length, free_bars as usize]
}

pub fn get_storage_spans(
    resources: &'_ ResourceMap,
    storage_capacity: u32,
    bars_length: usize,
) -> Vec<Span<'_>> {
    if let [gold_length, scraps_length, rum_length, free_bars] =
        get_storage_lengths(resources, storage_capacity, bars_length)[..4]
    {
        vec![
            Span::raw("Stiva  ".to_string()),
            Span::styled("▰".repeat(gold_length), Resource::GOLD.style()),
            Span::styled("▰".repeat(scraps_length), Resource::SCRAPS.style()),
            Span::styled("▰".repeat(rum_length), Resource::RUM.style()),
            Span::raw("▱".repeat(free_bars)),
            Span::raw(format!(
                " {:>04}/{:<04} ",
                resources.used_storage_capacity(),
                storage_capacity
            )),
        ]
    } else {
        vec![Span::raw("")]
    }
}

pub fn get_crew_spans<'a>(crew_size: usize, crew_capacity: usize) -> Vec<Span<'a>> {
    let bars_length = crew_capacity;
    let crew_length = crew_size;

    let crew_bars = format!(
        "{}{}",
        "▰".repeat(crew_length),
        "▱".repeat(bars_length.saturating_sub(crew_length)),
    );

    let crew_style = match crew_length {
        x if x < MIN_PLAYERS_PER_GAME => UiStyle::ERROR,
        x if x < crew_capacity => UiStyle::WARNING,
        _ => UiStyle::OK,
    };

    vec![
        Span::raw("Crew   ".to_string()),
        Span::styled(crew_bars, crew_style),
        Span::raw(format!(" {crew_size}/{crew_capacity}  ")),
    ]
}

pub fn get_energy_spans<'a>(average_tiredness: f32) -> Vec<Span<'a>> {
    let tiredness_length = (average_tiredness / MAX_SKILL * BARS_LENGTH as f32).round() as usize;
    let energy_string = format!(
        "{}{}",
        "▰".repeat(BARS_LENGTH.saturating_sub(tiredness_length)),
        "▱".repeat(tiredness_length),
    );
    let energy_style = ((MAX_SKILL - average_tiredness) / MAX_SKILL * GREEN_STYLE_SKILL).style();

    vec![
        Span::raw("Energy ".to_string()),
        Span::styled(energy_string, energy_style),
    ]
}

pub fn get_durability_spans<'a>(
    value: u32,
    max_value: u32,
    shield_value: u32,
    max_shield_value: u32,
    bars_length: usize,
) -> Vec<Span<'a>> {
    let value_bars_length = (max_value as f32 / (max_value + max_shield_value) as f32
        * bars_length as f32)
        .round() as usize;

    let mut shield_bars_length = (max_shield_value as f32 / (max_value + max_shield_value) as f32
        * bars_length as f32)
        .round() as usize;

    // In case we rounded up twice.
    if value_bars_length + shield_bars_length > bars_length {
        shield_bars_length -= 1;
    }

    let value_length =
        (value as f32 / max_value as f32 * value_bars_length as f32).round() as usize;
    let shield_value_length = (shield_value as f32 / max_shield_value as f32
        * shield_bars_length as f32)
        .round() as usize;

    let value_bars = "▰".repeat(value_length).to_string();
    let empty_bars = "▱"
        .repeat(value_bars_length.saturating_sub(value_length))
        .to_string();
    let shield_bars = "▰".repeat(shield_value_length).to_string();
    let shield_empty_bars = "▱"
        .repeat(shield_bars_length.saturating_sub(shield_value_length))
        .to_string();

    let value_style = (GREEN_STYLE_SKILL * (value as f32 / max_value as f32))
        .bound()
        .style();

    vec![
        Span::raw("Hull   "),
        Span::styled(value_bars, value_style),
        Span::styled(empty_bars, value_style),
        Span::styled(shield_bars, UiStyle::SHIELD),
        Span::styled(shield_empty_bars, UiStyle::SHIELD),
        Span::raw(format!(
            " {value}/{max_value}{}",
            if max_shield_value > 0 {
                format!("+{shield_value}/{max_shield_value}")
            } else {
                "".to_string()
            }
        )),
    ]
}

pub fn get_charge_spans<'a>(
    value: u32,
    max_value: u32,
    is_recharging: bool,
    bars_length: usize,
) -> Vec<Span<'a>> {
    let length = (value as f32 / max_value as f32 * bars_length as f32).round() as usize;
    let bars = format!("{}{}", "▰".repeat(length), "▱".repeat(bars_length - length),);

    let style = if is_recharging {
        MIN_SKILL.style()
    } else {
        (GREEN_STYLE_SKILL * (value as f32 / max_value as f32))
            .bound()
            .style()
    };

    vec![
        Span::raw(if is_recharging {
            format!("{:>6} ", "Reload",)
        } else {
            format!("{:>6} ", "Charge",)
        }),
        Span::styled(bars, style),
        Span::raw(format!(" {value}/{max_value}")),
    ]
}

pub fn get_fuel_spans<'a>(fuel: u32, fuel_capacity: u32, bars_length: usize) -> Vec<Span<'a>> {
    let fuel_length = (fuel.min(fuel_capacity) as f32 / fuel_capacity as f32 * bars_length as f32)
        .round() as usize;
    let fuel_bars = format!(
        "{}{}",
        "▰".repeat(fuel_length),
        "▱".repeat(bars_length.saturating_sub(fuel_length)),
    );

    let fuel_style = (GREEN_STYLE_SKILL * (fuel as f32 / fuel_capacity as f32))
        .bound()
        .style();

    vec![
        Span::raw("Tank   ".to_string()),
        Span::styled(fuel_bars, fuel_style),
        Span::raw(format!(" {fuel}/{fuel_capacity}")),
    ]
}

pub fn render_spaceship_description(
    team: &Team,
    world: &World,
    team_rating: Skill,
    full_info: bool,
    with_average_energy: bool,
    gif_map: &mut GifMap,
    tick: usize,
    frame: &mut UiFrame,
    area: Rect,
) {
    let spaceship_split = Layout::horizontal([
        Constraint::Length(SPACESHIP_IMAGE_WIDTH as u16 + 2),
        Constraint::Min(1),
    ])
    .split(area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    }));

    if let Ok(lines) = gif_map.on_planet_spaceship_lines(&team.spaceship, tick) {
        let paragraph = Paragraph::new(lines);
        frame.render_widget(
            paragraph.centered(),
            spaceship_split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );
    }

    if full_info {
        let average_tiredness = team.average_tiredness(world);
        let speed_bonus = TeamBonus::SpaceshipSpeed
            .current_team_bonus(world, &team.id)
            .unwrap_or(1.0);
        let weapon_bonus = TeamBonus::Weapons
            .current_team_bonus(world, &team.id)
            .unwrap_or(1.0);
        let widget = Paragraph::new(vec![
            Line::default(),
            Line::from(get_crew_spans(
                team.player_ids.len(),
                team.spaceship.crew_capacity() as usize,
            )),
            Line::from(get_energy_spans(average_tiredness)),
            Line::from(get_durability_spans(
                team.spaceship.current_durability(),
                team.spaceship.max_durability(),
                team.spaceship.shield_max_durability() as u32,
                team.spaceship.shield_max_durability() as u32,
                BARS_LENGTH,
            )),
            Line::from(get_charge_spans(
                team.spaceship.max_charge(),
                team.spaceship.max_charge(),
                false,
                BARS_LENGTH,
            )),
            Line::from(get_fuel_spans(
                team.fuel(),
                team.spaceship.fuel_capacity(),
                BARS_LENGTH,
            )),
            Line::from(get_storage_spans(
                &team.resources,
                team.spaceship.storage_capacity(),
                BARS_LENGTH,
            )),
            Line::from(format!(
                "Speed {:.3} AU/h",
                team.spaceship_speed() * speed_bonus * HOURS as f32 / AU as f32
            )),
            Line::from(format!(
                "Shooters {}x{}",
                (team.spaceship.damage() * team.spaceship.fire_rate() * weapon_bonus) as u8,
                team.spaceship.shooting_points()
            )),
            Line::from(format!(
                "Consumption {:.2} t/h  Max distance {:<5.3} AU ",
                team.spaceship_fuel_consumption_per_tick() * HOURS as f32,
                team.spaceship.max_distance(team.fuel()) / AU as f32,
            )),
            Line::from(format!(
                "Distance travelled {}",
                format_au(team.total_travelled as f32 / AU as f32)
            )),
        ]);

        frame.render_widget(
            widget,
            spaceship_split[1].inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
        );
    } else {
        let area = spaceship_split[1].inner(Margin {
            horizontal: 0,
            vertical: 1,
        });

        let split = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

        let rating_span =  HoverTextSpan::new(
            Span::raw(format!(
                "Rating {}  ",
                team_rating.stars()
            )),
            format!("The rating is an indicator of the overall basketball proficiency of the crew. (current value {})", team_rating.value()),
        );
        frame.render_interactive_widget(rating_span, split[1]);

        let reputation_span =
        HoverTextSpan::new(
            Span::raw(format!(
                "Reputation {}  ",
                team.reputation.stars()
            )),
            format!("Reputation indicates how much the team is respected in the galaxy. It affects hiring costs. (current value {})", team.reputation.value()),
        );
        frame.render_interactive_widget(reputation_span, split[2]);

        let game_record = format!(
            "Local Elo {:.0}{}",
            team.local_game_rating.rating,
            if team.peer_id.is_some() || team.id == world.own_team_id {
                format!("  Network Elo {:.0}", team.network_game_rating.rating)
            } else {
                "".to_string()
            }
        );

        let mut lines = vec![
            HoverTextLine::from(format!("Treasury {}", format_satoshi(team.balance()))),
            HoverTextLine::from(game_record),
            HoverTextLine::from(get_crew_spans(
                team.player_ids.len(),
                team.spaceship.crew_capacity() as usize,
            )),
        ];

        if with_average_energy {
            let average_tiredness = team.average_tiredness(world);
            lines.push(HoverTextLine::from(get_energy_spans(average_tiredness)));
        }

        if team.creation_time != Tick::default() {
            let creation_date = team.creation_time.formatted_as_date();
            lines.push(HoverTextLine::from(format!(
                "Roaming the galaxy since {creation_date}"
            )));
        } else {
            lines.push(HoverTextLine::default());
        }

        if !team.honours.is_empty() {
            lines.append(&mut honour_lines(&team.honours));
        }

        let lines_split = Layout::vertical([1].repeat(lines.len())).split(split[3]);
        for (line, &split) in zip(lines, lines_split.iter()) {
            frame.render_interactive_widget(line, split);
        }
    }

    // Render main block
    let block = default_block().title(format!("Spaceship - {}", team.spaceship.name));
    frame.render_widget(block, area);
}

fn honour_lines<'a>(team_honours: &HashSet<Honour>) -> Vec<HoverTextLine<'a>> {
    let honour_color = |honour| match honour {
        Honour::Defiant => (Color::Blue, Color::Yellow),
        Honour::Galactic => (Color::White, Color::Blue),
        Honour::Maximalist => (Color::Yellow, Color::DarkGray),
        Honour::MultiKulti => (Color::Red, Color::LightCyan),
        Honour::Pirate => (Color::Black, Color::Red),
        Honour::Traveller => (Color::Blue, Color::LightMagenta),
        Honour::Veteran => (Color::DarkGray, Color::White),
    };

    let mut top_spans = vec![];
    let mut spans = vec![];
    let mut btm_spans = vec![];

    let title = "Honours ";
    top_spans.push(HoverTextSpan::new(Span::raw(" ".repeat(title.len())), ""));
    spans.push(HoverTextSpan::new(Span::raw(title), ""));
    btm_spans.push(HoverTextSpan::new(Span::raw(" ".repeat(title.len())), ""));

    for &honour in team_honours {
        let (fg, bg) = honour_color(honour);
        top_spans.push(HoverTextSpan::new(
            Span::styled("▄ ▄", Style::default().fg(bg)),
            honour.description().to_string(),
        ));
        spans.push(HoverTextSpan::new(
            Span::styled(
                format!(" {} ", honour.symbol()),
                Style::default().bg(bg).fg(fg).bold(),
            ),
            honour.description().to_string(),
        ));
        btm_spans.push(HoverTextSpan::new(
            Span::styled(" ▀ ", Style::default().fg(bg)),
            honour.description().to_string(),
        ));
    }
    vec![
        HoverTextLine::from(top_spans),
        HoverTextLine::from(spans),
        HoverTextLine::from(btm_spans),
    ]
}

pub fn render_spaceship_upgrade(
    team: &Team,
    upgrade_target: SpaceshipUpgradeTarget,
    in_shipyard: bool,
    gif_map: &mut GifMap,
    tick: usize,
    frame: &mut UiFrame,
    area: Rect,
) {
    let spaceship_split = Layout::horizontal([
        Constraint::Length(SPACESHIP_IMAGE_WIDTH as u16 + 2),
        Constraint::Min(1),
    ])
    .split(area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    }));
    let mut upgraded_ship = team.spaceship.clone();

    match upgrade_target {
        SpaceshipUpgradeTarget::ChargeUnit { component } => upgraded_ship.charge_unit = component,
        SpaceshipUpgradeTarget::Hull { component } => upgraded_ship.hull = component,
        SpaceshipUpgradeTarget::Engine { component } => upgraded_ship.engine = component,
        SpaceshipUpgradeTarget::Shield { component } => upgraded_ship.shield = component,
        SpaceshipUpgradeTarget::Storage { component } => upgraded_ship.storage = component,
        SpaceshipUpgradeTarget::Shooter { component } => upgraded_ship.shooter = component,
        SpaceshipUpgradeTarget::Repairs { .. } => upgraded_ship.reset_durability(),
    }

    if in_shipyard {
        if let Ok(lines) = gif_map.in_shipyard_spaceship_lines(&upgraded_ship, tick) {
            let paragraph = Paragraph::new(lines);
            frame.render_widget(
                paragraph.centered(),
                spaceship_split[0].inner(Margin::new(1, 1)),
            );
        }
    } else if matches!(upgrade_target, SpaceshipUpgradeTarget::Shooter { .. })
        && upgraded_ship.fire_rate() > 0.0
    {
        if let Ok(lines) = gif_map.shooting_spaceship_lines(&upgraded_ship, tick) {
            let paragraph = Paragraph::new(lines);
            frame.render_widget(
                paragraph.centered(),
                spaceship_split[0].inner(Margin::new(1, 1)),
            );
        }
    } else if matches!(upgrade_target, SpaceshipUpgradeTarget::Shield { .. }) {
        if let Ok(lines) = gif_map.with_shield_spaceship_lines(&upgraded_ship, tick) {
            let paragraph = Paragraph::new(lines);
            frame.render_widget(
                paragraph.centered(),
                spaceship_split[0].inner(Margin::new(1, 1)),
            );
        }
    } else if let Ok(lines) = gif_map.on_planet_spaceship_lines(&upgraded_ship, tick) {
        let paragraph = Paragraph::new(lines);
        frame.render_widget(
            paragraph.centered(),
            spaceship_split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );
    }

    let storage_units = 0;
    let spaceship_info = Paragraph::new(vec![
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:.3}",
                "Max speed",
                team.spaceship.speed(storage_units) * HOURS as f32 / AU as f32
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{:.3}",
                    upgraded_ship.speed(storage_units) * HOURS as f32 / AU as f32
                ),
                if upgraded_ship.speed(storage_units) > team.spaceship.speed(storage_units) {
                    UiStyle::OK
                } else if upgraded_ship.speed(storage_units) < team.spaceship.speed(storage_units) {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
            Span::raw(" AU/h"),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:<5}",
                "Max crew",
                team.spaceship.crew_capacity()
            )),
            Span::raw(" --> "),
            Span::styled(
                format!("{}", upgraded_ship.crew_capacity()),
                if upgraded_ship.crew_capacity() > team.spaceship.crew_capacity() {
                    UiStyle::OK
                } else if upgraded_ship.crew_capacity() < team.spaceship.crew_capacity() {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:<5}",
                "Tank",
                team.spaceship.fuel_capacity()
            )),
            Span::raw(" --> "),
            Span::styled(
                format!("{}", upgraded_ship.fuel_capacity()),
                if upgraded_ship.fuel_capacity() > team.spaceship.fuel_capacity() {
                    UiStyle::OK
                } else if upgraded_ship.fuel_capacity() < team.spaceship.fuel_capacity() {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
            Span::raw(" t"),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:<5}",
                "Max charge",
                team.spaceship.max_charge()
            )),
            Span::raw(" --> "),
            Span::styled(
                format!("{}", upgraded_ship.max_charge()),
                if upgraded_ship.max_charge() > team.spaceship.max_charge() {
                    UiStyle::OK
                } else if upgraded_ship.max_charge() < team.spaceship.max_charge() {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
            Span::raw(" MW"),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:<5}",
                "Stiva",
                team.spaceship.storage_capacity(),
            )),
            Span::raw(" --> "),
            Span::styled(
                format!("{}", upgraded_ship.storage_capacity()),
                if upgraded_ship.storage_capacity() > team.spaceship.storage_capacity() {
                    UiStyle::OK
                } else if upgraded_ship.storage_capacity() < team.spaceship.storage_capacity() {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:>02}/{:<02}",
                "Durability",
                team.spaceship.current_durability(),
                team.spaceship.max_durability(),
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{:>2}/{:<2}",
                    upgraded_ship.max_durability(),
                    upgraded_ship.max_durability()
                ),
                if upgraded_ship.max_durability() > team.spaceship.max_durability() {
                    UiStyle::OK
                } else if upgraded_ship.max_durability() < team.spaceship.max_durability() {
                    UiStyle::ERROR
                } else if upgraded_ship.current_durability() > team.spaceship.current_durability() {
                    UiStyle::OK
                } else if upgraded_ship.current_durability() < team.spaceship.current_durability() {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:5}",
                "Shield",
                format!(
                    "{}/{:.0}%",
                    team.spaceship.shield_max_durability(),
                    (1.0 - team.spaceship.shield_damage_reduction()) * 100.0
                )
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{}/{:.0}%",
                    upgraded_ship.shield_max_durability(),
                    (1.0 - upgraded_ship.shield_damage_reduction()) * 100.0
                ),
                if upgraded_ship.shield_max_durability() > team.spaceship.shield_max_durability() {
                    UiStyle::OK
                } else if upgraded_ship.shield_max_durability()
                    < team.spaceship.shield_max_durability()
                {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:5}",
                "Shooters",
                format!(
                    "{}x{}",
                    (team.spaceship.damage() * team.spaceship.fire_rate()) as u8,
                    team.spaceship.shooting_points()
                )
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{}x{}",
                    (upgraded_ship.damage() * upgraded_ship.fire_rate()) as u8,
                    upgraded_ship.shooting_points()
                ),
                if (upgraded_ship.damage()
                    * upgraded_ship.fire_rate()
                    * upgraded_ship.shooting_points() as f32)
                    > (team.spaceship.damage()
                        * team.spaceship.fire_rate()
                        * team.spaceship.shooting_points() as f32)
                {
                    UiStyle::OK
                } else if (upgraded_ship.damage()
                    * upgraded_ship.fire_rate()
                    * upgraded_ship.shooting_points() as f32)
                    < (team.spaceship.damage()
                        * team.spaceship.fire_rate()
                        * team.spaceship.shooting_points() as f32)
                {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:.3}",
                "Consumption",
                team.spaceship.fuel_consumption_per_tick(storage_units) * HOURS as f32
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{:.3}",
                    upgraded_ship.fuel_consumption_per_tick(storage_units) * HOURS as f32
                ),
                if upgraded_ship.fuel_consumption_per_tick(storage_units)
                    < team.spaceship.fuel_consumption_per_tick(storage_units)
                {
                    UiStyle::OK
                } else if upgraded_ship.fuel_consumption_per_tick(storage_units)
                    > team.spaceship.fuel_consumption_per_tick(storage_units)
                {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
            Span::raw(" t/h"),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<12} {:.3}",
                "Max distance",
                team.spaceship.max_distance(team.spaceship.fuel_capacity()) / AU as f32
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{:.3}",
                    upgraded_ship.max_distance(upgraded_ship.fuel_capacity()) / AU as f32
                ),
                if upgraded_ship.max_distance(upgraded_ship.fuel_capacity())
                    > team.spaceship.max_distance(team.spaceship.fuel_capacity())
                {
                    UiStyle::OK
                } else if upgraded_ship.max_distance(upgraded_ship.fuel_capacity())
                    < team.spaceship.max_distance(team.spaceship.fuel_capacity())
                {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
            Span::raw(" AU"),
        ]),
    ]);

    frame.render_widget(
        spaceship_info,
        spaceship_split[1].inner(Margin {
            horizontal: 0,
            vertical: 2,
        }),
    );
}

pub fn render_player_description(
    player: &Player,
    view: PlayerWidgetView,
    gif_map: &mut GifMap,
    tick: usize,
    world: &World,
    frame: &mut UiFrame,
    area: Rect,
) {
    let h_split = Layout::horizontal([
        Constraint::Length(PLAYER_IMAGE_WIDTH as u16 + 4),
        Constraint::Min(2),
    ])
    .split(area);

    let header_body_img = Layout::vertical([Constraint::Length(2), Constraint::Min(2)]).split(
        h_split[0].inner(Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );

    let header_body_stats = Layout::vertical([
        Constraint::Length(2),  //margin
        Constraint::Length(1),  //header
        Constraint::Length(1),  //header
        Constraint::Length(1),  //header
        Constraint::Length(1),  //header
        Constraint::Length(1),  //margin
        Constraint::Length(20), //skills/stats
    ])
    .split(h_split[1]);

    if let Ok(lines) = gif_map.player_frame_lines(player, tick) {
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, header_body_img[1]);
    }

    let trait_span = if let Some(t) = player.special_trait {
        Span::styled(format!("{t}"), t.style())
    } else {
        Span::raw("")
    };

    let line = HoverTextLine::from(vec![
        HoverTextSpan::new(
            Span::raw(format!(
                "Reputation {}  ",
                player.reputation.stars()
            )),
            format!("Reputation indicates how much the player is respected in the galaxy. It affects special bonuses. (current value {})", player.reputation.value()),
        ),
        HoverTextSpan::new(
            trait_span,
            if let Some(t) = player.special_trait {
                t.description(player)
            } else {
                    "".to_string()
            },
        )
    ]);
    frame.render_interactive_widget(line, header_body_stats[1]);

    let morale = player.current_morale(world);
    let morale_length = (morale / MAX_SKILL * BARS_LENGTH as f32).round() as usize;
    let morale_string = format!(
        "{}{}",
        "▰".repeat(morale_length),
        "▱".repeat(BARS_LENGTH.saturating_sub(morale_length)),
    );
    let morale_style = (morale / MAX_SKILL * GREEN_STYLE_SKILL).style();

    frame.render_interactive_widget(
        HoverTextLine::from(vec![
            HoverTextSpan::new(
                Span::raw("Morale ".to_string()),
                format!(
                    "When morale is low, pirates may decide to leave the team! (current value {morale:.2})"
                ),
            ),
            HoverTextSpan::new(
                Span::styled(morale_string, morale_style),
                ""
            ),
        ]),
        header_body_stats[2],
    );

    let tiredness = player.current_tiredness(world);
    let tiredness_length = (tiredness / MAX_SKILL * BARS_LENGTH as f32).round() as usize;
    let energy_string = format!(
        "{}{}",
        "▰".repeat(BARS_LENGTH.saturating_sub(tiredness_length)),
        "▱".repeat(tiredness_length),
    );
    let energy_style = ((MAX_SKILL - tiredness) / MAX_SKILL * GREEN_STYLE_SKILL).style();

    frame.render_interactive_widget(
        HoverTextLine::from(vec![
            HoverTextSpan::new(
                Span::raw("Energy ".to_string()),
                format!("Energy affects player's performance in a game. When the energy goes to 0, the player is exhausted and will fail most game actions. (current value {:.2})", (MAX_SKILL-tiredness)),
            ),
            HoverTextSpan::new(Span::styled( energy_string, energy_style),"", 
           ),
        ]),
        header_body_stats[3],
    );

    frame.render_widget(
        Paragraph::new(format!(
            "{} yo, {} cm, {} kg, {}",
            player.info.age as u8,
            player.info.height as u8,
            player.info.weight as u8,
            player.info.population,
        )),
        header_body_stats[4],
    );

    match view {
        PlayerWidgetView::Skills => frame.render_widget(
            Paragraph::new(format_player_skills(player)),
            header_body_stats[6],
        ),
        PlayerWidgetView::Stats => frame.render_widget(
            Paragraph::new(format_player_stats(player)),
            header_body_stats[6],
        ),
    }

    // Render main block
    let block = default_block().title(format!(
        "{} {} {}",
        player.info.first_name,
        player.info.last_name,
        player.stars()
    ));
    frame.render_widget(block, area);
}

pub fn upgrade_resources_lines<U: UpgradeableElement>(upgrade: U, team: &Team) -> Vec<Line<'_>> {
    let mut lines = vec![];
    for &(resource, amount) in upgrade.upgrade_cost().iter() {
        if amount == 0 {
            continue;
        }
        let have = team.resources.value(&resource);
        if amount > have {
            lines.push(Line::from(vec![
                Span::styled(format!("{:<7} ", resource.to_string()), resource.style()),
                Span::styled(format!("{have:>5}/{amount:<5} ❌"), UiStyle::ERROR),
            ]))
        } else {
            lines.push(Line::from(vec![
                Span::styled(format!("{:<7} ", resource.to_string()), resource.style()),
                Span::styled(format!("{amount:^11} ✅"), UiStyle::OK),
            ]))
        }
    }

    lines
}

fn improvement_indicator<'a>(skill: f32, previous: f32) -> Span<'a> {
    // We only update at the end of the day, so we can display if something went recently up or not.
    if skill.value() > previous.value() {
        UP_ARROW_SPAN.clone()
    } else if skill > previous + 0.33 {
        UP_RIGHT_ARROW_SPAN.clone()
    } else if skill.value() < previous.value() {
        DOWN_ARROW_SPAN.clone()
    } else if skill < previous - 0.33 {
        DOWN_RIGHT_ARROW_SPAN.clone()
    } else {
        Span::styled(" ", UiStyle::DEFAULT)
    }
}

fn format_player_skills(player: &'_ Player) -> Vec<Line<'_>> {
    let skills = player.current_skill_array();
    let mut text = vec![];
    let mut roles = (0..MAX_GAME_POSITION)
        .map(|i: GamePosition| {
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
    spans.push(Span::styled(
        format!("Athletics {:<5}", player.athletics.stars()),
        player.athletics.rating().style(),
    ));
    text.push(Line::from(spans));

    for i in 0..4 {
        let mut spans = vec![];
        spans.push(Span::styled(
            format!("{:<2} {:<5}       ", roles[i + 1].0, roles[i + 1].1.stars()),
            roles[i + 1].1.style(),
        ));

        spans.push(Span::styled(
            format!(
                "   {:<MAX_NAME_LENGTH$}{:02} ",
                SKILL_NAMES[i],
                skills[i].value(),
            ),
            skills[i].style(),
        ));
        spans.push(improvement_indicator(skills[i], player.previous_skills[i]));

        text.push(Line::from(spans));
    }
    text.push(Line::default());

    text.push(Line::from(vec![
        Span::styled(
            format!("{} {:<5}     ", "Offense", player.offense.stars()),
            player.offense.rating().style(),
        ),
        Span::styled(
            format!("{} {}", "Defense", player.defense.stars()),
            player.defense.rating().style(),
        ),
    ]));
    for i in 0..4 {
        let mut spans = vec![];
        spans.push(Span::styled(
            format!("{:<10}{:02} ", SKILL_NAMES[i + 4], skills[i + 4].value(),),
            skills[i + 4].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 4],
            player.previous_skills[i + 4],
        ));

        spans.push(Span::styled(
            format!(
                "    {:<MAX_NAME_LENGTH$}{:02} ",
                SKILL_NAMES[i + 8],
                skills[i + 8].value(),
            ),
            skills[i + 8].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 8],
            player.previous_skills[i + 8],
        ));

        text.push(Line::from(spans));
    }
    text.push(Line::default());
    text.push(Line::from(vec![
        Span::styled(
            format!("{} {:<5}   ", "Technical", player.technical.stars()),
            player.technical.rating().style(),
        ),
        Span::styled(
            format!("{} {}", "Mental", player.mental.stars()),
            player.mental.rating().style(),
        ),
    ]));

    for i in 0..4 {
        let mut spans = vec![];
        spans.push(Span::styled(
            format!("{:<10}{:02} ", SKILL_NAMES[i + 12], skills[i + 12].value(),),
            skills[i + 12].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 12],
            player.previous_skills[i + 12],
        ));

        spans.push(Span::styled(
            format!(
                "    {:<MAX_NAME_LENGTH$}{:02} ",
                SKILL_NAMES[i + 16],
                skills[i + 16].value(),
            ),
            skills[i + 16].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 16],
            player.previous_skills[i + 16],
        ));

        text.push(Line::from(spans));
    }

    text
}

fn format_player_stats(player: &'_ Player) -> Vec<Line<'_>> {
    let stats = &player.historical_stats;
    let mut text = vec![];

    let games_played = stats.games.iter().sum::<u16>().max(1) as f32;

    text.push(Line::from(format!(
        "{:<12} W{}/L{}/D{}",
        "Games", stats.games[0], stats.games[1], stats.games[2]
    )));

    text.push(Line::from(format!(
        "{:<12} W{}/L{}/D{}",
        "Brawls", stats.brawls[0], stats.brawls[1], stats.brawls[2]
    )));

    text.push(Line::default());
    text.push(Line::from(Span::styled(
        format!("{:<12} {:^9} {:>9}", "Stat", "Total", "Per game"),
        UiStyle::HEADER.bold(),
    )));

    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9}",
        "Play time",
        (stats.seconds_played as Tick * SECONDS).formatted(),
        ((stats.seconds_played as f32 * SECONDS as f32 / games_played) as Tick).formatted()
    )));

    text.push(Line::from(format!(
        "{:<12} {:>+9} {:>+9.1}",
        "Plus/Minus",
        stats.plus_minus,
        stats.plus_minus as f32 / games_played
    )));

    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9}",
        "2 points",
        format!("{}/{}", stats.made_2pt, stats.attempted_2pt),
        format!(
            "{:3.1}/{:3.1}",
            stats.made_2pt as f32 / games_played,
            stats.attempted_2pt as f32 / games_played
        ),
    )));
    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9}",
        "3 points",
        format!("{}/{}", stats.made_3pt, stats.attempted_3pt),
        format!(
            "{:3.1}/{:3.1}",
            stats.made_3pt as f32 / games_played,
            stats.attempted_3pt as f32 / games_played
        ),
    )));
    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9.1}",
        "Points",
        stats.points,
        stats.points as f32 / games_played
    )));

    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9.1}",
        "Def Rebounds",
        stats.defensive_rebounds,
        stats.defensive_rebounds as f32 / games_played
    )));
    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9.1}",
        "Off Rebounds",
        stats.offensive_rebounds,
        stats.offensive_rebounds as f32 / games_played
    )));
    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9.1}",
        "Assists",
        stats.assists,
        stats.assists as f32 / games_played
    )));
    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9.1}",
        "Steals",
        stats.steals,
        stats.steals as f32 / games_played
    )));
    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9.1}",
        "Blocks",
        stats.blocks,
        stats.blocks as f32 / games_played
    )));
    text.push(Line::from(format!(
        "{:<12} {:>9} {:>9.1}",
        "Turnovers",
        stats.turnovers,
        stats.turnovers as f32 / games_played
    )));

    text
}

pub fn spaceship_upgrade_target_description_lines<'a>(
    upgrade_target: SpaceshipUpgradeTarget,
) -> Vec<Line<'a>> {
    match upgrade_target {
        SpaceshipUpgradeTarget::ChargeUnit { component } => {
            let mut lines = spaceship_component_description_lines(component);
            lines.append(&mut charge_unit_extra_description_lines(component));
            lines
        }
        SpaceshipUpgradeTarget::Hull { component } => {
            spaceship_component_description_lines(component)
        }
        SpaceshipUpgradeTarget::Engine { component } => {
            spaceship_component_description_lines(component)
        }
        SpaceshipUpgradeTarget::Shield { component } => {
            let mut lines = spaceship_component_description_lines(component);
            lines.append(&mut shield_extra_description_lines(component));
            lines
        }
        SpaceshipUpgradeTarget::Shooter { component } => {
            let mut lines = spaceship_component_description_lines(component);
            lines.append(&mut shooter_extra_description_lines(component));
            lines
        }
        SpaceshipUpgradeTarget::Storage { component } => {
            spaceship_component_description_lines(component)
        }
        SpaceshipUpgradeTarget::Repairs { .. } => vec![],
    }
}

fn charge_unit_extra_description_lines<'a>(component: ChargeUnit) -> Vec<Line<'a>> {
    let previous_component = component.previous();
    let mut lines = Vec::new();

    let max_charge = component.max_charge()
        - previous_component
            .map(|c| c.max_charge())
            .unwrap_or_default();
    if max_charge != 0.0 {
        lines.push(Line::from(vec![
            Span::raw("Max charge "),
            Span::styled(
                format!("{max_charge:<+.0}"),
                if max_charge > 0.0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    lines
}

fn shield_extra_description_lines<'a>(component: Shield) -> Vec<Line<'a>> {
    let previous_component = component.previous();
    let mut lines = Vec::new();

    let absorbtion = previous_component
        .map(|c| c.damage_reduction())
        .unwrap_or_default()
        - component.damage_reduction();
    if absorbtion != 0.0 {
        lines.push(Line::from(vec![
            Span::raw("Damage absorbtion "),
            Span::styled(
                format!("{:<+.0}%", (absorbtion * 100.0).round()),
                if absorbtion > 0.0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    let shield_durability = component.max_durability()
        - previous_component
            .map(|c| c.max_durability())
            .unwrap_or_default();
    if shield_durability != 0.0 {
        lines.push(Line::from(vec![
            Span::raw("Shield durability "),
            Span::styled(
                format!("{shield_durability:<+.0}"),
                if shield_durability > 0.0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    lines
}

fn shooter_extra_description_lines<'a>(component: Shooter) -> Vec<Line<'a>> {
    let previous_component = component.previous();
    let mut lines = Vec::new();

    let damage = component.damage() - previous_component.map(|c| c.damage()).unwrap_or_default();
    if damage != 0.0 {
        lines.push(Line::from(vec![
            Span::raw("Damage "),
            Span::styled(
                format!("{damage:+.0}"),
                if damage > 0.0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    let fire_rate = component.fire_rate()
        - previous_component
            .map(|c| c.fire_rate())
            .unwrap_or_default();
    if fire_rate != 0.0 {
        lines.push(Line::from(vec![
            Span::raw("Fire rate "),
            Span::styled(
                format!("{fire_rate:<+.0}"),
                if fire_rate > 0.0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    let shooting_points = component.shooting_points() as i8
        - previous_component
            .map(|c| c.shooting_points() as i8)
            .unwrap_or_default();
    if shooting_points != 0 {
        lines.push(Line::from(vec![
            Span::raw("Bullets "),
            Span::styled(
                format!("{shooting_points:+}"),
                if shooting_points > 0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    lines
}

fn spaceship_component_description_lines<'a, C: SpaceshipComponent>(component: C) -> Vec<Line<'a>> {
    let previous_component = component.previous();
    let mut lines = Vec::new();

    let crew = component.crew_capacity() as i8
        - previous_component
            .map(|c| c.crew_capacity() as i8)
            .unwrap_or_default();
    if crew != 0 {
        lines.push(Line::from(vec![
            Span::raw("Crew "),
            Span::styled(
                format!("{crew:+}"),
                if crew > 0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    let storage = component.storage_capacity() as i32
        - previous_component
            .map(|c| c.storage_capacity() as i32)
            .unwrap_or_default();
    if storage != 0 {
        lines.push(Line::from(vec![
            Span::raw("Stiva "),
            Span::styled(
                format!("{storage:+}"),
                if storage > 0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    let tank = component.fuel_capacity() as i32
        - previous_component
            .map(|c| c.fuel_capacity() as i32)
            .unwrap_or_default();
    if tank != 0 {
        lines.push(Line::from(vec![
            Span::raw("Tank "),
            Span::styled(
                format!("{tank:+}"),
                if tank > 0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    let consumption = component.fuel_consumption_per_tick()
        - previous_component
            .map(|c| c.fuel_consumption_per_tick())
            .unwrap_or_default();
    if consumption != 0.0 {
        lines.push(Line::from(vec![
            Span::raw("Consumption "),
            Span::styled(
                format!("{:<+.0}%", (consumption * 100.0).round()),
                if consumption < 0.0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    let speed = component.speed() - previous_component.map(|c| c.speed()).unwrap_or_default();
    if speed != 0.0 {
        lines.push(Line::from(vec![
            Span::raw("Speed "),
            Span::styled(
                format!("{:<+.0}%", (speed * 100.0).round()),
                if speed > 0.0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    let durability = component.durability() as i32
        - previous_component
            .map(|c| c.durability() as i32)
            .unwrap_or_default();
    if durability != 0 {
        lines.push(Line::from(vec![
            Span::raw("Durability "),
            Span::styled(
                format!("{durability:+}"),
                if durability > 0 {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                },
            ),
        ]));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::{AppResult, BARS_LENGTH};
    use crate::{
        core::{resources::Resource, spaceship::SpaceshipPrefab, team::Team},
        ui::widgets::get_storage_lengths,
    };

    #[test]
    fn test_storage_spans() -> AppResult<()> {
        let mut team = Team::random(None);
        team.spaceship = SpaceshipPrefab::Bresci.spaceship();

        let bars_length = BARS_LENGTH;
        if let [gold_length, scraps_length, rum_length, free_bars] = get_storage_lengths(
            &team.resources,
            team.spaceship.storage_capacity(),
            bars_length,
        )[..4]
        {
            println!("{:?}", team.resources);
            println!(
                "gold={} scraps={} rum={} free={} storage={}/{}",
                gold_length,
                scraps_length,
                rum_length,
                free_bars,
                team.used_storage_capacity(),
                team.storage_capacity()
            );
            assert_eq!(gold_length, 0);
            assert_eq!(scraps_length, 0);
            assert_eq!(rum_length, 0);
            assert_eq!(free_bars, bars_length);
            assert_eq!(
                gold_length + scraps_length + rum_length + free_bars,
                bars_length
            );
        } else {
            panic!("Failed to calculate resource length");
        }

        team.add_resource(Resource::SCRAPS, 178)?;
        team.add_resource(Resource::RUM, 11)?;

        if let [gold_length, scraps_length, rum_length, free_bars] = get_storage_lengths(
            &team.resources,
            team.spaceship.storage_capacity(),
            bars_length,
        )[..4]
        {
            println!("{:?}", team.resources);
            println!(
                "gold={} scraps={} rum={} free={} storage={}/{}",
                gold_length,
                scraps_length,
                rum_length,
                free_bars,
                team.used_storage_capacity(),
                team.storage_capacity()
            );
            assert_eq!(gold_length, 0);
            assert_eq!(scraps_length, 21);
            assert_eq!(rum_length, 1);
            assert_eq!(free_bars, 3);
            assert_eq!(
                gold_length + scraps_length + rum_length + free_bars,
                bars_length
            );
        } else {
            panic!("Failed to calculate resource length");
        }
        team.add_resource(Resource::SCRAPS, 24)?;
        team.add_resource(Resource::GOLD, 1)?;

        if let [gold_length, scraps_length, rum_length, free_bars] = get_storage_lengths(
            &team.resources,
            team.spaceship.storage_capacity(),
            bars_length,
        )[..4]
        {
            println!("{:?}", team.resources);
            println!(
                "gold={} scraps={} rum={} free={} storage={}/{}",
                gold_length,
                scraps_length,
                rum_length,
                free_bars,
                team.used_storage_capacity(),
                team.storage_capacity()
            );
            assert_eq!(gold_length, 1);
            assert_eq!(scraps_length, 23);
            assert_eq!(rum_length, 1);
            assert_eq!(free_bars, 0);
            assert_eq!(
                gold_length + scraps_length + rum_length + free_bars,
                bars_length
            );
        } else {
            panic!("Failed to calculate resource length");
        }

        Ok(())
    }
}
