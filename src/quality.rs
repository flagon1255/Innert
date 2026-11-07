//! Quality presets: Low, Medium, High and Ultra. The first time the game starts one's picked to suit
//! the graphics card, Low for one like a GTX 960, and after that the one chosen in the settings is
//! kept from one run to the next, in %LOCALAPPDATA%\Innert\quality.txt. `--set quality=low` and so on
//! tries one out for a run without keeping it.

use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use crate::shafts;

/// The preset that suits the graphics card, once it's been picked, see set_card()
static CARD: (Mutex<Option<Preset>>, Condvar) = (Mutex::new(None), Condvar::new());

/// Says which preset suits the graphics card picked, see suited()
pub fn set_card(preset: Preset) {
    let (card, picked) = &CARD;
    *card.lock().unwrap_or_else(|e| e.into_inner()) = Some(preset);
    picked.notify_all();
}

/// The preset to play with: `--set quality=low` and so on if given, else the one chosen in an
/// earlier run, else the one that suits the graphics card, waiting for it to be picked, as the
/// world's started on another thread meanwhile. Screenshot runs go by the card alone, so they come
/// out the same whatever was chosen.
pub fn chosen() -> Preset {
    let args: Vec<String> = std::env::args().collect();
    let asked = args.windows(2).filter(|w| w[0] == "--set").filter_map(|w| w[1].strip_prefix("quality=")).last().and_then(Preset::parse);
    if let Some(preset) = asked {
        return preset;
    }
    if !args.iter().any(|a| a == "--screenshot")
        && let Some(preset) = kept()
    {
        return preset;
    }
    let (card, picked) = &CARD;
    let guard = card.lock().unwrap_or_else(|e| e.into_inner());
    let (guard, _) = picked.wait_timeout_while(guard, Duration::from_secs(20), |c| c.is_none()).unwrap_or_else(|e| e.into_inner());
    guard.unwrap_or(Preset::High)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Low,
    Medium,
    High,
    Ultra,
}

/// What a preset sets
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// Of the window's resolution, moving or still
    pub render_scale: f32,
    pub shadows: bool,
    pub ambient_occlusion: bool,
    pub bounce_light: bool,
    pub smooth_shading: bool,
    pub wind: bool,
    pub denoise: bool,
    pub bloom: f32,
    /// How the sun shafts in the haze are drawn, see shafts.rs
    pub sun_shafts: shafts::Quality,
    /// How far off things fade into the sky, as a part of as far as the world reaches
    pub view_distance: f32,
    /// Whether water and metal mirror the world around them, rather than just the sky
    pub reflections: bool,
    /// The cheaper way of building the shown image from the rendered one, see shaders\upscale.comp
    pub fast_upscale: bool,
    /// How far across, in meters, the water flows around the camera; further off it keeps still
    /// till the camera comes near
    pub flowing_water_m: f32,
    /// How wide the piece of the landscape kept around the player is wanted, in meters, which
    /// memory.rs narrows if the card hasn't room for it; one sees about half of it
    pub world_width_m: u32,
    /// How many voxels to a meter the world's built with. Fewer would take a quarter of the memory
    /// and be quicker to draw, but blockier, see the screenshots of Low at 8 a meter.
    pub voxels_per_meter: u32,
}

impl Preset {
    pub const ALL: [Preset; 4] = [Preset::Low, Preset::Medium, Preset::High, Preset::Ultra];

    pub fn name(self) -> &'static str {
        match self {
            Preset::Low => "Low",
            Preset::Medium => "Medium",
            Preset::High => "High",
            Preset::Ultra => "Ultra",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Preset::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(text.trim()))
    }

    pub fn settings(self) -> Settings {
        // High is how the game has always looked. Low is for 60 frames a second at 1080p on a
        // GTX 960, a quarter as fast as an RTX 4050 laptop's.
        let high = Settings {
            render_scale: 0.45,
            shadows: true,
            ambient_occlusion: true,
            bounce_light: true,
            smooth_shading: true,
            wind: true,
            denoise: true,
            bloom: 0.3,
            sun_shafts: shafts::Quality::High,
            view_distance: 1.0,
            reflections: true,
            fast_upscale: false,
            flowing_water_m: 64.0,
            world_width_m: 224,
            voxels_per_meter: 16,
        };
        match self {
            Preset::Low => Settings {
                render_scale: 0.33,
                wind: false,
                bloom: 0.0,
                // The sky smeared towards the sun, a tenth of a millisecond on a 4050 laptop
                sun_shafts: shafts::Quality::Low,
                // Nearer than this the mountains fade away, which shows more than it saves
                view_distance: 1.0,
                reflections: false,
                fast_upscale: true,
                flowing_water_m: 32.0,
                ..high
            },
            Preset::Medium => Settings {
                render_scale: 0.4,
                sun_shafts: shafts::Quality::Low,
                fast_upscale: true,
                flowing_water_m: 48.0,
                ..high
            },
            Preset::High => high,
            // At the window's own resolution, for cards a few times faster than a 4050 laptop's
            Preset::Ultra => Settings { render_scale: 1.0, ..high },
        }
    }
}

/// The preset that suits a graphics card, from what it's called, whose it is and how much memory it
/// has of its own: `vendor` is its PCI vendor id, `discrete` whether it's a card of its own rather
/// than part of the processor
pub fn suited(name: &str, vendor: u32, discrete: bool, memory: u64) -> Preset {
    const NVIDIA: u32 = 0x10de;
    const AMD: u32 = 0x1002;
    let name = name.to_ascii_lowercase();
    // The first number in the name past a word like "rtx", so "geforce rtx 4050 laptop gpu" is 4050
    let model = |after: &str| -> Option<u32> {
        let at = name.find(after)? + after.len();
        let digits: String = name[at..].trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().ok()
    };
    let by_name = if !discrete {
        Preset::Low
    } else if vendor == NVIDIA {
        if let Some(n) = model("rtx") {
            // The series and how far up it the card is: 4050 is series 40, tier 50
            let tier = n % 100;
            match tier {
                // An RTX 2050 is a laptop's, about as fast as a GTX 1650
                0..=49 => Preset::Medium,
                50 if n / 100 == 20 => Preset::Medium,
                50..=69 => Preset::High,
                _ if tier >= 80 || (tier >= 70 && name.contains(" ti")) => Preset::Ultra,
                _ => Preset::High,
            }
        } else if let Some(n) = model("gtx") {
            // A GTX 1060 or 1660 and up manages more than Low
            if n >= 1060 { Preset::Medium } else { Preset::Low }
        } else {
            Preset::Low
        }
    } else if vendor == AMD {
        if let Some(n) = model("rx") {
            match n {
                // RX 460 to 590
                0..=599 => Preset::Low,
                // RX 5500 to 5700, 6400 to 6600, 7600
                5000..=5599 | 6000..=6599 => Preset::Medium,
                6600..=6799 | 5600..=5999 | 7000..=7699 => Preset::High,
                _ => Preset::Ultra,
            }
        } else {
            Preset::Low
        }
    } else if name.contains("arc") {
        Preset::Medium
    } else {
        Preset::Low
    };
    // Whatever it's called, the bigger presets need the memory for them
    let gb = memory as f64 / (1u64 << 30) as f64;
    let by_memory = if gb < 3.5 {
        Preset::Low
    } else if gb < 5.5 {
        Preset::Medium
    } else if gb < 9.5 {
        Preset::High
    } else {
        Preset::Ultra
    };
    let rank = |p: Preset| Preset::ALL.iter().position(|&q| q == p).unwrap();
    if rank(by_memory) < rank(by_name) { by_memory } else { by_name }
}

fn kept_file() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Innert").join("quality.txt")
}

/// The preset chosen in an earlier run, if one was
pub fn kept() -> Option<Preset> {
    Preset::parse(&std::fs::read_to_string(kept_file()).ok()?)
}

/// Keeps `preset` for the next run
pub fn keep(preset: Preset) {
    let file = kept_file();
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, preset.name());
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1 << 30;

    #[test]
    fn cards_get_the_presets_that_suit_them() {
        assert_eq!(suited("NVIDIA GeForce GTX 960", 0x10de, true, 2 * GB), Preset::Low);
        assert_eq!(suited("NVIDIA GeForce GTX 960", 0x10de, true, 4 * GB), Preset::Low);
        assert_eq!(suited("NVIDIA GeForce GTX 1060 6GB", 0x10de, true, 6 * GB), Preset::Medium);
        assert_eq!(suited("NVIDIA GeForce RTX 4050 Laptop GPU", 0x10de, true, 6 * GB), Preset::High);
        assert_eq!(suited("NVIDIA GeForce RTX 3050", 0x10de, true, 8 * GB), Preset::High);
        assert_eq!(suited("NVIDIA GeForce RTX 4090", 0x10de, true, 24 * GB), Preset::Ultra);
        assert_eq!(suited("NVIDIA GeForce RTX 4070 Ti SUPER", 0x10de, true, 16 * GB), Preset::Ultra);
        assert_eq!(suited("AMD Radeon RX 470", 0x1002, true, 4 * GB), Preset::Low);
        assert_eq!(suited("AMD Radeon RX 6600", 0x1002, true, 8 * GB), Preset::High);
        assert_eq!(suited("Intel(R) Iris(R) Xe Graphics", 0x8086, false, 2 * GB), Preset::Low);
        assert_eq!(suited("Intel(R) Arc(TM) A770 Graphics", 0x8086, true, 16 * GB), Preset::Medium);
        // However fast, without the memory for more
        assert_eq!(suited("NVIDIA GeForce RTX 3080", 0x10de, true, 4 * GB), Preset::Medium);
    }

    #[test]
    fn presets_read_back_by_name() {
        for p in Preset::ALL {
            assert_eq!(Preset::parse(p.name()), Some(p));
            assert_eq!(Preset::parse(&p.name().to_lowercase()), Some(p));
        }
        assert_eq!(Preset::parse("nonsense"), None);
    }

    #[test]
    fn lower_presets_draw_fewer_pixels() {
        let scales: Vec<f32> = Preset::ALL.iter().map(|p| p.settings().render_scale).collect();
        assert!(scales.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(Preset::High.settings().render_scale, 0.45);
    }
}
