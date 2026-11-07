//! Dependency-free preview of the production touch HUD drawing and hit rectangles.
//!
//! Run from the repository root:
//! `rustc --edition=2024 tools/preview_touch_hud.rs -o /tmp/preview-touch-hud`
//! `/tmp/preview-touch-hud /tmp/touch-hud.svg`
//! `/tmp/preview-touch-hud /tmp/touch-inventory.svg --menu`
//!
//! This is a layout preview, not a captured game frame. It uses the production
//! control arrangement, icons, fills and labels, with an SVG recorder replacing
//! the GPU batch. Text uses a monospace substitute for the player's resource-pack
//! font; the neutral backdrop and numbered hotbar placeholders are preview-only.

#![allow(dead_code)]

extern crate self as horizon_client;

#[path = "../crates/horizon-client/src/game/touch/draw.rs"]
mod production_draw;
#[path = "../crates/horizon-client/src/touch.rs"]
pub mod touch;

pub mod lang {
    const ENGLISH: &str = include_str!("../crates/horizon-client/assets/horizon/lang/en_us.json");

    pub fn get(key: &str) -> String {
        // The bundled catalogue has one JSON string pair per line. Preview
        // labels contain no escape sequences; a missing key stays visible.
        let prefix = format!("\"{key}\":");
        ENGLISH
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix(&prefix)
                    .and_then(|value| value.trim().strip_prefix('"'))
                    .and_then(|value| value.split('"').next())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| key.to_owned())
    }

    pub fn get_or(key: &str, fallback: &str) -> String {
        let value = get(key);
        if value == key { fallback.into() } else { value }
    }
}

pub mod hud {
    use std::fmt::Write;

    pub struct HudAtlas;

    pub enum Draw {
        Rect {
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            color: u32,
        },
        Text {
            x: f32,
            y: f32,
            sx: f32,
            sy: f32,
            value: String,
            color: u32,
        },
    }

    pub struct HudBatch {
        pub vertices: Vec<Draw>,
        pub scale: f32,
        pub width: f32,
        pub height: f32,
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
        u32::from(a) << 24 | u32::from(b) << 16 | u32::from(g) << 8 | u32::from(r)
    }

    impl HudBatch {
        pub fn fill(&mut self, _: &HudAtlas, x: f32, y: f32, w: f32, h: f32, color: u32) {
            self.vertices.push(Draw::Rect { x, y, w, h, color });
        }

        pub fn text_width(_: &HudAtlas, text: &str) -> f32 {
            text.chars().count() as f32 * 6.0
        }

        pub fn text(
            &mut self,
            atlas: &HudAtlas,
            value: &str,
            x: f32,
            y: f32,
            color: u32,
            shadow: bool,
        ) -> f32 {
            if shadow {
                self.vertices.push(Draw::Text {
                    x: x + 1.0,
                    y: y + 1.0,
                    sx: 1.0,
                    sy: 1.0,
                    value: value.into(),
                    color: rgba(0, 0, 0, (color >> 24) as u8),
                });
            }
            self.vertices.push(Draw::Text {
                x,
                y,
                sx: 1.0,
                sy: 1.0,
                value: value.into(),
                color,
            });
            Self::text_width(atlas, value)
        }

        pub fn scale_since(&mut self, start: usize, cx: f32, cy: f32, sx: f32, sy: f32) {
            for draw in &mut self.vertices[start..] {
                match draw {
                    Draw::Rect { x, y, w, h, .. } => {
                        *x = cx + (*x - cx) * sx;
                        *y = cy + (*y - cy) * sy;
                        *w *= sx;
                        *h *= sy;
                    }
                    Draw::Text {
                        x,
                        y,
                        sx: tx,
                        sy: ty,
                        ..
                    } => {
                        *x = cx + (*x - cx) * sx;
                        *y = cy + (*y - cy) * sy;
                        *tx *= sx;
                        *ty *= sy;
                    }
                }
            }
        }

        pub fn svg(&self) -> String {
            let mut out = String::new();
            for draw in &self.vertices {
                let color = match draw {
                    Draw::Rect { color, .. } | Draw::Text { color, .. } => *color,
                };
                let ink = format!(
                    "#{:02x}{:02x}{:02x}",
                    color & 255,
                    color >> 8 & 255,
                    color >> 16 & 255
                );
                let opacity = (color >> 24) as f32 / 255.0;
                match draw {
                    Draw::Rect { x, y, w, h, .. } => {
                        writeln!(out, "<rect x='{x:.3}' y='{y:.3}' width='{w:.3}' height='{h:.3}' fill='{ink}' opacity='{opacity:.3}'/>").unwrap();
                    }
                    Draw::Text {
                        x,
                        y,
                        sx,
                        sy,
                        value,
                        ..
                    } => {
                        writeln!(out, "<text transform='translate({x:.3} {y:.3}) scale({sx:.3} {sy:.3})' y='8' font-family='monospace' font-size='9.6' fill='{ink}' opacity='{opacity:.3}'>{}</text>", super::escape(value)).unwrap();
                    }
                }
            }
            out
        }
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::fmt::Write;
    use touch::{Insets, Layout, Mode, Rect, Screen, TouchControls};

    let args: Vec<String> = std::env::args().skip(1).collect();
    let output = args.first().map_or("/tmp/touch-hud.svg", String::as_str);
    let menu = args.iter().any(|arg| arg == "--menu");
    let dimensions = args
        .iter()
        .position(|arg| arg == "--size")
        .and_then(|i| args.get(i + 1))
        .and_then(|value| value.split_once('x'))
        .map(|(w, h)| Ok::<_, std::num::ParseFloatError>((w.parse::<f32>()?, h.parse::<f32>()?)))
        .transpose()?
        .unwrap_or((2532.0, 1170.0));
    let (width, height) = dimensions;
    let density = args
        .iter()
        .position(|arg| arg == "--density")
        .and_then(|i| args.get(i + 1))
        .map(|value| value.parse::<f32>())
        .transpose()?
        .unwrap_or(3.0);
    if !width.is_finite()
        || !height.is_finite()
        || !density.is_finite()
        || width < 320.0
        || height < 320.0
        || density <= 0.0
    {
        return Err("preview requires dimensions >=320 and positive density".into());
    }
    let mut scale = 1.0;
    while width / (scale + 1.0) >= 320.0 && height / (scale + 1.0) >= 240.0 {
        scale += 1.0;
    }
    let insets = Insets {
        left: 44.0 * density,
        right: 44.0 * density,
        top: 0.0,
        bottom: 21.0 * density,
    };
    // Match HudBatch's GUI rounding and the game's home-indicator reservation.
    let gui_width = (width / scale).ceil();
    let gui_height = (height / scale).ceil();
    let bottom_gui = (insets.bottom.max(Layout::default().margin * density) / scale).ceil();
    let hotbar = Rect {
        x: (gui_width / 2.0 - 91.0) * scale,
        y: (gui_height - bottom_gui - 22.0) * scale,
        w: 182.0 * scale,
        h: 22.0 * scale,
    };
    let screen = Screen {
        width,
        height,
        density,
        hotbar,
        slot: 20.0 * scale,
        insets,
    };
    let mut controls = TouchControls::new(Layout::default(), screen);
    if menu {
        controls.set_mode(Mode::Inventory);
    }
    let view = controls.view();
    let mut batch = hud::HudBatch {
        vertices: Vec::new(),
        scale,
        width: gui_width,
        height: gui_height,
    };
    production_draw::draw(
        &mut batch,
        &hud::HudAtlas,
        &view,
        !menu,
        false,
        false,
        false,
    );

    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'>\n<title>Touch HUD production layout preview</title>\n<desc>Layout and icons from production Rust; neutral backdrop, placeholder hotbar and substitute font. This is not a game screenshot.</desc>\n<defs><linearGradient id='backdrop' x2='0' y2='1'><stop stop-color='#7897ab'/><stop offset='1' stop-color='#56685b'/></linearGradient></defs>\n<rect width='100%' height='100%' fill='url(#backdrop)'/>\n"
    );
    writeln!(
        svg,
        "<path d='M 0 {} H {width}' stroke='#99ada2' opacity='.35' stroke-width='2'/>",
        height * 0.48
    )?;
    if menu {
        let (cw, ch) = (176.0 * scale, 166.0 * scale);
        let (cx, cy) = ((width - cw) / 2.0, (height - ch) / 2.0);
        writeln!(
            svg,
            "<rect x='{cx}' y='{cy}' width='{cw}' height='{ch}' fill='#c3c3c3' stroke='#333' stroke-width='6'/>"
        )?;
        for row in 0..3 {
            for col in 0..9 {
                let x = cx + (8.0 + col as f32 * 18.0) * scale;
                let y = cy + (84.0 + row as f32 * 18.0) * scale;
                writeln!(
                    svg,
                    "<rect x='{x}' y='{y}' width='{}' height='{}' fill='#777' stroke='#ddd' stroke-width='2'/>",
                    16.0 * scale,
                    16.0 * scale
                )?;
            }
        }
    } else {
        writeln!(
            svg,
            "<path d='M {} {} h 32 M {} {} v 32' fill='none' stroke='white' stroke-width='4'/>",
            width / 2.0 - 16.0,
            height / 2.0,
            width / 2.0,
            height / 2.0 - 16.0
        )?;
        writeln!(
            svg,
            "<rect x='{}' y='{}' width='{}' height='{}' fill='#1d2628' stroke='#b6c1bb' stroke-width='4'/>",
            hotbar.x, hotbar.y, hotbar.w, hotbar.h
        )?;
        for slot in 0..9 {
            let x = hotbar.x + (1.0 + slot as f32 * 20.0) * scale;
            writeln!(
                svg,
                "<rect x='{x}' y='{}' width='{}' height='{}' fill='#39453c' stroke='#828d81' stroke-width='3'/><text x='{}' y='{}' fill='#e8ede8' font-family='monospace' font-size='{}' text-anchor='middle'>{}</text>",
                hotbar.y + scale,
                20.0 * scale,
                20.0 * scale,
                x + 10.0 * scale,
                hotbar.y + 14.0 * scale,
                9.0 * scale,
                slot + 1
            )?;
        }
    }
    writeln!(
        svg,
        "<g transform='scale({scale})' shape-rendering='crispEdges'>{}</g>",
        batch.svg()
    )?;
    writeln!(
        svg,
        "<text x='{}' y='{}' text-anchor='middle' font-family='monospace' font-size='19' fill='#dce6df' opacity='.7'>Production HUD layout preview · substitute font · placeholder hotbar</text>\n</svg>",
        width / 2.0,
        32.0
    )?;
    std::fs::write(output, svg)?;
    println!(
        "Wrote {output} ({} controls; {}×{} physical pixels)",
        view.buttons.len(),
        width,
        height
    );
    Ok(())
}
