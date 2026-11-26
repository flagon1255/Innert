//! What floats rather than walks: the Sky Isles' own bosses, see src\bosses\roster.rs. Nothing of
//! them touches the ground. They drift over it, rising and falling slowly, leaning into the way
//! they're going and turning to face what they're after.
//!
//! The Aeolith, a spirit of the wind bound in carved stone: a heart of glowing runestone in a cage
//! of stone bands, a stone mask floating before it with eyes of light and a crest of fins, three
//! rings of carved stone turning round it each its own way like an orrery, rune-studs glowing on
//! them, two fists of stone floating either side, a halo of stone shards circling it and a funnel of
//! wind whirling under it. It flings its great ring out like a discus and calls it back; lays its
//! rings level and spins them up into a whirlwind; raises its shards high over it and flings them
//! down. Dying, its rings slow and tip, and it all falls out of the air into a heap of stone.
//!
//! The Stormbell, a jelly of thundercloud: a bell of white cloud, dark underneath, lightning glowing
//! in veins down it, a frill of grey cloud round its rim and a heart crackling under it, and eight
//! tendrils trailing from its rim, beaded with light, swaying as it swims through the air with slow
//! beats of its bell. It swells and rises and rains lightning from its rim; lashes its tendrils out
//! at the player; draws them up tight and charges till it crackles, then flings them out as it lets
//! it all go. Dying, it sags, its tendrils go limp, and it sinks to the ground and comes apart.

use bytemuck::{Pod, Zeroable};

use super::*;
use crate::animals::{ball, ellipsoid, limb};
use crate::bosses::roster::sky::{RINGCAST_BACK, RINGCAST_LANDS};
use crate::worldgen::material::FUR_WHITE;

// The Aeolith's pieces: its heart, its mask, its three rings from the greatest in, its fists, right
// then left, its six shards, and the wind under it
const CORE: usize = 0;
const MASK: usize = 1;
const RINGS: [usize; 3] = [2, 3, 4];
const FISTS: [usize; 2] = [5, 6];
const SHARDS: [usize; 6] = [7, 8, 9, 10, 11, 12];
const VORTEX: usize = 13;

// The Stormbell's: its bell, the heart under it, the frill round its rim, and each of its eight
// tendrils' upper and lower halves, round from ahead to the left
const BELL: usize = 0;
const HEART: usize = 1;
const FRILL: usize = 2;
const UPPER: [usize; 8] = [3, 4, 5, 6, 7, 8, 9, 10];
const LOWER: [usize; 8] = [11, 12, 13, 14, 15, 16, 17, 18];

/// How high the Aeolith's heart floats and the middle of the Stormbell's bell, in meters at their
/// builds' size
const HOVER: f64 = 1.7;
const BELL_HIGH: f64 = 2.3;
/// How far round each of the Aeolith's rings is, the greatest first, and how thick
const RING_R: [f64; 3] = [1.25, 0.95, 0.68];
const RING_THICK: [f64; 3] = [0.11, 0.09, 0.075];
/// How each of its rings is tipped as it rests, about +x and +y, and how fast it turns, radians a
/// second, and its tipping goes round
const RING_TIP: [[f64; 2]; 3] = [[1.25, 0.0], [0.0, 1.15], [-0.6, 0.7]];
const RING_TURN: [f64; 3] = [1.1, -1.6, 2.3];
const RING_ROUND: [f64; 3] = [0.3, -0.25, 0.4];
/// How far round its heart its shards circle
const SHARD_R: f64 = 1.7;
/// How far the Stormbell's tendrils hang from round its rim, and how long each half of one is
const RIM_R: f64 = 0.88;
const TENDRIL: f64 = 0.85;

/// Seconds it takes to die, and how far into that it starts to fall apart
pub const DEATH: f64 = 2.8;
const CRUMBLE: f64 = 1.0;
/// Seconds into dying what's left of it hits the ground
pub const THUDS: [f64; 2] = [1.25, 1.7];

/// How it holds itself: where its heart or bell floats, ahead, left and up; tipped forward, left
/// side up and round to the left; how fast its rings or its bell's beats go, as a share of how
/// they go at rest; how level its rings lie, 0 each tipped its own way to 1 flat round it; how far
/// out its shards circle or its tendrils spread, 1 as it rests; where the Aeolith's fists are,
/// right then left; how high its shards are raised, 0 circling to 1 high overhead; how far the
/// Stormbell's tendrils are drawn up, 0 hanging; and how far they're lashed out at what it's aimed at
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Stance {
    pub core: [f64; 3],
    pub pitch: f64,
    pub roll: f64,
    pub yaw: f64,
    pub spin: f64,
    pub level: f64,
    pub spread: f64,
    pub hands: [[f64; 3]; 2],
    pub raise: f64,
    pub curl: f64,
    pub lash: f64,
}

/// Its pose, eased from frame to frame, and how far round its rings, its bell's beats and its
/// shards have gone, which only ever go on
#[derive(Clone, Copy, Debug)]
pub struct State {
    pose: Stance,
    turns: [f64; 4],
}

impl State {
    pub fn new(look: Look) -> State {
        State { pose: rest(look), turns: [0.0, 1.3, 2.9, 0.7] }
    }
}

fn stormbell(look: Look) -> bool {
    look == Look::Stormbell
}

/// What each piece is called, for its file, and the piece it hangs from, see edit::bone_defs()
pub fn piece_names(look: Look) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = if stormbell(look) {
        let mut v = vec![("bell".to_owned(), 0), ("heart".to_owned(), BELL), ("frill".to_owned(), BELL)];
        v.extend((0..8).map(|k| (format!("tendril_{}", k + 1), BELL)));
        v.extend((0..8).map(|k| (format!("tendril_{}_tip", k + 1), UPPER[k])));
        v
    } else {
        let mut v = vec![("heart".to_owned(), 0), ("mask".to_owned(), CORE)];
        v.extend(["great_ring", "middle_ring", "inner_ring", "fist_right", "fist_left"].iter().map(|n| (n.to_string(), CORE)));
        v.extend((0..6).map(|k| (format!("shard_{}", k + 1), CORE)));
        v.push(("wind".to_owned(), CORE));
        v
    };
    while out.len() < PIECES {
        let k = out.len();
        out.push((format!("extra_{}", k + 1), 0));
    }
    out
}

pub fn rest(look: Look) -> Stance {
    if stormbell(look) {
        Stance { core: [0.0, 0.0, BELL_HIGH], spin: 1.0, spread: 1.0, curl: 0.12, ..Stance::default() }
    } else {
        Stance { core: [0.0, 0.0, HOVER], spin: 1.0, spread: 1.0, hands: [[0.5, -1.2, 1.5], [0.5, 1.2, 1.5]], ..Stance::default() }
    }
}

/// Points round a circle of `r` in the plane of `u` and `v` about `middle`, `n` of them, joined by
/// limbs `thick` round of `material`, every `every`th of `other` if that's not 0
#[allow(clippy::too_many_arguments)]
fn hoop(p: &mut Vec<Part>, middle: [f64; 3], u: [f64; 3], v: [f64; 3], r: f64, thick: f64, n: usize, material: u8, other: (usize, u8)) {
    let at = |k: usize| {
        let a = k as f64 / n as f64 * TAU;
        [0, 1, 2].map(|i| middle[i] + (u[i] * a.cos() + v[i] * a.sin()) * r)
    };
    for k in 0..n {
        let m = if other.0 > 0 && k % other.0 == 0 { other.1 } else { material };
        p.push(limb(at(k), at(k + 1), thick, thick, m));
    }
}

/// Every piece of `look`, each in its own frame about its joint
pub fn pieces(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (body, dark, accent, glow, extra) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    if stormbell(look) {
        // Its bell: lightning veined down it from a crackling crown, hollow underneath and dark
        // there, lumps of cloud heaped over it
        let b = &mut out[BELL];
        for k in 0..8 {
            let a = k as f64 / 8.0 * TAU + 0.2;
            let point = |t: f64| [a.cos() * t.sin() * 1.01, a.sin() * t.sin() * 1.01, t.cos() * 0.83];
            let steps = [1.45, 1.15, 0.85, 0.55, 0.3];
            for w in steps.windows(2) {
                let wobble = 0.06 * ((k * 3 + (w[0] * 10.0) as usize) % 3) as f64 - 0.06;
                let (mut from, to) = (point(w[0]), point(w[1]));
                from[0] += wobble * a.sin();
                from[1] -= wobble * a.cos();
                b.push(limb(from, to, 0.035, 0.03, glow));
            }
        }
        b.push(ball([0.0, 0.0, 0.84], 0.09, extra));
        b.push(ellipsoid([0.0, 0.0, -0.22], [0.86, 0.86, 0.62], AIR));
        b.push(ellipsoid([0.0, 0.0, -0.16], [0.9, 0.9, 0.7], dark));
        for k in 0..11 {
            let a = k as f64 * 2.3;
            let t = 0.35 + 0.8 * ((k * 7) % 11) as f64 / 11.0;
            let at = [a.cos() * t.sin() * 0.95, a.sin() * t.sin() * 0.95, t.cos() * 0.8];
            b.push(ball(at, 0.17 + 0.05 * (k % 3) as f64, if k % 4 == 0 { accent } else { body }));
        }
        b.push(ellipsoid([0.0, 0.0, 0.0], [1.0, 1.0, 0.82], body));
        // Its heart, under the bell, lightning in a cloud, and arms frilled with foam hanging off it
        let h = &mut out[HEART];
        h.push(ball([0.0, 0.0, -0.22], 0.2, glow));
        for k in 0..4 {
            let a = k as f64 / 4.0 * TAU + 0.4;
            let end = [a.cos() * 0.2, a.sin() * 0.2, -0.85];
            h.push(ball([end[0] * 0.6, end[1] * 0.6, -0.6], 0.05, glow));
            h.push(limb([a.cos() * 0.08, a.sin() * 0.08, -0.32], end, 0.08, 0.03, accent));
        }
        h.push(ball([0.0, 0.0, -0.28], 0.27, dark));
        // A frill of cloud round its rim, scalloped
        let f = &mut out[FRILL];
        for k in 0..16 {
            let a = k as f64 / 16.0 * TAU;
            f.push(ball([a.cos() * 1.0, a.sin() * 1.0, -0.06], 0.13, if k % 2 == 0 { dark } else { accent }));
        }
        hoop(f, [0.0, 0.0, -0.02], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.98, 0.06, 24, dark, (0, AIR));
        // Its tendrils, storm grey, beaded with light, the tips glowing
        for k in 0..8 {
            out[UPPER[k]] = vec![ball([0.0, 0.0, -0.32], 0.065, glow), ball([0.0, 0.0, -0.66], 0.06, glow), limb([0.0, 0.0, 0.0], [0.0, 0.0, -TENDRIL], 0.075, 0.055, dark)];
            out[LOWER[k]] = vec![
                ball([0.0, 0.0, -TENDRIL], 0.065, glow),
                ball([0.0, 0.0, -0.4], 0.05, glow),
                ball([0.0, 0.0, -0.05], 0.06, dark),
                limb([0.0, 0.0, 0.0], [0.0, 0.0, -TENDRIL], 0.055, 0.02, dark),
            ];
        }
        return out;
    }
    // The Aeolith's heart: runestone glowing in a cage of three stone bands, carved caps over and
    // under it
    let c = &mut out[CORE];
    c.push(ellipsoid([0.0, 0.0, 0.36], [0.12, 0.12, 0.07], extra));
    c.push(ellipsoid([0.0, 0.0, -0.36], [0.12, 0.12, 0.07], extra));
    hoop(c, [0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.33, 0.045, 16, dark, (4, extra));
    hoop(c, [0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.33, 0.045, 16, dark, (0, AIR));
    hoop(c, [0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.33, 0.045, 16, dark, (0, AIR));
    c.push(ball([0.0, 0.0, 0.0], 0.26, glow));
    // Its mask, floating before its heart: eyes of light under a heavy brow, a slit of a mouth, a
    // gold rune on its brow, teal lines down its cheeks, and a crest of stone fins over it
    let m = &mut out[MASK];
    for s in [-1.0, 1.0] {
        m.push(ellipsoid([0.5, s * 0.1, 0.05], [0.025, 0.065, 0.028], glow));
        m.push(limb([0.49, s * 0.17, -0.02], [0.47, s * 0.12, -0.24], 0.022, 0.016, accent));
        m.push(limb([0.34, s * 0.12, 0.28], [0.18, s * 0.2, 0.55], 0.045, 0.015, body));
    }
    m.push(ball([0.5, 0.0, 0.22], 0.035, extra));
    m.push(limb([0.49, -0.07, -0.15], [0.49, 0.07, -0.15], 0.018, 0.018, dark));
    m.push(limb([0.48, -0.19, 0.13], [0.48, 0.19, 0.13], 0.05, 0.05, dark));
    m.push(limb([0.36, 0.0, 0.3], [0.2, 0.0, 0.62], 0.06, 0.02, body));
    m.push(ellipsoid([0.42, 0.0, 0.02], [0.08, 0.26, 0.33], body));
    // Its rings: carved stone, rune-studs glowing round their tops, gold bands and teal inlay, the
    // great ring with four blades of stone standing out from it to catch the wind
    for (i, &piece) in RINGS.iter().enumerate() {
        let (r, thick) = (RING_R[i], RING_THICK[i]);
        let p = &mut out[piece];
        let studs = [12, 10, 8][i];
        for k in 0..studs {
            let a = (k as f64 + 0.5) / studs as f64 * TAU;
            p.push(ball([a.cos() * r, a.sin() * r, thick * 0.75], thick * 0.42, glow));
        }
        if i == 0 {
            for k in 0..4 {
                let a = k as f64 / 4.0 * TAU + 0.4;
                let (c, s) = (a.cos(), a.sin());
                p.push(limb([c * (r + 0.02), s * (r + 0.02), 0.0], [c * (r + 0.26) - s * 0.08, s * (r + 0.26) + c * 0.08, 0.0], 0.07, 0.015, body));
            }
        }
        hoop(p, [0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r - thick * 0.55, thick * 0.4, [28, 22, 18][i], accent, (0, AIR));
        hoop(p, [0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r, thick, [28, 22, 18][i], body, (7, extra));
    }
    // Its fists: stone, knuckles over a seam of light, a teal band round the wrist
    for &piece in &FISTS {
        let p = &mut out[piece];
        p.push(limb([0.2, -0.15, -0.03], [0.2, 0.15, -0.03], 0.028, 0.028, glow));
        for k in 0..4 {
            p.push(ball([0.2, -0.13 + k as f64 * 0.087, 0.09], 0.065, body));
        }
        hoop(p, [-0.18, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.17, 0.04, 12, accent, (0, AIR));
        p.push(ellipsoid([0.02, 0.0, 0.0], [0.25, 0.2, 0.21], dark));
    }
    // Its shards, pointing out along +z, glowing at the tip, a gold band round the middle
    for (k, &piece) in SHARDS.iter().enumerate() {
        let long = 0.3 + 0.05 * (k % 3) as f64;
        out[piece] = vec![ball([0.0, 0.0, long], 0.045, glow), ellipsoid([0.0, 0.0, 0.0], [0.08, 0.08, 0.04], extra), limb([0.0, 0.0, -long * 0.8], [0.0, 0.0, long], 0.1, 0.02, body)];
    }
    // The wind under it: three bands of cloud winding down in a narrowing funnel, motes in it
    let w = &mut out[VORTEX];
    for j in 0..3 {
        let mut last: Option<[f64; 3]> = None;
        for s in 0..=12 {
            let f = s as f64 / 12.0;
            let a = j as f64 / 3.0 * TAU + f * 4.2;
            let r = 0.42 - 0.3 * f;
            let at = [a.cos() * r, a.sin() * r, -1.15 * f];
            if s % 5 == 2 {
                w.push(ball([at[0] * 1.15, at[1] * 1.15, at[2]], 0.035, glow));
            }
            if let Some(from) = last {
                w.push(limb(from, at, 0.055 - 0.025 * f, 0.055 - 0.025 * f, if (s + j) % 3 == 0 { accent } else { FUR_WHITE }));
            }
            last = Some(at);
        }
    }
    out
}

/// Seconds its blows take to come down, the last of winding them up
const SNAP: f64 = 0.14;

/// What it wants to be doing with itself as it goes about `m`, and how quickly to get there
fn fight(look: Look, rest: &Stance, m: &Moment) -> (Stance, f64) {
    let up = |s: Stance, z: f64| Stance { core: [s.core[0], s.core[1], s.core[2] + z], ..s };
    let guard = if !m.engaged {
        *rest
    } else if stormbell(look) {
        Stance { spin: 1.3, curl: 0.22, ..up(*rest, 0.15) }
    } else {
        Stance { pitch: 0.05, spin: 1.4, hands: [[0.85, -1.05, 1.65], [0.85, 1.05, 1.65]], ..up(*rest, 0.12) }
    };
    let Some(d) = m.doing else {
        return (guard, 4.0);
    };
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 36.0 } else { slow });
    let lead = (d.len - SNAP).max(0.05);
    if stormbell(look) {
        return match (d.m, d.stage) {
            // Swelling up and rising, its beats quickening and its tendrils drawn up, then splayed
            // out wide round it as the lightning rains from its rim
            (Move::Staticfall, Stage::Wind) => {
                let swollen = Stance { spin: 2.6, curl: 0.95, spread: 1.15, ..up(guard, 0.6) };
                pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, swollen, Ease::Gather)]), 5.0)
            }
            (Move::Staticfall, Stage::Act) => {
                let mut raining = Stance { spin: 3.0, curl: 0.3, spread: 1.4, ..up(guard, 0.55) };
                raining.roll += 0.06 * (d.t * 9.0).sin();
                pick((raining, false), 8.0)
            }
            // Drawn back, its tendrils pulled up behind it, then lashed out at the player
            (Move::Thunderlash, Stage::Wind) => {
                let drawn = Stance { pitch: -0.25, curl: 0.85, spin: 1.8, core: [guard.core[0] - 0.35, 0.0, guard.core[2] + 0.2], ..guard };
                let lashed = Stance { pitch: 0.35, curl: 0.0, lash: 1.0, core: [guard.core[0] + 0.4, 0.0, guard.core[2] - 0.1], ..guard };
                pick(along(d.t, &[(0.0, guard, Ease::Linear), (lead, drawn, Ease::Gather), (d.len, lashed, Ease::Snap)]), 8.0)
            }
            (Move::Thunderlash, Stage::Act) => {
                let lashed = Stance { pitch: 0.35, curl: 0.0, lash: 1.0, core: [guard.core[0] + 0.4, 0.0, guard.core[2] - 0.1], ..guard };
                pick((lashed, false), 30.0)
            }
            // Drawing its tendrils up tight and rising, trembling as it charges; then letting it go,
            // dropping, its tendrils flung out and down; then sagging, spent, dim
            (Move::Overcharge, Stage::Wind) => {
                let mut charged = Stance { spin: 4.0, curl: 1.15, spread: 0.75, ..up(guard, 0.45) };
                let k = (d.t / d.len).min(1.0);
                charged.core[0] += 0.04 * k * (d.t * 37.0).sin();
                charged.core[1] += 0.04 * k * (d.t * 29.0).sin();
                pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, charged, Ease::Gather)]), 7.0)
            }
            (Move::Overcharge, Stage::Act) => {
                let loosed = Stance { spin: 0.6, curl: 0.4, spread: 3.2, ..up(guard, -0.3) };
                pick(along(d.t, &[(0.0, loosed, Ease::Linear), (d.len, loosed, Ease::Linear)]), 18.0)
            }
            (Move::Overcharge, Stage::Recover) => {
                let spent = Stance { spin: 0.35, curl: 0.0, spread: 0.5, pitch: 0.12, ..up(guard, -0.35) };
                pick(along(d.t, &[(0.0, spent, Ease::Linear), (d.len * 0.7, spent, Ease::Linear), (d.len, guard, Ease::Smooth)]), 5.0)
            }
            // Its tendrils stung down on the ground in front of it
            (Move::Slam, Stage::Wind) => {
                let drawn = Stance { pitch: -0.2, curl: 0.7, ..up(guard, 0.25) };
                let stung = Stance { pitch: 0.3, curl: 0.0, lash: 0.8, ..up(guard, -0.2) };
                pick(along(d.t, &[(0.0, guard, Ease::Linear), (lead, drawn, Ease::Gather), (d.len, stung, Ease::Snap)]), 9.0)
            }
            (Move::Slam, Stage::Act) => pick((Stance { pitch: 0.3, curl: 0.0, lash: 0.8, ..up(guard, -0.2) }, false), 24.0),
            // A ball of lightning gathering under its bell, then let go at the player
            (Move::Spit, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, Stance { spin: 2.2, curl: 0.55, pitch: -0.1, ..guard }, Ease::Gather)]), 7.0),
            (Move::Spit, Stage::Act) => pick((Stance { spin: 1.6, curl: 0.2, pitch: 0.2, ..guard }, false), 20.0),
            (Move::Roar, Stage::Act) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.3, Stance { spin: 3.0, curl: 0.6, spread: 1.45, ..up(guard, 0.3) }, Ease::Gather)]), 9.0),
            (_, Stage::Reel) => {
                let reel = Stance { pitch: -0.35, roll: 0.2, spin: 0.4, curl: -0.1, core: [guard.core[0] - 0.4, 0.1, guard.core[2] - 0.15], ..guard };
                pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.16, reel, Ease::Gather), (d.len, guard, Ease::Smooth)]), 12.0)
            }
            (_, Stage::Stun) => {
                let mut dazed = Stance { pitch: 0.25, spin: 0.4, curl: -0.1, ..up(guard, -0.4) };
                dazed.roll += 0.15 * (d.t * 2.2).sin();
                pick((dazed, false), 5.0)
            }
            _ => pick((guard, false), 6.0),
        };
    }
    let fists = |right: [f64; 3], left: [f64; 3]| [right, left];
    match (d.m, d.stage) {
        // Its great ring drawn back over its right shoulder, whirring faster, its right fist back
        // with it; then flung, its fist following through across it
        (Move::Ringcast, Stage::Wind) => {
            let drawn = Stance { yaw: 0.4, pitch: -0.1, spin: 2.6, hands: fists([-0.3, -1.3, 2.3], [0.9, 0.9, 1.8]), ..guard };
            let flung = Stance { yaw: -0.3, pitch: 0.12, spin: 1.2, hands: fists([1.4, -0.4, 1.9], [0.3, 1.2, 1.4]), ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (lead, drawn, Ease::Gather), (d.len, flung, Ease::Snap)]), 7.0)
        }
        (Move::Ringcast, Stage::Act) => {
            // Its right hand held out, calling the ring back
            let calling = Stance { yaw: -0.1, pitch: 0.05, spin: 1.6, hands: fists([1.5, -0.6, 2.1], [0.5, 1.1, 1.5]), ..guard };
            pick(along(d.t, &[(0.0, Stance { yaw: -0.3, pitch: 0.12, hands: fists([1.4, -0.4, 1.9], [0.3, 1.2, 1.4]), ..guard }, Ease::Linear), (d.len * 0.5, calling, Ease::Smooth)]), 10.0)
        }
        // Sinking lower, its fists drawn in, its rings laid level and spun up into a blur; then
        // whirling, its fists circling it; then wobbling, slowed and tipped, drooping: the moment
        // to hit it
        (Move::Gyre, Stage::Wind) => {
            let gathered = Stance { level: 1.0, spin: 4.5, pitch: 0.0, hands: fists([0.1, -0.75, 1.3], [0.1, 0.75, 1.3]), ..up(guard, -0.3) };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, gathered, Ease::Gather)]), 6.0)
        }
        (Move::Gyre, Stage::Act) => {
            let a = d.t * 6.5;
            let whirl = Stance {
                level: 1.0,
                spin: 6.5,
                hands: fists([a.cos() * 1.35, a.sin() * 1.35, 1.35], [-a.cos() * 1.35, -a.sin() * 1.35, 1.35]),
                ..up(guard, -0.3 + 0.08 * (d.t * 5.0).sin())
            };
            pick((whirl, false), 14.0)
        }
        (Move::Gyre, Stage::Recover) => {
            let mut spent = Stance { level: 0.45, spin: 0.35, pitch: 0.18, hands: fists([0.6, -0.9, 0.95], [0.6, 0.9, 0.95]), ..up(guard, -0.45) };
            spent.roll += 0.08 * (d.t * 4.0).sin();
            pick(along(d.t, &[(0.0, spent, Ease::Linear), (d.len * 0.75, spent, Ease::Linear), (d.len, guard, Ease::Smooth)]), 5.0)
        }
        // Its shards raised up high over it, its fists after them, looking up; then its fists
        // thrown down and the shards with them
        (Move::Shardfall, Stage::Wind) => {
            let raised = Stance { raise: 1.0, pitch: -0.2, spin: 2.0, hands: fists([0.3, -0.85, 2.9], [0.3, 0.85, 2.9]), ..up(guard, 0.15) };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, raised, Ease::Gather)]), 6.0)
        }
        (Move::Shardfall, Stage::Act) => {
            let thrown = Stance { raise: 1.0, pitch: 0.28, hands: fists([1.25, -0.6, 1.0], [1.25, 0.6, 1.0]), ..guard };
            pick(along(d.t, &[(0.0, thrown, Ease::Linear)]), 30.0)
        }
        // Both fists raised high together, then brought down on the ground in front of it
        (Move::Slam, Stage::Wind) => {
            let raised = Stance { pitch: -0.2, hands: fists([0.35, -0.45, 3.0], [0.35, 0.45, 3.0]), ..up(guard, 0.2) };
            let smashed = Stance { pitch: 0.35, hands: fists([1.45, -0.4, 0.35], [1.45, 0.4, 0.35]), ..up(guard, -0.25) };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (lead, raised, Ease::Gather), (d.len, smashed, Ease::Snap)]), 9.0)
        }
        (Move::Slam, Stage::Act) => pick((Stance { pitch: 0.35, hands: fists([1.45, -0.4, 0.35], [1.45, 0.4, 0.35]), ..up(guard, -0.25) }, false), 24.0),
        // Its fists spread wide as a bolt of the wind gathers before its mask, then thrust at the
        // player as it's loosed
        (Move::Spit, Stage::Wind) => {
            let gathering = Stance { pitch: -0.08, spin: 2.2, hands: fists([0.9, -1.4, 2.0], [0.9, 1.4, 2.0]), ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, gathering, Ease::Gather)]), 7.0)
        }
        (Move::Spit, Stage::Act) => pick((Stance { pitch: 0.15, hands: fists([1.4, -0.5, 1.8], [1.4, 0.5, 1.8]), ..guard }, false), 20.0),
        (Move::Roar, Stage::Act) => {
            let roaring = Stance { pitch: -0.25, spin: 4.0, spread: 1.25, hands: fists([0.2, -1.8, 2.3], [0.2, 1.8, 2.3]), ..up(guard, 0.2) };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.3, roaring, Ease::Gather)]), 10.0)
        }
        (_, Stage::Reel) => {
            let reel = Stance { pitch: -0.35, roll: 0.2, spin: 0.3, core: [guard.core[0] - 0.4, 0.1, guard.core[2] - 0.1], hands: fists([0.2, -1.6, 2.2], [-0.2, 1.5, 1.9]), ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.16, reel, Ease::Gather), (d.len, guard, Ease::Smooth)]), 12.0)
        }
        (_, Stage::Stun) => {
            let mut dazed = Stance { pitch: 0.3, spin: 0.4, level: 0.3, hands: fists([0.6, -1.0, 0.9], [0.6, 1.0, 0.9]), ..up(guard, -0.4) };
            dazed.roll += 0.12 * (d.t * 2.2).sin();
            pick((dazed, false), 5.0)
        }
        (_, Stage::Recover) => pick(along(d.t, &[(0.0, Stance { raise: 0.0, ..guard }, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
        _ => pick((guard, false), 6.0),
    }
}

/// Dying: slowing and sagging, sinking lower and tipping over, before it falls apart
fn death(look: Look, rest: &Stance, t: f64) -> (Stance, f64) {
    let sunk = if stormbell(look) {
        Stance { spin: 0.0, curl: -0.3, spread: 0.45, pitch: 0.25, roll: 0.2, core: [0.0, 0.0, 1.0], ..*rest }
    } else {
        Stance { spin: 0.0, level: 0.6, pitch: 0.35, roll: -0.25, core: [0.1, 0.0, 0.9], hands: [[0.6, -1.1, 0.4], [0.6, 1.1, 0.4]], ..*rest }
    };
    let (s, _) = along(t, &[(0.0, *rest, Ease::Linear), (CRUMBLE, sunk, Ease::Gather)]);
    (s, 8.0)
}

/// Its pose for what it's doing, its drifting and flinching laid over it, worked out into its pieces;
/// and dying, its pieces falling into a heap
pub(super) fn animate(r: &mut Rig, m: &Moment) {
    let look = r.look;
    let rest = rest(look);
    let (want, rate) = match m.dying {
        Some(t) => death(look, &rest, t),
        None => fight(look, &rest, m),
    };
    let first = !r.posed;
    let state = r.drifter();
    if first {
        state.pose = want;
    }
    ease(&mut state.pose, &want, rate, m.dt);
    let mut s = state.pose;
    // Its rings turn on, each its own way, as fast as it's spinning them, and its shards circle
    // slower the higher they're raised
    for i in 0..3 {
        state.turns[i] += m.dt * s.spin * RING_TURN[i];
    }
    state.turns[3] += m.dt * (0.6 + 0.4 * s.spin) * (1.0 - 0.85 * s.raise.clamp(0.0, 1.0));
    let turns = state.turns;
    if m.dying.is_none() {
        // Leaning into the way it drifts, rising and falling slowly
        let (walking, running) = m.pace();
        s.pitch += 0.1 * walking + 0.12 * running;
        s.core[2] += 0.1 * (r.clock * 1.3).sin() + 0.05 * (r.clock * 0.47 + 1.0).sin();
        s.yaw += 0.25 * r.glance[0];
        let fl = r.flinched();
        s.pitch -= 0.4 * fl[0];
        s.core[0] -= 0.25 * fl[0];
        s.roll += 0.4 * fl[2];
        s.yaw += 0.3 * fl[1];
    }
    if stormbell(look) {
        bell(r, m, &s, &turns);
    } else {
        orrery(r, m, &s, &turns);
    }
    biped::fall_apart(r, m, CRUMBLE, DEATH);
}

/// The Aeolith's pieces where its pose puts them
fn orrery(r: &mut Rig, m: &Moment, s: &Stance, turns: &[f64; 4]) {
    let core = V::from(s.core);
    let body = rz(s.yaw) * ry(s.pitch) * rx(s.roll);
    r.bones[CORE] = Bone::new(core, body * rz(turns[3] * 0.5));
    r.bones[MASK] = Bone::new(core, body * rz(0.8 * r.glance[0]) * ry(0.7 * r.glance[1]));
    let level = s.level.clamp(0.0, 1.0);
    for i in 0..3 {
        let tip = rx(RING_TIP[i][0] * (1.0 - level)) * ry(RING_TIP[i][1] * (1.0 - level));
        r.bones[RINGS[i]] = Bone::new(core, body * rz(turns[3] * RING_ROUND[i]) * tip * rz(turns[i]));
    }
    // The great ring flung out at where it's aimed and back, see bosses::roster, or drawn back over
    // its shoulder to be
    if let Some(d) = m.doing.filter(|d| d.m == Move::Ringcast) {
        let spun = rz(turns[0] * 2.5);
        match d.stage {
            Stage::Wind => {
                let k = gather(d.t / d.len.max(0.1));
                let at = core + rz(s.yaw) * V::new(-0.45 * k, -0.45 * k, 0.4 * k);
                r.bones[RINGS[0]] = Bone::new(at, rz(s.yaw) * rx(1.3 * (1.0 - k) + 0.35 * k) * spun);
            }
            Stage::Act => {
                let f = (d.t / d.len.max(0.1)).clamp(0.0, 1.0);
                let out = if f < RINGCAST_LANDS {
                    smooth(f / RINGCAST_LANDS)
                } else if f < RINGCAST_BACK {
                    1.0
                } else {
                    1.0 - smooth((f - RINGCAST_BACK) / (1.0 - RINGCAST_BACK))
                };
                let to = m.aim.map_or(core, |a| a + V::new(0.0, 0.0, 0.3));
                let arc = 4.0 * out * (1.0 - out) * 0.9;
                let at = core.lerp(&to, out) + V::new(0.0, 0.0, arc);
                r.bones[RINGS[0]] = Bone::new(at, rx(0.22) * spun);
            }
            _ => {}
        }
    }
    // Its fists, bobbing, turned knuckles up the higher they're raised
    for k in 0..2 {
        let at = V::from(s.hands[k]) + V::new(0.0, 0.0, 0.05 * (r.clock * 1.5 + k as f64 * 2.0).sin());
        let raised = ((at.z - core.z) / 1.2).clamp(-1.0, 1.0);
        r.bones[FISTS[k]] = Bone::new(at, rz(s.yaw) * ry(s.pitch - 0.7 * raised));
    }
    // Its shards circling it, raised high and turned point down to be flung; gone once they're
    // flung, then coming back out of its heart to circle it again
    let (hidden, back) = match m.doing {
        Some(d) if d.m == Move::Shardfall && d.stage == Stage::Act => (true, 1.0),
        Some(d) if d.m == Move::Shardfall && d.stage == Stage::Recover => (d.t < 0.15, smooth((d.t - 0.15) / (d.len * 0.7).max(0.1))),
        _ => (false, 1.0),
    };
    let raise = s.raise.clamp(0.0, 1.2);
    for (k, &piece) in SHARDS.iter().enumerate() {
        let a = turns[3] * 0.9 + k as f64 / 6.0 * TAU;
        let round = SHARD_R * s.spread * (1.0 - 0.45 * raise);
        let high = 0.25 + 0.18 * (a * 2.0 + r.clock).sin() + raise * 1.6;
        let circling = core + rz(s.yaw * 0.5) * V::new(a.cos() * round, a.sin() * round, high);
        r.bones[piece] = Bone { at: core.lerp(&circling, back), turn: rz(a + s.yaw * 0.5) * ry(0.5 + raise * 2.1), shown: !hidden };
    }
    r.bones[VORTEX] = Bone::new(core - V::new(0.0, 0.0, 0.42), rz(turns[0] * 1.8) * rx(0.15 * s.pitch));
    // What it looses comes from before its mask; its fists are its hands
    let cast = core + body * V::new(0.75, 0.0, 0.0);
    r.sockets = Sockets { mouth: cast, hands: [r.bones[FISTS[0]].at, r.bones[FISTS[1]].at], tip: r.bones[FISTS[0]].at, grip: r.bones[FISTS[0]].at, cast };
}

/// The Stormbell's pieces where its pose puts them
fn bell(r: &mut Rig, m: &Moment, s: &Stance, turns: &[f64; 4]) {
    // Its bell beating slowly as it swims, quicker as it's stirred up
    let beat = turns[0].sin();
    let at = V::from(s.core) + V::new(0.0, 0.0, 0.07 * beat);
    let turn = rz(s.yaw) * ry(s.pitch) * rx(s.roll);
    r.bones[BELL] = Bone::new(at, turn * rz(turns[3] * 0.15));
    r.bones[HEART] = Bone::new(at + turn * V::new(0.0, 0.0, -0.03 * beat), turn * rz(turns[2] * 0.6));
    r.bones[FRILL] = Bone::new(at + turn * V::new(0.0, 0.0, -0.04 - 0.04 * beat), turn * rz(-turns[1] * 0.25) * rx(0.04 * (turns[0] + 1.0).sin()));
    // What its tendrils lash at: where it's aimed, or the ground in front of it
    let lash_at = match m.doing {
        Some(d) if d.m == Move::Thunderlash => m.aim.map_or(V::new(3.2, 0.0, 0.6), |a| a + V::new(0.0, 0.0, 0.6)),
        _ => V::new(2.2, 0.0, 0.1),
    };
    let lash = s.lash.clamp(0.0, 1.2);
    let mut tips = [V::zeros(); 8];
    for k in 0..8 {
        let a = k as f64 / 8.0 * TAU + 0.2 + turns[3] * 0.15;
        let out = V::new(a.cos(), a.sin(), 0.0);
        let root = at + turn * V::new(a.cos() * RIM_R * (1.0 + 0.05 * beat), a.sin() * RIM_R * (1.0 + 0.05 * beat), -0.12);
        // Hanging a little out from its rim and swaying, drawn up and out as it curls them, never
        // up over its bell, and trailing behind it as it drifts along
        let wave = (r.clock * 1.7 + k as f64 * 0.9).sin();
        let curl = s.curl.max(0.0);
        let o1 = ((0.12 + 0.08 * wave - 0.05 * beat) * s.spread + 1.05 * curl).clamp(-0.1, 1.3);
        let trail = turn * V::new(-0.45 * m.pace().0 - 0.3 * m.pace().1, 0.0, 0.0);
        let hang = |o: f64| (turn * V::new(a.cos() * o.sin(), a.sin() * o.sin(), -o.cos()) + trail).try_normalize(1e-9).unwrap_or(-V::z());
        // The tendrils in front lash out most, those behind hardly
        let front = (0.55 + 0.45 * a.cos()) * lash;
        let toward = (lash_at - root).try_normalize(1e-9).unwrap_or(-V::z());
        let d1 = hang(o1).lerp(&toward, front.min(1.0)).try_normalize(1e-9).unwrap_or(-V::z());
        r.bones[UPPER[k]] = Bone::new(root, aim(-V::z(), V::x(), d1, turn * out));
        let knee = root + d1 * TENDRIL;
        let wave2 = (r.clock * 1.7 + k as f64 * 0.9 - 0.8).sin();
        let o2 = (o1 + (0.1 + 0.25 * wave2) * s.spread + 0.5 * curl).clamp(-0.2, 1.75);
        let toward2 = (lash_at - knee).try_normalize(1e-9).unwrap_or(d1);
        let d2 = hang(o2).lerp(&toward2, (front * 0.9).min(1.0)).try_normalize(1e-9).unwrap_or(-V::z());
        r.bones[LOWER[k]] = Bone::new(knee, aim(-V::z(), V::x(), d2, turn * out));
        tips[k] = knee + d2 * TENDRIL;
    }
    let cast = at + turn * V::new(0.0, 0.0, -0.45);
    r.sockets = Sockets { mouth: cast, hands: [tips[0], tips[4]], tip: tips[0], grip: cast, cast };
}
