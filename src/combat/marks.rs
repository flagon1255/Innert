//! What a spell of each element leaves where it goes off, a while after its burst, so each reads as
//! its own: fire a scorch with flames licking over it, frost spikes of ice bursting up and the ground
//! frozen round them, lightning a fork striking down out of the sky onto it, venom a cloud drifting
//! off on the wind, arcane magic a rune written there that blasts up a beat later, the void tendrils
//! of shadow reaching up, holy light a pillar of it coming down out of the sky, and blood a pool of
//! it. Only every so often, see MARK_GAP, and only one of each kind on the ground at once at the
//! most, see Combat::scorch_with(), so a stream of spells costs no more to draw than one.

use super::*;
use super::signatures::powers::Lasting;

/// Seconds after a spell leaves its mark before another can
const MARK_GAP: f64 = 0.45;

impl Combat {
    /// What a spell of `profile`'s element, of `form`, leaves where it went off at `at`, doing
    /// `damage`
    pub(super) fn spell_mark(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, form: Form, damage: f64) {
        if self.mark_timer > 0.0 {
            return;
        }
        self.mark_timer = MARK_GAP;
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let ground = Combat::floor_at(arena, at);
        // Only near the ground, not up on something in the air
        if (at - ground).norm() > 2.5 * vpm {
            return;
        }
        let near_eye = (ground - arena.view.eye).norm() < 3.0 * vpm;
        let Some(element) = profile.element else {
            return;
        };
        let look = Element::look(Some(element));
        match element {
            Element::Fire => {
                self.scorch_with(ground, 1.1, Element::Fire, 2.4);
                for _ in 0..5 {
                    let v = (self.any_direction() * 0.5 + up * 1.4) * vpm;
                    let at = ground + self.any_direction().component_mul(&Vector3::new(0.6, 0.6, 0.0)) * vpm;
                    self.fine(at, v, look, 0.6, -0.15);
                }
            }
            Element::Frost => {
                if !near_eye {
                    self.swells.push(Swell { model: ICE_BURST, position: ground, radius: 0.75, life: 1.1, lifetime: 1.1 });
                }
                self.scorch_with(ground, 1.4, Element::Frost, 3.0);
                for i in Combat::foes_round(arena, ground, 1.4) {
                    let s = &mut arena.animals[i].status;
                    s.chill = (s.chill + 0.25).min(1.0);
                    s.slowed = s.slowed.max(1.0);
                    s.slow = s.slow.max(0.3);
                }
            }
            Element::Storm => {
                let look = Element::look(Some(Element::Storm));
                self.sky_bolt(ground, look, 0.6, 0.0);
            }
            // A cloud of venom drifting off along the ground on the wind, thinning
            Element::Poison => {
                let drift = Vector3::new((self.time * 0.07).cos(), (self.time * 0.07).sin(), 0.0) * 0.6 * vpm;
                for _ in 0..4 {
                    let life = 1.8 + 0.8 * random(&mut self.rng);
                    let at = ground + (self.any_direction().component_mul(&Vector3::new(0.8, 0.8, 0.2)) + up * 0.5) * vpm;
                    let velocity = drift + self.any_direction() * 0.15 * vpm;
                    self.sparks.push(Spark { position: at, velocity, look, life, lifetime: life, gravity: -0.01, slot: SLOT_CLOUD });
                }
                self.cap_sparks();
                self.keep_pools_few();
                self.lingering.push(Lingering { cloud: true, tick: 0.4, ..Lingering::new(ground + up * 0.4 * vpm, 1.6, 2.2, Some(Element::Poison), profile, damage * 0.12) });
            }
            // A rune written on the ground, blasting up a beat later, see arsenal.rs
            Element::Arcane if form != Form::Hex => {
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Sigil)), profile, ground, Vector3::zeros(), 1.3, 0.85, damage * 0.25);
            }
            // Tendrils of shadow reaching up out of the ground, holding what's there a moment
            Element::Void => {
                if !near_eye {
                    self.swells.push(Swell { model: crate::effects::TENDRILS, position: ground, radius: 0.9, life: 1.2, lifetime: 1.2 });
                }
                for i in Combat::foes_round(arena, ground, 1.2) {
                    if arena.animals[i].boss.is_none() {
                        let s = &mut arena.animals[i].status;
                        s.rooted = s.rooted.max(0.6);
                    }
                }
            }
            // A pillar of light coming down out of the sky onto it
            Element::Holy => {
                if !near_eye {
                    self.swells.push(Swell { model: PILLAR, position: ground, radius: 0.55, life: 0.6, lifetime: 0.6 });
                }
                self.flashes.push(Flash { position: ground + up * 2.0 * vpm, color: Element::Holy.light(), reach: 7.0, life: 0.35, lifetime: 0.35 });
            }
            Element::Blood => self.scorch_with(ground, 1.0, Element::Blood, 2.6),
            _ => {}
        }
    }
}
