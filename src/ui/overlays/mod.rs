mod help_overlay;
mod offer_overlay;
mod traits;

pub(crate) use help_overlay::HelpOverlay;
pub(crate) use offer_overlay::{OfferOverlay, OfferSide};
pub(crate) use traits::Overlay;

use ratatui::layout::Rect;

/// Popup messages sit well above the overlay layers.
pub const POPUP_LAYER: usize = 100;

/// An overlay plus help on top of it.
pub const MAX_OVERLAY_DEPTH: usize = 2;

/// Centered rect covering `pct_w` by `pct_h` percent of `area`,
/// at least `min` in size but never larger than `area`.
pub fn centered_rect(area: Rect, pct_w: u16, pct_h: u16, min: (u16, u16)) -> Rect {
    let width = (area.width * pct_w / 100).max(min.0).min(area.width);
    let height = (area.height * pct_h / 100).max(min.1).min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

#[derive(Debug)]
pub enum OverlayKind {
    Help(HelpOverlay),
    Offer(OfferOverlay),
}

impl OverlayKind {
    pub fn as_dyn(&self) -> &dyn Overlay {
        match self {
            Self::Help(overlay) => overlay,
            Self::Offer(overlay) => overlay,
        }
    }

    pub fn as_dyn_mut(&mut self) -> &mut dyn Overlay {
        match self {
            Self::Help(overlay) => overlay,
            Self::Offer(overlay) => overlay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::centered_rect;
    use ratatui::layout::Rect;

    const HELP: (u16, u16, (u16, u16)) = (60, 80, (50, 20));

    #[test]
    fn test_centered_rect_matches_legacy_help_geometry() {
        let (w, h, min) = HELP;
        let rect = centered_rect(Rect::new(0, 0, 160, 48), w, h, min);
        assert_eq!(rect, Rect::new(32, 5, 96, 38));
    }

    #[test]
    fn test_centered_rect_grows_to_the_minimum() {
        let (w, h, min) = HELP;
        let rect = centered_rect(Rect::new(0, 0, 80, 24), w, h, min);
        assert_eq!(rect.width, 50);
        assert_eq!(rect.height, 20);
    }

    #[test]
    fn test_centered_rect_never_exceeds_the_area() {
        let (w, h, min) = HELP;
        // The area is smaller than the minimum, so it is returned as is.
        let area = Rect::new(3, 7, 40, 10);
        let rect = centered_rect(area, w, h, min);
        assert_eq!(rect, area);
    }

    #[test]
    fn test_centered_rect_respects_a_non_zero_origin() {
        let rect = centered_rect(Rect::new(10, 20, 100, 50), 50, 50, (0, 0));
        assert_eq!(rect, Rect::new(35, 32, 50, 25));
    }
}
