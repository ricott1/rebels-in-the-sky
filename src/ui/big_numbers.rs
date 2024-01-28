use ratatui::widgets::Paragraph;

use super::utils::big_text;

pub fn dots() -> Paragraph<'static> {
    big_text(&["   ", "██╗", "╚═╝", "██╗", "╚═╝", "   "])
}

pub fn hyphen() -> Paragraph<'static> {
    big_text(&["     ", "     ", "████╗", "╚═══╝", "     ", "     "])
}

pub fn zero() -> Paragraph<'static> {
    big_text(&[
        " ██████╗ ",
        "██╔═████╗",
        "██║██╔██║",
        "████╔╝██║",
        "╚██████╔╝",
        " ╚═════╝ ",
    ])
}

pub fn one() -> Paragraph<'static> {
    big_text(&[" ██╗", "███║", "╚██║", " ██║", " ██║", " ╚═╝"])
}

pub fn two() -> Paragraph<'static> {
    big_text(&[
        "██████╗ ",
        "╚════██╗",
        " █████╔╝",
        "██╔═══╝ ",
        "███████╗",
        "╚══════╝",
    ])
}

pub fn three() -> Paragraph<'static> {
    big_text(&[
        "██████╗ ",
        "╚════██╗",
        " █████╔╝",
        " ╚═══██╗",
        "██████╔╝",
        "╚═════╝ ",
    ])
}

pub fn four() -> Paragraph<'static> {
    big_text(&[
        "██╗  ██╗",
        "██║  ██║",
        "███████║",
        "╚════██║",
        "     ██║",
        "     ╚═╝",
    ])
}

pub fn five() -> Paragraph<'static> {
    big_text(&[
        "███████╗",
        "██╔════╝",
        "███████╗",
        "╚════██║",
        "███████║",
        "╚══════╝",
    ])
}

pub fn six() -> Paragraph<'static> {
    big_text(&[
        " ██████╗ ",
        "██╔════╝ ",
        "███████╗ ",
        "██╔═══██╗",
        "╚██████╔╝",
        " ╚═════╝ ",
    ])
}

pub fn seven() -> Paragraph<'static> {
    big_text(&[
        "███████╗",
        "╚════██║",
        "    ██╔╝",
        "   ██╔╝ ",
        "   ██║  ",
        "   ╚═╝  ",
    ])
}

pub fn eight() -> Paragraph<'static> {
    big_text(&[
        " █████╗ ",
        "██╔══██╗",
        "╚█████╔╝",
        "██╔══██╗",
        "╚█████╔╝",
        " ╚════╝ ",
    ])
}

pub fn nine() -> Paragraph<'static> {
    big_text(&[
        " █████╗ ",
        "██╔══██╗",
        "╚██████║",
        " ╚═══██║",
        " █████╔╝",
        " ╚════╝ ",
    ])
}

pub trait BigNumberFont {
    fn big_font(&self) -> Paragraph<'static>;
}
impl BigNumberFont for u8 {
    fn big_font(&self) -> Paragraph<'static> {
        match self {
            0 => zero(),
            1 => one(),
            2 => two(),
            3 => three(),
            4 => four(),
            5 => five(),
            6 => six(),
            7 => seven(),
            8 => eight(),
            9 => nine(),
            _ => dots(),
        }
    }
}

impl BigNumberFont for u16 {
    fn big_font(&self) -> Paragraph<'static> {
        match self {
            0 => zero(),
            1 => one(),
            2 => two(),
            3 => three(),
            4 => four(),
            5 => five(),
            6 => six(),
            7 => seven(),
            8 => eight(),
            9 => nine(),
            _ => dots(),
        }
    }
}
