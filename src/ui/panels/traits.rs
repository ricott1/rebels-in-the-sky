use crate::core::world::World;
use crate::types::AppResult;
use crate::ui::link_lines::{render_lines_with_links, LinkAlign};
use crate::ui::ui_frame::UiFrame;
use crate::ui::utils::wrap_text;
use crate::ui::UiCallback;
use ratatui::crossterm;
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::prelude::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

pub enum IndexBound {
    Wrap,
    Clamp,
}

pub(crate) fn normalize_index(index: usize, len: usize, bound: IndexBound) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match bound {
        IndexBound::Wrap => index % len,
        IndexBound::Clamp => index.min(len - 1),
    })
}

pub trait SplitPanel {
    fn index(&self) -> Option<usize> {
        None
    }
    fn max_index(&self) -> usize {
        0
    }
    fn set_index(&mut self, _index: usize) {}
    fn index_bound(&self) -> IndexBound {
        IndexBound::Wrap
    }
    fn previous_index(&mut self) {
        let len = self.max_index();
        let bound = self.index_bound();
        if let Some(i) = self.index() {
            if let Some(next) = normalize_index(i + 1, len, bound) {
                self.set_index(next);
            }
        }
    }
    fn next_index(&mut self) {
        let len = self.max_index();
        let bound = self.index_bound();
        if let Some(i) = self.index() {
            let raw = match bound {
                IndexBound::Wrap => (i + len).saturating_sub(1),
                IndexBound::Clamp => i.saturating_sub(1),
            };
            if let Some(next) = normalize_index(raw, len, bound) {
                self.set_index(next);
            }
        }
    }
}

pub trait Screen: HelpPanel {
    fn tick(&mut self);
    fn update(&mut self, world: &World) -> AppResult<()>;
    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        debug_view: bool,
    ) -> AppResult<()>;

    fn handle_key_events(
        &mut self,
        _key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<UiCallback> {
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![]
    }

    /// Returns true when the panel currently has an active text input that
    /// should receive raw character keys. Suppresses global character-key
    /// shortcuts (currently '?' for help) so the user can type those characters.
    fn is_capturing_text(&self) -> bool {
        false
    }
}

pub struct HelpContent {
    pub description: String,
    pub links: Vec<(String, UiCallback)>,
    pub controls: Vec<Line<'static>>,
}

pub trait HelpPanel {
    fn help_content(&self) -> HelpContent;
}

pub fn render_help_content(frame: &mut UiFrame, area: Rect, content: HelpContent) {
    let area = area.inner(Margin::new(1, 0));
    let desc_rows = content
        .description
        .split('\n')
        .map(|seg| wrap_text(seg, area.width as usize).len().max(1))
        .sum::<usize>() as u16;

    let split = Layout::vertical([Constraint::Length(desc_rows), Constraint::Fill(1)]).split(area);

    render_lines_with_links(
        frame,
        split[0],
        &content.description,
        &content.links,
        LinkAlign::Left,
    );
    frame.render_widget(Paragraph::new(content.controls), split[1]);
}
