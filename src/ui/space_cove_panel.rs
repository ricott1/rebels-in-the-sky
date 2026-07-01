use super::ui_frame::UiFrame;
use super::{traits::Screen, ui_callback::UiCallback};
use crate::game_engine::{TournamentId, TournamentType};
use crate::image::player::PLAYER_IMAGE_WIDTH;
use crate::image::utils::ExtraImageUtils;
use crate::image::utils::{open_image, LightMaskStyle};
use crate::types::{
    HashMapWithResult, PlanetId, PlayerId, StorableResourceMap, SystemTimeTick, TeamId,
};
use crate::ui::button::Button;
use crate::ui::checkbox::Checkbox;
use crate::ui::clickable_list::ClickableListState;
use crate::ui::traits::SplitPanel;
use crate::ui::ui_screen::{render_help_block, tab_link, UiTab};
use crate::ui::utils::{img_to_lines, normalize_index, IndexBound};
use crate::ui::widgets::{
    default_block, go_to_planet_button, render_available_upgrades, selectable_list, teleport_button,
};
use crate::ui::{constants::*, ui_key};
use crate::{core::*, types::AppResult};
use core::fmt::Debug;
use image::RgbaImage;
use itertools::Itertools;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::prelude::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::collections::HashSet;
use std::fmt::{self, Display};

const BUILDINGS: [SpaceCoveUpgradeTarget; 4] = [
    SpaceCoveUpgradeTarget::TeleportationPad,
    SpaceCoveUpgradeTarget::Tavern,
    SpaceCoveUpgradeTarget::Stadium,
    SpaceCoveUpgradeTarget::Market,
];

#[derive(Debug, Default, PartialEq)]
enum PanelList {
    #[default]
    Top,
    Bottom,
}

enum ActiveSelection {
    Building,
    Cove,
    VisitingTeam,
    Tournament,
    TavernPirate,
    None,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SpaceCoveView {
    #[default]
    OwnCove,
    AllCoves,
}

impl SpaceCoveView {
    pub const fn next(&self) -> Self {
        match self {
            Self::OwnCove => Self::AllCoves,
            Self::AllCoves => Self::OwnCove,
        }
    }

    pub const fn previous(&self) -> Self {
        match self {
            Self::OwnCove => Self::AllCoves,
            Self::AllCoves => Self::OwnCove,
        }
    }
}

impl Display for SpaceCoveView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnCove => write!(f, "Space cove"),
            Self::AllCoves => write!(f, "All coves"),
        }
    }
}

// Top-left of each 5x8 lamp slot in tavern.png.
const TAVERN_LAMP_POSITIONS: [(u32, u32); 3] = [(40, 23), (88, 24), (108, 28)];

#[derive(Debug, Default)]
pub struct SpaceCovePanel {
    tick: usize,
    view: SpaceCoveView,
    cove_index: Option<usize>,
    cove_entries: Vec<(TeamId, PlanetId)>,
    cached_teams_len: usize,
    visiting_team_ids: Vec<TeamId>,
    cove_image_widgets: [Paragraph<'static>; 4], // no blinking, left, right, both
    cove_list_state: ClickableListState,
    building_index: Option<usize>,
    tournament_index: Option<usize>,
    tournament_ids: Vec<TournamentId>,
    visiting_team_index: Option<usize>,
    tavern_pirate_index: Option<usize>,
    tavern_widget: Paragraph<'static>,
    tavern_lamps_on: bool,
    tavern_pirate_ids: Vec<PlayerId>,
    market_widget: Paragraph<'static>,
    stadium_widget: Paragraph<'static>,
    active_list: PanelList,
}

impl SpaceCovePanel {
    pub fn new() -> Self {
        let widgets = Self::build_image_widgets(&[]).expect("Should be able to create cove image");
        let tavern_widget = {
            let img =
                Self::get_tavern_image(false, &[]).expect("Should be able to create tavern image");
            Paragraph::new(img_to_lines(&img))
        };
        let market_widget = {
            let mut base =
                open_image("cove/market.png").expect("Should be able to create market image");
            let outer = open_image("cove/base_outer.png")
                .expect("Should be able to create base outer image");
            base.copy_non_trasparent_from(&outer, 0, 0)
                .expect("Should be able to copy image");
            Paragraph::new(img_to_lines(&base))
        };
        let stadium_widget = {
            let mut base =
                open_image("cove/stadium.png").expect("Should be able to create stadium image");
            let outer = open_image("cove/base_outer.png")
                .expect("Should be able to create base outer image");
            base.copy_non_trasparent_from(&outer, 0, 0)
                .expect("Should be able to copy image");
            Paragraph::new(img_to_lines(&base))
        };
        Self {
            cove_image_widgets: widgets,
            tavern_widget,
            market_widget,
            stadium_widget,
            ..Default::default()
        }
    }

    pub fn set_view(&mut self, view: SpaceCoveView) {
        self.view = view;
    }

    fn active_selection(&self) -> ActiveSelection {
        match self.view {
            SpaceCoveView::OwnCove => {
                if self.active_list == PanelList::Top {
                    return ActiveSelection::Building;
                }
                match self.building_index.and_then(|i| BUILDINGS.get(i)) {
                    Some(SpaceCoveUpgradeTarget::TeleportationPad) => ActiveSelection::VisitingTeam,
                    Some(SpaceCoveUpgradeTarget::Stadium) => ActiveSelection::Tournament,
                    Some(SpaceCoveUpgradeTarget::Tavern) => ActiveSelection::TavernPirate,
                    _ => ActiveSelection::None,
                }
            }
            SpaceCoveView::AllCoves => {
                if self.active_list == PanelList::Top {
                    ActiveSelection::Cove
                } else {
                    ActiveSelection::VisitingTeam
                }
            }
        }
    }

    fn get_cove_images(
        teams: &[&Team],
        is_blinking_left: bool,
        is_blinking_right: bool,
    ) -> AppResult<RgbaImage> {
        let mut base = open_image("cove/base.png").expect("Cove image base.png should exist");

        const SKULL_POSITION: (u32, u32) = (98, 1);
        const LEFT_EYE_POSITION: (u32, u32) = (SKULL_POSITION.0 + 4, SKULL_POSITION.1 + 11);
        const RIGHT_EYE_POSITION: (u32, u32) = (SKULL_POSITION.0 + 13, SKULL_POSITION.1 + 11);

        let skull = open_image("cove/skull.png").expect("Cove image skull.png should exist");
        base.copy_non_trasparent_from(&skull, SKULL_POSITION.0, SKULL_POSITION.1)?;

        if is_blinking_left {
            let left_eye = open_image("cove/left_eye_mask.png")
                .expect("Cove image left_eye_mask.png should exist");
            base.copy_non_trasparent_from(&left_eye, LEFT_EYE_POSITION.0, LEFT_EYE_POSITION.1)?;
        }

        if is_blinking_right {
            let right_eye = open_image("cove/right_eye_mask.png")
                .expect("Cove image right_eye_mask.png should exist");
            base.copy_non_trasparent_from(&right_eye, RIGHT_EYE_POSITION.0, RIGHT_EYE_POSITION.1)?;
        }

        let mut x = 5;
        for team in teams.iter().take(4) {
            let ship_img = &team.spaceship.compose_image_in_shipyard()?[0];
            let y = 40;
            base.copy_non_trasparent_from(ship_img, x, y)?;
            x += ship_img.width();
            if x + ship_img.width() > base.width() {
                break;
            }
        }

        if !is_blinking_left {
            base.apply_light_mask(&LightMaskStyle::skull_eye((
                LEFT_EYE_POSITION.0 + 2,
                LEFT_EYE_POSITION.1 + 2,
            )));
        }

        if !is_blinking_right {
            base.apply_light_mask(&LightMaskStyle::skull_eye((
                RIGHT_EYE_POSITION.0 + 2,
                RIGHT_EYE_POSITION.1 + 2,
            )));
        }

        let outer =
            open_image("cove/base_outer.png").expect("Cove image base_outer.png should exist");
        base.copy_non_trasparent_from(&outer, 0, 0)?;
        Ok(base)
    }

    fn get_cove_image_widgets<'a>(
        teams: &[&Team],
        is_blinking_left: bool,
        is_blinking_right: bool,
    ) -> AppResult<Paragraph<'a>> {
        let img = Self::get_cove_images(teams, is_blinking_left, is_blinking_right)?;
        let cove_image_lines = img_to_lines(&img);
        Ok(Paragraph::new(cove_image_lines))
    }

    fn build_image_widgets(teams: &[&Team]) -> AppResult<[Paragraph<'static>; 4]> {
        Ok([
            Self::get_cove_image_widgets(teams, false, false)?,
            Self::get_cove_image_widgets(teams, true, false)?,
            Self::get_cove_image_widgets(teams, false, true)?,
            Self::get_cove_image_widgets(teams, true, true)?,
        ])
    }

    fn get_tavern_image(lamps_on: bool, pirate_frames: &[RgbaImage]) -> AppResult<RgbaImage> {
        let mut base = open_image("cove/tavern.png")?;
        let lamp = open_image(if lamps_on {
            "cove/lamp_on.png"
        } else {
            "cove/lamp_off.png"
        })?;
        for &(x, y) in TAVERN_LAMP_POSITIONS.iter() {
            base.copy_non_trasparent_from(&lamp, x, y)?;
            if lamps_on {
                base.apply_light_mask(&LightMaskStyle::lamp((x + 2, y + 4)));
            }
        }

        // Blit each free pirate standing in the tavern, clustered and centered.
        const PIRATE_X_STEP: u32 = 20;
        const PIRATE_BASELINES_Y: [u32; MAX_TAVERN_POPULATION as usize] = [66, 70, 67];
        let pirates = &pirate_frames[..pirate_frames.len().min(MAX_TAVERN_POPULATION as usize)];
        if !pirates.is_empty() {
            let group_width = PIRATE_X_STEP * (pirates.len() as u32 - 1) + PLAYER_IMAGE_WIDTH;
            let mut x = base.width().saturating_sub(group_width) / 2 + 4;
            for (idx, frame) in pirates.iter().enumerate() {
                let y = PIRATE_BASELINES_Y[idx] - frame.height();
                base.copy_non_trasparent_from(frame, x, y)?;
                x += PIRATE_X_STEP;
            }
        }

        let outer = open_image("cove/base_outer.png")?;
        base.copy_non_trasparent_from(&outer, 0, 0)?;
        Ok(base)
    }

    fn render_view_buttons(
        &self,
        frame: &mut UiFrame,
        world: &World,
        own_cove_area: Rect,
        other_coves_area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let own_cove_label = match own_team.has_space_cove_on() {
            Some(cove_planet) => {
                let asteroid_name = world
                    .planets
                    .get(&cove_planet)
                    .map(|p| p.name.as_str())
                    .unwrap_or("???");
                format!("Space cove on {}", asteroid_name)
            }
            None => "No own space cove".to_string(),
        };

        let mut own_button = Button::new(
            own_cove_label,
            UiCallback::SetSpaceCovePanelView {
                view: SpaceCoveView::OwnCove,
            },
        )
        .bold()
        .set_hover_text("Manage your own space cove.");
        if own_team.space_cove.is_none() {
            own_button.disable(Some("You don't have a space cove yet".to_string()));
        }

        let mut other_button = Button::new(
            SpaceCoveView::AllCoves.to_string(),
            UiCallback::SetSpaceCovePanelView {
                view: SpaceCoveView::AllCoves,
            },
        )
        .bold()
        .set_hover_text("Browse coves owned by other crews.");

        match self.view {
            SpaceCoveView::OwnCove => own_button.select(),
            SpaceCoveView::AllCoves => other_button.select(),
        }

        frame.render_interactive_widget(own_button, own_cove_area);
        frame.render_interactive_widget(other_button, other_coves_area);
        Ok(())
    }

    fn render_cove_list(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if self.cove_entries.is_empty() {
            frame.render_widget(default_block().title("No known coves"), area);
            return Ok(());
        }

        let mut options = vec![];
        for (team_id, asteroid_id) in self.cove_entries.iter() {
            let team = match world.teams.get(team_id) {
                Some(t) => t,
                None => continue,
            };
            let style = if team.id == world.own_team_id {
                UiStyle::OWN_TEAM
            } else if team.peer_id.is_some() {
                UiStyle::NETWORK
            } else {
                UiStyle::DEFAULT
            };
            let asteroid_name = world
                .planets
                .get(asteroid_id)
                .map(|p| p.name.as_str())
                .unwrap_or("???");
            // Pad team-name + parenthesised asteroid so stars align across rows.
            let label = format!("{} ({})", team.name, asteroid_name);
            let text = format!(
                "{:<width$} {}",
                label,
                world.team_rating(&team.id).unwrap_or_default().stars(),
                width = MAX_NAME_LENGTH * 2,
            );
            options.push((text, style));
        }

        let list = selectable_list(options);
        self.cove_list_state.select(self.cove_index);
        frame.render_stateful_interactive_widget(
            list.block(default_block().title("Coves ↓/↑")),
            area,
            &mut self.cove_list_state,
        );
        if frame.is_hovering(area) {
            self.active_list = PanelList::Top;
        }
        Ok(())
    }

    fn render_visiting_teams(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let mut options = vec![];
        for team_id in self.visiting_team_ids.iter() {
            let team = if let Some(team) = world.teams.get(team_id) {
                team
            } else {
                continue;
            };
            let mut style = UiStyle::DEFAULT;
            if team.id == world.own_team_id {
                style = UiStyle::OWN_TEAM;
            } else if team.peer_id.is_some() {
                style = UiStyle::NETWORK;
            }
            let text = format!(
                "{:<MAX_NAME_LENGTH$} {}",
                team.name,
                world.team_rating(&team.id).unwrap_or_default().stars()
            );
            options.push((text, style));
        }

        if options.is_empty() {
            frame.render_widget(default_block().title("No visiting teams"), area);
            return Ok(());
        }

        let list = selectable_list(options);
        let mut state = ClickableListState::default();
        state.select(self.visiting_team_index);
        frame.render_stateful_interactive_widget(
            list.block(default_block().title("Visiting teams")),
            area,
            &mut state,
        );
        if frame.is_hovering(area) {
            self.active_list = PanelList::Bottom;
        }

        Ok(())
    }

    fn render_tournament_button(
        &self,
        frame: &mut UiFrame,
        own_team: &Team,
        asteroid: &Planet,
        tournament_type: TournamentType,
        area: Rect,
    ) {
        let (label, hotkey, blurb) = match tournament_type {
            TournamentType::Cup => (
                "Organize quick tournament",
                ui_key::ORGANIZE_QUICK_TOURNAMENT,
                "Registrations close in 5 minutes, max 4 participants.",
            ),
            TournamentType::Supercup => (
                "Organize big tournament",
                ui_key::ORGANIZE_BIG_TOURNAMENT,
                "Registrations close in 1 hour, max 8 participants.",
            ),
        };

        let hover = format!("Organize on {}. {blurb}", asteroid.name);

        let mut button = Button::new(label, UiCallback::OrganizeNewTournament { tournament_type })
            .set_hotkey(hotkey)
            .set_hover_text(hover);

        if let Err(err) = own_team.can_organize_tournament() {
            button.disable(Some(err.to_string()));
        }

        frame.render_interactive_widget(button, area);
    }

    fn render_building_list(&mut self, frame: &mut UiFrame, cove: &SpaceCove, area: Rect) {
        let options = BUILDINGS
            .iter()
            .map(|building| {
                let text = building.to_string();

                let style = if cove.upgrades.contains(building) {
                    UiStyle::DEFAULT
                } else if cove.pending_upgrade.is_some_and(|u| u.target == *building) {
                    UiStyle::WARNING
                } else {
                    UiStyle::DISCONNECTED
                };
                (text, style)
            })
            .collect::<Vec<_>>();

        let list = selectable_list(options);
        let mut state = ClickableListState::default();
        state.select(self.building_index);
        frame.render_stateful_interactive_widget(
            list.block(default_block().title("Buildings ↓/↑")),
            area,
            &mut state,
        );

        if frame.is_hovering(area) {
            self.active_list = PanelList::Top;
        }
    }

    fn render_building_detail(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        own_team: &Team,
        asteroid: &Planet,
        cove: &SpaceCove,
        building: &SpaceCoveUpgradeTarget,
        area: Rect,
    ) -> AppResult<()> {
        if cove.upgrades.contains(building) {
            match building {
                SpaceCoveUpgradeTarget::TeleportationPad => {
                    return self.render_teleportation_pad_detail(frame, world, asteroid, area);
                }
                SpaceCoveUpgradeTarget::Tavern => {
                    return self.render_tavern_detail(frame, world, own_team, cove, area);
                }

                SpaceCoveUpgradeTarget::Stadium => {
                    return self.render_stadium_detail(frame, world, asteroid, own_team, area);
                }

                SpaceCoveUpgradeTarget::Market => return self.render_market_detail(frame, area),
            }
        }

        self.render_missing_building(frame, world, own_team, cove, building, area)
    }

    fn render_market_detail(&self, frame: &mut UiFrame, area: Rect) -> AppResult<()> {
        let layout = Layout::vertical([
            Constraint::Length(3), // market
            Constraint::Fill(1),   // list?
        ])
        .split(area);

        let button = Button::new("Go to market", UiCallback::GoToMarket)
            .set_hover_text("Trade resources at the cove market.")
            .set_hotkey(ui_key::GO_TO_MARKET);
        frame.render_interactive_widget(button, layout[0]);

        Ok(())
    }

    fn render_missing_building(
        &self,
        frame: &mut UiFrame,
        world: &World,
        own_team: &Team,
        cove: &SpaceCove,
        building: &SpaceCoveUpgradeTarget,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Fill(1),
        ])
        .split(area);

        let bonus = TeamBonus::Upgrades.current_team_bonus(world, &own_team.id)?;
        let upgrade = Upgrade::new(*building, bonus);
        let pending_upgrade = cove.pending_upgrade.filter(|u| u.target == *building);

        let cost_block = default_block().title("Building cost");
        let cost_inner = cost_block.inner(split[1]);
        frame.render_widget(cost_block, split[1]);
        render_available_upgrades(
            pending_upgrade,
            Some(upgrade),
            world,
            own_team,
            frame,
            cost_inner,
        )?;

        if pending_upgrade.is_some() {
            let mut button = Button::new(format!("Building {building}"), UiCallback::None);
            button.disable(Some("In progress".to_string()));
            frame.render_interactive_widget(button, split[0]);
        } else {
            let mut button = Button::new(
                format!("Build {} ({})", building, upgrade.duration.formatted()),
                UiCallback::SetSpaceCovePendingUpgrade { upgrade },
            )
            .set_hover_text(building.description());
            if let Err(e) = own_team.can_upgrade_space_cove(*building) {
                button.disable(Some(e.to_string()));
            }
            frame.render_interactive_widget(button, split[0]);
        }

        Ok(())
    }

    fn render_teleportation_pad_detail(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        asteroid: &Planet,
        area: Rect,
    ) -> AppResult<()> {
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);
        let checkbox = Checkbox::new(
            "Allow external teleport",
            UiCallback::ToggleAsteroidExternalTeleport {
                asteroid_id: asteroid.id,
            },
            asteroid.allow_external_teleport,
        )
        .set_hover_text("Let other crews teleport to this asteroid.");
        frame.render_interactive_widget(checkbox, layout[0]);
        frame.render_interactive_widget(teleport_button(world, asteroid.id)?, layout[1]);

        self.render_visiting_teams(frame, world, layout[2])?;

        Ok(())
    }

    fn render_tavern_detail(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        own_team: &Team,
        cove: &SpaceCove,
        area: Rect,
    ) -> AppResult<()> {
        let layout = Layout::vertical([
            Constraint::Length(3), // add rum to store
            Constraint::Length(3), // rum served per day
            Constraint::Fill(1),   // free pirates list
        ])
        .split(area);

        let rum_per_day = cove
            .tavern
            .as_ref()
            .and_then(|tavern| tavern.upkeep_cost.get(&Resource::RUM).copied())
            .unwrap_or(0);

        let options = self
            .tavern_pirate_ids
            .iter()
            .filter_map(|id| world.players.get(id))
            .map(|player| {
                (
                    format!(
                        "{:<width$} {}",
                        player.info.full_name(),
                        player.stars(),
                        width = MAX_NAME_LENGTH * 2,
                    ),
                    UiStyle::DEFAULT,
                )
            })
            .collect::<Vec<_>>();

        if options.is_empty() {
            frame.render_widget(
                Paragraph::new("No free pirates on the cove.")
                    .centered()
                    .block(default_block().title("Free Pirates")),
                layout[2],
            );
        } else {
            let list = selectable_list(options);
            let mut state = ClickableListState::default();
            state.select(self.tavern_pirate_index);
            frame.render_stateful_interactive_widget(
                list.block(default_block().title("Free Pirates")),
                layout[2],
                &mut state,
            );
            if frame.is_hovering(layout[2]) {
                self.active_list = PanelList::Bottom;
            }
        }

        // Move rum from the crew stores into the cove, mirroring the market buttons.
        let available_rum = own_team.resources.get(&Resource::RUM).copied().unwrap_or(0);
        let add_rum_split = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(6),
            Constraint::Length(6),
        ])
        .split(layout[0]);

        let border_style = if cove.can_pay_tavern_upkeep() {
            UiStyle::DEFAULT
        } else {
            UiStyle::WARNING
        };
        frame.render_widget(
            Paragraph::new(format!(
                "{} rum stored",
                cove.resources.value(&Resource::RUM)
            ))
            .centered()
            .block(default_block().border_style(border_style)),
            add_rum_split[0],
        );

        for (idx, amount) in [1_u32, 10].iter().enumerate() {
            let amount = (*amount).min(available_rum);
            let mut button = Button::new(format!("+{amount}"), UiCallback::AddRumToCove { amount })
                .set_hover_text(format!(
                    "Store {amount} rum in the tavern (you have {available_rum})."
                ))
                .block(default_block().border_style(UiStyle::OK));
            if available_rum < amount {
                button.disable(Some("Not enough rum"));
            }
            frame.render_interactive_widget(button, add_rum_split[idx + 1]);
        }

        let rum_per_day_split = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(6),
            Constraint::Length(6),
        ])
        .split(layout[1]);
        let border_style = if cove.can_pay_tavern_upkeep() {
            UiStyle::DEFAULT
        } else {
            UiStyle::WARNING
        };
        frame.render_widget(
            Paragraph::new(format!("{rum_per_day} rum/day"))
                .centered()
                .block(default_block().border_style(border_style)),
            rum_per_day_split[0],
        );
        let mut less = Button::new("-1", UiCallback::ChangeTavernRumPerDay { delta: -1 })
            .set_hover_text("Serve one less rum per day.")
            .block(default_block().border_style(UiStyle::ERROR));
        if rum_per_day == 0 {
            less.disable(Some("Already zero"));
        }
        frame.render_interactive_widget(less, rum_per_day_split[1]);

        let more = Button::new("+1", UiCallback::ChangeTavernRumPerDay { delta: 1 })
            .set_hover_text("Serve one more rum per day.")
            .block(default_block().border_style(UiStyle::OK));
        frame.render_interactive_widget(more, rum_per_day_split[2]);

        Ok(())
    }

    fn render_stadium_detail(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        asteroid: &Planet,
        own_team: &Team,
        area: Rect,
    ) -> AppResult<()> {
        let layout = Layout::vertical([
            Constraint::Length(3), // organize
            Constraint::Length(3), // organize
            Constraint::Fill(1),   // tournaments
        ])
        .split(area);

        let mut options = vec![];
        for id in &self.tournament_ids {
            if let Some(t) = world.tournaments.get(id) {
                options.push((
                    format!("{:<24} {}", t.name(), t.stars()),
                    UiStyle::HIGHLIGHT,
                ));
            } else if let Some(s) = world.past_tournaments.get(id) {
                options.push((format!("{:<24} {}", s.name(), s.stars()), UiStyle::DEFAULT));
            }
        }

        let list = selectable_list(options);
        let mut state = ClickableListState::default();
        state.select(self.tournament_index);
        frame.render_stateful_interactive_widget(
            list.block(default_block().title(format!("Tournaments on {}", asteroid.name))),
            layout[2],
            &mut state,
        );
        if frame.is_hovering(layout[2]) {
            self.active_list = PanelList::Bottom;
        }

        self.render_tournament_button(frame, own_team, asteroid, TournamentType::Cup, layout[0]);
        self.render_tournament_button(
            frame,
            own_team,
            asteroid,
            TournamentType::Supercup,
            layout[1],
        );

        Ok(())
    }
}

impl Screen for SpaceCovePanel {
    fn tick(&mut self) {
        self.tick += 1;
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let cove_planet = own_team.has_space_cove_on();
        let lamps_on = own_team.is_on_planet() == own_team.has_space_cove_on();
        let tavern_pirate_ids: Vec<PlayerId> = match cove_planet {
            Some(planet) => world
                .players
                .values()
                .filter(|p| p.team.is_none() && p.is_on_planet() == Some(planet))
                .map(|p| p.id)
                .collect(),
            None => Vec::new(),
        };
        if lamps_on != self.tavern_lamps_on || tavern_pirate_ids != self.tavern_pirate_ids {
            let pirate_frames: Vec<RgbaImage> = tavern_pirate_ids
                .iter()
                .filter_map(|id| world.players.get(id))
                .filter_map(|player| player.compose_image().ok())
                .filter_map(|gif| gif.into_iter().next())
                .collect();
            self.tavern_widget = {
                let img = Self::get_tavern_image(lamps_on, &pirate_frames)?;
                Paragraph::new(img_to_lines(&img))
            };
            self.tavern_lamps_on = lamps_on;
            self.tavern_pirate_ids = tavern_pirate_ids;
        }

        // Rebuild the cove entries only when the team set or contents may have shifted.
        let mut entries_changed = false;
        if world.dirty_ui || world.teams.len() != self.cached_teams_len {
            let mut decorated: Vec<(TeamId, PlanetId, &str)> = world
                .teams
                .values()
                .filter_map(|team| {
                    team.space_cove
                        .as_ref()
                        .filter(|cove| cove.is_ready())
                        .map(|cove| (team.id, cove.planet_id, team.name.as_str()))
                })
                .collect();

            decorated.sort_by(|a, b| {
                let a_own = a.0 == own_team.id;
                let b_own = b.0 == own_team.id;
                match (a_own, b_own) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.2.cmp(b.2),
                }
            });

            let new_entries: Vec<(TeamId, PlanetId)> =
                decorated.into_iter().map(|(t, p, _)| (t, p)).collect();

            entries_changed = new_entries != self.cove_entries;
            self.cove_entries = new_entries;
            self.cached_teams_len = world.teams.len();
        }

        let prev_index = self.cove_index;
        self.cove_index = normalize_index(
            self.cove_index.unwrap_or(0),
            self.cove_entries.len(),
            IndexBound::Wrap,
        );
        let index_changed = prev_index != self.cove_index;

        let selected_asteroid_id = match self.view {
            SpaceCoveView::OwnCove => own_team.has_space_cove_on(),
            SpaceCoveView::AllCoves => self
                .cove_index
                .and_then(|i| self.cove_entries.get(i).map(|(_, p)| *p)),
        };

        match selected_asteroid_id.and_then(|id| world.planets.get(&id)) {
            Some(asteroid) => {
                let previous_visitors = std::mem::take(&mut self.visiting_team_ids);
                let new_set: HashSet<TeamId> = asteroid.team_ids.iter().copied().collect();
                self.visiting_team_ids = previous_visitors
                    .iter()
                    .copied()
                    .filter(|id| new_set.contains(id))
                    .collect();
                for id in &asteroid.team_ids {
                    if !self.visiting_team_ids.contains(id) {
                        self.visiting_team_ids.push(*id);
                    }
                }

                let visitors_changed = previous_visitors != self.visiting_team_ids;
                if world.dirty_ui || entries_changed || index_changed || visitors_changed {
                    let teams = self
                        .visiting_team_ids
                        .iter()
                        .take(4)
                        .filter(|id| world.teams.contains_key(id))
                        .map(|id| world.teams.get(id).unwrap())
                        .collect_vec();
                    self.cove_image_widgets = Self::build_image_widgets(&teams)?;
                }
            }
            None => {
                let was_populated = !self.visiting_team_ids.is_empty();
                self.visiting_team_ids.clear();
                if entries_changed || index_changed || was_populated {
                    self.cove_image_widgets = Self::build_image_widgets(&[])?;
                }
            }
        }

        self.building_index = if own_team.space_cove.is_none() {
            None
        } else {
            Some(self.building_index.unwrap_or(0).min(BUILDINGS.len() - 1))
        };

        if world.dirty_ui {
            self.tournament_ids = match cove_planet {
                Some(planet_id) => {
                    let mut current = world
                        .tournaments
                        .iter()
                        .filter(|(_, t)| t.planet_id == planet_id)
                        .collect::<Vec<_>>();
                    current.sort_by_key(|(_, t)| t.name());

                    let mut past = world
                        .past_tournaments
                        .iter()
                        .filter(|(_, s)| s.planet_id == planet_id)
                        .collect::<Vec<_>>();
                    past.sort_by_key(|(_, s)| std::cmp::Reverse(s.ended_at));

                    current
                        .into_iter()
                        .map(|(id, _)| *id)
                        .chain(past.into_iter().map(|(id, _)| *id))
                        .collect()
                }
                None => Vec::new(),
            };
        }

        self.tournament_index = normalize_index(
            self.tournament_index.unwrap_or(0),
            self.tournament_ids.len(),
            IndexBound::Wrap,
        );
        self.visiting_team_index = normalize_index(
            self.visiting_team_index.unwrap_or(0),
            self.visiting_team_ids.len(),
            IndexBound::Wrap,
        );
        self.tavern_pirate_index = normalize_index(
            self.tavern_pirate_index.unwrap_or(0),
            self.tavern_pirate_ids.len(),
            IndexBound::Wrap,
        );

        if own_team.space_cove.is_none() {
            self.view = SpaceCoveView::AllCoves;
        }

        Ok(())
    }

    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Fill(1)])
            .split(area);

        frame.render_widget(default_block(), split[1]);

        let own_team = world.get_own_team()?;

        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(split[0]);

        self.render_view_buttons(frame, world, layout[0], layout[1])?;

        match self.view {
            SpaceCoveView::OwnCove => {
                let sub_layout = Layout::vertical([
                    Constraint::Length(BUILDINGS.len() as u16 + 2),
                    Constraint::Fill(1),
                ])
                .split(layout[2]);

                match own_team.space_cove.as_ref() {
                    None => {}
                    Some(cove) if !cove.is_ready() => {
                        let countdown = own_team
                            .has_space_cove_on()
                            .and_then(|id| world.planets.get(&id))
                            .and_then(|asteroid| asteroid.pending_upgrade)
                            .map(|upgrade| {
                                (upgrade.started + upgrade.duration)
                                    .saturating_sub(world.last_tick_short_interval)
                                    .formatted()
                            })
                            .unwrap_or_default();
                        frame.render_widget(
                            Paragraph::new(vec![
                                Line::from(Span::styled(
                                    "Space cove under construction",
                                    UiStyle::HEADER.bold(),
                                )),
                                Line::from(countdown),
                            ])
                            .centered()
                            .block(default_block().title("Buildings")),
                            sub_layout[0],
                        );
                        self.render_visiting_teams(frame, world, sub_layout[1])?;
                    }
                    Some(cove) => {
                        let asteroid_id = if let Some(id) = own_team.has_space_cove_on() {
                            id
                        } else {
                            return Ok(());
                        };

                        self.render_building_list(frame, cove, sub_layout[0]);
                        if self.view != SpaceCoveView::OwnCove {
                            return Ok(());
                        }
                        let building = if let Some(b) =
                            self.building_index.and_then(|index| BUILDINGS.get(index))
                        {
                            b
                        } else {
                            return Ok(());
                        };

                        let asteroid = world.planets.get_or_err(&asteroid_id)?;

                        self.render_building_detail(
                            frame,
                            world,
                            own_team,
                            asteroid,
                            cove,
                            building,
                            sub_layout[1],
                        )?;

                        // Render right side
                        if cove.upgrades.contains(building) {
                            let right_area = split[1].inner(Margin::new(1, 1));
                            match building {
                                SpaceCoveUpgradeTarget::TeleportationPad => {
                                    let t = self.tick % 60;
                                    let left_eye_blinking = [2, 3, 5, 13, 33].contains(&t);
                                    let right_eye_blinking = [2, 3, 6, 7, 41].contains(&t);
                                    let widget = match (left_eye_blinking, right_eye_blinking) {
                                        (false, false) => &self.cove_image_widgets[0],
                                        (true, false) => &self.cove_image_widgets[1],
                                        (false, true) => &self.cove_image_widgets[2],
                                        (true, true) => &self.cove_image_widgets[3],
                                    };
                                    frame.render_widget(widget, right_area);
                                }
                                SpaceCoveUpgradeTarget::Tavern => {
                                    frame.render_widget(&self.tavern_widget, right_area);
                                }
                                SpaceCoveUpgradeTarget::Stadium => {
                                    frame.render_widget(&self.stadium_widget, right_area);
                                }

                                SpaceCoveUpgradeTarget::Market => {
                                    frame.render_widget(&self.market_widget, right_area);
                                }
                            }
                        }
                    }
                }
            }
            SpaceCoveView::AllCoves => {
                let sub_layout = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Fill(1),
                ])
                .split(layout[2]);
                self.render_cove_list(frame, world, sub_layout[0])?;
                self.render_visiting_teams(frame, world, sub_layout[3])?;

                if let Some(asteroid) = self
                    .cove_index
                    .and_then(|i| self.cove_entries.get(i).map(|(_, p)| *p))
                    .and_then(|id| world.planets.get(&id))
                {
                    frame.render_interactive_widget(
                        go_to_planet_button(world, asteroid.id)?,
                        sub_layout[1],
                    );
                    frame.render_interactive_widget(
                        teleport_button(world, asteroid.id)?,
                        sub_layout[2],
                    );
                }

                let t = self.tick % 60;
                let left_eye_blinking = [2, 3, 5, 13, 33].contains(&t);
                let right_eye_blinking = [2, 3, 6, 7, 41].contains(&t);
                let widget = match (left_eye_blinking, right_eye_blinking) {
                    (false, false) => &self.cove_image_widgets[0],
                    (true, false) => &self.cove_image_widgets[1],
                    (false, true) => &self.cove_image_widgets[2],
                    (true, true) => &self.cove_image_widgets[3],
                };
                frame.render_widget(widget, split[1].inner(Margin::new(1, 1)));
            }
        }

        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, world: &World) -> Option<UiCallback> {
        let own_team = world.get_own_team().ok()?;
        let has_space_cove = own_team.has_space_cove_on().is_some();
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            ui_key::CYCLE_VIEW if has_space_cove => {
                return Some(UiCallback::SetSpaceCovePanelView {
                    view: self.view.next(),
                });
            }
            ui_key::CYCLE_VIEW_BACK if has_space_cove => {
                return Some(UiCallback::SetSpaceCovePanelView {
                    view: self.view.previous(),
                });
            }
            KeyCode::Enter => {
                return match self.active_selection() {
                    ActiveSelection::VisitingTeam => {
                        let team_id = *self.visiting_team_ids.get(self.visiting_team_index?)?;
                        Some(UiCallback::GoToTeam { team_id })
                    }
                    ActiveSelection::Tournament => {
                        let tournament_id = *self.tournament_ids.get(self.tournament_index?)?;
                        Some(UiCallback::GoToTournament { tournament_id })
                    }
                    ActiveSelection::TavernPirate => {
                        let player_id = *self.tavern_pirate_ids.get(self.tavern_pirate_index?)?;
                        Some(UiCallback::GoToPlayer { player_id })
                    }
                    _ => None,
                };
            }
            _ => {}
        }
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![
            format!(" {} ", ui_key::CYCLE_VIEW),
            " Cycle view ".to_string(),
        ]
    }

    fn render_help_widget(
        &self,
        frame: &mut UiFrame,
        _world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        render_help_block(
            frame,
            area,
            vec![
                Line::from(" Manage your own space cove and browse other crews' coves."),
                Line::from(" Use the two buttons at the top to switch view: yours"),
                Line::from(" (tournaments + upgrades) or other coves (list + travel)."),
                Line::from(""),
                Line::from(" Manage the asteroid that hosts your cove from My Team."),
                Line::from(" Inspect visiting crews directly, or browse all in Crews."),
                Line::from(" To find another asteroid candidate, explore the Galaxy."),
            ],
            vec![
                tab_link("My Team", UiTab::MyTeam),
                tab_link("Crews", UiTab::Crews),
                tab_link("Galaxy", UiTab::Galaxy),
            ],
            vec![
                Line::from(" Controls:"),
                Line::from(format!(
                    "   {}        Cycle between Own cove and Other coves view",
                    ui_key::CYCLE_VIEW
                )),
                Line::from("   ↑/↓        Move highlight in the cove list (Other coves view)"),
                Line::from(format!(
                    "   {}          Teleport / Travel to the selected cove asteroid",
                    ui_key::TRAVEL
                )),
                Line::from(format!(
                    "   {}          Organize a quick tournament (own cove only)",
                    ui_key::ORGANIZE_QUICK_TOURNAMENT
                )),
                Line::from(format!(
                    "   {}          Organize a big tournament (own cove only)",
                    ui_key::ORGANIZE_BIG_TOURNAMENT
                )),
            ],
        );
        Ok(())
    }
}

impl SplitPanel for SpaceCovePanel {
    fn index(&self) -> Option<usize> {
        match self.active_selection() {
            ActiveSelection::Building => self.building_index,
            ActiveSelection::Cove => self.cove_index,
            ActiveSelection::VisitingTeam => self.visiting_team_index,
            ActiveSelection::Tournament => self.tournament_index,
            ActiveSelection::TavernPirate => self.tavern_pirate_index,
            ActiveSelection::None => None,
        }
    }

    fn max_index(&self) -> usize {
        match self.active_selection() {
            ActiveSelection::Building => BUILDINGS.len(),
            ActiveSelection::Cove => self.cove_entries.len(),
            ActiveSelection::VisitingTeam => self.visiting_team_ids.len(),
            ActiveSelection::Tournament => self.tournament_ids.len(),
            ActiveSelection::TavernPirate => self.tavern_pirate_ids.len(),
            ActiveSelection::None => 0,
        }
    }

    fn set_index(&mut self, index: usize) {
        let len = self.max_index();
        if len == 0 {
            return;
        }
        let clamped = index % len;
        match self.active_selection() {
            ActiveSelection::Building => self.building_index = Some(clamped),
            ActiveSelection::Cove => self.cove_index = Some(clamped),
            ActiveSelection::VisitingTeam => self.visiting_team_index = Some(clamped),
            ActiveSelection::Tournament => self.tournament_index = Some(clamped),
            ActiveSelection::TavernPirate => self.tavern_pirate_index = Some(clamped),
            ActiveSelection::None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SpaceCovePanel;
    use crate::core::{Player, Population};
    use crate::types::AppResult;
    use image::{self, RgbaImage};
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use std::path::Path;
    use strum::IntoEnumIterator;

    #[ignore]
    #[test]
    fn test_generate_tavern_image_with_pirates() -> AppResult<()> {
        let rng = &mut ChaCha8Rng::seed_from_u64(0);

        // One free pirate per population; the tavern caps how many it draws.
        let mut pirate_frames: Vec<RgbaImage> = Vec::new();
        for population in Population::iter() {
            let player = Player::default()
                .with_population(population)
                .randomize(Some(&mut *rng));
            pirate_frames.push(player.compose_image()?[0].clone());
        }

        for lamps_on in [false, true] {
            let img = SpaceCovePanel::get_tavern_image(lamps_on, &pirate_frames)?;
            let (width, height) = (img.width(), img.height());
            image::save_buffer(
                Path::new(&format!(
                    "tests/images/tavern_image_lamps_{}.png",
                    if lamps_on { "on" } else { "off" }
                )),
                &img,
                width,
                height,
                image::ColorType::Rgba8,
            )?;
        }

        Ok(())
    }
}
