//! Panel geometry: sizes, placement on the monitor and hit testing (slot from angle).
//!
//! Everything is computed here and sent to the frontend so that what is drawn and what is
//! hit-tested always match. Logical units are CSS pixels; physical are screen pixels.

use serde::Serialize;

use crate::profiles::model::Layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// Logical (CSS px) panel geometry, sent to the overlay and editor preview.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelGeometry {
    /// Window size.
    pub width: f64,
    pub height: f64,
    /// Radial: outer radius of the ring and radius of the centre button.
    pub outer: f64,
    pub inner: f64,
    /// Grid: cell size, gap between cells and padding around the grid.
    pub cell: f64,
    pub gap: f64,
    pub pad: f64,
    pub cols: u8,
    pub rows: u8,
    /// Empty border around the panel inside the window (room for the shadow).
    pub margin: f64,
    /// Settings (gear) button that opens the editor: centre and radius.
    pub gear_x: f64,
    pub gear_y: f64,
    pub gear_r: f64,
}

const MARGIN: f64 = 14.0;
const GEAR_R: f64 = 11.0;
const RADIAL_DIAMETER: f64 = 330.0;
const INNER_RATIO: f64 = 0.3;
const GRID_CELL: f64 = 84.0;
const GRID_GAP: f64 = 8.0;
const GRID_PAD: f64 = 12.0;

/// Panel geometry for a layout at a size scale in percent (100 = default).
pub fn panel_geometry(layout: Layout, size_percent: u16) -> PanelGeometry {
    let s = size_percent.clamp(60, 200) as f64 / 100.0;
    match layout {
        Layout::Radial => {
            let outer = RADIAL_DIAMETER * s / 2.0;
            let side = outer * 2.0 + MARGIN * 2.0;
            PanelGeometry {
                width: side,
                height: side,
                outer,
                inner: outer * INNER_RATIO,
                cell: 0.0,
                gap: 0.0,
                pad: 0.0,
                cols: 3,
                rows: 3,
                margin: MARGIN,
                // Bottom-right corner, outside the ring.
                gear_x: side - MARGIN - 2.0,
                gear_y: side - MARGIN - 2.0,
                gear_r: GEAR_R,
            }
        }
        Layout::Grid { cols, rows } => {
            let (cols, rows) = (cols.clamp(2, 8), rows.clamp(2, 8));
            let cell = GRID_CELL * s;
            let gap = GRID_GAP * s;
            let pad = GRID_PAD * s;
            let w = cols as f64 * cell + (cols as f64 - 1.0) * gap + pad * 2.0;
            let h = rows as f64 * cell + (rows as f64 - 1.0) * gap + pad * 2.0;
            let width = w + MARGIN * 2.0;
            PanelGeometry {
                width,
                height: h + MARGIN * 2.0,
                outer: 0.0,
                inner: 0.0,
                cell,
                gap,
                pad,
                cols,
                rows,
                margin: MARGIN,
                // On the top-right corner of the panel border.
                gear_x: width - MARGIN,
                gear_y: MARGIN,
                gear_r: GEAR_R,
            }
        }
    }
}

/// Window rectangle centred on the cursor, pushed inside the monitor work area.
pub fn place(cursor: (i32, i32), size: (i32, i32), work: Rect) -> Rect {
    let (w, h) = size;
    let clamp = |v: i32, lo: i32, hi: i32| if hi < lo { lo } else { v.clamp(lo, hi) };
    let left = clamp(cursor.0 - w / 2, work.left, work.right - w);
    let top = clamp(cursor.1 - h / 2, work.top, work.bottom - h);
    Rect {
        left,
        top,
        right: left + w,
        bottom: top + h,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Centre of the radial panel (back / close).
    Center,
    Slot(usize),
    /// The settings (gear) button.
    Gear,
    /// Outside of every button.
    None,
}

/// Radial slice from the offset to the panel centre. Slot 0 is at the top, clockwise,
/// each slice spans 45°. Only the angle matters (so fast "flicks" work); `max_radius`
/// limits the distance when set (tap mode).
pub fn radial_hit(dx: f64, dy: f64, inner: f64, max_radius: Option<f64>) -> Hit {
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < inner {
        return Hit::Center;
    }
    if let Some(max) = max_radius {
        if dist > max {
            return Hit::None;
        }
    }
    // Angle measured from "up", clockwise, in degrees [0, 360).
    let angle = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
    Hit::Slot((((angle + 22.5) / 45.0).floor() as usize) % 8)
}

/// Grid cell at a logical point relative to the window's top-left corner.
pub fn grid_hit(x: f64, y: f64, g: &PanelGeometry) -> Hit {
    let gx = x - MARGIN - g.pad;
    let gy = y - MARGIN - g.pad;
    if gx < 0.0 || gy < 0.0 {
        return Hit::None;
    }
    let pitch = g.cell + g.gap;
    let col = (gx / pitch).floor() as usize;
    let row = (gy / pitch).floor() as usize;
    if col >= g.cols as usize || row >= g.rows as usize {
        return Hit::None;
    }
    // In the gap between cells.
    if gx - col as f64 * pitch > g.cell || gy - row as f64 * pitch > g.cell {
        return Hit::None;
    }
    Hit::Slot(row * g.cols as usize + col)
}

/// Whether a logical point hits the gear button.
pub fn gear_hit(x: f64, y: f64, g: &PanelGeometry) -> bool {
    let (dx, dy) = (x - g.gear_x, y - g.gear_y);
    (dx * dx + dy * dy).sqrt() <= g.gear_r + 3.0
}

/// Direction from held arrow keys to a radial slot (Up+Right = top-right...).
pub fn radial_slot_from_arrows(up: bool, down: bool, left: bool, right: bool) -> Option<usize> {
    let dx = (right as i32 - left as i32) as f64;
    let dy = (down as i32 - up as i32) as f64;
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    match radial_hit(dx, dy, 0.0, None) {
        Hit::Slot(s) => Some(s),
        _ => None,
    }
}

/// Moves a grid selection with an arrow key, staying inside the grid.
pub fn grid_move(current: Option<usize>, cols: usize, rows: usize, dx: i32, dy: i32) -> usize {
    let Some(cur) = current else {
        return 0;
    };
    let col = (cur % cols) as i32 + dx;
    let row = (cur / cols) as i32 + dy;
    let col = col.clamp(0, cols as i32 - 1) as usize;
    let row = row.clamp(0, rows as i32 - 1) as usize;
    row * cols + col
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_from_angle() {
        // Straight up, right, down, left.
        assert_eq!(radial_hit(0.0, -100.0, 30.0, None), Hit::Slot(0));
        assert_eq!(radial_hit(100.0, 0.0, 30.0, None), Hit::Slot(2));
        assert_eq!(radial_hit(0.0, 100.0, 30.0, None), Hit::Slot(4));
        assert_eq!(radial_hit(-100.0, 0.0, 30.0, None), Hit::Slot(6));
        // Diagonals.
        assert_eq!(radial_hit(70.0, -70.0, 30.0, None), Hit::Slot(1));
        assert_eq!(radial_hit(-70.0, -70.0, 30.0, None), Hit::Slot(7));
        // Boundaries: 22° is still top, 23° is top-right.
        let a = 22f64.to_radians();
        assert_eq!(
            radial_hit(a.sin() * 100.0, -a.cos() * 100.0, 30.0, None),
            Hit::Slot(0)
        );
        let a = 23f64.to_radians();
        assert_eq!(
            radial_hit(a.sin() * 100.0, -a.cos() * 100.0, 30.0, None),
            Hit::Slot(1)
        );
        // Just left of top wraps to slot 0, not 8.
        let a = 350f64.to_radians();
        assert_eq!(
            radial_hit(a.sin() * 100.0, -a.cos() * 100.0, 30.0, None),
            Hit::Slot(0)
        );
    }

    #[test]
    fn centre_and_flick() {
        assert_eq!(radial_hit(5.0, 5.0, 30.0, None), Hit::Center);
        // Far beyond the ring still selects by angle (flick) ...
        assert_eq!(radial_hit(0.0, -2000.0, 30.0, None), Hit::Slot(0));
        // ... unless a maximum radius is given.
        assert_eq!(radial_hit(0.0, -2000.0, 30.0, Some(165.0)), Hit::None);
    }

    #[test]
    fn placement_clamps_to_work_area() {
        let work = Rect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        // Centred when there is room.
        assert_eq!(
            place((960, 500), (200, 200), work),
            Rect {
                left: 860,
                top: 400,
                right: 1060,
                bottom: 600
            }
        );
        // Top-left corner.
        assert_eq!(
            place((10, 10), (200, 200), work),
            Rect {
                left: 0,
                top: 0,
                right: 200,
                bottom: 200
            }
        );
        // Bottom-right corner.
        assert_eq!(
            place((1919, 1039), (200, 200), work),
            Rect {
                left: 1720,
                top: 840,
                right: 1920,
                bottom: 1040
            }
        );
        // Second monitor to the left with negative coordinates.
        let left_mon = Rect {
            left: -2560,
            top: -200,
            right: 0,
            bottom: 1240,
        };
        assert_eq!(
            place((-5, 0), (400, 400), left_mon),
            Rect {
                left: -400,
                top: -200,
                right: 0,
                bottom: 200
            }
        );
    }

    #[test]
    fn grid_hit_cells_and_gaps() {
        let g = panel_geometry(Layout::Grid { cols: 3, rows: 2 }, 100);
        let origin = MARGIN + g.pad;
        assert_eq!(grid_hit(origin + 1.0, origin + 1.0, &g), Hit::Slot(0));
        let pitch = g.cell + g.gap;
        assert_eq!(
            grid_hit(origin + pitch * 2.0 + 5.0, origin + pitch + 5.0, &g),
            Hit::Slot(5)
        );
        // In the gap.
        assert_eq!(grid_hit(origin + g.cell + 2.0, origin + 5.0, &g), Hit::None);
        // Outside.
        assert_eq!(grid_hit(1.0, 1.0, &g), Hit::None);
        assert_eq!(grid_hit(g.width - 1.0, g.height - 1.0, &g), Hit::None);
    }

    #[test]
    fn gear_is_outside_buttons() {
        let g = panel_geometry(Layout::Radial, 100);
        assert!(gear_hit(g.gear_x, g.gear_y, &g));
        let c = g.width / 2.0;
        let d = ((g.gear_x - c).powi(2) + (g.gear_y - c).powi(2)).sqrt();
        assert!(d > g.outer + g.gear_r);
        let grid = panel_geometry(Layout::Grid { cols: 3, rows: 3 }, 100);
        assert!(gear_hit(grid.gear_x, grid.gear_y, &grid));
        assert_eq!(grid_hit(grid.gear_x, grid.gear_y, &grid), Hit::None);
    }

    #[test]
    fn geometry_scales() {
        let a = panel_geometry(Layout::Radial, 100);
        let b = panel_geometry(Layout::Radial, 200);
        assert!((b.outer - a.outer * 2.0).abs() < 1e-9);
        let g = panel_geometry(Layout::Grid { cols: 8, rows: 8 }, 100);
        assert_eq!(g.cols, 8);
    }

    #[test]
    fn arrows_to_slots() {
        assert_eq!(radial_slot_from_arrows(true, false, false, false), Some(0));
        assert_eq!(radial_slot_from_arrows(true, false, false, true), Some(1));
        assert_eq!(radial_slot_from_arrows(false, true, true, false), Some(5));
        assert_eq!(radial_slot_from_arrows(false, false, false, false), None);
        assert_eq!(grid_move(None, 3, 3, 1, 0), 0);
        assert_eq!(grid_move(Some(0), 3, 3, 1, 0), 1);
        assert_eq!(grid_move(Some(2), 3, 3, 1, 0), 2);
        assert_eq!(grid_move(Some(2), 3, 3, 0, 1), 5);
    }
}
