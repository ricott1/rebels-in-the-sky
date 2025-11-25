use super::{
    constants::{UiStyle, MAX_NAME_LENGTH, MIN_NAME_LENGTH},
    widgets::default_block,
};
use crate::world::{AU, LIGHT_YEAR, SATOSHI_PER_BITCOIN};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use image::RgbaImage;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{block::Title, Paragraph},
};
use tui_textarea::{Input, Key, TextArea};

pub fn input_from_key_event(key: KeyEvent) -> Input {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let key = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Tab => Key::Tab,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Esc => Key::Esc,
        KeyCode::F(x) => Key::F(x),
        _ => Key::Null,
    };
    Input {
        key,
        ctrl,
        alt,
        shift,
    }
}

pub fn img_to_lines<'a>(img: &RgbaImage) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = vec![];
    let width = img.width();
    let height = img.height();

    for y in (0..height - 1).step_by(2) {
        let mut line: Vec<Span> = vec![];

        for x in 0..width {
            let top_pixel = img.get_pixel(x, y);
            let btm_pixel = img.get_pixel(x, y + 1);
            if top_pixel[3] == 0 && btm_pixel[3] == 0 {
                line.push(Span::raw(" "));
                continue;
            }

            if top_pixel[3] > 0 && btm_pixel[3] == 0 {
                let [r, g, b, _] = top_pixel.0;
                let color = Color::Rgb(r, g, b);
                line.push(Span::styled("▀", Style::default().fg(color)));
            } else if top_pixel[3] == 0 && btm_pixel[3] > 0 {
                let [r, g, b, _] = btm_pixel.0;
                let color = Color::Rgb(r, g, b);
                line.push(Span::styled("▄", Style::default().fg(color)));
            } else {
                let [fr, fg, fb, _] = top_pixel.0;
                let fg_color = Color::Rgb(fr, fg, fb);
                let [br, bg, bb, _] = btm_pixel.0;
                let bg_color = Color::Rgb(br, bg, bb);
                line.push(Span::styled(
                    "▀",
                    Style::default().fg(fg_color).bg(bg_color),
                ));
            }
        }
        lines.push(Line::from(line));
    }
    // append last line if height is odd
    if height % 2 == 1 {
        let mut line: Vec<Span> = vec![];
        for x in 0..width {
            let top_pixel = img.get_pixel(x, height - 1);
            if top_pixel[3] == 0 {
                line.push(Span::raw(" "));
                continue;
            }
            let [r, g, b, _] = top_pixel.0;
            let color = Color::Rgb(r, g, b);
            line.push(Span::styled("▀", Style::default().fg(color)));
        }
        lines.push(Line::from(line));
    }

    lines
}

pub fn big_text<'a>(text: &'a [&str]) -> Paragraph<'a> {
    let lines = text
        .iter()
        .map(|line| {
            let mut spans = vec![];
            for c in line.chars() {
                if c == '█' {
                    spans.push(Span::styled("█", UiStyle::SHADOW));
                } else {
                    spans.push(Span::styled(c.to_string(), UiStyle::HIGHLIGHT));
                }
            }
            Line::from(spans)
        })
        .collect::<Vec<Line>>();
    Paragraph::new(lines).centered()
}

pub fn validate_textarea_input<'a>(
    textarea: &mut TextArea<'a>,
    title: impl Into<Title<'a>>,
) -> bool {
    let text = textarea.lines()[0].trim();
    if text.len() < MIN_NAME_LENGTH {
        textarea.set_style(UiStyle::ERROR);
        textarea.set_block(default_block().title(title).title("(too short)"));
        false
    } else if text.len() > MAX_NAME_LENGTH {
        textarea.set_style(UiStyle::ERROR);
        textarea.set_block(default_block().title(title).title("(too long)"));
        false
    } else {
        textarea.set_style(UiStyle::DEFAULT);
        textarea.set_block(default_block().title(title));
        true
    }
}

pub fn format_satoshi(amount: u32) -> String {
    if amount >= 1_000_000 {
        let f_amount =
            (amount as f32 / SATOSHI_PER_BITCOIN as f32 * 1_000_000.0).round() / 1_000_000.0;
        return format!("{f_amount:.4} BTC");
    }

    format!("{amount} sat")
}

pub fn format_au(amount: f32) -> String {
    const AU_PER_LIGHT_YEAR: f32 = LIGHT_YEAR as f32 / AU as f32;
    if amount >= 10_000.0 {
        let f_amount = (amount / AU_PER_LIGHT_YEAR * 10_000.0).round() / 10_000.0;
        return format!("{f_amount:.4} ly");
    }

    format!("{amount:.4} AU")
}

#[cfg(test)]
mod test {
    use super::format_satoshi;

    #[test]
    fn test_format_satoshi() {
        assert_eq!(format_satoshi(1), "1 sat");
        assert_eq!(format_satoshi(10), "10 sat");
        assert_eq!(format_satoshi(1_000), "1000 sat");
        assert_eq!(format_satoshi(99_999), "99999 sat");
        assert_eq!(format_satoshi(999_999), "999999 sat");
        assert_eq!(format_satoshi(1_000_000), "0.0100 BTC");
        assert_eq!(format_satoshi(2_345_678), "0.0235 BTC");
        assert_eq!(format_satoshi(100_000_000), "1.0000 BTC");
        assert_eq!(format_satoshi(1_234_567_890), "12.3457 BTC");
    }
}
