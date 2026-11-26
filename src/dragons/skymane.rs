//! The Skymane: a great lion on wings of feathers, tawny with a mane of bronze and gold, white
//! wings tipped with gold, a circlet of gold beads across its brow and the sigils of the holy light
//! glowing on its flanks. It keeps the towers and colonnades up on the bare rock of the mountains
//! and the high ruins of the plains.
//!
//! It's flown and posed as the dragons are, see src\dragons.rs, out of jointed pieces of its own
//! shape, see the shapes here. On the ground it rakes at whoever's close with its forepaws, one and
//! then the other, pounces on them from further off, and rears up and roars, the holy light in it
//! rolling out across the ground in rings, to be jumped; from the ground or the air it beats its
//! wings forward and flings a storm of gold-edged feathers at them; and it takes to the air to
//! swoop down on them, raking them as it passes, and to fall on them out of the sky.

use super::*;

/// How fast the rings of its roar run out across the ground, in meters a second, how far, and
/// seconds into its roar each starts
const SHOCK_PACE: f64 = 12.0;
const SHOCK_REACH: f64 = 9.0;
const SHOCK_RINGS: [f64; 3] = [0.1, 0.45, 0.8];
/// How long its pounce takes through the air at the least, and how high it goes, in seconds and
/// meters, at its size
const POUNCE_TIME: f64 = 0.55;
const POUNCE_HIGH: f64 = 1.6;
/// Seconds into its rake each forepaw comes down, of how long it takes
const RAKES: [f64; 2] = [0.22, 0.55];
const RAKE_SECONDS: f64 = 0.8;

/// Its moves, near and far, on the ground and in the air, see dragons::Skill
pub(super) const SKILLS: [Skill; 9] = [
    skill(Rake, false, 0.0, 3.8, 2.4, 0.5, 0.7, 0, 3.0),
    skill(Pounce, false, 4.5, 14.0, 6.0, 0.75, 0.9, 0, 2.4),
    skill(Shockroar, false, 0.0, 9.0, 10.0, 0.9, 1.1, 0, 1.6),
    skill(Featherstorm, false, 6.0, 26.0, 7.5, 0.8, 0.7, 0, 1.6),
    skill(Gust, false, 0.0, 6.0, 10.0, 0.7, 0.6, 0, 0.5),
    skill(Swoop, true, 6.0, 40.0, 6.0, 0.7, 0.4, 0, 2.2),
    skill(Dive, true, 6.0, 40.0, 10.0, 1.1, 0.4, 0, 2.0),
    skill(Featherstorm, true, 8.0, 36.0, 7.0, 0.75, 0.4, 0, 1.6),
    skill(Hover, true, 0.0, 30.0, 10.0, 0.45, 0.4, 0, 0.8),
];

/// Its own moves, that no other dragon makes
#[cfg(test)]
pub const OWN: [Move; 4] = [Rake, Pounce, Shockroar, Featherstorm];

/// How it's built: as big as a drake, four legs, a short thick neck and a long thin tail, tawny,
/// its wings white, its claws bone, the holy light in it and in its eyes
pub(super) fn spec() -> Spec {
    let hide = Hide { scale: FUR_TAN, dark: FUR_BROWN, belly: FUR_CREAM, wing: FUR_WHITE, horn: BONE, glow: GLOW_HOLY, eye: GLOW_HOLY };
    Spec { s: 1.75, four: true, necks: 2, tails: 4, neck: 0.22, tail: 0.4, girth: 1.12, span: 1.12, hide, crown: Crown::Mane, wing: WingShape::Feathered, club: false }
}

// Its shapes, each in its own frame with its joint at the middle, in units, as the dragons' are

/// Its body: a lion's, deep in the chest and lean at the waist, its mane over its shoulders, the
/// holy sigils on its flanks
pub(super) fn torso(p: &mut Vec<Part>, sp: &Spec) {
    let (h, g) = (sp.hide, sp.girth);
    for side in [-1.0, 1.0] {
        // Sigils glowing on its flanks, three to a side
        for k in 0..3 {
            p.push(ball([0.05 - 0.17 * k as f64, side * 0.29 * g, 0.06 * g], 0.032, h.glow));
        }
        // Gold capping where its wings join it
        p.push(ball([0.3, side * 0.25 * g, 0.22 * g], 0.085 * g, GOLD));
        p.push(ball([0.42, side * 0.2 * g, -0.08 * g], 0.14 * g, h.scale));
        p.push(ball([-0.38, side * 0.2 * g, -0.04 * g], 0.15 * g, h.scale));
    }
    // Its mane, deep over its shoulders and chest, gold tufts standing out of it
    for k in 0..7 {
        let a = -1.2 + k as f64 * 0.4;
        p.push(limb([0.45, a.sin() * 0.24 * g, 0.12 * g + a.cos() * 0.12], [0.3, a.sin() * 0.36 * g, 0.18 * g + a.cos() * 0.2], 0.07, 0.03, GOLD));
    }
    p.push(ellipsoid([0.48, 0.0, 0.08 * g], [0.3, 0.36 * g, 0.38 * g], BRONZE));
    p.push(ellipsoid([0.02, 0.0, -0.16 * g], [0.5, 0.2 * g, 0.12 * g], h.belly));
    p.push(ellipsoid([0.32, 0.0, 0.0], [0.36, 0.31 * g, 0.33 * g], h.scale));
    p.push(ellipsoid([-0.05, 0.0, 0.03 * g], [0.62, 0.26 * g, 0.26 * g], h.scale));
    p.push(ellipsoid([-0.45, 0.0, 0.02 * g], [0.3, 0.27 * g, 0.28 * g], h.scale));
}

/// A piece of its short neck, thick with mane
pub(super) fn neck(p: &mut Vec<Part>, sp: &Spec, k: usize) {
    let h = sp.hide;
    let l = sp.neck;
    let r = 0.17 * sp.girth.sqrt() * (1.0 - 0.1 * k as f64);
    for j in 0..4 {
        let a = -0.9 + j as f64 * 0.6;
        p.push(limb([l * 0.6, a.sin() * r * 0.8, a.cos() * r * 0.8], [l * 0.1, a.sin() * r * 1.9, a.cos() * r * 1.6], 0.06, 0.025, if j % 2 == 0 { GOLD } else { BRONZE }));
    }
    p.push(ellipsoid([l * 0.5, 0.0, 0.04], [l * 0.75, r * 1.55, r * 1.5], BRONZE));
    p.push(limb([0.0, 0.0, 0.0], [l, 0.0, 0.0], r, r * 0.95, h.scale));
}

/// Its head: a lion's, its muzzle broad, its ears round, a ruff of mane behind its face, gold beads
/// across its brow and the holy light in its eyes
pub(super) fn head(p: &mut Vec<Part>, sp: &Spec) {
    let h = sp.hide;
    for side in [-1.0, 1.0] {
        p.push(ball([0.25, side * 0.075, 0.06], 0.03, h.eye));
        p.push(limb([0.17, side * 0.09, 0.105], [0.3, side * 0.07, 0.09], 0.028, 0.02, h.dark));
        // Its ears, round, half hidden in its ruff
        p.push(limb([0.03, side * 0.11, 0.11], [-0.01, side * 0.14, 0.2], 0.045, 0.025, h.scale));
        // Whisker pads
        p.push(ball([0.4, side * 0.04, -0.04], 0.04, h.belly));
        // Fangs, the upper ones showing
        p.push(limb([0.38, side * 0.035, -0.07], [0.38, side * 0.035, -0.12], 0.014, 0.004, BONE));
    }
    // Gold beads across its brow, and a spike of gold standing up out of its ruff over them
    for k in 0..5 {
        let y = (k as f64 - 2.0) * 0.035;
        p.push(ball([0.14 - 0.02 * y.abs() / 0.035, y, 0.15], 0.018, GOLD));
    }
    p.push(limb([0.05, 0.0, 0.18], [-0.02, 0.0, 0.3], 0.022, 0.006, GOLD));
    p.push(ball([0.45, 0.0, 0.01], 0.032, OBSIDIAN));
    // Its ruff, framing its face, tufts of gold standing out of it
    for k in 0..8 {
        let a = k as f64 / 8.0 * TAU;
        p.push(limb([0.0, a.sin() * 0.14, a.cos() * 0.14], [-0.12, a.sin() * 0.3, a.cos() * 0.28], 0.05, 0.02, if k % 2 == 0 { GOLD } else { BRONZE }));
    }
    p.push(ellipsoid([-0.04, 0.0, 0.0], [0.12, 0.26, 0.25], BRONZE));
    p.push(limb([0.2, 0.0, -0.02], [0.43, 0.0, -0.04], 0.1, 0.075, h.scale));
    p.push(ellipsoid([0.12, 0.0, 0.03], [0.18, 0.14, 0.14], h.scale));
}

/// Its lower jaw, a lion's, fangs standing up out of it
pub(super) fn jaw(p: &mut Vec<Part>, sp: &Spec) {
    let h = sp.hide;
    for side in [-1.0, 1.0] {
        p.push(limb([0.3, side * 0.035, 0.0], [0.3, side * 0.035, 0.045], 0.012, 0.004, BONE));
    }
    p.push(ellipsoid([0.2, 0.0, 0.03], [0.13, 0.04, 0.02], MEAT));
    p.push(limb([0.0, 0.0, -0.02], [0.34, 0.0, -0.04], 0.065, 0.045, h.belly));
}

/// A piece of its long thin tail, a tuft of mane at its end
pub(super) fn tail(p: &mut Vec<Part>, sp: &Spec, k: usize) {
    let h = sp.hide;
    let n = sp.tails as f64;
    let l = sp.tail;
    let (r0, r1) = (0.07 - 0.025 * k as f64 / n, 0.07 - 0.025 * (k + 1) as f64 / n);
    if k + 1 == sp.tails {
        p.push(ellipsoid([-l - 0.06, 0.0, 0.0], [0.14, 0.08, 0.08], BRONZE));
        p.push(limb([-l, 0.0, 0.0], [-l - 0.2, 0.0, 0.02], 0.05, 0.02, GOLD));
    }
    p.push(limb([0.0, 0.0, 0.0], [-l, 0.0, 0.0], r0, r1, h.scale));
}

/// A feather lying along the wing out from `root` to `tip`, `width` wide
fn feather(root: Vec3, tip: Vec3, width: f64, material: u8) -> Part {
    limb(root, tip, 0.03, 0.012, material).stretched([(width / 0.03).max(1.0), 1.0, 0.45])
}

/// The inner piece of a wing, from the shoulder out to the wrist: its arm, the coverts over its
/// leading edge, and long feathers back from it to the body, white tipped with gold
pub(super) fn wing_in(p: &mut Vec<Part>, sp: &Spec, side: f64) {
    let h = sp.hide;
    let w = wrist(sp);
    let w = [w.x, side * w.y, w.z];
    p.push(ball(w, 0.055, GOLD));
    p.push(limb([0.0, 0.0, 0.0], w, 0.06, 0.045, h.wing));
    let reach = w[1].abs();
    // Coverts in a row along its leading edge
    p.push(limb([0.02, side * 0.05, 0.03], [w[0] - 0.02, w[1], 0.03], 0.07, 0.05, h.belly).stretched([1.6, 1.0, 0.5]));
    let strips = 8;
    for i in 0..strips {
        let f = (i as f64 + 0.5) / strips as f64;
        let y = side * (0.04 + (reach - 0.04) * f);
        let lead = [0.06 * f, y, 0.02];
        let back = -0.85 + 0.25 * f;
        p.push(skin(lead, [back + 0.08, y, 0.0], (reach - 0.04) / strips as f64, h.wing));
        // Its gold tip, pointed back past the rest
        p.push(feather([back + 0.1, y, 0.0], [back - 0.08, y, -0.01], 0.045, GOLD));
    }
}

/// The outer piece, from the wrist out to the tip: long primary feathers fanned out from it, the
/// longest at the tip, each tipped with gold
pub(super) fn wing_out(p: &mut Vec<Part>, sp: &Spec, side: f64) {
    let h = sp.hide;
    let l = WING_OUT * sp.span;
    p.push(limb([0.0, 0.0, 0.0], [-0.1, side * l * 0.35, 0.02], 0.045, 0.03, h.wing));
    for k in 0..8 {
        let f = k as f64 / 7.0;
        // From along the bones of its hand, fanning out from straight back to straight out
        let root = [-0.04 - 0.04 * f, side * (0.05 + 0.32 * l * f), 0.02];
        let turn = 1.45 - 1.3 * f;
        let long = 0.5 + 0.55 * f;
        let tip = [root[0] - turn.sin() * long, root[1] + side * turn.cos() * long, 0.0];
        let gold = [root[0] + (tip[0] - root[0]) * 0.78, root[1] + (tip[1] - root[1]) * 0.78, 0.0];
        p.push(feather(gold, tip, 0.04, GOLD));
        p.push(feather(root, gold, 0.06, h.wing));
    }
    // The coverts over the bases of them
    p.push(ellipsoid([-0.12, side * l * 0.18, 0.02], [0.17, l * 0.2, 0.03], h.wing));
}

/// A foreleg's or a hind leg's upper part: a lion's, heavy at the shoulder and the haunch
pub(super) fn thigh(p: &mut Vec<Part>, sp: &Spec, hind: bool) {
    let (h, g) = (sp.hide, sp.girth);
    let (lt, _) = leg_lengths(hind);
    p.push(ellipsoid([0.02, 0.0, -lt * 0.3], [0.14 * g, 0.1 * g, 0.22], h.scale));
    p.push(limb([0.0, 0.0, 0.0], [KNEE_AHEAD, 0.0, -lt], 0.11 * g, 0.075 * g, h.scale));
}

/// A shin and its paw: broad and round, claws of bone just showing
pub(super) fn shin_and_paw(p: &mut Vec<Part>, sp: &Spec, hind: bool, tilt: f64) {
    let (h, g) = (sp.hide, sp.girth);
    let (_, ls) = leg_lengths(hind);
    let ankle = [0.0, 0.0, -ls];
    let (c, s) = (tilt.cos(), tilt.sin());
    let at = |x: f64, y: f64, z: f64| [ankle[0] + x * c + z * s, y, ankle[2] - x * s + z * c];
    for y in [-0.05, -0.017, 0.017, 0.05] {
        p.push(limb(at(0.12, y * g, -0.02), at(0.16, y * g, -0.05), 0.014, 0.004, h.horn));
    }
    p.push(ellipsoid(at(0.05, 0.0, -0.015), [0.1, 0.075 * g, 0.045], h.scale));
    p.push(limb([0.0, 0.0, 0.0], ankle, 0.07 * g, 0.055 * g, h.scale));
}

// Its moves, played out as the dragons' are, see think()

/// Getting ready to make move `m`, `t` seconds of `wind` into it
#[allow(clippy::too_many_arguments)]
pub(super) fn wind(m: Move, t: f64, wind: f64, d: &mut Dragon, h: &Here, boss: &mut Boss, goal: &mut Goal, want: &mut Rig, wings: &mut WingSet) {
    let vpm = h.vpm;
    let sp = h.sp;
    let s = sp.s;
    let feet = h.feet.unwrap_or(boss.home);
    let forward = Vector3::new(d.yaw.cos(), d.yaw.sin(), 0.0);
    let p = (t / wind).min(1.0);
    let e = ease(p);
    let (_, claw_r, _, _, _) = reaches(&sp);
    match m {
        // Crouched, one forepaw drawn back high, snarling
        Rake => {
            goal.face = Some(feet);
            look_at(d, h, want, feet + Vector3::new(0.0, 0.0, 0.8 * vpm), 1.0);
            want.shift[2] -= 0.05 * e;
            want.pitch += 0.08 * e;
            want.legs[2] = LegPose { swing: 0.2 + 1.0 * e, knee: 0.45 + 0.9 * e, plant: 1.0 - e, foot: [0.0; 2] };
            want.jaw = 0.35 * e;
            *wings = WingSet::held(blend(FOLDED, MANTLED, 0.4 * e));
            let at = d.center + forward * bite_reach(&sp) * vpm;
            mark(boss, Vector3::new(at.x, at.y, ground_of(h, at)), claw_r, 1.0 - p);
        }
        // Down low on its haunches, its tail lashing, its wings half open, its eyes on where it'll
        // land, which shows
        Pounce => {
            goal.face = Some(feet);
            look_at(d, h, want, feet, 1.0);
            want.shift[2] -= 0.18 * e;
            want.shift[0] -= 0.06 * e;
            want.pitch -= 0.06 * e;
            want.neck[0][1] -= 0.25 * e;
            for j in 0..4 {
                want.tail[j][0] += 0.35 * (t * 11.0 + j as f64 * 0.8).sin() * e;
            }
            *wings = WingSet::held(blend(FOLDED, MANTLED, 0.7 * e));
            if t < wind - 0.25 {
                d.aim = feet;
            }
            mark(boss, d.aim, 1.4 * s.sqrt() + 0.4, 1.0 - p);
        }
        // Rearing back, its mane standing up, drawing breath, its wings raised high; the rings its
        // roar will roll out to shown round it
        Shockroar => {
            goal.face = Some(feet);
            rear(want, &sp, 1.1 * e);
            want.jaw = 0.4 * p;
            *wings = WingSet::held(blend(MANTLED, RAISED, e));
            let ground = Vector3::new(d.center.x, d.center.y, ground_of(h, d.center));
            mark(boss, ground, reach_of(s), 1.0 - p);
        }
        // Its wings raised high and swept forward, the light gathering in its feathers
        Featherstorm => {
            goal.face = Some(feet);
            *wings = WingSet { base: blend(MANTLED, WingPose { flap: 1.25, sweep: -0.45, twist: 0.25, fold: 0.05, bend: 0.35 }, e), amp: 0.03, rate: 1.0, hover: 0.0, quick: 1.8 };
            if d.airborne {
                goal.to = d.center;
                goal.speed = 2.0;
                want.pitch += 0.3 * e;
            } else {
                want.pitch += 0.18 * e;
                lift_fore(want, 0.6 * e, LegPose { swing: 0.8, knee: 0.9, ..Default::default() });
            }
            want.neck[0][1] += 0.2 * e;
            if t < wind - 0.3 {
                d.aim = feet;
            }
            mark(boss, d.aim, 2.6, 1.0 - p);
        }
        _ => {}
    }
}

/// How far the rings of its roar run out, in meters
fn reach_of(s: f64) -> f64 {
    SHOCK_REACH * (s / 1.75).sqrt()
}

/// Making move `m`, `t` seconds into it: whether it's done
#[allow(clippy::too_many_arguments)]
pub(super) fn act(m: Move, t: f64, d: &mut Dragon, h: &Here, boss: &mut Boss, goal: &mut Goal, want: &mut Rig, wings: &mut WingSet) -> bool {
    let vpm = h.vpm;
    let sp = h.sp;
    let s = sp.s;
    let pw = power(h.look);
    let (_, claw_r, _, _, _) = reaches(&sp);
    let feet = h.feet.unwrap_or(boss.home);
    let forward = Vector3::new(d.yaw.cos(), d.yaw.sin(), 0.0);
    let crossed = |at: f64| t >= at && t - h.dt < at;
    match m {
        // One forepaw raking down across in front, then the other, lunging in after each
        Rake => {
            goal.face = Some(feet);
            goal.to = d.center + forward * 2.0 * vpm;
            goal.speed = 1.5 * s.sqrt();
            look_at(d, h, want, feet + Vector3::new(0.0, 0.0, 0.6 * vpm), 1.0);
            for (k, &at) in RAKES.iter().enumerate() {
                let leg = 2 + k;
                let local = (t - (at - 0.18)) / 0.3;
                if (0.0..1.0).contains(&local) {
                    let up = (PI * local).sin();
                    want.legs[leg] = LegPose { swing: 0.2 + 1.2 * up, knee: 0.45 + 0.8 * up * (1.0 - local), plant: 1.0 - up, foot: [0.0; 2] };
                    want.roll += if k == 0 { 0.06 } else { -0.06 } * up;
                }
                if crossed(at) {
                    let side = Vector3::new(-forward.y, forward.x, 0.0) * if k == 0 { 0.4 } else { -0.4 } * s;
                    let hit = d.center + forward * bite_reach(&sp) * vpm + side * vpm;
                    act_out(boss, Act::Claw { at: Vector3::new(hit.x, hit.y, ground_of(h, hit)), radius: claw_r, damage: pw.claw * 0.75 });
                }
            }
            want.jaw = 0.5;
            want.shift[2] -= 0.04;
            t >= RAKE_SECONDS
        }
        // Up off its haunches through the air in an arc, its forepaws flung out ahead and its wings
        // spread, coming down claws first where it showed
        Pounce => {
            if t - h.dt <= 0.0 {
                let toward = flat(d.aim - d.center).try_normalize(1e-6).unwrap_or(forward);
                let land = d.aim - toward * 0.8 * s * vpm;
                d.path = [d.center, Vector3::new(land.x, land.y, ground_of(h, land) + h.stand), Vector3::zeros()];
                act_out(boss, Act::TakeOff { at: d.center - Vector3::new(0.0, 0.0, h.stand), size: s * 0.6 });
            }
            let [from, to, _] = d.path;
            let long = (POUNCE_TIME + flat(to - from).norm() / vpm / 22.0).min(1.2);
            let f = (t / long).min(1.0);
            let high = POUNCE_HIGH * s * vpm * (PI * f).sin();
            d.center = from.lerp(&to, f) + Vector3::new(0.0, 0.0, high);
            d.velocity = (to - from) / long;
            goal.to = d.center;
            goal.speed = -1.0;
            goal.low = true;
            goal.face = Some(to + flat(to - from) * 2.0);
            want.pitch += 0.25 * (1.0 - 2.0 * f);
            for leg in 2..4 {
                want.legs[leg] = LegPose { swing: 1.25, knee: 0.25, plant: 0.0, foot: [0.0; 2] };
            }
            for leg in 0..2 {
                want.legs[leg] = LegPose { swing: -0.6, knee: 0.4, plant: 0.0, foot: [0.0; 2] };
            }
            want.jaw = 0.8;
            *wings = WingSet { quick: 3.0, ..WingSet::held(blend(MANTLED, SPREAD, 0.8)) };
            mark(boss, Vector3::new(to.x, to.y, to.z - h.stand), 1.4 * s.sqrt() + 0.4, 1.0 - f);
            if f >= 1.0 {
                d.center = to;
                d.velocity = Vector3::zeros();
                act_out(boss, Act::Claw { at: Vector3::new(to.x, to.y, to.z - h.stand), radius: 1.4 * s.sqrt() + 0.4, damage: pw.claw * 1.15 });
                act_out(boss, Act::Land { at: Vector3::new(to.x, to.y, to.z - h.stand), size: s });
                d.spin[AT_SHIFT + 2] -= 1.8;
                return true;
            }
            false
        }
        // Roaring, its head shaking with it, its mane up and its wings raised, the holy light in it
        // rolling out across the ground in rings one after another, to be jumped
        Shockroar => {
            let length = SHOCK_RINGS[if boss.enraged { 2 } else { 1 }] + reach_of(s) / SHOCK_PACE;
            rear(want, &sp, 0.7);
            thrust(want, &sp, 0.35);
            want.head[0] += 0.09 * (t * 21.0).sin();
            want.jaw = 1.0;
            *wings = WingSet::held(RAISED);
            if crossed(0.05) {
                act_out(boss, Act::Roar { at: d.mouth, brood: None, first: false, angry: false });
            }
            let ground = Vector3::new(d.center.x, d.center.y, ground_of(h, d.center));
            let rings = if boss.enraged { 3 } else { 2 };
            for &start in &SHOCK_RINGS[..rings] {
                let local = t - start;
                let reach = reach_of(s);
                if local <= 0.0 || (local - h.dt) * SHOCK_PACE >= reach {
                    continue;
                }
                let from = ((local - h.dt) * SHOCK_PACE).max(0.6);
                let to = (local * SHOCK_PACE).min(reach);
                if to > from {
                    boss.deeds.push(Deed::Quake { at: ground, from, to, dt: h.dt });
                }
            }
            t >= length
        }
        // Its wings beaten forward hard, a storm of feathers flung off each at the player
        Featherstorm => {
            let length = 0.5;
            let q = t / length;
            let down = ease(q / 0.4);
            let top = WingPose { flap: 1.25, sweep: -0.45, twist: 0.25, fold: 0.05, bend: 0.35 };
            let bottom = WingPose { flap: -0.4, sweep: -0.3, twist: -0.1, fold: 0.05, bend: -0.25 };
            *wings = WingSet { base: blend(top, bottom, down), amp: 0.0, rate: 1.0, hover: 0.0, quick: 6.0 };
            if d.airborne {
                goal.to = d.center;
                goal.speed = 2.0;
                want.pitch += 0.25;
            }
            if crossed(0.16) {
                let count = if boss.enraged { 7 } else { 5 };
                for side in [-1.0, 1.0] {
                    let from = d.center + rz(d.yaw) * Vector3::new(0.1, side * 1.1 * s, 0.5 * s) * vpm;
                    let to = d.aim + Vector3::new(0.0, 0.0, 1.0 * vpm) + Vector3::new(-forward.y, forward.x, 0.0) * side * 1.2 * vpm;
                    boss.deeds.push(Deed::Aerie(crate::avians::Act::Feathers { from, to, count, spread: 2.6, speed: 24.0, damage: 7.0 }));
                }
            }
            t >= length
        }
        _ => true,
    }
}

/// The holy light in it as it gathers its roar or its feathers and lets them loose: where, and how
/// bright, 0 to 1, see dragons::glow()
pub(super) fn glow(d: &Dragon) -> Option<(Vector3<f64>, f64)> {
    match d.state {
        State::Wind(Shockroar | Featherstorm, t) => {
            let wind = d.skill.map_or(0.8, |s| s.wind);
            Some((d.mouth, 0.3 + 0.7 * (t / wind).min(1.0).powi(2)))
        }
        State::Do(Shockroar | Featherstorm, t) => Some((d.mouth, 0.9 + 0.1 * (t * 17.0).sin())),
        _ => None,
    }
}
