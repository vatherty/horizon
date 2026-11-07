//! Shared touch HUD painter. The standalone preview uses this same geometry.
use horizon_client::{
    hud::{HudAtlas, HudBatch, rgba},
    lang,
    touch::{Button, View},
};

pub(super) fn draw(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    view: &View,
    playing: bool,
    sneak: bool,
    sprint: bool,
    flying: bool,
) {
    let s = batch.scale;
    if playing {
        let stick = view.stick;
        let (cx, cy, r) = (stick.base.0 / s, stick.base.1 / s, stick.radius / s);
        round_rect(
            batch,
            atlas,
            cx - r,
            cy - r,
            2.0 * r,
            2.0 * r,
            r,
            rgba(225, 235, 240, 155),
        );
        round_rect(
            batch,
            atlas,
            cx - r + 0.7,
            cy - r + 0.7,
            2.0 * r - 1.4,
            2.0 * r - 1.4,
            r,
            rgba(25, 30, 33, 100),
        );
        let (kx, ky, kr) = (stick.knob.0 / s, stick.knob.1 / s, r * 0.36);
        round_rect(
            batch,
            atlas,
            kx - kr,
            ky - kr,
            2.0 * kr,
            2.0 * kr,
            kr,
            rgba(235, 242, 245, 190),
        );
        round_rect(
            batch,
            atlas,
            kx - kr + 0.7,
            ky - kr + 0.7,
            2.0 * kr - 1.4,
            2.0 * kr - 1.4,
            kr,
            rgba(190, 202, 205, if stick.active { 210 } else { 145 }),
        );
    }
    for b in &view.buttons {
        let r = b.rect;
        let (x, y, w, h) = (r.x / s, r.y / s, r.w / s, r.h / s);
        let active = b.lit
            || match b.button {
                Button::Sneak => sneak,
                Button::Sprint => sprint,
                Button::FlyToggle => flying,
                _ => false,
            };
        let radius = (h * 0.19).min(5.0);
        round_rect(
            batch,
            atlas,
            x,
            y,
            w,
            h,
            radius,
            if active {
                rgba(235, 255, 235, 230)
            } else {
                rgba(213, 226, 232, 165)
            },
        );
        round_rect(
            batch,
            atlas,
            x + 0.65,
            y + 0.65,
            w - 1.3,
            h - 1.3,
            radius - 0.5,
            if active {
                rgba(54, 88, 62, 210)
            } else {
                rgba(24, 30, 35, 165)
            },
        );
        let label = match b.button {
            Button::MenuHotbar(slot) => (slot + 1).to_string(),
            _ => lang::get(b.button.label_key()),
        };
        let ink = rgba(248, 251, 252, 255);
        // Primary controls get recognizable pixel icons and a text label. Utility
        // keys use their full plate for text so translations remain readable.
        if let Some(rows) = icon(b.button) {
            let size = (h * 0.47).min(w * 0.52);
            let pixel = size / 12.0;
            let ix = x + (w - size) / 2.0;
            let iy = y + h * 0.13;
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..12 {
                    if bits & (1 << (11 - col)) != 0 {
                        batch.fill(
                            atlas,
                            ix + col as f32 * pixel,
                            iy + row as f32 * pixel,
                            pixel + 0.02,
                            pixel + 0.02,
                            ink,
                        );
                    }
                }
            }
            label_fit(
                batch,
                atlas,
                &label,
                x + 1.0,
                y + h * 0.73,
                w - 2.0,
                (h * 0.20).min(8.0),
                ink,
            );
        } else {
            label_fit(
                batch,
                atlas,
                &label,
                x + 1.0,
                y + (h - 8.0) / 2.0,
                w - 2.0,
                8.0,
                ink,
            );
        }
    }
}

fn label_fit(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    ink: u32,
) {
    let width = HudBatch::text_width(atlas, text).max(1.0);
    let scale = (w / width).min(h / 8.0).min(1.0);
    let start = batch.vertices.len();
    batch.text(atlas, text, x + (w - width * scale) / 2.0, y, ink, true);
    batch.scale_since(start, x + (w - width * scale) / 2.0, y, scale, scale);
}

/// Original twelve-pixel glyphs, independent of resource packs and Unicode fonts.
fn icon(button: Button) -> Option<[u16; 12]> {
    Some(match button {
        Button::Break => [
            0x1f0, 0x3fc, 0x706, 0x602, 0x060, 0x0e0, 0x1c0, 0x380, 0x700, 0xe00, 0xc00, 0x000,
        ],
        Button::Place => [
            0x060, 0x198, 0x606, 0x996, 0x966, 0x906, 0x906, 0x966, 0x198, 0x066, 0x00f, 0x006,
        ],
        Button::Jump => [
            0, 0x060, 0x0f0, 0x198, 0x30c, 0x606, 0, 0x060, 0x0f0, 0x198, 0x30c, 0x606,
        ],
        Button::Sneak => [
            0x030, 0x078, 0x030, 0x1e0, 0x3c0, 0x660, 0x030, 0x1f0, 0x330, 0x630, 0x600, 0,
        ],
        Button::Sprint => [
            0x018, 0x03c, 0x018, 0x0f0, 0x3b0, 0x630, 0x060, 0x0d8, 0x198, 0x318, 0x600, 0,
        ],
        Button::FlyUp | Button::ScrollUp => [
            0, 0x060, 0x0f0, 0x1f8, 0x36c, 0x666, 0x060, 0x060, 0x060, 0x060, 0, 0,
        ],
        Button::FlyDown | Button::Drop | Button::ScrollDown | Button::MenuDrop => [
            0, 0x060, 0x060, 0x060, 0x060, 0x060, 0x666, 0x36c, 0x1f8, 0x0f0, 0x060, 0,
        ],
        Button::Inventory => [
            0, 0x79e, 0x492, 0x492, 0x79e, 0, 0, 0x79e, 0x492, 0x492, 0x79e, 0,
        ],
        Button::Chat => [
            0, 0x7fe, 0xc03, 0xc03, 0xc03, 0xc03, 0xc03, 0x7fe, 0x180, 0x300, 0x200, 0,
        ],
        Button::Pause => [
            0, 0x198, 0x198, 0x198, 0x198, 0x198, 0x198, 0x198, 0x198, 0x198, 0, 0,
        ],
        Button::Perspective => [
            0x060, 0x0f0, 0x7fe, 0x402, 0x462, 0x4f2, 0x492, 0x4f2, 0x462, 0x402, 0x7fe, 0,
        ],
        Button::SwapHands | Button::MenuSwapHands => [
            0, 0x00c, 0x006, 0x7ff, 0x006, 0x00c, 0x300, 0x600, 0xffe, 0x600, 0x300, 0,
        ],
        Button::PickBlock | Button::MenuClone => [
            0xe07, 0x801, 0x060, 0x198, 0x606, 0x666, 0x666, 0x606, 0x198, 0x060, 0x801, 0xe07,
        ],
        Button::FlyToggle => [
            0x060, 0x0f0, 0x1f8, 0x36c, 0x060, 0, 0, 0x060, 0x36c, 0x1f8, 0x0f0, 0x060,
        ],
        _ => return None,
    })
}

/// Rounded plates from one-pixel rows in GUI coordinates.
fn round_rect(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: u32,
) {
    let radius = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    for i in 0..h.ceil() as i32 {
        let mid = i as f32 + 0.5;
        let edge = mid.min(h - mid);
        let inset = if edge < radius {
            radius
                - (radius * radius - (radius - edge) * (radius - edge))
                    .max(0.0)
                    .sqrt()
        } else {
            0.0
        };
        batch.fill(
            atlas,
            x + inset,
            y + i as f32,
            (w - 2.0 * inset).max(0.0),
            1.0_f32.min(h - i as f32).max(0.0),
            color,
        );
    }
}
