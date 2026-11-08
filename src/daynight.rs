//! The day going round: a long day, a dusk, a night and a dawn, about twenty minutes in all. The
//! sun and the moon go round the sky, and the sky's color and the light the world's lit by follow
//! them: the sun's by day, golden low in the sky, and the moon's at night, dim and blue, but enough
//! to see by. Monsters come at night, see raids.rs.
//!
//! What's drawn is moved on in small steps, a few tenths of a degree every second or two, never
//! every frame: the light the renderer keeps, how much of the sun reaches each face and the light
//! bounced onto it (see LightCache in shaders\traverse.comp), what each pixel's averaged over
//! frames, and the map of where sunlight stops for sun shafts (see shafts.rs), are all kept for the
//! sun where it is, and each step only nudges them along rather than throwing them away. The light
//! bounced onto faces is kept relative to how bright the day is, see Sky::cache_level, so it follows
//! the dusk without being gathered again.
//!
//! The clock goes by hours, 0 to 24, `--set time=21.5` sets it; `--set day_cycle=0` holds it still.

use std::f64::consts::{PI, TAU};

use egui_winit_vulkano::egui::{self, Align2, Color32, FontId, Pos2, Stroke, pos2, vec2};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};

/// Real seconds each part of the day lasts: the whole of it about twenty minutes, the night five
/// or six of them
pub const DAWN_SECONDS: f64 = 90.0;
pub const DAY_SECONDS: f64 = 660.0;
pub const DUSK_SECONDS: f64 = 90.0;
pub const NIGHT_SECONDS: f64 = 360.0;
pub const CYCLE_SECONDS: f64 = DAWN_SECONDS + DAY_SECONDS + DUSK_SECONDS + NIGHT_SECONDS;

/// Where each part starts on the clock, in hours
pub const DAWN_AT: f64 = 5.0;
pub const DAY_AT: f64 = 6.5;
pub const DUSK_AT: f64 = 18.0;
pub const NIGHT_AT: f64 = 19.5;
/// When the sun comes up and goes down, in hours: halfway through the dawn and the dusk
pub const SUNRISE: f64 = 5.75;
pub const SUNSET: f64 = 18.75;
/// Where a new world's day starts, in hours: just after dawn, for the longest first day
pub const START_AT: f64 = 7.0;

/// Which way the sun comes up, round from +x in degrees, and how far its path leans from straight
/// overhead: it's 90 less this high at noon, round towards 90 more than where it came up
pub const RISE_AZIMUTH: f64 = 60.0;
pub const TILT: f64 = 35.0;
/// What's drawn moves on once the sun's this many degrees on from where it was drawn, a second or
/// two by day
pub const STEP_DEGREES: f64 = 0.4;
/// Below this, the sinus of how high the sun is, the moon lights the world instead: the sun's light
/// is gone, and the moon's only just coming in
const SWAP_Z: f64 = -0.025;
/// The moon's light at its brightest, high in the sky
const MOONLIGHT: [f64; 3] = [0.050, 0.066, 0.112];
/// How much brighter the night's shown, as eyes get used to the dark
pub const NIGHT_EXPOSURE: f64 = 1.7;
/// Nights from one full moon to the next: it waxes and wanes through new, crescent and half
pub const LUNAR_NIGHTS: u32 = 8;
/// How much of the full moon's light there is when it's new: some, from the stars and the sky, so
/// no night's black
const NEW_MOON_LIGHT: f64 = 0.3;
/// The real seconds before the night when the dusk is warned of
pub const WARN_SECONDS: f64 = DUSK_SECONDS + 20.0;

/// Which part of the day it is
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Dawn,
    Day,
    Dusk,
    Night,
}

/// The day's clock, kept with the rest of the bases' state, see bases.rs
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Clock {
    /// Real seconds since the last dawn started, 0 to CYCLE_SECONDS
    pub seconds: f64,
    /// Days gone by since the world began: the first is 0
    pub day: u32,
    /// Whether it goes round at all
    pub running: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Clock { seconds: seconds_at(START_AT), day: 0, running: true }
    }
}

/// The real seconds since dawn started at `hour` on the clock
pub fn seconds_at(hour: f64) -> f64 {
    let h = (hour - DAWN_AT).rem_euclid(24.0) + DAWN_AT;
    if h < DAY_AT {
        (h - DAWN_AT) / (DAY_AT - DAWN_AT) * DAWN_SECONDS
    } else if h < DUSK_AT {
        DAWN_SECONDS + (h - DAY_AT) / (DUSK_AT - DAY_AT) * DAY_SECONDS
    } else if h < NIGHT_AT {
        DAWN_SECONDS + DAY_SECONDS + (h - DUSK_AT) / (NIGHT_AT - DUSK_AT) * DUSK_SECONDS
    } else {
        DAWN_SECONDS + DAY_SECONDS + DUSK_SECONDS + (h - NIGHT_AT) / (24.0 + DAWN_AT - NIGHT_AT) * NIGHT_SECONDS
    }
}

/// The hour on the clock `seconds` real seconds since dawn started, 0 to 24
pub fn hour_at(seconds: f64) -> f64 {
    let s = seconds.rem_euclid(CYCLE_SECONDS);
    let h = if s < DAWN_SECONDS {
        DAWN_AT + s / DAWN_SECONDS * (DAY_AT - DAWN_AT)
    } else if s < DAWN_SECONDS + DAY_SECONDS {
        DAY_AT + (s - DAWN_SECONDS) / DAY_SECONDS * (DUSK_AT - DAY_AT)
    } else if s < DAWN_SECONDS + DAY_SECONDS + DUSK_SECONDS {
        DUSK_AT + (s - DAWN_SECONDS - DAY_SECONDS) / DUSK_SECONDS * (NIGHT_AT - DUSK_AT)
    } else {
        NIGHT_AT + (s - DAWN_SECONDS - DAY_SECONDS - DUSK_SECONDS) / NIGHT_SECONDS * (24.0 + DAWN_AT - NIGHT_AT)
    };
    h.rem_euclid(24.0)
}

/// Which part of the day it is at `hour`
pub fn part_at(hour: f64) -> Part {
    let h = hour.rem_euclid(24.0);
    if (DAWN_AT..DAY_AT).contains(&h) {
        Part::Dawn
    } else if (DAY_AT..DUSK_AT).contains(&h) {
        Part::Day
    } else if (DUSK_AT..NIGHT_AT).contains(&h) {
        Part::Dusk
    } else {
        Part::Night
    }
}

/// How far round its path the sun is at `hour`, in radians: 0 coming up, PI going down, and on
/// round under the world through the night
pub fn sun_angle(hour: f64) -> f64 {
    let up = SUNSET - SUNRISE;
    let since = (hour - SUNRISE).rem_euclid(24.0);
    if since < up { since / up * PI } else { PI + (since - up) / (24.0 - up) * PI }
}

/// Which way the sun is, `angle` round its path, see sun_angle()
pub fn sun_direction(angle: f64) -> Vector3<f64> {
    let rise = RISE_AZIMUTH.to_radians();
    let east = Vector3::new(rise.cos(), rise.sin(), 0.0);
    let south = Vector3::new(-rise.sin(), rise.cos(), 0.0);
    let tilt = TILT.to_radians();
    (east * angle.cos() + (Vector3::z() * tilt.cos() + south * tilt.sin()) * angle.sin()).normalize()
}

fn smoothstep(lo: f64, hi: f64, x: f64) -> f64 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// The sun's own light with it `z` up, the sinus of how high it is: golden low in the sky, never
/// quite white, gone once it's set. Has to match what traverse.comp made of it before it was given
/// it, so the day looks as it did.
pub fn sun_light(z: f64) -> [f64; 3] {
    let color = mix3([1.0, 0.45, 0.2], [1.0, 0.9, 0.78], smoothstep(0.0, 0.65, z));
    let strength = 2.4 * smoothstep(-0.02, 0.06, z);
    color.map(|c| c * strength)
}

/// The moon's, with it `z` up and `phase` radians round from new, PI full
pub fn moon_light(z: f64, phase: f64) -> [f64; 3] {
    let strength = smoothstep(0.0, 0.3, z) * (NEW_MOON_LIGHT + (1.0 - NEW_MOON_LIGHT) * lit_part(phase));
    MOONLIGHT.map(|c| c * strength)
}

/// How much of the moon's face is lit at `phase`, 0 new to 1 full
pub fn lit_part(phase: f64) -> f64 {
    0.5 - 0.5 * phase.cos()
}

/// The moon's phase on the night after day `day`, radians round from new: full the first night,
/// then waning to new and waxing back, LUNAR_NIGHTS nights round, the same all night long
pub fn moon_phase(day: u32) -> f64 {
    let k = (day + LUNAR_NIGHTS / 2) % LUNAR_NIGHTS;
    k as f64 / LUNAR_NIGHTS as f64 * TAU
}

/// What the moon's called at `phase`, for the HUD
pub fn phase_name(phase: f64) -> &'static str {
    let k = (phase / TAU * 8.0).round() as i64 % 8;
    ["New moon", "Waxing crescent", "Half moon", "Waxing moon", "Full moon", "Waning moon", "Half moon", "Waning crescent"][k as usize]
}

/// How much of the sky's daylight there is with the sun `z` up, as daylight() in traverse.comp
pub fn daylight(z: f64) -> f64 {
    smoothstep(-0.1, 0.25, z)
}

/// The sky straight up with the sun `z` up, as skyColor() in traverse.comp has it
fn zenith(z: f64) -> [f64; 3] {
    let day = daylight(z);
    let night = [0.008, 0.014, 0.040];
    mix3(night, [0.08, 0.3, 0.92], day)
}

/// What the renderer's given of the day, see Sky in shaders\traverse.comp
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    /// Towards the sun itself, and the moon
    pub sun: Vector3<f64>,
    pub moon: Vector3<f64>,
    /// Towards whichever of them lights the world: the sun, or once it's set, the moon. Shadows,
    /// and everything kept for them, go by this.
    pub key: Vector3<f64>,
    /// That one's light
    pub key_light: [f64; 3],
    /// The sun's own light, for drawing it, 0 once it's set
    pub sun_light: [f64; 3],
    /// How bright the moon's disc is, 0-1, and how much the stars show
    pub moon_shine: f64,
    pub stars: f64,
    /// The moon's phase, radians round from new, PI full
    pub moon_phase: f64,
    /// What the light bounced onto faces is kept relative to, see cacheAdd() in traverse.comp:
    /// about how bright everything lit by the sun and the sky is
    pub cache_level: [f64; 3],
    /// How much brighter it's all shown, see post.comp, and how much of the color goes from it at
    /// night, 0 by day to 1
    pub exposure: f64,
    pub night: f64,
    /// Whether the moon's what lights the world
    pub moonlit: bool,
}

impl Sky {
    /// The sky with the sun `angle` round its path, see sun_angle(), and a full moon
    #[cfg(test)]
    pub fn at(angle: f64) -> Sky {
        Sky::with_moon(angle, std::f64::consts::PI)
    }

    /// The sky with the sun `angle` round its path, and the moon at `phase`, see moon_phase()
    pub fn with_moon(angle: f64, phase: f64) -> Sky {
        let sun = sun_direction(angle);
        // Opposite the sun, rising as it sets and setting as it rises, whatever its phase
        let moon = -sun;
        let moonlit = sun.z < SWAP_Z;
        let (key, key_light) = if moonlit { (moon, moon_light(moon.z, phase)) } else { (sun, sun_light(sun.z)) };
        let day = daylight(sun.z);
        let sky = zenith(sun.z);
        let cache_level = [0, 1, 2].map(|i| (0.3 * key_light[i] + 0.7 * sky[i]).max(0.012));
        Sky {
            sun,
            moon,
            key,
            key_light,
            sun_light: sun_light(sun.z),
            moon_shine: smoothstep(-0.05, 0.1, moon.z) * (1.0 - 0.85 * day),
            stars: (1.0 - smoothstep(0.0, 0.6, day)) * smoothstep(-0.02, 0.1, -sun.z),
            moon_phase: phase,
            cache_level,
            exposure: 1.0 + (NIGHT_EXPOSURE - 1.0) * (1.0 - day),
            night: 1.0 - day,
            moonlit,
        }
    }
}

/// The day going round, and the sky drawn for it
pub struct DayNight {
    pub clock: Clock,
    /// The sun's angle round its path last drawn, see sun_angle(): it's only moved on in steps
    drawn: f64,
    /// Changes whenever the time's set rather than gone by, so what's kept for the last sun is
    /// thrown away, see main.rs
    pub jumps: u32,
    /// Set for a moment when the dusk starts, and the night, for the HUD and the bell
    pub warned: Option<Part>,
    /// Seconds since the part of the day changed, for the HUD to say so
    pub since_change: f64,
}

impl DayNight {
    pub fn new(clock: Clock) -> Self {
        let mut cycle = DayNight { clock, drawn: 0.0, jumps: 0, warned: None, since_change: 100.0 };
        cycle.drawn = cycle.true_angle();
        cycle
    }

    pub fn hour(&self) -> f64 {
        hour_at(self.clock.seconds)
    }

    pub fn part(&self) -> Part {
        part_at(self.hour())
    }

    /// Whether monsters are about: from dusk to dawn
    pub fn monsters_out(&self) -> bool {
        matches!(self.part(), Part::Dusk | Part::Night)
    }

    /// Whether it's night proper, past the dusk
    pub fn is_night(&self) -> bool {
        self.part() == Part::Night
    }

    /// Real seconds till the night starts, 0 during it
    pub fn till_night(&self) -> f64 {
        let night = DAWN_SECONDS + DAY_SECONDS + DUSK_SECONDS;
        if self.is_night() { 0.0 } else { (night - self.clock.seconds).rem_euclid(CYCLE_SECONDS) }
    }

    /// Real seconds till the dawn, 0 unless monsters are out
    pub fn till_dawn(&self) -> f64 {
        if self.monsters_out() { (CYCLE_SECONDS - self.clock.seconds).rem_euclid(CYCLE_SECONDS) } else { 0.0 }
    }

    fn true_angle(&self) -> f64 {
        sun_angle(self.hour())
    }

    /// Sets the clock to `hour`, as from the settings: everything's drawn for it at once
    pub fn set_hour(&mut self, hour: f64) {
        self.clock.seconds = seconds_at(hour);
        self.drawn = self.true_angle();
        self.jumps = self.jumps.wrapping_add(1);
    }

    /// Takes the day on by `dt` real seconds. Returns the part of the day if it's just changed.
    pub fn advance(&mut self, dt: f64) -> Option<Part> {
        let before = self.part();
        if self.clock.running && dt > 0.0 {
            self.clock.seconds += dt;
            if self.clock.seconds >= CYCLE_SECONDS {
                self.clock.seconds -= CYCLE_SECONDS;
                self.clock.day += 1;
            }
        }
        self.since_change += dt;
        // Drawn on a step at a time, see the top of this file
        let angle = self.true_angle();
        let off = (angle - self.drawn + PI).rem_euclid(TAU) - PI;
        if off.abs() >= STEP_DEGREES.to_radians() {
            let steps = (off / STEP_DEGREES.to_radians()).trunc();
            self.drawn = (self.drawn + steps * STEP_DEGREES.to_radians()).rem_euclid(TAU);
            // Far behind, as after a long hitch, it catches up all at once
            if off.abs() > 10.0f64.to_radians() {
                self.drawn = angle;
            }
        }
        let now = self.part();
        (now != before).then(|| {
            self.since_change = 0.0;
            self.warned = Some(now);
            now
        })
    }

    /// The sky as drawn now
    pub fn sky(&self) -> Sky {
        Sky::with_moon(self.drawn, self.moon_phase())
    }

    /// The moon's phase tonight, or last night till the dawn's over: the day's counted from the
    /// dawn, and the moon's the night's
    pub fn moon_phase(&self) -> f64 {
        let night = if self.clock.seconds < DAWN_SECONDS + DAY_SECONDS { self.clock.day.saturating_sub(1) } else { self.clock.day };
        moon_phase(night)
    }

    /// The sky for the sun at `azimuth` and `elevation` in degrees, held there, as the old settings
    /// had it, for `--set sun_azimuth=` and the like
    pub fn fixed_sky(azimuth: f64, elevation: f64) -> Sky {
        let (a, e) = (azimuth.to_radians(), elevation.to_radians());
        let sun = Vector3::new(e.cos() * a.cos(), e.cos() * a.sin(), e.sin());
        let mut sky = Sky::with_moon(PI / 2.0, PI);
        sky.sun = sun;
        sky.moon = -sun;
        sky.key = sun;
        sky.key_light = sun_light(sun.z);
        sky.sun_light = sky.key_light;
        let day = daylight(sun.z);
        sky.stars = (1.0 - smoothstep(0.0, 0.6, day)) * smoothstep(-0.02, 0.1, -sun.z);
        sky.moon_shine = 0.0;
        sky.exposure = 1.0;
        sky.night = 0.0;
        sky.moonlit = false;
        let z = zenith(sun.z);
        sky.cache_level = [0, 1, 2].map(|i| (0.3 * sky.key_light[i] + 0.7 * z[i]).max(0.012));
        sky
    }

    /// A small dial at the top right of the screen: the sun or the moon going round over the land,
    /// the time, and as the dusk draws in, a warning that the night's coming
    pub fn hud(&self, ctx: &egui::Context, raid: Option<&str>) {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("day dial")));
        let screen = ctx.screen_rect();
        let center = pos2(screen.right() - 52.0, screen.top() + 52.0);
        let radius = 30.0;
        let hour = self.hour();
        let part = self.part();
        let t = ctx.input(|i| i.time) as f32;
        let warning = matches!(part, Part::Dusk) || (part == Part::Day && self.till_night() < WARN_SECONDS);
        // The sky in the top half, darker towards night, and the land under it
        let day = daylight(sun_direction(sun_angle(hour)).z) as f32;
        let sky = lerp_color(Color32::from_rgb(20, 26, 60), Color32::from_rgb(90, 150, 220), day);
        painter.circle_filled(center, radius + 3.0, Color32::from_black_alpha(130));
        let top: Vec<Pos2> = (0..=24).map(|k| center + vec2(-(PI as f32 * k as f32 / 24.0).cos(), -(PI as f32 * k as f32 / 24.0).sin()) * radius).collect();
        painter.add(egui::Shape::convex_polygon(top, sky, Stroke::NONE));
        let ground = lerp_color(Color32::from_rgb(18, 22, 16), Color32::from_rgb(52, 74, 40), day);
        let bottom: Vec<Pos2> = (0..=24).map(|k| center + vec2((PI as f32 * k as f32 / 24.0).cos(), (PI as f32 * k as f32 / 24.0).sin()) * radius).collect();
        painter.add(egui::Shape::convex_polygon(bottom, ground, Stroke::NONE));
        painter.line_segment([center - vec2(radius, 0.0), center + vec2(radius, 0.0)], Stroke::new(1.0f32, Color32::from_white_alpha(70)));
        // The sun going over by day, the moon by night, both round the same way
        let angle = sun_angle(hour) as f32;
        let (shown, moon) = if angle <= PI as f32 { (angle, false) } else { (angle - PI as f32, true) };
        let at = center + vec2(-shown.cos(), -shown.sin()) * (radius - 7.0);
        if moon {
            // As much of it lit as tonight's phase
            let lit = lit_part(self.moon_phase()) as f32;
            painter.circle_filled(at, 6.0, Color32::from_rgb(225, 230, 240));
            if lit < 0.98 {
                painter.circle_filled(at + vec2(12.0 * lit, 0.0), 6.0, sky);
            }
            painter.circle_stroke(at, 6.0, Stroke::new(1.0f32, Color32::from_white_alpha(70)));
        } else {
            let glow = if warning { Color32::from_rgb(255, 150, 60) } else { Color32::from_rgb(255, 220, 90) };
            painter.circle_filled(at, 9.0, glow.gamma_multiply(0.25));
            painter.circle_filled(at, 6.0, glow);
        }
        // A ring round it, pulsing amber as the night draws in, and red through it
        let ring = match part {
            _ if self.monsters_out() && !warning => Color32::from_rgb(200, 50, 50).gamma_multiply(0.6 + 0.25 * (t * 2.0).sin()),
            _ if warning => Color32::from_rgb(255, 160, 40).gamma_multiply(0.55 + 0.45 * (t * 4.0).sin().abs()),
            _ => Color32::from_white_alpha(60),
        };
        painter.circle_stroke(center, radius + 2.0, Stroke::new(2.0f32, ring));
        let (h, m) = (hour.floor() as u32, ((hour.fract() * 60.0).floor() as u32).min(59));
        let label = format!("{h:02}:{m:02}");
        painter.text(center + vec2(0.0, radius + 14.0), Align2::CENTER_CENTER, &label, FontId::proportional(14.0), Color32::from_gray(225));
        // What's said under it, kept on the screen
        let note_at = pos2(screen.right() - 12.0, center.y + radius + 31.0);
        let note = match part {
            _ if raid.is_some() => raid.map(str::to_string),
            Part::Night => {
                let dawn = self.till_dawn() as u32;
                Some(format!("Night {}, {} · dawn in {}:{:02}", self.clock.day + 1, phase_name(self.moon_phase()).to_lowercase(), dawn / 60, dawn % 60))
            }
            Part::Dusk => Some("Nightfall: get inside".to_string()),
            Part::Day if warning => Some("Dusk is coming".to_string()),
            Part::Dawn => Some("Dawn".to_string()),
            _ => None,
        };
        if let Some(note) = note {
            let color = if matches!(part, Part::Night | Part::Dusk) || warning { Color32::from_rgb(255, 190, 120) } else { Color32::from_gray(210) };
            painter.text(note_at, Align2::RIGHT_CENTER, note, FontId::proportional(13.0), color);
        }
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The clock and the real seconds go back and forth, and each part of the day lasts as long as
    // it's meant to
    #[test]
    fn the_clock_goes_round() {
        assert!((CYCLE_SECONDS - 1200.0).abs() < 1e-9, "a day's about twenty minutes");
        assert!((300.0..=360.0).contains(&NIGHT_SECONDS), "the night's five or six minutes");
        for k in 0..240 {
            let hour = k as f64 * 0.1;
            let back = hour_at(seconds_at(hour));
            assert!((back - hour).abs() < 1e-6 || (back - hour).abs() > 23.99, "{hour} came back {back}");
        }
        assert_eq!(part_at(hour_at(0.0)), Part::Dawn);
        assert_eq!(part_at(hour_at(DAWN_SECONDS + 1.0)), Part::Day);
        assert_eq!(part_at(hour_at(DAWN_SECONDS + DAY_SECONDS + 1.0)), Part::Dusk);
        assert_eq!(part_at(hour_at(DAWN_SECONDS + DAY_SECONDS + DUSK_SECONDS + 1.0)), Part::Night);
        assert_eq!(part_at(hour_at(CYCLE_SECONDS - 1.0)), Part::Night);
        assert_eq!(part_at(21.5), Part::Night);
        assert_eq!(part_at(12.0), Part::Day);
    }

    // The sun's up by day and down by night, highest at noon, the moon the other way round; the
    // moon lights the night, and the night's dark but not black
    #[test]
    fn the_sun_and_the_moon_go_round() {
        let noon = Sky::at(sun_angle(12.25));
        let midnight = Sky::at(sun_angle(0.25));
        assert!(noon.sun.z > 0.75 && !noon.moonlit && noon.key == noon.sun);
        assert!(midnight.sun.z < -0.75 && midnight.moonlit && midnight.key == midnight.moon);
        assert!(midnight.moon.z > 0.75);
        // Highest at noon, coming up at sunrise and going down at sunset
        assert!(sun_direction(sun_angle(SUNRISE)).z.abs() < 1e-9);
        assert!(sun_direction(sun_angle(SUNSET)).z.abs() < 1e-6);
        let highest = (0..240).map(|k| k as f64 * 0.1).max_by(|a, b| sun_direction(sun_angle(*a)).z.total_cmp(&sun_direction(sun_angle(*b)).z)).unwrap();
        assert!((highest - 12.25).abs() < 0.2, "highest at {highest}");
        // Moonlight's a small part of sunlight, but some
        let luma = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        assert!(luma(midnight.key_light) > 0.04 && luma(midnight.key_light) < 0.05 * luma(noon.key_light));
        assert!(midnight.exposure > 1.5 && (noon.exposure - 1.0).abs() < 1e-6 && midnight.night > 0.99 && noon.night < 1e-6);
        // The light given over is continuous where the moon takes over from the sun
        let before = Sky::at(sun_angle(SUNSET) + 0.02);
        let after = Sky::at(sun_angle(SUNSET) + 0.06);
        assert!(!before.moonlit && after.moonlit);
        assert!(luma(before.key_light) < 0.02 && luma(after.key_light) < 0.02);
        assert!(noon.stars < 1e-9 && midnight.stars > 0.9);
    }

    // What's drawn only moves on in steps, at most one every second or so, but keeps up
    #[test]
    fn the_sun_moves_in_steps() {
        let mut cycle = DayNight::new(Clock::default());
        let mut changes = 0;
        let mut last = cycle.sky().key;
        let dt = 1.0 / 60.0;
        let frames = (120.0 / dt) as usize;
        for _ in 0..frames {
            cycle.advance(dt);
            let key = cycle.sky().key;
            if key != last {
                changes += 1;
                last = key;
            }
            let off = (cycle.true_angle() - cycle.drawn + PI).rem_euclid(TAU) - PI;
            assert!(off.abs() < STEP_DEGREES.to_radians() + 1e-9);
        }
        // Two minutes of the day: the sun moves on about 0.24 degrees a second
        assert!((40..=110).contains(&changes), "{changes} steps in two minutes");
        // A whole day goes round, and on to the next
        let mut cycle = DayNight::new(Clock::default());
        let mut parts = Vec::new();
        for _ in 0..(CYCLE_SECONDS as usize) {
            if let Some(p) = cycle.advance(1.0) {
                parts.push(p);
            }
        }
        assert_eq!(parts, vec![Part::Dusk, Part::Night, Part::Dawn, Part::Day]);
        assert_eq!(cycle.clock.day, 1);
    }

    // The moon goes through its phases night by night, the same all night, and even new there's
    // light to see by
    #[test]
    fn the_moon_waxes_and_wanes() {
        assert!((moon_phase(0) - PI).abs() < 1e-9, "full the first night");
        let parts: Vec<f64> = (0..LUNAR_NIGHTS).map(|d| lit_part(moon_phase(d))).collect();
        assert!(parts.iter().any(|&p| p < 0.01) && parts.iter().any(|&p| p > 0.99) && parts.iter().any(|&p| (p - 0.5).abs() < 0.01));
        assert_eq!(moon_phase(LUNAR_NIGHTS), moon_phase(0));
        let luma = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let new = luma(moon_light(0.8, 0.0));
        let full = luma(moon_light(0.8, PI));
        assert!(new > 0.25 * full && new < 0.5 * full);
        // All night long the phase stays, and the dawn after it too
        let mut cycle = DayNight::new(Clock::default());
        cycle.set_hour(20.0);
        let tonight = cycle.moon_phase();
        cycle.set_hour(23.5);
        assert_eq!(cycle.moon_phase(), tonight);
        cycle.set_hour(3.0);
        assert_eq!(cycle.moon_phase(), tonight);
        // The day's counted from the dawn
        cycle.clock.day += 1;
        cycle.set_hour(5.5);
        assert_eq!(cycle.moon_phase(), tonight);
        cycle.set_hour(21.0);
        assert_ne!(cycle.moon_phase(), tonight);
        assert_eq!(phase_name(PI), "Full moon");
        assert_eq!(phase_name(0.0), "New moon");
    }

    // Setting the time draws it at once, and says so
    #[test]
    fn setting_the_time() {
        let mut cycle = DayNight::new(Clock::default());
        let jumps = cycle.jumps;
        cycle.set_hour(21.5);
        assert!(cycle.is_night() && cycle.monsters_out() && cycle.jumps != jumps);
        assert!(cycle.sky().moonlit);
        assert!(cycle.till_dawn() > 0.0 && cycle.till_night() == 0.0);
        cycle.set_hour(12.0);
        assert!(!cycle.monsters_out() && cycle.till_night() > DUSK_SECONDS);
    }
}
