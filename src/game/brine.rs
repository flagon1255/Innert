//! What the Tidewrack Kraken and the Grotto Squid do to the player and round them, see
//! src\bosses\kraken.rs: the water thrown up as it rises or its arms burst up out of it, splashing
//! into the water sim as everything that goes into the water does; the player seized in a tentacle
//! and dragged in; ink flung over them that blooms into a cloud they can't see through; water
//! jetted at them; and the whirlpool drawing them round and in.

use super::*;
use crate::bosses::kraken::{self, Act};

/// A cloud of ink: where its middle is in voxels, how far round it is in meters, seconds till it
/// blooms there, as the ink's still flying, and seconds it lasts after, and what it's made of
struct Cloud {
    at: Vector3<f64>,
    radius: f64,
    wait: f64,
    left: f64,
    element: Element,
}

/// How long a cloud of ink hangs about once it's bloomed, in seconds
const CLOUD_SECONDS: f64 = 6.0;
/// How many there can be at once
const MOST_CLOUDS: usize = 4;
/// How hard the water jetted at the player hurts, a second, and how hard it throws them back, in
/// meters a second gained each second
const JET_DPS: f64 = 22.0;
const JET_PUSH: f64 = 26.0;
/// How quickly a tentacle drags whoever it's caught in, in meters a second gained each second
const DRAG_PULL: f64 = 30.0;

/// Everything of the krakens' still going on round the player
#[derive(Default)]
pub struct Deep {
    clouds: Vec<Cloud>,
    /// How far the player's blinded by ink, 0 to 1, and its color, and how far they've been
    inked: f64,
    ink: [u8; 3],
    /// Seconds till the water jetted at the player next splashes where it hits
    splash_wait: f64,
}

impl Deep {
    pub fn recenter(&mut self, shift: Vector3<f64>) {
        for c in &mut self.clouds {
            c.at -= shift;
        }
    }

    pub fn clear(&mut self) {
        self.clouds.clear();
        self.inked = 0.0;
    }
}

/// The color of ink of `element`: black with the void in it, or blue with the light of the caves
fn ink_of(element: Element) -> [u8; 3] {
    match element {
        Element::Arcane => [10, 22, 48],
        _ => [12, 6, 20],
    }
}

/// The kind of drop ink of `element` is made of
fn drop_of(element: Element) -> usize {
    match element {
        Element::Arcane => DROP_WATER,
        _ => DROP_VOID,
    }
}

impl Game {
    /// What a kraken or a squid did, see kraken::Act
    pub(super) fn kraken_act(&mut self, look: Look, element: Option<Element>, act: Act, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, view: &View) {
        let vpm = world.voxels_per_meter as f64;
        let felt = |at: Vector3<f64>| (1.0 - (at - view.eye).norm() / vpm / 40.0).max(0.0);
        let element = element.unwrap_or(Element::Void);
        let k = kraken::size(look);
        match act {
            // The water thrown up and knocked down, rolling out as a wave, or the ground burst open
            Act::Surge { at, size } => {
                let top = water(at.x, at.y);
                if top.is_finite() && top > at.z - 1.5 * vpm {
                    self.combat.splash_up(Vector3::new(at.x, at.y, top), size);
                } else {
                    self.combat.burst_up(at, DROP_MUD);
                }
                self.shake = self.shake.max(0.3 * size.min(1.5) * felt(at));
                if size > 1.0 {
                    self.cues.push(Cue::at(Sound::Splash, at, 1.0).pitched(0.45 / k.sqrt() as f32));
                }
            }
            // A tentacle flung out, cracking like a whip, and if it caught them, its grip
            Act::Lash { from, to, caught } => {
                self.cues.push(Cue::at(Sound::Swoop, from, 0.9).pitched(0.6));
                self.cues.push(Cue::at(Sound::Squelch, to, 0.9).pitched(0.7));
                if caught {
                    if let Some(feet) = player {
                        self.hurt(8.0, from, feet, world, look.name());
                        self.say(format!("The {} has you in its tentacle!", look.name()));
                    }
                } else if let Some(feet) = player {
                    self.near_miss(feet);
                }
            }
            // Dragged in towards its beak, along the top of the water or over the ground
            Act::Drag { to, dt } => {
                if let Some(feet) = player {
                    let off = (to - feet).xy();
                    let d = off.norm() / vpm;
                    if d > 1.5 {
                        let way = off / off.norm();
                        self.thrown += Vector3::new(way.x, way.y, 0.15) * DRAG_PULL * vpm * dt;
                    }
                    if random(&mut self.rng) < dt * 6.0 {
                        let top = water(feet.x, feet.y);
                        if top.is_finite() && top > feet.z - 0.5 * vpm {
                            self.combat.stir(Vector3::new(feet.x, feet.y, top), 0.6, dt * 4.0, DROP_WATER);
                        }
                    }
                }
            }
            // A ball of ink flung, flying the whole way, and blooming where it comes down
            Act::Ink { from, to, time, radius } => {
                self.combat.hurl(HostileKind::Spell(element), from, to, time, 12.0, 2.0);
                self.cues.push(Cue::at(Sound::Squelch, from, 1.0).pitched(0.55));
                self.cues.push(Cue::at(Sound::Cast(Element::look(Some(element)) as u8), from, 0.7).pitched(0.5));
                self.brine.clouds.push(Cloud { at: to - Vector3::new(0.0, 0.0, 1.0 * vpm), radius, wait: time, left: CLOUD_SECONDS, element });
                while self.brine.clouds.len() > MOST_CLOUDS {
                    self.brine.clouds.remove(0);
                }
            }
            // Water jetting out in a stream, knocking whoever it catches back and off their feet
            Act::Jet { from, dir, dt, reach } => {
                self.combat.water_jet(from, dir, dt);
                if self.breath_sound <= 0.0 {
                    self.breath_sound = 1.4;
                    self.cues.push(Cue::at(Sound::Breath, from, 1.0).pitched(0.7));
                }
                if let Some(feet) = player {
                    let to = feet + Vector3::new(0.0, 0.0, 1.0 * vpm) - from;
                    let along = to.dot(&dir);
                    if along > 0.0 && along < reach * vpm && (to - dir * along).norm() < 0.8 * vpm + along * 0.08 {
                        self.hurt(JET_DPS * dt, from, feet, world, look.name());
                        self.thrown += Vector3::new(dir.x, dir.y, 0.2) * JET_PUSH * vpm * dt;
                    }
                }
                // Where it comes down on the water, splashing
                self.brine.splash_wait -= dt;
                if self.brine.splash_wait <= 0.0 {
                    self.brine.splash_wait = 0.25;
                    let land = from + dir * 0.7 * reach * vpm;
                    let top = water(land.x, land.y);
                    if top.is_finite() && top > land.z - 3.0 * vpm {
                        self.combat.splash_up(Vector3::new(land.x, land.y, top), 0.5);
                    }
                }
            }
            // The water spinning round it in rings, and whoever's in it drawn round and in
            Act::Swirl { at, radius, dt } => {
                let time = self.combat.time();
                let top = water(at.x, at.y);
                let level = if top.is_finite() { top } else { at.z };
                for ring in 0..3 {
                    let round = radius * (0.3 + 0.3 * ring as f64);
                    for j in 0..2 {
                        let a = time * (2.2 - 0.4 * ring as f64) + ring as f64 * 2.1 + j as f64 * std::f64::consts::PI;
                        self.combat.stir(Vector3::new(at.x + a.cos() * round * vpm, at.y + a.sin() * round * vpm, level), 0.6, dt, DROP_WATER);
                    }
                }
                if let Some(feet) = player {
                    let off = (feet - at).xy() / vpm;
                    let d = off.norm();
                    if d > 0.8 && d < radius && (feet.z - level).abs() < 3.0 * vpm {
                        let round = Vector2::new(-off.y, off.x) / d;
                        let pull = (round * 1.8 - off / d * 1.5) * (1.0 - 0.6 * d / radius);
                        self.thrown += Vector3::new(pull.x, pull.y, 0.0) * vpm * dt * 9.0;
                    }
                }
                if random(&mut self.rng) < dt * 1.5 {
                    self.cues.push(Cue::at(Sound::Splash, at, 0.7).pitched(0.5));
                }
            }
        }
    }

    /// The krakens' ink blooming into clouds where it came down and hanging there, dark drops
    /// drifting in it, and whoever's inside blinded, see ink_veil()
    pub(super) fn tend_brine(&mut self, player: Option<Vector3<f64>>, dt: f64) {
        let vpm = self.combat.vpm();
        let mut inside = None;
        for c in &mut self.brine.clouds {
            if c.wait > 0.0 {
                c.wait -= dt;
                if c.wait <= 0.0 {
                    self.combat.burst_up(c.at, drop_of(c.element));
                    self.cues.push(Cue::at(Sound::Squelch, c.at, 0.9).pitched(0.4));
                }
                continue;
            }
            c.left -= dt;
            let thick = (c.left / 1.5).min(1.0).max(0.0);
            // Drops of it hanging and drifting, thick at first and thinning out
            self.combat.stir(c.at + Vector3::new(0.0, 0.0, 1.0 * vpm), c.radius * 0.8, dt * 3.0 * thick, drop_of(c.element));
            self.combat.mark(c.at, c.radius, 0.0);
            if let Some(feet) = player
                && (feet - c.at).xy().norm() < c.radius * vpm
                && (feet.z - c.at.z).abs() < 3.5 * vpm
            {
                inside = Some((thick, c.element));
            }
        }
        self.brine.clouds.retain(|c| c.left > 0.0);
        match inside {
            Some((thick, element)) => {
                let was = self.brine.inked;
                self.brine.inked = (self.brine.inked + dt * 2.5).min(0.92 * thick.max(0.3));
                self.brine.ink = ink_of(element);
                if was < 0.05 && self.brine.inked >= 0.05 {
                    self.say("Ink! You can barely see.".to_owned());
                }
            }
            None => self.brine.inked = (self.brine.inked - dt * 0.9).max(0.0),
        }
    }

    /// The player blinded by ink, darkest in from the edges, see tend_brine()
    pub(super) fn ink_veil(&self, painter: &egui::Painter, screen: egui::Rect) {
        let g = self.brine.inked as f32;
        if g < 0.01 {
            return;
        }
        let [r, gr, b] = self.brine.ink;
        let shade = |a: f32| Color32::from_rgba_unmultiplied(r, gr, b, (a * g * 255.0).min(255.0) as u8);
        painter.rect_filled(screen, 0.0, shade(0.7));
        // Thicker still round the edges, in four bands closing in
        for k in 0..4 {
            let inset = screen.width().min(screen.height()) * (0.05 + 0.07 * k as f32);
            let inner = screen.shrink(inset);
            let a = 0.12;
            painter.rect_filled(egui::Rect::from_min_max(screen.min, egui::pos2(screen.max.x, inner.min.y)), 0.0, shade(a));
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(screen.min.x, inner.max.y), screen.max), 0.0, shade(a));
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(screen.min.x, inner.min.y), egui::pos2(inner.min.x, inner.max.y)), 0.0, shade(a));
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(inner.max.x, inner.min.y), egui::pos2(screen.max.x, inner.max.y)), 0.0, shade(a));
        }
    }
}
