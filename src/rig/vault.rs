//! A jointed boss bounding up out of where it's stuck, see src\bosses\vault.rs. What has a leap of
//! its own, the beasts, the eight-legged, the knights and the Grovemaul, bounds as it leaps; what
//! walks on two legs and has none crouches low with its arms swung back, springs up with its knees
//! drawn up under it and its arms thrown forward, and lands heavily, sinking into its knees; what
//! floats only rises.

use super::biped::{Frame, Stance};
use super::*;

/// What `m` is to be posed as: a bound as a leap for those with a leap of their own
pub fn recast(look: Look, m: &Moment) -> Moment {
    let leaps = matches!(family(look), Family::Beast | Family::Crawler | Family::Knight) || look == Look::Grovemaul;
    match m.doing {
        Some(d) if d.m == Move::Vault && leaps => Moment { doing: Some(Doing { m: Move::Leap, ..d }), ..*m },
        _ => *m,
    }
}

/// The pose a biped of frame `f` wants, `want` as its build would have it, for a bound, if it's
/// making one: crouched, then in the air, then landed and getting up out of it
pub fn over(f: &Frame, m: &Moment, want: (Stance, f64, f64)) -> (Stance, f64, f64) {
    let Some(d) = m.doing.filter(|d| d.m == Move::Vault) else {
        return want;
    };
    let base = want.0;
    let legs = f.leg[0] + f.leg[1];
    let wide = f.hip_w + 0.06;
    let arm = f.arm[0] + f.arm[1];
    let shoulder = f.waist + f.shoulder.z;
    let side = f.shoulder.y.abs() + 0.12;
    let crouched = Stance {
        hips: [-0.06 * legs, 0.0, -0.3 * legs],
        lean: 0.4,
        bend: 0.25,
        roll: 0.0,
        nod: -0.35,
        hands: [[-0.45 * arm, -side, shoulder - 0.75 * arm], [-0.45 * arm, side, shoulder - 0.75 * arm]],
        feet: [[0.12 * legs, -wide, f.ankle], [-0.08 * legs, wide, f.ankle]],
        toes: [0.15, 0.15],
        ..base
    };
    let flying = Stance {
        hips: [0.0, 0.0, 0.04 * legs],
        lean: -0.12,
        bend: -0.15,
        roll: 0.0,
        nod: -0.2,
        hands: [[0.35 * arm, -side, shoulder + 0.35 * arm], [0.35 * arm, side, shoulder + 0.35 * arm]],
        feet: [[0.22 * legs, -f.hip_w, f.ankle + 0.38 * legs], [-0.12 * legs, f.hip_w, f.ankle + 0.3 * legs]],
        toes: [0.5, 0.5],
        ..base
    };
    let landed = Stance {
        hips: [0.08 * legs, 0.0, -0.34 * legs],
        lean: 0.35,
        bend: 0.3,
        roll: 0.0,
        nod: 0.05,
        hands: [[0.55 * arm, -side, shoulder - 0.6 * arm], [0.55 * arm, side, shoulder - 0.6 * arm]],
        feet: [[0.18 * legs, -wide - 0.05, f.ankle], [-0.18 * legs, wide + 0.05, f.ankle]],
        toes: [0.0, 0.0],
        ..base
    };
    match d.stage {
        Stage::Wind => (along(d.t, &[(0.0, base, Ease::Linear), (d.len, crouched, Ease::Gather)]).0, 14.0, 0.0),
        Stage::Act => {
            let (s, quick) = along(d.t, &[(0.0, crouched, Ease::Linear), (d.len * 0.16, flying, Ease::Over), (d.len * 0.78, flying, Ease::Linear), (d.len, landed, Ease::Snap)]);
            (s, if quick { 40.0 } else { 18.0 }, 0.0)
        }
        Stage::Recover => (along(d.t, &[(0.0, landed, Ease::Linear), (d.len, base, Ease::Smooth)]).0, 10.0, 0.0),
        _ => want,
    }
}
