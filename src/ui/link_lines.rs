use super::button::Button;
use super::constants::UiStyle;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::utils::wrap_text;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::widgets::Paragraph;

#[derive(Clone, Copy)]
pub(crate) enum LinkAlign {
    Left,
    Center,
}

pub(crate) fn render_lines_with_links<S: AsRef<str>>(
    frame: &mut UiFrame,
    area: Rect,
    text: &str,
    links: &[(S, UiCallback)],
    align: LinkAlign,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut row: u16 = 0;
    for segment in text.split('\n') {
        for line in wrap_text(segment, area.width as usize) {
            let y = area.y + row;
            if y >= area.bottom() {
                return;
            }
            let line_w = (line.chars().count() as u16).min(area.width);
            let x = match align {
                LinkAlign::Left => area.x,
                LinkAlign::Center => area.x + area.width.saturating_sub(line_w) / 2,
            };
            frame.render_widget(Paragraph::new(line.as_str()), Rect::new(x, y, line_w, 1));
            overlay_line_links(frame, &line, x, y, links);
            row += 1;
        }
    }
}

fn overlay_line_links<S: AsRef<str>>(
    frame: &mut UiFrame,
    text: &str,
    x: u16,
    y: u16,
    links: &[(S, UiCallback)],
) {
    for (label, callback) in links {
        let label = label.as_ref();
        if let Some(byte_col) = text.find(label) {
            let col = text[..byte_col].chars().count() as u16;
            let button = Button::no_box(label, callback.clone())
                .set_style(UiStyle::HELP_LINK)
                .set_layer(1);
            frame.render_interactive_widget(
                button,
                Rect::new(x + col, y, label.chars().count() as u16, 1),
            );
        }
    }
}
