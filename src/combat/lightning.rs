//! Lightning, drawn as jagged, branching bolts that strike, flicker out and strike again down the
//! same channel a few times in a third of a second, each strike lighting up what's round it: a
//! rod's lightning leaping through a pack, one hop after another, a priest's spears arcing on, and
//! bolts out of the sky with a flash, crackling sparks and a scorch mark where they come down.
//!
//! Each stretch of a bolt is one instance of a glowing bar, see effects::BOLT_BARS, stretched to
//! the stretch's length by the shader, see AnimalInstance::shaped(): a bolt with its branches is a
//! couple of dozen of them at most, fewer than the sparks strung along a line that lightning used
//! to be, and there are only ever so many at once, see MOST_BOLTS and MOST_STRETCHES.

use super::*;
use crate::effects::BOLT_BARS;

/// When each strike of a bolt comes, seconds after the first, and how long each shows
const STRIKES: [f64; 3] = [0.0, 0.085, 0.19];
const STRIKE_SHOWN: f64 = 0.065;
/// How long the channel glows on faintly after the last strike, thinning
const AFTERGLOW: f64 = 0.08;
/// The most bolts there are at once, the oldest going first, and the most stretches of them drawn
/// in a frame, the main channels before their branches
const MOST_BOLTS: usize = 14;
const MOST_STRETCHES: usize = 60;
/// How far a bolt from the sky starts over where it comes down, in meters
pub(super) const SKY_BOLT_HEIGHT: f64 = 13.0;
/// How many scorch marks lightning leaves on the ground at once
const MOST_SCORCHES: usize = 3;

/// A bolt of lightning
pub(super) struct Bolt {
    from: Vector3<f64>,
    to: Vector3<f64>,
    look: usize,
    /// How thick its main channel is, in meters, how many branches fork off it, and how many times
    /// it strikes, see STRIKES
    thick: f64,
    branches: u8,
    strikes: u8,
    /// Seconds till it strikes, for one hop of a chain leaping on after the last, and seconds since
    delay: f64,
    age: f64,
    seed: u32,
    /// Its channel and branches as the strike now showing has them: runs of points, each with how
    /// thick it is
    paths: Vec<(Vec<Vector3<f64>>, f64)>,
    /// Which strike the paths are for, None till it's first struck
    struck: Option<usize>,
    /// How brightly its strikes light what's round, how far, and whether it rolls thunder
    flash: f64,
}

/// A jagged run of points from `from` to `to`, each stretch split `levels` times with its middle
/// pushed off to the side by up to `wander` of its length, less each time, from `seed`
fn jagged(from: Vector3<f64>, to: Vector3<f64>, levels: u32, wander: f64, seed: &mut u32) -> Vec<Vector3<f64>> {
    let mut points = vec![from, to];
    let mut amount = wander;
    for _ in 0..levels {
        let mut next = Vec::with_capacity(points.len() * 2);
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let along = b - a;
            let length = along.norm();
            let way = along / length.max(1e-9);
            let side = way.cross(&if way.z.abs() < 0.9 { Vector3::z() } else { Vector3::x() }).normalize();
            let over = way.cross(&side);
            let (u, v) = (random(seed) * 2.0 - 1.0, random(seed) * 2.0 - 1.0);
            next.push(a);
            next.push((a + b) * 0.5 + (side * u + over * v) * length * amount);
        }
        next.push(*points.last().unwrap());
        points = next;
        amount *= 0.55;
    }
    points
}

impl Bolt {
    /// Its channel and branches for strike `k`: the main channel much the same each strike, down
    /// the same way, the branches forking off it somewhere new each time
    fn shape(&mut self, k: usize, vpm: f64) {
        let length = (self.to - self.from).norm() / vpm;
        let levels = if length > 9.0 { 4 } else if length > 3.0 { 3 } else { 2 };
        let mut main_seed = self.seed;
        let mut channel = jagged(self.from, self.to, levels, 0.2, &mut main_seed);
        // A little shift of the channel each strike, never at its ends
        let mut seed = hash(self.seed ^ (k as u32 + 1).wrapping_mul(0x9e37_79b9));
        let n = channel.len();
        for (j, p) in channel.iter_mut().enumerate().skip(1).take(n.saturating_sub(2)) {
            let s = (j as f64 / (n - 1) as f64 * PI).sin();
            *p += Vector3::new(random(&mut seed) - 0.5, random(&mut seed) - 0.5, random(&mut seed) - 0.5) * 0.12 * length.min(6.0) * s * vpm * 0.15;
        }
        let mut paths = vec![(channel.clone(), self.thick)];
        for _ in 0..self.branches {
            // Forking off partway along, on the way it's going but turned off from it
            let at = 1 + ((random(&mut seed) * 0.7 + 0.15) * (n - 2) as f64) as usize;
            let at = at.min(n - 2);
            let start = channel[at];
            let on = (channel[(at + 1).min(n - 1)] - start).try_normalize(1e-9).unwrap_or_else(Vector3::z);
            let off = Vector3::new(random(&mut seed) - 0.5, random(&mut seed) - 0.5, random(&mut seed) - 0.5) * 2.0;
            let way = (on + off * 0.9).try_normalize(1e-9).unwrap_or(on);
            let reach = (self.to - start).norm() * (0.25 + 0.3 * random(&mut seed));
            let end = start + way * reach;
            paths.push((jagged(start, end, levels.saturating_sub(1).max(1), 0.25, &mut seed), self.thick * 0.55));
        }
        self.paths = paths;
    }

    /// The world moved over by `shift` voxels
    pub(super) fn recenter(&mut self, shift: Vector3<f64>) {
        self.from -= shift;
        self.to -= shift;
        for (points, _) in &mut self.paths {
            for p in points {
                *p -= shift;
            }
        }
    }

    /// Whether it shows now, and how bright, 0-1: each strike for a moment, then a faint glow
    fn shown(&self) -> f64 {
        if self.delay > 0.0 {
            return 0.0;
        }
        let strikes = &STRIKES[..self.strikes.clamp(1, 3) as usize];
        for &s in strikes {
            if self.age >= s && self.age < s + STRIKE_SHOWN {
                return 1.0 - 0.3 * (self.age - s) / STRIKE_SHOWN;
            }
        }
        let last = strikes.last().unwrap() + STRIKE_SHOWN;
        if self.age >= last && self.age < last + AFTERGLOW { 0.35 * (1.0 - (self.age - last) / AFTERGLOW) } else { 0.0 }
    }

    fn over(&self) -> bool {
        self.delay <= 0.0 && self.age > STRIKES[self.strikes.clamp(1, 3) as usize - 1] + STRIKE_SHOWN + AFTERGLOW
    }
}

impl Combat {
    /// A bolt of lightning of `look` from `from` to `to`, `thick` meters thick, with `branches`
    /// forking off it, striking `strikes` times, after `delay` seconds; `flash` how brightly each
    /// strike lights what's round, 0 for none
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lightning(&mut self, from: Vector3<f64>, to: Vector3<f64>, look: usize, thick: f64, branches: u8, strikes: u8, delay: f64, flash: f64) {
        if self.bolts.len() >= MOST_BOLTS {
            self.bolts.remove(0);
        }
        let seed = hash(self.rng ^ (from.x as u32).wrapping_mul(0x85eb_ca6b) ^ (to.y as u32));
        self.rng = hash(self.rng.wrapping_add(0x6d2b_79f5));
        self.bolts.push(Bolt { from, to, look, thick, branches, strikes, delay, age: 0.0, seed, paths: Vec::new(), struck: None, flash });
    }

    /// A bolt of lightning of `look` coming down out of the sky onto `ground`, `power` 1 for an
    /// ordinary one: forked, striking three times, a flash lighting everything round, brighter by
    /// night, sparks crackling off where it hits, thunder, and a scorch mark left there
    pub(super) fn sky_bolt(&mut self, ground: Vector3<f64>, look: usize, power: f64, delay: f64) {
        let vpm = self.vpm;
        let lean = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.0) * 3.0 * vpm;
        let top = ground + Vector3::new(0.0, 0.0, SKY_BOLT_HEIGHT * power.sqrt() * vpm) + lean;
        self.lightning(top, ground, look, 0.12 * power.sqrt(), 3, 3, delay, 1.6 * power);
        if delay <= 0.0 {
            self.bolt_lands(ground, look, power);
        } else {
            // The rest once it's struck, see update_bolts()
            if let Some(b) = self.bolts.last_mut() {
                b.flash = -1.6 * power;
            }
        }
    }

    /// Where a bolt out of the sky comes down at `ground`: sparks crackling off, thunder, a scorch
    /// left there, and the view shaking if it's near
    fn bolt_lands(&mut self, ground: Vector3<f64>, look: usize, power: f64) {
        let vpm = self.vpm;
        let up = Vector3::z();
        self.sparks(ground + up * 0.15 * vpm, (8.0 * power).round() as usize, 5.0, look, 0.8, 0.35);
        for _ in 0..(6.0 * power) as usize {
            let v = (self.any_direction() * 1.5 + up) * 3.5 * vpm;
            self.fine(ground + up * 0.1 * vpm, v, look, 0.3, 0.6);
        }
        self.blasts.push(Blast { position: ground + up * 0.3 * vpm, look, radius: 0.5 * power, life: 0.2, lifetime: 0.2 });
        self.cues.push(Cue::at(Sound::Thunder, ground, (0.6 * power).min(1.0) as f32).pitched(1.1 / power.sqrt() as f32));
        self.cues.push(Cue::at(Sound::Crackle, ground, 0.8).pitched(0.9));
        self.scorch(ground, 0.9 * power.sqrt());
        let near = (1.0 - (ground - self.eye_at).norm() / vpm / 30.0).max(0.0);
        self.shake = self.shake.max(near * 0.3 * power.min(1.5));
    }

    /// A scorch mark left on the ground at `ground`, `radius` meters round: a ring of burnt earth
    /// with lightning still jumping round it a moment, dying down, hurting nothing
    pub(super) fn scorch(&mut self, ground: Vector3<f64>, radius: f64) {
        self.scorch_with(ground, radius, Element::Storm, 2.6);
    }

    /// What an element leaves on the ground a while where it struck, `radius` meters round `ground`,
    /// for `life` seconds, hurting nothing: a scorch for lightning or fire, frozen ground for frost,
    /// see pool_look(); never more than a few of each at once, the oldest going first
    pub(super) fn scorch_with(&mut self, ground: Vector3<f64>, radius: f64, element: Element, life: f64) {
        let Some(profile) = self.last_profile.clone() else {
            return;
        };
        let mark = |l: &Lingering| l.pool && l.damage == 0.0 && l.tick.is_infinite() && l.element == Some(element);
        if self.lingering.iter().filter(|l| mark(l)).count() >= MOST_SCORCHES
            && let Some(old) = self.lingering.iter().position(mark)
        {
            self.lingering.remove(old);
        }
        self.lingering.push(Lingering { pool: true, tick: f64::INFINITY, ..Lingering::new(ground, radius, life, Some(element), &profile, 0.0) });
    }

    /// Every bolt going on: each striking as its time comes, its channel and branches forking out
    /// anew, lighting up what's round it; gone once it's done
    pub(super) fn update_bolts(&mut self, dt: f64) {
        let vpm = self.vpm;
        let mut landed = Vec::new();
        for b in &mut self.bolts {
            if b.delay > 0.0 {
                b.delay -= dt;
                if b.delay > 0.0 {
                    continue;
                }
                b.age = -b.delay;
                b.delay = 0.0;
            } else {
                b.age += dt;
            }
            let due = STRIKES[..b.strikes.clamp(1, 3) as usize].iter().rposition(|&s| b.age >= s);
            if let Some(k) = due
                && b.struck != Some(k)
            {
                b.struck = Some(k);
                b.shape(k, vpm);
                // Lighting what's round it as it first strikes, a bolt big enough to: its strikes
                // after flicker in that light, see lights()
                let strength = b.flash.abs();
                if strength >= 0.9 && k == 0 {
                    let color = crate::weapons::Element::ALL.get(b.look.wrapping_sub(1)).map_or([0.8, 0.85, 1.0], |e| e.light());
                    // White-hot, flickering, as storm light does, see lights()
                    let color = if b.look == Element::look(Some(Element::Storm)) { Element::Storm.light() } else { color };
                    let reach = 4.0 + 4.5 * strength;
                    let life = STRIKES[b.strikes.clamp(1, 3) as usize - 1] + STRIKE_SHOWN;
                    self.flashes.push(Flash { position: b.to + Vector3::new(0.0, 0.0, 0.8 * vpm), color, reach, life, lifetime: life });
                }
                // A bolt from the sky that waited: its landing as it first strikes
                if k == 0 && b.flash < 0.0 {
                    landed.push((b.to, b.look, -b.flash / 1.6));
                }
            }
        }
        for (at, look, power) in landed {
            self.bolt_lands(at, look, power);
        }
        self.bolts.retain(|b| !b.over());
    }

    /// How every bolt showing now is drawn, each stretch of it a glowing bar from one point to the
    /// next, the main channels first, as thick as it is and as bright as it shows, see Bolt::shown()
    pub(super) fn bolt_instances(&self, view: &View, out: &mut Vec<(f64, AnimalInstance)>) {
        let vpm = view.vpm;
        let mut drawn = 0;
        // Main channels, then branches
        for pass in 0..2 {
            for b in &self.bolts {
                let shown = b.shown();
                if shown <= 0.0 {
                    continue;
                }
                let paths = if pass == 0 { &b.paths[..b.paths.len().min(1)] } else { b.paths.get(1..).unwrap_or(&[]) };
                for (points, thick) in paths {
                    for pair in points.windows(2) {
                        if drawn >= MOST_STRETCHES {
                            return;
                        }
                        // Thinner the dimmer it shows, never filling the view right at the eye
                        let mid = (pair[0] + pair[1]) * 0.5;
                        let d = (mid - view.eye).norm() / vpm;
                        if d < 0.5 {
                            continue;
                        }
                        let near = (d / 2.0).clamp(0.35, 1.0);
                        let thick = thick * (0.45 + 0.55 * shown) * near * (d / 20.0).max(1.0).sqrt();
                        if let Some(instance) = self.bar(pair[0], pair[1], thick, b.look) {
                            out.push((d * 0.1, instance));
                            drawn += 1;
                        }
                    }
                }
            }
        }
    }

    /// A glowing bar of `look` from `a` to `b`, `thick` meters across, see effects::BOLT_BARS
    pub(super) fn bar(&self, a: Vector3<f64>, b: Vector3<f64>, thick: f64, look: usize) -> Option<AnimalInstance> {
        let vpm = self.vpm;
        let along = b - a;
        let length = along.norm();
        if length < 1e-3 || thick <= 0.0 {
            return None;
        }
        let way = along / length;
        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(|| UnitQuaternion::from_axis_angle(&Vector3::z_axis(), PI));
        let (model, bar) = self.model_at(BOLT_BARS + look.min(ELEMENT_LOOKS - 1));
        let [sx, sy, sz] = bar.0.size.map(|s| s as f64);
        // Each voxel of it a third of its thickness; stretched along its length to reach
        let scale = thick * vpm / sy.max(1.0);
        let stretch = length / (scale * sx);
        let position = (a + b) * 0.5 - rotation * Vector3::new(0.0, 0.0, sz * 0.5 * scale);
        let reach = length * 0.5 + thick * vpm;
        // White-hot enough to be seen by day, see TINT_BOLT
        Some(AnimalInstance::new(model, position, rotation, scale, reach, MOVING).shaped([stretch, 1.0], [0.0; 4]).tinted(crate::animals::TINT_BOLT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bolt's channel runs from one end to the other, jagged, never straight, its branches fork
    /// off it, and each strike it forks anew down much the same channel, then it's gone
    #[test]
    fn lightning_forks_flickers_and_strikes_again() {
        let mut combat = Combat::new(1, 0);
        let vpm = combat.vpm;
        let (from, to) = (Vector3::new(0.0, 0.0, 12.0) * vpm, Vector3::new(1.0, 0.5, 0.0) * vpm);
        combat.lightning(from, to, Element::look(Some(Element::Storm)), 0.1, 3, 3, 0.0, 1.0);
        let dt = 1.0 / 120.0;
        let mut shapes = Vec::new();
        let mut shown_frames = 0;
        let mut dark_frames = 0;
        for _ in 0..60 {
            combat.update_bolts(dt);
            let Some(b) = combat.bolts.first() else {
                break;
            };
            if b.shown() > 0.5 {
                shown_frames += 1;
            } else {
                dark_frames += 1;
            }
            if shapes.last() != Some(&b.struck) {
                shapes.push(b.struck);
                let (channel, _) = &b.paths[0];
                assert_eq!(channel.first(), Some(&from));
                assert_eq!(channel.last(), Some(&to));
                // Jagged: its points stray off the straight line between its ends
                let off = channel.iter().map(|p| crate::combat::signatures::off_line(*p, from, to) / vpm).fold(0.0, f64::max);
                assert!(off > 0.2, "a channel only {off:.2} m off straight");
                assert_eq!(b.paths.len(), 4, "a channel and its three branches");
            }
        }
        assert_eq!(shapes.len(), 3, "it struck {} times", shapes.len());
        assert!(shown_frames > 6 && dark_frames > 6, "it shows {shown_frames} frames and is dark {dark_frames}: it should flicker");
        assert!(combat.bolts.is_empty(), "it's gone once it's done");
    }
}
