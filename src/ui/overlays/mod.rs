mod help_overlay;
mod traits;
mod trade_overlay;

pub(crate) use help_overlay::HelpOverlay;
pub(crate) use trade_overlay::{TradeOverlay, TradeSide};
pub(crate) use traits::Overlay;

use ratatui::layout::Rect;

/// Layer the modal popup messages draw and claim input on. Kept well above the
/// overlay layers, which start at 1 and grow with the overlay stack.
pub const POPUP_LAYER: usize = 100;

/// Help on top of another overlay is as deep as the stack ever needs to go.
pub const MAX_OVERLAY_DEPTH: usize = 2;

/// Centered rect covering `pct_w`/`pct_h` percent of `area`, never smaller than
/// `min` and never larger than `area` itself. Clamping to `area` last matters:
/// on a terminal narrower than `min` a plain `clamp(min, max)` would panic.
pub fn centered_rect(area: Rect, pct_w: u16, pct_h: u16, min: (u16, u16)) -> Rect {
    let width = (area.width * pct_w / 100).max(min.0).min(area.width);
    let height = (area.height * pct_h / 100).max(min.1).min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

/// The overlays currently stacked on the screen, innermost last.
///
/// An enum rather than `Box<dyn Overlay>`: callbacks need `&mut` access to a
/// concrete overlay to edit its state, which through a trait object would mean
/// an `Any` supertrait and a downcast at every call site.
#[derive(Debug)]
pub enum OverlayKind {
    Help(HelpOverlay),
    Trade(TradeOverlay),
}

impl OverlayKind {
    pub fn as_dyn(&self) -> &dyn Overlay {
        match self {
            Self::Help(overlay) => overlay,
            Self::Trade(overlay) => overlay,
        }
    }

    pub fn as_dyn_mut(&mut self) -> &mut dyn Overlay {
        match self {
            Self::Help(overlay) => overlay,
            Self::Trade(overlay) => overlay,
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
        // Smaller than the preferred minimum in both axes: clamp, do not panic.
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
