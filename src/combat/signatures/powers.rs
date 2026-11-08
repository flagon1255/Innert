//! The right-click powers of legendary and mythic weapons, see weapons::Power: one for each
//! family, or a few families that fight alike, flavoured by the weapon's element, so a fire
//! greatsword's differs from a frost one's. Where right click already does something it carries
//! the power: a heavy blow lets it go as it lands and a bow drawn full looses it. A crossbow or gun
//! shoots it on right click, loading early moving to Q; bombs throw it when right click's let go
//! of after aiming; a weapon that casts casts it. Each has a cooldown, and one that casts costs
//! mana too, so it never outshines the special attack on R.
//!
//! Like the signature special attacks, what it shoots and leaves going is the fight's own
//! machinery: projectiles carrying Sig::Power and zones of SigZone::Power, done here.

use super::*;
use crate::weapons::Power;

/// How long till the power's ready again, and how long right click's been held, for bombs
#[derive(Default)]
pub(in crate::combat) struct PowerState {
    pub(in crate::combat) cooldown: f64,
    held: f64,
}

/// What one of their shots is
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::combat) enum Shape {
    /// A crescent flying on, `width` meters across, cutting through what's in its swath
    Wave { width: f32 },
    /// A spinning wheel hurled out, and once it's as far as it goes, coming back
    Wheel { back: bool },
    /// A spear of light, through everything in a line
    Javelin,
    /// An arrow like a comet, through everything, bursting where it comes down
    Arrow,
    /// A bolt that sticks where it strikes and bursts apart
    Bolt,
    /// A glowing slug glancing on this many more times
    Slug(u8),
    /// A lance, through everything, leaving a line that bursts after it
    Lance,
    /// One of a breath's embers, or a burst bolt's shards
    Ember,
    /// A mortar's shell
    Shell,
    /// A vortex charge
    Charge,
    /// A great slow orb
    Orb,
    /// A seeking mote
    Mote,
}

/// What they leave going a while
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::combat) enum Lasting {
    /// The ground splitting open from `position` on along `velocity`, a step of it at a time,
    /// `owed` steps so far
    Fissure,
    /// A burst bolt stuck fast, in `target` at `velocity` off its middle or in the ground,
    /// pointing `way`, bursting when its time's out
    Stuck { way: [f32; 3] },
    /// A vortex dragging everything in, bursting when its time's out
    Vortex,
    /// A lance's line hanging from `position` along `velocity`, bursting along when `tick`'s out
    Line,
    /// A ring rolling out from `position` to `radius` meters, `hit` what it's reached
    Nova,
}

/// How many steps a fissure takes, how far apart, in meters, and seconds between each
const FISSURE_STEPS: f64 = 8.0;
const FISSURE_STEP: f64 = 1.5;
const FISSURE_GAP: f64 = 0.07;
/// Seconds a burst bolt waits before it bursts, a vortex drags things in, and a lance's line hangs
const STUCK_SECONDS: f64 = 0.7;
const VORTEX_SECONDS: f64 = 1.4;
const VORTEX_REACH: f64 = 6.0;
const LINE_SECONDS: f64 = 0.45;
/// How fast a nova rolls out, in meters a second, and how far
const NOVA_SPEED: f64 = 12.0;
const NOVA_REACH: f64 = 9.0;
/// How fast a wheel flies, out and back, and how far out it goes, in meters
const WHEEL_SPEED: f64 = 18.0;
const WHEEL_REACH: f64 = 13.0;

/// A heavy blow's crescents: how many, how wide, how fast and far they fly, in meters and meters a
/// second, and how hard each hits next to a blow
fn wave_of(power: Power) -> (usize, f64, f64, f64, f64) {
    match power {
        Power::Rending => (1, 3.4, 16.0, 16.0, 2.0),
        Power::WindCut => (1, 1.4, 38.0, 22.0, 1.5),
        Power::BladeFan => (3, 1.1, 30.0, 11.0, 0.9),
        Power::LashWave => (1, 1.8, 30.0, 14.0, 1.4),
        _ => (1, 2.2, 26.0, 14.0, 1.6),
    }
}

/// How long a power's own move takes, for those shot, thrown or cast with a move of their own, and
/// when what it lets go leaves the weapon, as part of it
fn power_timing(power: Power) -> (f64, f64) {
    match power {
        Power::Mortar => (0.7, 0.4),
        Power::BurstBolt => (0.6, 0.34),
        Power::Vortex => (0.8, BOMB_RELEASE),
        Power::Comet | Power::Nova => (1.0, 0.42),
        Power::Seekers => (0.8, 0.34),
        _ => (0.6, 0.32),
    }
}

/// The move a power's shot, thrown or cast with, from where the weapon's held at rest
fn power_keys(power: Power, profile: &Profile) -> Vec<(f64, Pose, bool)> {
    let (_, release) = power_timing(power);
    match power {
        // Levelled and steadied, then kicking back hard as it goes off, a crossbow less so; a
        // mortar raised high to lob its shell, a breath held low and level
        Power::Ricochet | Power::LanceShot | Power::Breath | Power::Mortar | Power::BurstBolt => {
            let up = match power {
                Power::Mortar => 0.42,
                Power::Breath => -0.02,
                _ => 0.04,
            };
            let kick = if power == Power::BurstBolt { 1.0 } else { 1.8 };
            let aimed = levelled(up, 0.0);
            let mut lowered = levelled(0.02, 0.0);
            lowered.hand.z -= 0.02;
            vec![
                (0.12, levelled(up + 0.1, 0.12), false),
                (release - 0.1, aimed.clone(), false),
                (release - 0.03, steadied(&aimed, 1.0), false),
                (release, gun_kicked(&aimed, kick), true),
                (release + 0.2, rocked(&aimed, kick), false),
                (0.85, lowered, false),
            ]
        }
        // Drawn far back over the shoulder, held a moment, and hurled
        Power::Vortex => vec![
            (0.22, pose([0.3, 0.3, -0.12], [0.15, -0.35, 0.92], [1.0, 0.0, 0.0]), false),
            (release - 0.08, pose([0.36, 0.1, 0.06], [0.1, -0.62, 0.78], [1.0, 0.0, 0.0]), false),
            (release, pose([0.22, 0.64, -0.02], [0.0, 0.88, 0.48], [1.0, 0.0, 0.0]), true),
            (0.78, pose([0.12, 0.6, -0.38], [0.0, 0.6, -0.8], [1.0, 0.0, 0.0]), false),
        ],
        // Raised high and held trembling with what gathers in it, then brought down and out hard
        Power::Comet | Power::Nova => {
            let at = spell_at(profile.style);
            vec![
                (0.12, at([0.0, -0.05, 0.12], [0.0, -0.3, 0.95]), false),
                (0.3, at([0.01, -0.07, 0.15], [0.02, -0.42, 0.9]), false),
                (release - 0.05, at([0.0, -0.06, 0.14], [0.0, -0.38, 0.92]), false),
                (release, at([-0.07, 0.15, -0.01], [-0.06, 0.9, 0.42]), true),
                (0.62, at([-0.06, 0.13, 0.0], [-0.05, 0.85, 0.5]), false),
                (0.86, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
            ]
        }
        // Drawn in to the chest, then flicked out to one side and the other as the motes go
        Power::Seekers => {
            let at = spell_at(profile.style);
            vec![
                (0.12, at([0.03, -0.06, 0.06], [0.05, 0.1, 1.0]), false),
                (release - 0.06, at([0.02, -0.08, 0.08], [0.0, 0.0, 1.0]), false),
                (release, at([-0.08, 0.16, 0.04], [-0.15, 0.88, 0.45]), true),
                (release + 0.14, at([0.04, 0.14, 0.05], [0.18, 0.86, 0.47]), false),
                (0.8, at([-0.01, 0.03, 0.01], [-0.02, 0.45, 0.9]), false),
            ]
        }
        _ => Vec::new(),
    }
}

impl Combat {
    /// Seconds till the power on right click is ready again, for the HUD
    pub fn power_cooldown(&self) -> f64 {
        self.power.cooldown
    }

    /// What a player does with the mouse to use `weapon`'s power, `t` seconds after starting to,
    /// in frames `step` seconds long, for scripts and tests: a heavy blow; a bow drawn and loosed
    /// once it's full; bombs aimed a moment and let go; a siege crossbow braced and loosed; anything
    /// else, right click
    pub fn power_controls(&self, weapon: &Weapon, t: f64, step: f64) -> Controls {
        let profile = &weapon.profile;
        let mut c = Controls::default();
        if t < 0.0 {
            return c;
        }
        let used = self.power.cooldown > 0.0;
        let kind = if profile.style == Style::Glaive { Kind::Melee } else { profile.kind };
        match kind {
            Kind::Ranged if profile.style == Style::Lob => c.aiming = t < 0.3,
            Kind::Ranged if profile.build == Build::None => {
                c.aiming = !used;
                c.heavy = t < step;
                c.attack = !used && self.drawn(profile) >= 1.0;
            }
            Kind::Ranged if profile.build == Build::Siege => {
                c.aiming = !used && t < 2.5;
                c.attack = !used && self.brace >= 1.0;
            }
            _ => {
                c.heavy = t < step;
                c.aiming = t < 0.2;
            }
        }
        c
    }

    /// The weapon in hand's power on right click, if it has one, before anything else it does:
    /// letting it go as the heavy blow lands or the bow's loosed drawn full, starting its own move
    /// on right click, and that move going on. Takes from `controls` what it uses; Q loading a
    /// crossbow or gun early is put in as right click for the rest. True while its own move's
    /// going on, when nothing else is done with the weapon.
    pub(in crate::combat) fn wield_power(&mut self, arena: &mut Arena, index: usize, weapon: &Weapon, controls: &mut Controls, dt: f64) -> bool {
        let profile = &weapon.profile;
        let shoulder = profile.kind == Kind::Ranged && profile.build != Build::None;
        let Some(power) = profile.power else {
            if shoulder && controls.reload {
                controls.heavy = true;
            }
            return false;
        };
        // Its own move going on, what it lets go leaving the weapon as the move lets it go
        if let Action::Power { t, length } = self.action {
            let at = power_timing(power).1 * length;
            if t <= at && t + dt > at {
                self.unleash_power(arena, weapon, power);
            }
            self.action = if t + dt >= length { Action::Idle } else { Action::Power { t: t + dt, length } };
            return true;
        }
        if matches!(self.action, Action::Special { .. }) || self.thrown.is_some() || self.raise < 1.0 {
            return false;
        }
        let ready = self.power.cooldown <= 0.0;
        let kind = if profile.style == Style::Glaive { Kind::Melee } else { weapon.kind() };
        match kind {
            // The heavy blow carries it, letting it go just as the blow lands
            Kind::Melee => {
                if let Action::Swing { t, length, heavy: true, struck: false, combo } = self.action
                    && ready
                {
                    let lands = blow_times(profile, true, combo).1;
                    let step = if self.hitstop > 0.0 { dt * HITSTOP_PACE } else { dt };
                    if t + step >= length * lands {
                        self.power.cooldown = power.cooldown();
                        self.unleash_power(arena, weapon, power);
                    }
                }
            }
            // Bombs: aimed with right click held, the ring showing where it'll come down, and
            // thrown as it's let go of
            Kind::Ranged if profile.style == Style::Lob => {
                if controls.aiming {
                    self.power.held += dt;
                } else {
                    if self.power.held > 0.15 && !matches!(self.action, Action::Throw { .. }) {
                        if ready {
                            self.power.held = 0.0;
                            self.power.cooldown = power.cooldown();
                            self.action = Action::Power { t: 0.0, length: power_timing(power).0 };
                            self.cues.push(Cue::here(Sound::Fuse, 0.7).pitched(0.8));
                            return true;
                        }
                        self.not_ready(power, profile);
                    }
                    self.power.held = 0.0;
                }
            }
            // A bow drawn full looses it in place of an arrow
            Kind::Ranged if profile.build == Build::None => {
                if matches!(self.action, Action::Draw { .. }) && controls.attack && ready && self.drawn(profile) >= 1.0 {
                    self.power.cooldown = power.cooldown();
                    self.unleash_power(arena, weapon, power);
                    self.action = Action::Loose { t: 0.0 };
                    self.cooldown = 0.25;
                    controls.attack = false;
                }
            }
            Kind::Ranged => {
                let left = self.rounds_in(index, profile);
                if profile.build == Build::Siege {
                    // Braced, the first bolt loosed is it
                    let loosing = controls.attack || matches!(self.action, Action::Brace { loose: true, .. });
                    if matches!(self.action, Action::Brace { .. }) && self.brace >= 1.0 && self.cooldown <= 0.0 && ready && loosing && left > 0 {
                        self.power.cooldown = power.cooldown();
                        self.loads.insert(index, left - 1);
                        self.unleash_power(arena, weapon, power);
                        self.action = Action::Fire { t: 0.0, length: 0.5 };
                        self.cooldown = 0.5;
                        controls.attack = false;
                    }
                } else {
                    // Right click shoots it, but for letting go of a harpoon's line
                    let line_out = profile.build == Build::Harpoon && (self.tether.is_some() || self.projectiles.iter().any(|p| p.trick == Trick::Harpoon));
                    if controls.heavy && !line_out {
                        controls.heavy = false;
                        if ready && matches!(self.action, Action::Idle | Action::Fire { .. } | Action::Reload { .. }) {
                            self.power.cooldown = power.cooldown();
                            self.action = Action::Power { t: 0.0, length: power_timing(power).0 };
                            return true;
                        }
                        if !ready {
                            self.not_ready(power, profile);
                        }
                    }
                }
                if controls.reload {
                    controls.heavy = true;
                }
            }
            // Cast with a move of its own, for its mana
            Kind::Magic => {
                if controls.heavy && self.cooldown <= 0.0 && !matches!(self.action, Action::Channel { .. } | Action::Charge { .. }) {
                    let cost = profile.stat("manaCost") * power.mana();
                    if !ready {
                        self.not_ready(power, profile);
                    } else if self.mana < cost {
                        if self.mana_warning <= 0.0 {
                            self.messages.push("Not enough mana.".into());
                            self.mana_warning = 2.0;
                        }
                    } else {
                        self.mana -= cost;
                        self.power.cooldown = power.cooldown();
                        self.action = Action::Power { t: 0.0, length: power_timing(power).0 };
                        self.casts = self.casts.wrapping_add(1);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Saying the power's not ready yet, once
    fn not_ready(&mut self, power: Power, profile: &Profile) {
        let said = format!("{} ready in {:.0} s.", power.name(profile.element), self.power.cooldown.ceil().max(1.0));
        if self.messages.last() != Some(&said) {
            self.messages.push(said);
        }
    }

    /// Where the weapon in hand is in its power's own move, `f` of the way through, from `rest`
    pub(in crate::combat) fn power_pose(&self, weapon: &Weapon, rest: &Pose, f: f64) -> Pose {
        let Some(power) = weapon.profile.power else {
            return rest.clone();
        };
        let mut keys = power_keys(power, &weapon.profile);
        // A hand crossbow goes through it on the left
        if weapon.profile.build == Build::Hand {
            for key in &mut keys {
                key.1 = mirrored_shouldered(&key.1);
            }
        }
        animate(rest, &keys, f)
    }

    /// The power let go: what it shoots, hurls or sets off leaving the weapon
    fn unleash_power(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power) {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let profile = weapon.profile.clone();
        let element = profile.element;
        let look = Element::look(element);
        let light = element.map_or([0.95, 0.9, 0.8], Element::light);
        let magic = profile.kind == Kind::Magic && profile.style != Style::Glaive;
        let base = if magic { profile.stat("spellDamage") } else { profile.stat("physicalDamage") } + profile.stat("elementalDamage");
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let forward = view.forward();
        let flat = flat_of(forward);
        // Along where it's looked, never much up or down, so it runs on over the ground
        let pitch = forward.z.clamp(-0.18, 0.25);
        let level = (flat * (1.0 - pitch * pitch).sqrt() + up * pitch).normalize();
        let target = self.aim(arena);
        let set = |c: &mut Combat, shape: Shape, range: f64, size: f64| {
            c.signed(Sig::Power(shape));
            if let Some(p) = c.projectiles.last_mut() {
                p.range = range * vpm;
                p.size = size;
            }
        };
        match power {
            // Crescents flying on from the blow, out of the chest, or a whip's low over the ground
            Power::Crescent | Power::Rending | Power::WindCut | Power::BladeFan | Power::LashWave => {
                let (count, width, speed, reach, share) = wave_of(power);
                let low = power == Power::LashWave;
                let from = if low { feet + up * 0.8 * vpm + flat * 2.0 * vpm } else { view.eye + view.rotation * Vector3::new(0.05, 0.9, -0.35) * vpm };
                let way = if low { flat } else { level };
                for k in 0..count {
                    let yaw = (k as f64 - (count - 1) as f64 * 0.5) * 0.3;
                    let along = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), yaw) * way;
                    self.launch(&profile, None, Form::Bolt, from, along * speed * vpm, base * share, 0.0);
                    set(self, Shape::Wave { width: width as f32 }, reach, 1.0);
                }
                self.cues.push(Cue::here(Sound::Shing, 0.9).pitched(if power == Power::Rending { 0.7 } else { 1.1 }));
                self.cues.push(Cue::here(Sound::HeavySwing, 0.6).pitched(0.8));
                self.flashes.push(Flash { position: from, color: light, reach: 6.0, life: 0.2, lifetime: 0.2 });
                self.sparks(from, 12, 3.0, look, 0.0, 0.35);
            }
            // A wheel hurled out spinning, to come back
            Power::Wheel => {
                let from = view.eye + view.rotation * Vector3::new(0.1, 1.0, -0.3) * vpm;
                self.launch(&profile, None, Form::Bolt, from, level * WHEEL_SPEED * vpm, base * 1.3, 0.0);
                set(self, Shape::Wheel { back: false }, WHEEL_REACH, 1.0);
                self.cues.push(Cue::here(Sound::Whirr, 0.9).pitched(0.8));
                self.cues.push(Cue::here(Sound::Shing, 0.6).pitched(0.9));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
            }
            // A spear of light hurled at the crosshair
            Power::Javelin => {
                let from = view.eye + view.rotation * Vector3::new(0.2, 0.7, -0.15) * vpm;
                let way = lead(from, target, 34.0, 0.15, vpm);
                self.launch(&profile, None, Form::Bolt, from, way * 34.0 * vpm, base * 2.0, 0.15);
                set(self, Shape::Javelin, 26.0, 1.0);
                self.cues.push(Cue::here(Sound::Shing, 0.8).pitched(0.75));
                self.cues.push(Cue::here(Sound::Swing, 0.7).pitched(0.6));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
            }
            // The ground split open in a line ahead, step by step
            Power::Fissure => {
                let start = ground_under(arena.world, feet + flat * 2.6 * vpm + up * vpm, vpm);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Fissure)), &profile, start, flat * FISSURE_STEP * vpm, 1.6, FISSURE_STEPS * FISSURE_GAP + 0.3, base * 1.5);
                self.cues.push(Cue::here(Sound::Quake, 1.0).pitched(1.1));
                self.cues.push(Cue::here(Sound::Slam, 0.8).pitched(0.8));
                self.shake = self.shake.max(0.6);
            }
            // An arrow like a comet, from the nock at the crosshair
            Power::CometArrow => {
                let from = self.held_point(weapon, view, true);
                let way = lead(from, target, 50.0, 0.25, vpm);
                self.launch(&profile, Some(Missile { shot: Shot::Arrow, head: weapon.arrowhead(), power: true, size: 1.5 }), Form::Bolt, from, way * 50.0 * vpm, base * 2.2, 0.25);
                set(self, Shape::Arrow, f64::INFINITY, 1.5);
                self.cues.push(Cue::here(Sound::Twang, 1.0).pitched(0.8));
                self.cues.push(Cue::here(Sound::Shing, 0.5).pitched(1.4));
                self.flashes.push(Flash { position: from, color: light, reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.shake = self.shake.max(0.3);
            }
            Power::BurstBolt => {
                let from = self.held_point(weapon, view, false);
                let way = lead(from, target, 55.0, 0.3, vpm);
                self.launch(&profile, Some(Missile { shot: Shot::Quarrel, head: 0, power: true, size: 1.7 }), Form::Bolt, from, way * 55.0 * vpm, base * 1.5, 0.3);
                set(self, Shape::Bolt, f64::INFINITY, 1.7);
                self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.8));
                self.cues.push(Cue::here(Sound::Zap, 0.4).pitched(0.7));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
                self.kick += 0.04;
            }
            // A gun's: going off as its shots do, see gun_report(), harder
            Power::Ricochet | Power::LanceShot | Power::Breath | Power::Mortar => {
                let from = self.held_point(weapon, view, false);
                match power {
                    Power::Ricochet => {
                        let way = lead(from, target, 42.0, 0.1, vpm);
                        self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.4 }), Form::Bolt, from, way * 42.0 * vpm, base * 1.6, 0.1);
                        set(self, Shape::Slug(4), f64::INFINITY, 1.4);
                        self.gun_report(weapon, view, &profile, from, way, 1.3);
                    }
                    Power::LanceShot => {
                        let way = lead(from, target, 70.0, 0.0, vpm);
                        self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.4 }), Form::Bolt, from, way * 70.0 * vpm, base * 2.0, 0.0);
                        set(self, Shape::Lance, 40.0, 1.4);
                        self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Line)), &profile, from + way * 1.5 * vpm, way * 38.5 * vpm, 1.4, LINE_SECONDS + 0.2, base * 0.8);
                        if let Some(z) = self.zones.last_mut() {
                            z.tick = LINE_SECONDS;
                        }
                        self.gun_report(weapon, view, &profile, from, way, 1.5);
                    }
                    Power::Breath => {
                        let way = lead(from, target, 20.0, 0.15, vpm);
                        let across = way.cross(&up).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                        for k in 0..7 {
                            let s = (k as f64 - 3.0) / 3.0;
                            let along = (way + across * s * 0.45 + up * (0.08 * (k % 2) as f64)).normalize();
                            self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.0 }), Form::Bolt, from, along * (18.0 + 2.0 * (k % 3) as f64) * vpm, base * 0.7, 0.15);
                            set(self, Shape::Ember, 11.0, 1.0);
                        }
                        self.gun_report(weapon, view, &profile, from, way, 1.6);
                    }
                    _ => {
                        let reach = 40.0 * vpm;
                        let mut aim = target;
                        if (aim - from).norm() > reach {
                            aim = ground_under(arena.world, from + (aim - from).normalize() * reach, vpm);
                        }
                        let time = 1.1 + 0.015 * (aim - from).norm() / vpm;
                        self.launch(&profile, Some(Missile { shot: Shot::Shell, head: 0, power: true, size: 1.4 }), Form::Bolt, from, thrown(from, aim, time, 1.0, vpm), base * 2.2, 1.0);
                        set(self, Shape::Shell, f64::INFINITY, 1.4);
                        self.gun_report(weapon, view, &profile, from, flat, 1.8);
                    }
                }
                self.kick += 0.06;
            }
            // A charge lobbed at where the ring was
            Power::Vortex => {
                let (_, from, velocity, _) = self.lob_aim(arena, weapon);
                self.launch(&profile, Some(Missile { shot: Shot::Shell, head: 0, power: true, size: 1.2 }), Form::Bolt, from, velocity, base * 2.0, BOMB_GRAVITY);
                set(self, Shape::Charge, f64::INFINITY, 1.2);
                self.cues.push(Cue::here(Sound::Swing, 0.7).pitched(0.65));
                self.cues.push(Cue::here(Sound::Hex, 0.5).pitched(0.7));
            }
            Power::Comet => {
                let from = self.held_point(weapon, view, false);
                let way = lead(from, target, 15.0, 0.12, vpm);
                self.launch(&profile, None, Form::Bolt, from, way * 15.0 * vpm, base * 2.4, 0.12);
                set(self, Shape::Orb, f64::INFINITY, 1.0);
                self.cues.push(Cue::here(Sound::Summon, 0.9).pitched(0.6));
                self.flashes.push(Flash { position: from, color: light, reach: 8.0, life: 0.35, lifetime: 0.35 });
                self.shake = self.shake.max(0.3);
            }
            // Motes fanned out up and round, each then seeking what's near
            Power::Seekers => {
                let from = self.held_point(weapon, view, false);
                let way = (target - from).try_normalize(1e-6).unwrap_or(forward);
                let across = way.cross(&up).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                for k in 0..5 {
                    let s = (k as f64 - 2.0) / 2.0;
                    let along = (way + across * s * 0.7 + up * (0.35 - 0.15 * s.abs())).normalize();
                    self.launch(&profile, None, Form::Bolt, from, along * 16.0 * vpm, base * 0.8, 0.0);
                    set(self, Shape::Mote, 30.0, 1.0);
                    if let Some(p) = self.projectiles.last_mut() {
                        p.seek = 6.0;
                    }
                }
                self.cues.push(Cue::here(Sound::Summon, 0.7).pitched(1.4));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.25, lifetime: 0.25 });
            }
            // A ring rolling out all round
            Power::Nova => {
                let center = feet + up * 0.3 * vpm;
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Nova)), &profile, center, Vector3::zeros(), NOVA_REACH, NOVA_REACH / NOVA_SPEED + 0.35, base * 1.5);
                self.cues.push(Cue::here(Sound::Toll, 1.0).pitched(0.7));
                self.cues.push(Cue::here(Sound::Burst, 0.6).pitched(0.8));
                self.flashes.push(Flash { position: center + up * vpm, color: light, reach: 12.0, life: 0.4, lifetime: 0.4 });
                self.shake = self.shake.max(0.5);
            }
        }
    }

    /// What it does to animal `i` it strikes at `at` for `damage`, as its element has it, from
    /// `from`
    fn power_hit(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, at: Vector3<f64>, damage: f64, from: Vector3<f64>) {
        let element = profile.element;
        self.element_hit(at, element, 1.3);
        self.sparks(at, 8, 3.0, Element::look(element), 0.6, 0.35);
        self.cues.push(Cue::at(Sound::Hit, at, 0.8));
        self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from, depth: 0, element });
        self.empower(arena, profile, i, at, damage, from);
    }

    /// What its element does to animal `i` it struck at `at` for `damage`, from `from`: burning,
    /// freezing, lightning leaping on, poisoning, throwing back, dragging in, bleeding it and healing
    /// the player, or staggering it
    fn empower(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, at: Vector3<f64>, damage: f64, from: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let element = profile.element;
        let alive = arena.animals[i].health > 0.0;
        let boss = arena.animals[i].boss.is_some();
        let away = (arena.animals[i].position - from).try_normalize(1e-6).map_or(Vector3::zeros(), |v| Vector3::new(v.x, v.y, 0.0));
        match element {
            Some(Element::Fire) if alive => {
                let s = &mut arena.animals[i].status;
                s.burn = s.burn.max(3.0);
                s.burn_dps = s.burn_dps.max(damage * 0.12);
            }
            Some(Element::Frost) if alive => {
                let s = &mut arena.animals[i].status;
                if s.chill >= 0.5 && !boss {
                    s.frozen = s.frozen.max(1.2);
                }
                s.chill = (s.chill + 0.5).min(1.0);
                s.slowed = s.slowed.max(2.0);
                s.slow = s.slow.max(0.4);
            }
            // Lightning leaping on to the nearest other
            Some(Element::Storm) => {
                if alive {
                    arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(0.5);
                }
                let id = arena.animals[i].id;
                let next = Combat::foes_round(arena, at, 5.0).into_iter().find(|&j| arena.animals[j].id != id);
                if let Some(j) = next {
                    let to = arena.animals[j].center(vpm as u32);
                    self.arc(at, to);
                    self.strike(arena, profile, j, Blow { damage: damage * 0.4, crit: false, heavy: false, from: at, depth: 1, element: Some(Element::Storm) });
                    if arena.animals[j].health > 0.0 {
                        arena.animals[j].status.shocked = arena.animals[j].status.shocked.max(0.4);
                    }
                }
            }
            Some(Element::Poison) if alive => {
                let s = &mut arena.animals[i].status;
                s.poison = s.poison.max(4.0);
                s.poison_stacks = (s.poison_stacks + 1).min(5);
                s.poison_dps = s.poison_dps.max(damage * 0.1);
            }
            Some(Element::Arcane) if alive && !boss => {
                arena.animals[i].status.push = away * 9.0 * vpm;
                arena.animals[i].status.staggered = arena.animals[i].status.staggered.max(0.3);
            }
            // What's round it dragged in to it
            Some(Element::Void) => {
                for j in Combat::foes_round(arena, at, 4.0) {
                    let to = at - arena.animals[j].position;
                    if arena.animals[j].boss.is_none() && to.norm() > 0.5 * vpm {
                        arena.animals[j].status.push = Vector3::new(to.x, to.y, 0.0).normalize() * 5.0 * vpm;
                    }
                }
            }
            Some(Element::Blood) => {
                if alive {
                    let s = &mut arena.animals[i].status;
                    s.bleed = s.bleed.max(4.0);
                    s.bleed_stacks = (s.bleed_stacks + 1).min(5);
                    s.bleed_dps = s.bleed_dps.max(damage * 0.1);
                }
                self.heal += damage * 0.12;
                let to = (arena.view.eye - Vector3::new(0.0, 0.0, 0.5 * vpm) - at) * 1.6;
                self.fine(at, to, Element::look(Some(Element::Blood)), 0.5, 0.0);
            }
            Some(Element::Holy) => {
                if alive {
                    let s = &mut arena.animals[i].status;
                    s.staggered = s.staggered.max(if boss { 0.2 } else { 0.4 });
                }
                self.heal += damage * 0.05;
            }
            None if alive => {
                let s = &mut arena.animals[i].status;
                s.staggered = s.staggered.max(if boss { 0.2 } else { 0.5 });
                if !boss {
                    s.push = away * 4.0 * vpm;
                }
            }
            _ => {}
        }
        if boss && alive {
            arena.animals[i].status.limit_for_boss();
        }
    }

    /// A burst of the power's element `radius` meters round `at` for `damage`, leaving no mark, its
    /// element doing what it does to all it catches but `skip`
    fn power_burst(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, radius: f64, damage: f64, skip: Option<usize>) {
        let vpm = arena.view.vpm;
        let caught: Vec<usize> = Combat::foes_round(arena, at, radius).into_iter().filter(|&i| Some(i) != skip).collect();
        self.scarless = true;
        self.burst(arena, profile, at, radius, damage, 0, false, profile.element, skip);
        self.scarless = false;
        for i in caught {
            let center = arena.animals[i].center(vpm as u32);
            self.empower(arena, profile, i, center, damage * 0.6, at);
        }
    }

    /// A little pop of the power's element `radius` meters round `at` for `damage`, for the many
    /// small shots a power looses at once, embers, motes and shards: a puff and a spatter of sparks,
    /// what's in it struck and its element doing what it does, but none of an explosion's lightning
    /// out of the sky, ice, pools and the rest, which a handful going off together would cost too
    /// much to draw
    fn power_pop(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, radius: f64, damage: f64, skip: Option<usize>) {
        let vpm = arena.view.vpm;
        let look = Element::look(profile.element);
        self.blasts.push(Blast { position: at, look, radius: radius * 0.6, life: 0.25, lifetime: 0.25 });
        self.sparks(at, 6, 3.0, look, 0.4, 0.3);
        self.cues.push(Cue::at(Sound::Burst, at, 0.35).pitched(1.5));
        for i in Combat::foes_round(arena, at, radius).into_iter().filter(|&i| Some(i) != skip) {
            let center = arena.animals[i].center(vpm as u32);
            self.strike(arena, profile, i, Blow { damage, crit: false, heavy: false, from: at, depth: 1, element: profile.element });
            self.empower(arena, profile, i, center, damage * 0.6, at);
        }
    }

    /// One of their shots in flight: what's in a crescent's or a wheel's swath cut as it passes, a
    /// wheel turning back once it's as far as it goes and caught again at the hand, tracers, and
    /// coming down where it's flown as far as it goes. True if it's gone.
    pub(super) fn power_flight(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let look = Element::look(p.profile.element);
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        // Glowing tracers behind what's shot, never reaching back past the muzzle
        let tracer = match shape {
            Shape::Bolt | Shape::Slug(_) => Some(0.04),
            Shape::Lance => Some(0.06),
            Shape::Arrow => Some(0.035),
            _ => None,
        };
        if let Some(seconds) = tracer
            && p.flown > 0.4 * vpm
        {
            let back = (p.velocity.norm() * seconds).min(p.flown);
            self.rays.push((p.position - way * back, p.position, look));
        }
        match shape {
            Shape::Wheel { back: true } => {
                // Coming back to the hand, caught there
                let hand = arena.view.eye - Vector3::new(0.0, 0.0, 0.4 * vpm);
                let to = hand - p.position;
                if to.norm() < 1.4 * vpm {
                    self.sparks(p.position, 10, 2.5, look, 0.0, 0.3);
                    self.cues.push(Cue::here(Sound::Clang, 0.5).pitched(1.4));
                    return true;
                }
                p.velocity = to.normalize() * WHEEL_SPEED * 1.25 * vpm;
            }
            Shape::Wheel { back: false } if p.flown >= p.range => {
                p.trick = Trick::Signature(Sig::Power(Shape::Wheel { back: true }));
                p.hit.clear();
                return false;
            }
            _ if p.flown >= p.range => {
                let at = p.position;
                self.power_lands(arena, p, shape, at);
                return true;
            }
            _ => {}
        }
        // What's in its swath, cut once each way
        let half = match shape {
            Shape::Wave { width } => Some(width as f64 * 0.5),
            Shape::Wheel { .. } => Some(1.0),
            _ => None,
        };
        if let Some(half) = half {
            let step = p.velocity.norm() * dt;
            let caught: Vec<usize> = (0..arena.animals.len())
                .filter(|&i| {
                    let animal = &arena.animals[i];
                    if animal.health <= 0.0 || animal.look.is_pet() || p.hit.contains(&animal.id) {
                        return false;
                    }
                    let body = animal.look.body().1 * vpm;
                    let rel = animal.center(vpm as u32) - p.position;
                    let along = rel.dot(&way);
                    along > -body && along < step + body && (rel - way * along).norm() < half * vpm + body
                })
                .collect();
            let profile = p.profile.clone();
            for i in caught {
                let center = arena.animals[i].center(vpm as u32);
                p.hit.push(arena.animals[i].id);
                self.power_hit(arena, &profile, i, center, p.damage, p.position - way * vpm);
                // A greatsword's rending wave throws what it cuts back
                if half > 1.5 && arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                    arena.animals[i].status.push = Vector3::new(way.x, way.y, 0.0) * 8.0 * vpm;
                }
            }
            // Shedding sparks as it goes, so it's seen even with no element
            if random(&mut self.rng) < dt * 30.0 {
                let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                let at = p.position + across * (random(&mut self.rng) - 0.5) * 2.0 * half * vpm;
                let v = (-way * 2.0 + self.any_direction() * 0.5) * vpm;
                self.sparks.push(Spark { position: at, velocity: v, look, life: 0.3, lifetime: 0.3, gravity: 0.2, slot: SLOT_SPARK });
                self.cap_sparks();
            }
        }
        false
    }

    /// One of their shots striking animal `i` at `at`. True if it's gone, false if it goes on.
    pub(super) fn power_strikes(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, i: usize, at: Vector3<f64>) -> bool {
        let vpm = arena.view.vpm;
        let profile = p.profile.clone();
        let id = arena.animals[i].id;
        let from = p.position - p.velocity * 0.02;
        match shape {
            // Its swath does it, as it flies, see power_flight()
            Shape::Wave { .. } | Shape::Wheel { .. } => false,
            // Through all
            Shape::Javelin | Shape::Arrow | Shape::Lance => {
                p.hit.push(id);
                self.power_hit(arena, &profile, i, at, p.damage, from);
                false
            }
            // Stuck fast in it, to burst a moment later
            Shape::Bolt => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                let center = arena.animals[i].center(vpm as u32);
                self.stick_bolt(p, at, Some((id, at - center)));
                true
            }
            // Glancing on to the next near, or bursting if there's none
            Shape::Slug(left) => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                let mut hit = p.hit.clone();
                hit.push(id);
                let next = Combat::foes_round(arena, at, 12.0).into_iter().find(|&j| !hit.contains(&arena.animals[j].id));
                match next.filter(|_| left > 0) {
                    Some(j) => {
                        let to = arena.animals[j].center(vpm as u32);
                        let way = (to - at).try_normalize(1e-6).unwrap_or_else(Vector3::z);
                        self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.4 }), Form::Bolt, at + way * 0.3 * vpm, way * 45.0 * vpm, p.damage * 1.1, 0.0);
                        self.signed(Sig::Power(Shape::Slug(left - 1)));
                        if let Some(n) = self.projectiles.last_mut() {
                            n.hit = hit;
                            n.size = 1.4;
                        }
                        self.cues.push(Cue::at(Sound::Clang, at, 0.6).pitched(1.6));
                        self.sparks(at, 10, 4.0, Element::look(None), 0.5, 0.3);
                    }
                    None => self.power_burst(arena, &profile, at, 1.4, p.damage * 0.5, Some(i)),
                }
                true
            }
            Shape::Ember | Shape::Mote => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                self.power_pop(arena, &profile, at, 1.1, p.damage * 0.4, Some(i));
                true
            }
            Shape::Shell | Shape::Charge | Shape::Orb => {
                self.power_lands(arena, p, shape, at);
                true
            }
        }
    }

    /// One of their shots coming down at `at`, on the ground, a wall or the water, or as far as
    /// it goes
    pub(super) fn power_lands(&mut self, arena: &mut Arena, p: &Projectile, shape: Shape, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = p.profile.clone();
        let look = Element::look(profile.element);
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        match shape {
            // Breaking up into sparks and a flash where it ends
            Shape::Wave { .. } => {
                self.sparks(at, 16, 3.5, look, 0.3, 0.45);
                self.blasts.push(Blast { position: at, look, radius: 0.6, life: 0.3, lifetime: 0.3 });
                self.cues.push(Cue::at(Sound::Shing, at, 0.5).pitched(1.5));
            }
            // Glancing off what it struck and coming back
            Shape::Wheel { back: false } => {
                self.sparks(at, 10, 3.0, Element::look(None), 0.6, 0.3);
                self.cues.push(Cue::at(Sound::Clang, at, 0.6).pitched(1.1));
                self.launch(&profile, None, Form::Bolt, at - way * 0.3 * vpm, -way * WHEEL_SPEED * vpm, p.damage, 0.0);
                self.signed(Sig::Power(Shape::Wheel { back: true }));
            }
            Shape::Wheel { back: true } => self.sparks(at, 10, 2.5, look, 0.0, 0.3),
            Shape::Javelin => {
                self.power_burst(arena, &profile, at, 1.8, p.damage * 0.5, None);
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
                self.stuck.push(Stuck { position: at + way * 0.1 * vpm, rotation, look, head: Some(0), quarrel: true, element: profile.element, life: 2.0, glowing: 2.0 });
                self.keep_stuck_few();
            }
            Shape::Arrow => self.power_burst(arena, &profile, at, 2.2, p.damage * 0.45, None),
            Shape::Bolt => self.stick_bolt(p, at, None),
            // Bouncing on off the ground or a wall, or bursting at the last
            Shape::Slug(left) => {
                if left > 0 {
                    let mut v = p.velocity;
                    if v.z < 0.0 {
                        v.z = -v.z * 0.8;
                    } else {
                        v.x = -v.x;
                        v.y = -v.y;
                    }
                    self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.4 }), Form::Bolt, at + up * 0.15 * vpm - way * 0.1 * vpm, v * 0.85, p.damage, 0.1);
                    self.signed(Sig::Power(Shape::Slug(left - 1)));
                    if let Some(n) = self.projectiles.last_mut() {
                        n.size = 1.4;
                    }
                    self.sparks(at, 10, 4.0, Element::look(None), 0.5, 0.3);
                    self.cues.push(Cue::at(Sound::Clang, at, 0.5).pitched(1.7));
                } else {
                    self.power_burst(arena, &profile, at, 1.4, p.damage * 0.5, None);
                }
            }
            // The line it left shortened to where it stopped, and a burst there
            Shape::Lance => {
                if let Some(z) = self.zones.iter_mut().rev().find(|z| z.kind == ZoneKind::Signature(SigZone::Power(Lasting::Line)) && z.tick > 0.0) {
                    let along = z.velocity.try_normalize(1e-6).unwrap_or(way);
                    z.velocity = along * (at - z.position).dot(&along).max(0.5 * vpm);
                }
                self.power_burst(arena, &profile, at, 1.5, p.damage * 0.4, None);
            }
            Shape::Ember | Shape::Mote => self.power_pop(arena, &profile, at, 1.1, p.damage * 0.5, None),
            Shape::Shell => {
                self.power_burst(arena, &profile, at, 3.5, p.damage, None);
                for _ in 0..10 {
                    let v = (self.any_direction() + up * 1.4) * 4.0 * vpm;
                    self.drop(at, v, DROP_MUD, 1.0);
                }
                self.shake = self.shake.max(0.6);
            }
            Shape::Orb => {
                self.power_burst(arena, &profile, at, 3.5, p.damage, None);
                self.shake = self.shake.max(0.5);
            }
            // Dragging everything in where it lands
            Shape::Charge => {
                let ground = Combat::floor_at(arena, at) + up * 0.3 * vpm;
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Vortex)), &profile, ground, Vector3::zeros(), VORTEX_REACH, VORTEX_SECONDS, p.damage);
                self.cues.push(Cue::at(Sound::Hex, ground, 0.9).pitched(0.5));
                self.cues.push(Cue::at(Sound::Wail, ground, 0.3).pitched(1.2));
                self.flashes.push(Flash { position: ground + up * vpm, color: profile.element.map_or([0.9, 0.85, 0.8], Element::light), reach: 8.0, life: 0.3, lifetime: 0.3 });
            }
        }
    }

    /// A burst bolt stuck fast at `at`, in what it struck, `on` its id and where on it, or in the
    /// ground
    fn stick_bolt(&mut self, p: &Projectile, at: Vector3<f64>, on: Option<(u32, Vector3<f64>)>) {
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let kind = ZoneKind::Signature(SigZone::Power(Lasting::Stuck { way: [way.x as f32, way.y as f32, way.z as f32] }));
        self.zone(kind, &p.profile, at, on.map_or(Vector3::zeros(), |o| o.1), 2.4, STUCK_SECONDS, p.damage);
        if let Some(z) = self.zones.last_mut() {
            z.target = on.map(|o| o.0);
        }
        self.cues.push(Cue::at(Sound::ArrowHit, at, 0.7));
        self.cues.push(Cue::at(Sound::Fuse, at, 0.5).pitched(1.6));
    }
}

impl Combat {
    /// One of their zones going on
    pub(super) fn power_zone(&mut self, arena: &mut Arena, z: &mut Zone, kind: Lasting, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = z.profile.clone();
        let element = profile.element;
        let look = Element::look(element);
        let age = z.lifetime - z.life;
        match kind {
            // A step of it erupting each FISSURE_GAP, throwing up what stands on it
            Lasting::Fissure => {
                while z.owed < FISSURE_STEPS && age >= z.owed * FISSURE_GAP {
                    let at = ground_under(arena.world, z.position + z.velocity * z.owed + up * 1.5 * vpm, vpm);
                    z.owed += 1.0;
                    let model = match element {
                        Some(Element::Frost) => ICE_BURST,
                        Some(Element::Holy) => PILLAR,
                        Some(Element::Poison) => ROOTS,
                        _ => STONE_PILLAR,
                    };
                    // Not right in front of the eye, where it'd fill the view
                    if (at - arena.view.eye).norm() > 4.5 * vpm {
                        self.swells.push(Swell { model, position: at, radius: 0.85, life: 0.7, lifetime: 0.7 });
                    }
                    self.blasts.push(Blast { position: at + up * 0.3 * vpm, look, radius: 0.8, life: 0.35, lifetime: 0.35 });
                    for _ in 0..4 {
                        let v = (self.any_direction() * 0.6 + up * 1.4) * 4.0 * vpm;
                        self.drop(at, v, DROP_MUD, 0.9);
                    }
                    self.sparks(at + up * 0.3 * vpm, 6, 3.0, look, 0.4, 0.4);
                    self.cues.push(Cue::at(Sound::Quake, at, 0.5).pitched(1.3 + 0.05 * z.owed as f32));
                    for i in Combat::foes_round(arena, at, z.radius) {
                        let id = arena.animals[i].id;
                        if z.hit.contains(&id) {
                            continue;
                        }
                        z.hit.push(id);
                        let center = arena.animals[i].center(vpm as u32);
                        self.power_hit(arena, &profile, i, center, z.damage, at);
                        if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                            let s = &mut arena.animals[i].status;
                            s.push = (up * 6.0 + flat_of(z.velocity) * 2.0) * vpm;
                            s.staggered = s.staggered.max(0.6);
                        }
                    }
                }
                if z.owed >= FISSURE_STEPS {
                    z.life = z.life.min(0.3);
                }
            }
            // Stuck fast, blinking faster, riding along on what it's in
            Lasting::Stuck { .. } => {
                if let Some(id) = z.target {
                    match arena.animals.iter().position(|a| a.id == id && a.health > 0.0) {
                        Some(i) => z.position = arena.animals[i].center(vpm as u32) + z.velocity,
                        None => z.target = None,
                    }
                }
                if random(&mut self.rng) < dt * (10.0 + 30.0 * age / STUCK_SECONDS) {
                    let v = self.any_direction() * 1.5 * vpm;
                    self.fine(z.position, v, look, 0.25, 0.0);
                }
            }
            // Dragging everything near in, sparks spiralling down into it
            Lasting::Vortex => {
                for i in Combat::foes_round(arena, z.position, z.radius) {
                    let to = z.position - arena.animals[i].position;
                    if arena.animals[i].boss.is_none() && to.xy().norm() > 0.6 * vpm {
                        arena.animals[i].status.push = Vector3::new(to.x, to.y, 0.0).normalize() * 4.5 * vpm;
                    }
                }
                z.owed += dt * 40.0;
                while z.owed >= 1.0 {
                    z.owed -= 1.0;
                    let a = random(&mut self.rng) * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.0);
                    let at = z.position + out * (2.0 + 3.5 * random(&mut self.rng)) * vpm + up * 0.3 * vpm;
                    let v = (-out * 3.0 + Vector3::new(-out.y, out.x, 0.0) * 4.0) * vpm;
                    self.sparks.push(Spark { position: at, velocity: v, look, life: 0.5, lifetime: 0.5, gravity: 0.0, slot: SLOT_SPARK });
                }
                self.cap_sparks();
            }
            // Hanging there crackling till it bursts along its length
            Lasting::Line => {
                if z.tick <= 0.0 && z.dwell <= 0.0 {
                    z.dwell = 1.0;
                    let length = z.velocity.norm() / vpm;
                    let mut on = 2.0;
                    while on < length {
                        let at = z.position + z.velocity * (on / length);
                        self.blasts.push(Blast { position: at, look, radius: 0.7, life: 0.3, lifetime: 0.3 });
                        self.sparks(at, 3, 2.5, look, 0.3, 0.3);
                        on += 2.5;
                    }
                    self.cap_sparks();
                    let (a, b) = (z.position, z.position + z.velocity);
                    let cut: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| {
                            let animal = &arena.animals[i];
                            animal.health > 0.0 && !animal.look.is_pet() && off_line(animal.center(vpm as u32), a, b) / vpm - animal.look.body().1 < z.radius
                        })
                        .collect();
                    for i in cut {
                        let center = arena.animals[i].center(vpm as u32);
                        self.power_hit(arena, &profile, i, center, z.damage, a);
                    }
                    self.cues.push(Cue::at(Sound::Burst, a.lerp(&b, 0.5), 0.9).pitched(1.1));
                    z.life = z.life.min(0.2);
                } else if z.dwell <= 0.0 && random(&mut self.rng) < dt * 30.0 {
                    let at = z.position + z.velocity * random(&mut self.rng);
                    let v = self.any_direction() * 0.4 * vpm;
                    self.fine(at, v, look, 0.3, 0.0);
                }
            }
            // Rolling out, what it reaches struck once and thrown back
            Lasting::Nova => {
                let reached = (0.6 + NOVA_SPEED * age).min(z.radius);
                let near: Vec<usize> = Combat::foes_round(arena, z.position, reached).into_iter().filter(|&i| !z.hit.contains(&arena.animals[i].id)).collect();
                for i in near {
                    z.hit.push(arena.animals[i].id);
                    let center = arena.animals[i].center(vpm as u32);
                    self.power_hit(arena, &profile, i, center, z.damage, z.position);
                    if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                        let away = arena.animals[i].position - z.position;
                        arena.animals[i].status.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x) * 6.0 * vpm;
                    }
                }
                // Its element flaring up off it here and there as it goes
                if random(&mut self.rng) < dt * 40.0 && reached < z.radius {
                    let a = random(&mut self.rng) * TAU;
                    let at = z.position + Vector3::new(a.cos(), a.sin(), 0.0) * reached * vpm;
                    let v = (Vector3::new(a.cos(), a.sin(), 0.0) * 2.0 + up * 3.0) * vpm;
                    self.sparks.push(Spark { position: at, velocity: v, look, life: 0.4, lifetime: 0.4, gravity: 0.5, slot: SLOT_SPARK });
                    self.cap_sparks();
                }
            }
        }
    }

    /// One of their zones whose time is out
    pub(super) fn power_zone_ends(&mut self, arena: &mut Arena, z: &Zone, kind: Lasting) {
        let vpm = arena.view.vpm;
        let profile = z.profile.clone();
        match kind {
            // Bursting apart, its shards flying all round
            Lasting::Stuck { .. } => {
                self.power_burst(arena, &profile, z.position, z.radius, z.damage, None);
                for k in 0..8 {
                    let a = k as f64 / 8.0 * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.35).normalize();
                    self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: false, size: 0.8 }), Form::Bolt, z.position + out * 0.4 * vpm, out * 16.0 * vpm, z.damage * 0.35, 0.5);
                    self.signed(Sig::Power(Shape::Ember));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = 7.0 * vpm;
                        p.size = 0.8;
                    }
                }
            }
            Lasting::Vortex => {
                self.power_burst(arena, &profile, z.position, 3.0, z.damage, None);
                self.shake = self.shake.max(0.5);
            }
            Lasting::Fissure | Lasting::Line | Lasting::Nova => {}
        }
    }

    /// How one of their shots is drawn: a crescent as an arc of glowing streaks, a wheel as a ring
    /// of them spinning, a spear or lance with its streak behind it, arrows and bolts bigger than
    /// any, glowing orbs and shells. Shown however much else is about.
    pub(super) fn power_shot_instances(&self, p: &Projectile, shape: Shape, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let look = Element::look(p.profile.element);
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
        let spin = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 7.0) * rotation;
        // A little bigger the further off past 15 m, as other shots are
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let ball = |size: f64| {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            (eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, size * far, MOVING))
        };
        let mut out = Vec::new();
        match shape {
            Shape::Wave { width } => {
                let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                let w = width as f64 * vpm;
                for k in 0..7 {
                    let s = k as f64 / 3.0 - 1.0;
                    let at = p.position + across * s * 0.5 * w - way * s * s * 0.3 * w;
                    let tangent = across * 0.5 - way * s * 0.6;
                    out.extend(self.streak_at(eye, at, tangent, look, (1.3 + 0.25 * width as f64) * far * (1.0 - 0.3 * s.abs())));
                }
            }
            Shape::Wheel { .. } => out.extend(self.ring_instances(eye, p.position, Vector3::z(), 0.75, look, 1.2 * far, self.time * 16.0)),
            // A shaft of light, its head glowing
            Shape::Javelin => {
                out.extend(self.line_bolts(eye, p.position - way * 2.6 * vpm, p.position, look, 0.5, 2.2 * far, 0.0));
                out.extend(self.streak_at(eye, p.position + way * 0.2 * vpm, way, look, 3.2 * far));
                out.push(ball(0.12));
            }
            Shape::Arrow => {
                let (model, arrow) = self.effect_model(SLOT_ARROW + p.head.min(ARROW_HEADS - 1), look);
                out.push((eye, place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.5 * far, MOVING)));
                out.extend(self.line_bolts(eye, p.position - way * 2.4 * vpm, p.position - way * 0.4 * vpm, look, 0.6, 1.2 * far, 0.0));
            }
            Shape::Bolt => {
                let (model, quarrel) = self.effect_model(SLOT_QUARREL, look);
                out.push((eye, place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.7 * far, MOVING)));
                out.extend(self.line_bolts(eye, p.position - way * 1.6 * vpm, p.position - way * 0.3 * vpm, look, 0.55, 1.0 * far, 0.0));
            }
            Shape::Slug(_) => {
                out.push(ball(0.14));
                out.extend(self.line_bolts(eye, p.position - way * 1.4 * vpm, p.position, look, 0.5, 1.1 * far, 0.0));
            }
            Shape::Lance => {
                out.push(ball(0.11));
                out.extend(self.line_bolts(eye, p.position - way * 4.0 * vpm, p.position, look, 0.8, 1.4 * far, 0.0));
            }
            Shape::Ember => out.push(ball(0.11 * p.size.max(0.8))),
            Shape::Mote => {
                out.push(ball(0.09));
                out.extend(self.streak_at(eye, p.position - way * 0.3 * vpm, way, look, 0.9 * far));
            }
            Shape::Shell | Shape::Charge => {
                let (model, bomb) = self.effect_model(SLOT_BOMB, look);
                let tumble = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 8.0) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), p.position.x * 0.3);
                out.push((eye, place(model, bomb, BOMB_RES, Vector3::zeros(), p.position, tumble, vpm, 1.4 * far, MOVING)));
                if shape == Shape::Charge {
                    out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.5, look, 0.8 * far, self.time * 9.0));
                }
            }
            Shape::Orb => {
                out.push(ball(0.36));
                out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.75 * far, look, 1.0 * far, self.time * 4.0));
            }
        }
        out
    }

    /// How one of their zones is drawn
    pub(super) fn power_zone_instances(&self, z: &Zone, kind: Lasting, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let look = Element::look(z.profile.element);
        let age = z.lifetime - z.life;
        let up = Vector3::z();
        let mut out = Vec::new();
        match kind {
            // A glowing crack along the ground as far as it's split
            Lasting::Fissure => {
                let to = z.position + z.velocity * z.owed.max(0.5) + up * 0.15 * vpm;
                out.extend(self.line_bolts(eye, z.position + up * 0.15 * vpm, to, look, 1.2, 1.3, 0.15));
            }
            // The bolt stuck there, a ring blinking faster round it
            Lasting::Stuck { way } => {
                let way = Vector3::new(way[0] as f64, way[1] as f64, way[2] as f64);
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
                let (model, quarrel) = self.effect_model(SLOT_QUARREL, look);
                out.push((eye, place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), z.position + way * 0.15 * vpm, rotation, vpm, 1.7, MOVING)));
                if (age * (6.0 + 20.0 * age / STUCK_SECONDS)).fract() < 0.5 {
                    out.extend(self.ring_instances(eye, z.position, eye - z.position, 0.45, look, 0.9, self.time * 5.0));
                }
            }
            // Two rings turning opposite ways, drawing in, the charge at the middle
            Lasting::Vortex => {
                let f = age / VORTEX_SECONDS;
                let (model, bomb) = self.effect_model(SLOT_BOMB, look);
                out.push((eye, place(model, bomb, BOMB_RES, Vector3::zeros(), z.position, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 12.0), vpm, 1.3 * (1.0 + 0.15 * (self.time * 20.0).sin()), MOVING)));
                out.extend(self.circle_instances(eye, z.position, (z.radius * (1.0 - 0.75 * f)).max(0.8), look, 1.6, self.time * 3.0));
                out.extend(self.circle_instances(eye, z.position + up * 0.4 * vpm, (z.radius * 0.55 * (1.0 - 0.6 * f)).max(0.6), look, 1.3, -self.time * 4.5));
            }
            Lasting::Line if z.dwell <= 0.0 => {
                let grown = smooth(age / 0.1).max(0.05);
                out.extend(self.line_bolts(eye, z.position, z.position + z.velocity * grown, look, 1.6, 1.4, 0.05));
            }
            Lasting::Line => {}
            Lasting::Nova => {
                let fade = (z.life / 0.3).clamp(0.1, 1.0);
                let reached = (0.6 + NOVA_SPEED * age).min(z.radius);
                out.extend(self.circle_instances(eye, z.position, reached, look, 2.4 * fade, age * 0.6));
            }
        }
        out
    }

    /// What their zones light round them: where, how far, what color and how bright
    pub(super) fn power_light(&self, z: &Zone, kind: Lasting) -> Option<(Vector3<f64>, f64, [f32; 3], f32)> {
        let color = z.profile.element.map_or([0.95, 0.9, 0.8], Element::light);
        let fade = (z.life / 0.3).min(1.0) as f32;
        match kind {
            Lasting::Vortex => Some((z.position, 7.0, color, 0.9 * fade)),
            Lasting::Nova => Some((z.position + Vector3::new(0.0, 0.0, self.vpm), 10.0, color, 0.8 * fade)),
            Lasting::Stuck { .. } => Some((z.position, 3.0, color, 0.6)),
            Lasting::Fissure | Lasting::Line => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::Library;

    /// Open ground, 48 m across, four meters of earth under four of sky
    fn field() -> World {
        let size = [384u32, 384, 64];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::DIRT);
        world.voxels_per_meter = 8;
        world
    }

    /// A legendary or mythic weapon of `family`, as one would drop
    fn legendary(library: &Library, family: usize) -> Weapon {
        (0..40_000u32)
            .map(|seed| library.generate(hash(seed ^ 0x90e7), Some(family), 8.0))
            .find(|w| w.profile.power.is_some())
            .unwrap_or_else(|| panic!("no legendary or mythic {} rolled", library.families[family].id))
    }

    /// What using a power looked like, a pack ahead or not
    struct Used {
        /// Seconds after right click that anything of it was first to be seen out away from the
        /// weapon, and seconds it was to be seen in all
        first_seen: Option<f64>,
        seen: f64,
        /// How far the weapon's tip moved through it, in meters
        travel: f64,
        /// Seconds after right click that the pack was first hurt by it, and whether it went
        first_hurt: Option<f64>,
        went: bool,
    }

    /// Right click with `weapon` as a player would to use its power, standing on open ground,
    /// looking a little down at it, with `foes` a pack of bandits 8.5 m ahead, out of reach of the
    /// weapon's own blows, a whip's crack too, watching three seconds of it
    fn use_power(world: &World, weapon: &Weapon, foes: bool) -> Used {
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.12), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        let mut animals = if foes { crate::animals::spawn_pack(world, &water, &[Look::Bandit; 4], feet + Vector3::new(0.0, 8.5 * vpm, 0.0), crate::animals::PACK_SPREAD, 3) } else { Vec::new() };
        let health: Vec<f64> = animals.iter().map(|a| a.health).collect();
        let mut combat = Combat::new(1, 0);
        let dt = 1.0 / 120.0;
        let start = 0.4;
        let mut out = Used { first_seen: None, seen: 0.0, travel: 0.0, first_hurt: None, went: false };
        let mut last_tip: Option<Vector3<f64>> = None;
        for frame in 0..((start + 3.0) / dt) as usize {
            let t = frame as f64 * dt - start;
            // As a player would, see power_controls()
            let controls = combat.power_controls(weapon, t, dt);
            let mut arena = Arena { world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
            combat.update(&mut arena, Some((0, weapon)), None, controls, dt);
            if t < 0.0 {
                continue;
            }
            out.went |= combat.power.cooldown > 0.0;
            let tip = combat.held_point(weapon, &view, false);
            if let Some(last) = last_tip {
                out.travel += (tip - last).norm() / vpm;
            }
            last_tip = Some(tip);
            if out.first_hurt.is_none() && animals.iter().zip(&health).any(|(a, &h)| a.health < h - 1e-6) {
                out.first_hurt = Some(t);
            }
            // What of the power's to be seen: on screen, not lost in the distance, out away
            // from the weapon
            let hand = view.eye + view.rotation * combat.pose(weapon).hand * vpm;
            let away = |at: &Vector3<f64>| (at - tip).norm() > 1.2 * vpm && (at - hand).norm() > 1.2 * vpm;
            let on = |instance: &AnimalInstance| {
                let (at, reach) = instance.sphere();
                let d = (at - view.eye).norm();
                away(&at) && d > 0.4 * vpm && reach / d > 0.004 && view.project(at).is_some_and(|[x, y]| (-0.02..=1.02).contains(&x) && (-0.02..=1.02).contains(&y))
            };
            // What it shoots and leaves going, and what it sets off: bursts, eruptions, sparks
            if combat.flying_instances(&view).iter().any(|(_, i)| on(i)) {
                out.seen += dt;
                out.first_seen.get_or_insert(t);
            }
        }
        out
    }

    /// Every family's legendary and mythic weapons have a power on right click, its own family's,
    /// named and described in every element, with a cooldown, those that cast costing mana; and
    /// a weapon short of legendary has none
    #[test]
    fn every_legendary_family_has_a_power() {
        let library = Library::load();
        let mut seen = Vec::new();
        for (family, f) in library.families.iter().enumerate() {
            let power = Power::of(&f.id).unwrap_or_else(|| panic!("{} has no power", f.id));
            let weapon = legendary(&library, family);
            assert_eq!(weapon.profile.power, Some(power), "{}", f.id);
            seen.push(power);
            let common = library.generate(7, Some(family), 0.0);
            assert!(common.profile.power.is_none() || common.profile.special.is_some(), "a common {} has a power", f.id);
        }
        for power in Power::ALL {
            assert!(seen.contains(&power), "{power:?} isn't any family's");
            assert!(power.cooldown() >= 6.0, "{power:?} comes round too often");
            for element in std::iter::once(None).chain(Element::ALL.map(Some)) {
                assert!(power.name(element).split(' ').count() >= 2 && power.description(element).len() > 60, "{power:?} in {element:?}");
            }
        }
    }

    /// The rule for every family's legendary and mythic weapons' power: right click, as a player
    /// would, sets it going; the weapon moves through it; what it does flies, rolls or erupts out
    /// to be seen, alone or with a pack ahead; it never reaches the pack 8.5 m off the moment it's
    /// used; and it does reach it
    #[test]
    fn every_power_is_seen_and_animated() {
        let library = Library::load();
        let world = field();
        let mut broken = Vec::new();
        for (family, f) in library.families.iter().enumerate() {
            let weapon = legendary(&library, family);
            let name = format!("{} on a {}", weapon.profile.power.unwrap().name(weapon.profile.element), f.id);
            for foes in [false, true] {
                let used = use_power(&world, &weapon, foes);
                let pack = if foes { "with a pack ahead" } else { "alone" };
                if !used.went {
                    broken.push(format!("{name}, {pack}: right click never set it going"));
                    continue;
                }
                if used.travel < 0.2 {
                    broken.push(format!("{name}, {pack}: the weapon only moves {:.2} m", used.travel));
                }
                if used.seen < 0.3 {
                    broken.push(format!("{name}, {pack}: it's only to be seen for {:.2} s", used.seen));
                }
                if foes {
                    match used.first_hurt {
                        Some(h) if used.first_seen.is_some_and(|s| h <= s + 1.5 / 120.0) => broken.push(format!("{name}: the pack's hurt {h:.3} s in, the moment it's first seen")),
                        Some(_) => {}
                        None => broken.push(format!("{name}: it never reaches the pack 8.5 m ahead")),
                    }
                }
            }
        }
        assert!(broken.is_empty(), "{} break the rule:\n{}", broken.len(), broken.join("\n"));
    }

    /// Where a family's power's shots and zones go, used as a player would, to look at:
    /// POWER=bombs cargo test --release power_paths -- --ignored --nocapture
    #[test]
    #[ignore]
    fn power_paths() {
        let library = Library::load();
        let world = field();
        let only = std::env::var("POWER").unwrap_or_default();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.12), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        for (family, f) in library.families.iter().enumerate() {
            if !only.is_empty() && f.id != only {
                continue;
            }
            let weapon = legendary(&library, family);
            println!("== {} on a {}", weapon.profile.power.unwrap().name(weapon.profile.element), f.id);
            let mut animals = crate::animals::spawn_pack(&world, &water, &[Look::Bandit; 4], feet + Vector3::new(0.0, 8.5 * vpm, 0.0), crate::animals::PACK_SPREAD, 3);
            let mut combat = Combat::new(1, 0);
            let dt = 1.0 / 120.0;
            for frame in 0..(3.4 / dt) as usize {
                let t = frame as f64 * dt - 0.4;
                let controls = combat.power_controls(&weapon, t, dt);
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
                combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
                if frame % 12 == 0 && t >= 0.0 {
                    let at = |v: Vector3<f64>| format!("({:.1}, {:.1}, {:.1})", (v.x - feet.x) / vpm, (v.y - feet.y) / vpm, (v.z - feet.z) / vpm);
                    let shots: Vec<String> = combat.projectiles.iter().filter_map(|p| if let Trick::Signature(Sig::Power(s)) = p.trick { Some(format!("{s:?} {}", at(p.position))) } else { None }).collect();
                    let zones: Vec<String> = combat.zones.iter().filter_map(|z| if let ZoneKind::Signature(SigZone::Power(k)) = z.kind { Some(format!("{k:?} {}", at(z.position))) } else { None }).collect();
                    let hurt = animals.iter().filter(|a| a.health < a.max_health).count();
                    let doing = match combat.action {
                        Action::Power { .. } => "power",
                        Action::Swing { .. } => "swing",
                        Action::Draw { .. } => "draw",
                        Action::Brace { .. } => "brace",
                        Action::Throw { .. } => "throw",
                        Action::Idle => "idle",
                        _ => "other",
                    };
                    println!("{t:.2} s: {doing} shots {shots:?} zones {zones:?} hurt {hurt}");
                }
            }
        }
    }

    /// No basic function lost: a crossbow or gun with no power still loads early on right click,
    /// and one with a power shoots it on right click and loads early on Q
    #[test]
    fn right_click_still_loads_and_q_does_too() {
        let library = Library::load();
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::identity(), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        // A repeater, so one shot leaves it neither full nor empty
        let family = library.family("repeater").expect("repeaters");
        let plain = library.generate(7, Some(family), 0.0);
        let grand = legendary(&library, family);
        for (weapon, legendary) in [(&plain, false), (&grand, true)] {
            assert_eq!(weapon.profile.power.is_some(), legendary);
            let mut animals = Vec::new();
            let mut combat = Combat::new(1, 0);
            let mut step = |combat: &mut Combat, controls: Controls| {
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
                combat.update(&mut arena, Some((0, weapon)), None, controls, 1.0 / 60.0);
            };
            for _ in 0..30 {
                step(&mut combat, Controls::default());
            }
            // One shot, so it's not full, then right click
            step(&mut combat, Controls { attack: true, holding: true, ..Default::default() });
            for _ in 0..60 {
                step(&mut combat, Controls::default());
            }
            step(&mut combat, Controls { heavy: true, aiming: true, ..Default::default() });
            if legendary {
                assert!(matches!(combat.action, Action::Power { .. }), "right click shoots a legendary repeater's power");
                for _ in 0..60 {
                    step(&mut combat, Controls::default());
                }
                step(&mut combat, Controls { reload: true, ..Default::default() });
            }
            assert!(matches!(combat.action, Action::Reload { .. }), "a {} repeater loads early", if legendary { "legendary" } else { "plain" });
        }
    }
}
