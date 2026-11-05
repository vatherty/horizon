//! First-run defaults for phones and tablets (the phone app).
//!
//! Vanilla's defaults are made for a desktop. On the first start, before the game
//! reads its settings, the phone app writes `options.txt` and
//! `horizon-graphics.txt` with the lighter ones below, but only where the file does
//! not exist yet: whatever the player changes in the options is theirs from then
//! on. A desktop never calls this, so its defaults are untouched.
//!
//! What was chosen, and why (docs: `ANDROID.md`):
//!
//! | Setting | Desktop | Mobile | Why |
//! | --- | --- | --- | --- |
//! | render distance | 12 | 6 | chunk meshes and their GPU memory grow with its square |
//! | simulation distance | 12 | 5 (vanilla's minimum) | mob and block ticking |
//! | max FPS | 120 | 60 | heat and battery; the limiter lowers it further when the phone warms up |
//! | mipmap levels | 4 | 2 | texture memory and bandwidth |
//! | biome blend | 2 | 1 | meshing time |
//! | particles | all | decreased | fill rate |
//! | entity distance | 100% | 75% | draw calls |
//! | entity shadows | on | off | extra passes |
//! | clouds | fancy | fast | fill rate |
//! | menu blur | 5 | 2 | a full-screen blur is expensive on a mobile GPU |
//! | auto jump | off | on | there is no key to hold for a jump on a touch screen |
//! | battery mode | off | on | 60 FPS in a world, 30 in menus, half the worker threads |
use std::{fs, io, path::Path};

use crate::graphics::{self, Graphics, Preset};
use crate::options_file::{OptionsFile, V};

/// The options a phone starts with: vanilla's with the changes in the table above.
pub fn options() -> OptionsFile {
    let mut f = OptionsFile::default();
    f.set("renderDistance", V::Int(6));
    f.set("simulationDistance", V::Int(5));
    f.set("maxFps", V::Int(60));
    f.set("mipmapLevels", V::Int(2));
    f.set("biomeBlendRadius", V::Int(1));
    f.set("particles", V::Int(1));
    f.set("entityDistanceScaling", V::Double(0.75));
    f.set("entityShadows", V::Bool(false));
    f.set("menuBackgroundBlurriness", V::Int(2));
    f.set("autoJump", V::Bool(true));
    f.set_name("renderClouds", "fast");
    f
}

/// Horizon's own graphics settings for a phone: battery mode on, the rest as on the desktop.
pub fn graphics() -> Graphics {
    Graphics {
        preset: Preset::Custom,
        battery: true,
        ..Graphics::default()
    }
}

/// Writes the defaults into the game folder where the files are missing. Returns
/// the names of the files it wrote.
pub fn seed(dir: &Path) -> io::Result<Vec<&'static str>> {
    fs::create_dir_all(dir)?;
    let mut written = Vec::new();
    let options = dir.join("options.txt");
    if !options.exists() {
        fs::write(&options, self::options().serialize())?;
        written.push("options.txt");
    }
    let graphics_file = dir.join(graphics::FILE);
    if !graphics_file.exists() {
        fs::write(&graphics_file, self::graphics().serialize())?;
        written.push(graphics::FILE);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn the_options_are_lighter_than_the_desktop_ones() {
        let desktop = Settings::default();
        let phone = Settings::from_file(options());
        assert_eq!(phone.render_distance, 6);
        assert!(phone.render_distance < desktop.render_distance);
        assert_eq!(phone.simulation_distance, 5);
        assert_eq!(phone.max_fps, 60);
        assert_eq!(phone.mipmap_levels, 2);
        assert_eq!(phone.biome_blend, 1);
        assert_eq!(phone.particles, 1);
        assert!(!phone.entity_shadows);
        assert!(phone.auto_jump);
        assert_eq!(phone.clouds, "fast");
        assert!((phone.entity_distance - 0.75).abs() < 1e-6);
        assert_eq!(phone.menu_blur, 2);
        // What the changes did not name is vanilla's.
        assert_eq!(phone.fov, desktop.fov);
        assert_eq!(phone.gui_scale, 0, "the GUI scale stays automatic");
        assert!(phone.vsync);
    }

    #[test]
    fn the_phone_graphics_turn_battery_mode_on() {
        let g = Graphics::parse(&graphics().serialize());
        assert!(g.battery);
        assert_eq!(g.resolution_scale, 100);
        assert_eq!(g.preset, Preset::Custom);
    }

    #[test]
    fn seeding_writes_missing_files_once() {
        let dir = std::env::temp_dir().join(format!("horizon-mobile-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(seed(&dir).unwrap(), ["options.txt", graphics::FILE]);
        assert!(seed(&dir).unwrap().is_empty(), "nothing is overwritten");
        // The player's own choice survives another start.
        fs::write(dir.join("options.txt"), "version:0\nrenderDistance:20\n").unwrap();
        assert!(seed(&dir).unwrap().is_empty());
        let text = fs::read_to_string(dir.join("options.txt")).unwrap();
        assert!(text.contains("renderDistance:20"));
        let _ = fs::remove_dir_all(&dir);
    }
}
