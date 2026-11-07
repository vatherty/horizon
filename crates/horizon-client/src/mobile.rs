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
//! | mipmap levels | 4 | 2 | sampled LOD limit; the full texture chain is still allocated |
//! | biome blend | 2 | 1 | meshing time |
//! | particles | all | decreased | fill rate |
//! | entity distance | 100% | 75% | draw calls |
//! | entity shadows | on | off | extra passes |
//! | clouds | fancy | fast | fill rate |
//! | menu blur | 5 | 2 | a full-screen blur is expensive on a mobile GPU |
//! | auto jump | off | on | there is no key to hold for a jump on a touch screen |
//! | fullscreen | off | on | a phone's window fills the screen; asking for the desktop's windowed size instead made the surface a size the panel is not (3840x2100 on a 2532x1170 phone), so the GPU rendered pixels that cannot exist |
//! | battery mode | off | on | 60 FPS in a world, 30 in menus, half the worker threads — and on iOS it caps the world's render at 720p class (1280 px long edge) |
//! | world resolution | 100% | 75% | roughly 56% of native world pixels; HUD stays native. On iOS the render is capped at 1080p class (1920 px) even at 100%: 720p is the norm there, 1080p the push |
//!
//! Resolution scaling adds an intermediate target and a copy. It is not a
//! guaranteed FPS or RAM improvement. The texture budget limits top-mip color
//! data, not all texture, Metal, or process memory.
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
    f.set("fullscreen", V::Bool(true));
    f.set_name("renderClouds", "fast");
    f
}

/// Horizon's phone settings: battery mode and a smaller world target, with native HUD.
pub fn graphics() -> Graphics {
    Graphics {
        preset: Preset::Custom,
        battery: true,
        resolution_scale: 75,
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
        assert!(phone.fullscreen);
        assert!(!desktop.fullscreen);
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
        assert_eq!(g.resolution_scale, 75);
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
        fs::write(
            dir.join(graphics::FILE),
            "battery:false\nresolutionScale:90\n",
        )
        .unwrap();
        assert!(seed(&dir).unwrap().is_empty());
        let text = fs::read_to_string(dir.join("options.txt")).unwrap();
        assert!(text.contains("renderDistance:20"));
        assert_eq!(
            fs::read_to_string(dir.join(graphics::FILE)).unwrap(),
            "battery:false\nresolutionScale:90\n"
        );
        // An existing options file does not prevent seeding a missing graphics
        // file, and the existing file is preserved byte-for-byte.
        fs::remove_file(dir.join(graphics::FILE)).unwrap();
        assert_eq!(seed(&dir).unwrap(), [graphics::FILE]);
        assert_eq!(
            fs::read_to_string(dir.join("options.txt")).unwrap(),
            "version:0\nrenderDistance:20\n"
        );
        assert_eq!(
            Graphics::parse(&fs::read_to_string(dir.join(graphics::FILE)).unwrap())
                .resolution_scale,
            75
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
