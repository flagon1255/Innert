//! What each kind of special ammo looks like in flight, see the `ammo` in assets\weapons.json:
//! a model of its own, each built by a function of its own, and next to it, as plain data, the
//! glow of its tracer and trail, the trail it leaves and the puff where it strikes, so every kind
//! reads apart from every other at a glance. The models go at the end of effect_models(), from
//! AMMO_MODELS on, in the order of AMMO_STYLES.

use super::*;

/// What an ammo leaves behind it as it flies, see combat::Combat::ammo_trail()
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Trail {
    /// Drops of blood flicked off its barbs
    Blood,
    /// Embers rising off burning cloth or tow
    Embers,
    /// A bitter mist, flakes of ice falling out of it
    Frost,
    /// Venom dripping
    Venom,
    /// Powder smoke off a burning fuse
    Smoke,
    /// A hot white streak
    Tracer,
    /// Glints of polished steel
    Glint,
    /// Dust shaken off a heavy knob
    Dust,
    /// Lightning crackling off it
    Crackle,
    /// Darkness drawn in round it
    Void,
    /// Motes of gold light
    Holy,
    /// A choking green haze
    Miasma,
    /// Arcane motes circling
    Arcane,
    /// Burning brimstone dripping, and its fumes
    Brimstone,
    /// Grit trickling out
    Grit,
    /// Threads streaming off a furled net
    Fibres,
    /// Bright sparks thrown off
    Sparks,
    /// Two thin streams, three bolts bound as one
    Twin,
}

/// What bursts off where it strikes, see combat::Combat::ammo_puff()
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Puff {
    Blood,
    Fire,
    Frost,
    Venom,
    Powder,
    Sparks,
    Dust,
    Storm,
    Void,
    Holy,
    Miasma,
    Arcane,
    Grit,
    Shards,
    Net,
}

/// How a kind of ammo looks flying
pub struct AmmoStyle {
    /// Its id in assets\weapons.json
    pub id: &'static str,
    /// Its model: along +x, an arrow's or bolt's nock at 0 and its head where a plain one's is, a
    /// ball's middle at 0, at AMMO_RES or SHOT_RES voxels a meter
    pub model: fn() -> (Model, Vector3<f64>),
    /// The colors of its tracer and trail: an element's, or None for white-hot
    pub glow: Option<Element>,
    pub trail: Trail,
    pub puff: Puff,
}

/// Voxels a meter of the arrows' and bolts' models, finer than a plain arrow's so what's bound
/// round the head shows
pub const AMMO_RES: f64 = 64.0;
/// Of the shot's, drawn several times as big as they are so they're seen
pub const AMMO_SHOT_RES: f64 = 160.0;

/// Every kind of ammo's look, in the order of its model among effect_models(), see AMMO_MODELS
pub const AMMO_STYLES: [AmmoStyle; 38] = [
    AmmoStyle { id: "broadhead_arrows", model: broadhead_arrow, glow: None, trail: Trail::Blood, puff: Puff::Blood },
    AmmoStyle { id: "fire_arrows", model: fire_arrow, glow: Some(Element::Fire), trail: Trail::Embers, puff: Puff::Fire },
    AmmoStyle { id: "frost_arrows", model: frost_arrow, glow: Some(Element::Frost), trail: Trail::Frost, puff: Puff::Frost },
    AmmoStyle { id: "venom_arrows", model: venom_arrow, glow: Some(Element::Poison), trail: Trail::Venom, puff: Puff::Venom },
    AmmoStyle { id: "blast_arrows", model: blast_arrow, glow: Some(Element::Fire), trail: Trail::Smoke, puff: Puff::Powder },
    AmmoStyle { id: "bodkin_arrows", model: bodkin_arrow, glow: None, trail: Trail::Tracer, puff: Puff::Sparks },
    AmmoStyle { id: "seeker_arrows", model: seeker_arrow, glow: Some(Element::Arcane), trail: Trail::Arcane, puff: Puff::Arcane },
    AmmoStyle { id: "splash_arrows", model: splash_arrow, glow: None, trail: Trail::Grit, puff: Puff::Grit },
    AmmoStyle { id: "vortex_arrows", model: vortex_arrow, glow: Some(Element::Void), trail: Trail::Void, puff: Puff::Void },
    AmmoStyle { id: "miasma_arrows", model: miasma_arrow, glow: Some(Element::Poison), trail: Trail::Miasma, puff: Puff::Miasma },
    AmmoStyle { id: "barbed_bolts", model: barbed_bolt, glow: None, trail: Trail::Blood, puff: Puff::Blood },
    AmmoStyle { id: "incendiary_bolts", model: incendiary_bolt, glow: Some(Element::Fire), trail: Trail::Embers, puff: Puff::Fire },
    AmmoStyle { id: "rime_bolts", model: rime_bolt, glow: Some(Element::Frost), trail: Trail::Frost, puff: Puff::Frost },
    AmmoStyle { id: "venom_bolts", model: venom_bolt, glow: Some(Element::Poison), trail: Trail::Venom, puff: Puff::Venom },
    AmmoStyle { id: "storm_bolts", model: storm_bolt, glow: Some(Element::Storm), trail: Trail::Crackle, puff: Puff::Storm },
    AmmoStyle { id: "bursting_bolts", model: bursting_bolt, glow: Some(Element::Fire), trail: Trail::Smoke, puff: Puff::Powder },
    AmmoStyle { id: "piercing_bolts", model: piercing_bolt, glow: None, trail: Trail::Tracer, puff: Puff::Sparks },
    AmmoStyle { id: "blunt_bolts", model: blunt_bolt, glow: None, trail: Trail::Dust, puff: Puff::Dust },
    AmmoStyle { id: "crescent_bolts", model: crescent_bolt, glow: None, trail: Trail::Glint, puff: Puff::Shards },
    AmmoStyle { id: "net_bolts", model: net_bolt, glow: None, trail: Trail::Fibres, puff: Puff::Net },
    AmmoStyle { id: "glancing_bolts", model: glancing_bolt, glow: None, trail: Trail::Sparks, puff: Puff::Sparks },
    AmmoStyle { id: "splitting_bolts", model: splitting_bolt, glow: None, trail: Trail::Twin, puff: Puff::Shards },
    AmmoStyle { id: "seeker_bolts", model: seeker_bolt, glow: Some(Element::Arcane), trail: Trail::Arcane, puff: Puff::Arcane },
    AmmoStyle { id: "void_bolts", model: void_bolt, glow: Some(Element::Void), trail: Trail::Void, puff: Puff::Void },
    AmmoStyle { id: "bloodletter_bolts", model: bloodletter_bolt, glow: Some(Element::Blood), trail: Trail::Blood, puff: Puff::Blood },
    AmmoStyle { id: "sunsteel_bolts", model: sunsteel_bolt, glow: Some(Element::Holy), trail: Trail::Holy, puff: Puff::Holy },
    AmmoStyle { id: "brimstone_bolts", model: brimstone_bolt, glow: Some(Element::Fire), trail: Trail::Brimstone, puff: Puff::Fire },
    AmmoStyle { id: "thunderclap_bolts", model: thunderclap_bolt, glow: Some(Element::Storm), trail: Trail::Sparks, puff: Puff::Storm },
    AmmoStyle { id: "scatter_shot", model: scatter_shot, glow: None, trail: Trail::Smoke, puff: Puff::Dust },
    AmmoStyle { id: "jagged_shot", model: jagged_shot, glow: Some(Element::Blood), trail: Trail::Blood, puff: Puff::Shards },
    AmmoStyle { id: "ember_shot", model: ember_shot, glow: Some(Element::Fire), trail: Trail::Embers, puff: Puff::Fire },
    AmmoStyle { id: "rime_shot", model: rime_shot, glow: Some(Element::Frost), trail: Trail::Frost, puff: Puff::Frost },
    AmmoStyle { id: "venom_shot", model: venom_shot, glow: Some(Element::Poison), trail: Trail::Venom, puff: Puff::Venom },
    AmmoStyle { id: "bursting_shot", model: bursting_shot, glow: Some(Element::Fire), trail: Trail::Smoke, puff: Puff::Powder },
    AmmoStyle { id: "steel_slugs", model: steel_slug, glow: None, trail: Trail::Tracer, puff: Puff::Sparks },
    AmmoStyle { id: "skipping_shot", model: skipping_shot, glow: Some(Element::Holy), trail: Trail::Glint, puff: Puff::Sparks },
    AmmoStyle { id: "cluster_shot", model: cluster_shot, glow: Some(Element::Fire), trail: Trail::Brimstone, puff: Puff::Powder },
    AmmoStyle { id: "miasma_shot", model: miasma_shot, glow: Some(Element::Poison), trail: Trail::Miasma, puff: Puff::Miasma },
];

/// Which of AMMO_STYLES ammo `id` has, if it has one
pub fn ammo_style(id: &str) -> Option<usize> {
    AMMO_STYLES.iter().position(|s| s.id == id)
}

/// Every ammo's model, in the order of AMMO_STYLES
pub(super) fn models() -> Vec<(Model, Vector3<f64>)> {
    AMMO_STYLES.iter().map(|s| (s.model)()).collect()
}

/// Where an arrow's head starts, and a bolt's, along it, see ARROW_FLETCH and QUARREL_FLETCH
const ARROW_START: f64 = 0.6;
const BOLT_START: f64 = 0.3;

fn shaped(p: Vec<Part>) -> (Model, Vector3<f64>) {
    voxelize(&p, AMMO_RES)
}

fn shot(p: Vec<Part>) -> (Model, Vector3<f64>) {
    voxelize(&p, AMMO_SHOT_RES)
}

/// A band round a shaft `round` thick at `x`, `width` long
fn band(x: f64, width: f64, round: f64, material: u8) -> Part {
    limb([x, 0.0, 0.0], [x + width, 0.0, 0.0], round, round, material)
}

/// Wide barbed steel, the fletching dyed red, a red cord round the shaft below the head
fn broadhead_arrow() -> (Model, Vector3<f64>) {
    let mut p = vec![band(ARROW_START - 0.05, 0.02, 0.008, FUR_RED)];
    for s in [-1.0, 1.0] {
        p.push(limb([ARROW_START + 0.02, s * 0.02, 0.0], [ARROW_START - 0.03, s * 0.034, 0.0], 0.004, 0.002, STEEL));
    }
    p.extend(arrow(None, 2, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// Oiled rags bound fat behind the head, burning
fn fire_arrow() -> (Model, Vector3<f64>) {
    let mut p = vec![limb([ARROW_START - 0.03, 0.0, 0.015], [ARROW_START - 0.07, 0.0, 0.045], 0.014, 0.002, FLAME), ball([ARROW_START - 0.03, 0.0, 0.012], 0.014, GLOW_FIRE), ellipsoid([ARROW_START - 0.03, 0.0, 0.0], [0.04, 0.02, 0.02], CLOTH)];
    p.extend(arrow(Some(Element::Fire), 0, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// A long rime-iron head, crystals of ice frozen up the shaft behind it
fn frost_arrow() -> (Model, Vector3<f64>) {
    let mut p = Vec::new();
    for (k, a) in [0.0f64, 2.1, 4.2].into_iter().enumerate() {
        let x = ARROW_START - 0.03 - 0.03 * k as f64;
        p.push(limb([x, 0.0, 0.0], [x - 0.02, a.cos() * 0.02, a.sin() * 0.02], 0.006, 0.001, CRYSTAL));
    }
    p.extend(arrow(Some(Element::Frost), 1, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// A grooved head, a bulb of venom glowing behind it, green fletching
fn venom_arrow() -> (Model, Vector3<f64>) {
    let mut p = vec![ellipsoid([ARROW_START - 0.02, 0.0, 0.0], [0.022, 0.013, 0.013], GLOW_POISON)];
    p.extend(arrow(Some(Element::Poison), 0, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// A clay pot of black powder behind the head, its fuse sputtering
fn blast_arrow() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([ARROW_START - 0.045, 0.0, 0.03], 0.007, GLOW_FIRE), limb([ARROW_START - 0.045, 0.0, 0.015], [ARROW_START - 0.045, 0.0, 0.028], 0.003, 0.003, LINEN), ball([ARROW_START - 0.045, 0.0, 0.0], 0.022, CLAY_POT)];
    p.extend(arrow(None, 0, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// A long narrow steel point, a steel collar behind it
fn bodkin_arrow() -> (Model, Vector3<f64>) {
    let mut p = vec![band(ARROW_START - 0.02, 0.025, 0.007, DARK_STEEL), band(ARROW_START - 0.06, 0.008, 0.007, STEEL)];
    p.extend(arrow(None, 1, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// Owl-fletched, a ring of runes glowing round the shaft
fn seeker_arrow() -> (Model, Vector3<f64>) {
    let mut p = Vec::new();
    ring(&mut p, [ARROW_START - 0.06, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.025, 0.004, GLOW_ARCANE);
    p.extend(arrow(Some(Element::Arcane), 1, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// A pouch of grit bound behind the head, grey
fn splash_arrow() -> (Model, Vector3<f64>) {
    let mut p = vec![ellipsoid([ARROW_START - 0.035, 0.0, 0.0], [0.035, 0.02, 0.02], GRAVEL), band(ARROW_START - 0.075, 0.006, 0.012, LEATHER)];
    p.extend(arrow(None, 0, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// A knot of void bound behind the head, dark with a glowing heart
fn vortex_arrow() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([ARROW_START - 0.03, 0.0, 0.0], 0.012, GLOW_VOID), ball([ARROW_START - 0.03, 0.0, 0.0], 0.022, OBSIDIAN)];
    p.extend(arrow(Some(Element::Void), 0, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// Rags of grave-rot wound round the shaft, green and dripping
fn miasma_arrow() -> (Model, Vector3<f64>) {
    let mut p = Vec::new();
    for k in 0..3 {
        let x = ARROW_START - 0.025 - 0.035 * k as f64;
        p.push(ellipsoid([x, 0.0, 0.0], [0.016, 0.014, 0.014], if k == 1 { GLOW_POISON } else { MOSS }));
    }
    p.push(limb([ARROW_START - 0.06, 0.0, -0.012], [ARROW_START - 0.07, 0.0, -0.03], 0.004, 0.001, GLOW_POISON));
    p.extend(arrow(Some(Element::Poison), 2, AMMO_RES, &ARROW_FLETCH));
    shaped(p)
}

/// A broad barbed head, a red tassel streaming from it
fn barbed_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![limb([BOLT_START - 0.01, 0.0, 0.0], [BOLT_START - 0.06, 0.0, -0.02], 0.006, 0.002, FUR_RED)];
    p.extend(quarrel(None, 1, AMMO_RES));
    shaped(p)
}

/// Pitch-soaked tow wound behind the head, burning
fn incendiary_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![limb([BOLT_START - 0.02, 0.0, 0.012], [BOLT_START - 0.055, 0.0, 0.035], 0.012, 0.002, FLAME), ellipsoid([BOLT_START - 0.025, 0.0, 0.0], [0.03, 0.016, 0.016], BARK_DARK), ball([BOLT_START - 0.02, 0.0, 0.01], 0.01, GLOW_FIRE)];
    p.extend(quarrel(Some(Element::Fire), 0, AMMO_RES));
    shaped(p)
}

/// A shaft of frozen ash, frost feathered out along it
fn rime_bolt() -> (Model, Vector3<f64>) {
    let mut p = Vec::new();
    for k in 0..4 {
        let x = BOLT_START - 0.03 - 0.05 * k as f64;
        let s = if k % 2 == 0 { 1.0 } else { -1.0 };
        p.push(limb([x, 0.0, 0.0], [x - 0.02, s * 0.015, 0.012], 0.005, 0.001, ICE));
    }
    p.extend(quarrel(Some(Element::Frost), 0, AMMO_RES));
    shaped(p)
}

/// A hollow head, venom sloshing green in a glass sleeve behind it
fn venom_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![limb([BOLT_START - 0.045, 0.0, 0.0], [BOLT_START - 0.005, 0.0, 0.0], 0.011, 0.011, GLOW_POISON)];
    p.extend(quarrel(Some(Element::Poison), 0, AMMO_RES));
    shaped(p)
}

/// Copper coils bound thick behind the head, crackling
fn storm_bolt() -> (Model, Vector3<f64>) {
    let mut p = Vec::new();
    for k in 0..4 {
        p.push(band(BOLT_START - 0.015 - 0.016 * k as f64, 0.008, 0.011, if k % 2 == 0 { GLOW_STORM } else { BRONZE }));
    }
    p.extend(quarrel(Some(Element::Storm), 0, AMMO_RES));
    shaped(p)
}

/// A charge of powder in an iron sleeve behind the head, a fuse lit at its back
fn bursting_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([BOLT_START - 0.075, 0.0, 0.0], 0.008, GLOW_FIRE), limb([BOLT_START - 0.065, 0.0, 0.0], [BOLT_START - 0.005, 0.0, 0.0], 0.015, 0.015, DARK_STEEL), band(BOLT_START - 0.04, 0.006, 0.017, BRONZE)];
    p.extend(quarrel(Some(Element::Fire), 0, AMMO_RES));
    shaped(p)
}

/// A hardened square point, long, its socket bright steel
fn piercing_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![limb([BOLT_START, 0.0, 0.0], [BOLT_START + 0.09, 0.0, 0.0], 0.008, 0.002, STEEL), band(BOLT_START - 0.02, 0.02, 0.009, STEEL)];
    p.extend(quarrel(None, 0, AMMO_RES));
    shaped(p)
}

/// A great knob of turned iron for a head
fn blunt_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([BOLT_START + 0.02, 0.0, 0.0], 0.022, DARK_STEEL)];
    p.extend(quarrel(None, 3, AMMO_RES));
    shaped(p)
}

/// A forked head like a new moon, polished bright
fn crescent_bolt() -> (Model, Vector3<f64>) {
    let mut p = Vec::new();
    for s in [-1.0, 1.0] {
        p.push(limb([BOLT_START + 0.01, s * 0.004, 0.0], [BOLT_START + 0.05, s * 0.03, 0.0], 0.005, 0.002, STEEL));
    }
    p.extend(quarrel(None, 2, AMMO_RES));
    shaped(p)
}

/// A weighted net furled in a bundle behind its blunt head, lead weights showing
fn net_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![ellipsoid([BOLT_START - 0.045, 0.0, 0.0], [0.045, 0.022, 0.022], LINEN)];
    for k in 0..3 {
        let a = k as f64 / 3.0 * TAU;
        p.push(ball([BOLT_START - 0.045, a.cos() * 0.022, a.sin() * 0.022], 0.007, DARK_STEEL));
    }
    p.extend(quarrel(None, 3, AMMO_RES));
    shaped(p)
}

/// A hard polished point, gilt bands round the shaft
fn glancing_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![band(BOLT_START - 0.02, 0.008, 0.009, GOLD), band(BOLT_START - 0.12, 0.008, 0.009, GOLD)];
    p.extend(quarrel(None, 0, AMMO_RES));
    shaped(p)
}

/// Three slim bolts bound as one, side by side
fn splitting_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![band(BOLT_START - 0.06, 0.01, 0.016, LEATHER)];
    for s in [-1.0, 1.0] {
        p.push(limb([0.02, s * 0.012, 0.0], [BOLT_START + 0.05, s * 0.012, 0.0], 0.003, 0.0015, STEEL));
    }
    p.extend(quarrel(None, 1, AMMO_RES));
    shaped(p)
}

/// Owl feathers, a rune cut into a ring round the shaft glowing
fn seeker_bolt() -> (Model, Vector3<f64>) {
    let mut p = Vec::new();
    ring(&mut p, [BOLT_START - 0.04, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.024, 0.004, GLOW_ARCANE);
    p.extend(quarrel(Some(Element::Arcane), 0, AMMO_RES));
    shaped(p)
}

/// An obsidian head, a sphere of darkness swelling round it
fn void_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([BOLT_START + 0.03, 0.0, 0.0], 0.01, GLOW_VOID), ball([BOLT_START + 0.03, 0.0, 0.0], 0.02, OBSIDIAN)];
    p.extend(quarrel(Some(Element::Void), 0, AMMO_RES));
    shaped(p)
}

/// A hollow crescent of dark steel, blood brimming in it
fn bloodletter_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([BOLT_START + 0.01, 0.0, 0.0], 0.012, GLOW_BLOOD), band(BOLT_START - 0.03, 0.03, 0.009, MEAT)];
    p.extend(quarrel(Some(Element::Blood), 2, AMMO_RES));
    shaped(p)
}

/// A gilded point, a halo of light round the shaft
fn sunsteel_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![limb([BOLT_START, 0.0, 0.0], [BOLT_START + 0.07, 0.0, 0.0], 0.01, 0.002, GOLD)];
    ring(&mut p, [BOLT_START - 0.05, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.028, 0.005, GLOW_HOLY);
    p.extend(quarrel(Some(Element::Holy), 0, AMMO_RES));
    shaped(p)
}

/// A cup of burning brimstone behind the head, yellow and smoking
fn brimstone_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([BOLT_START - 0.03, 0.0, 0.012], 0.012, FLOWER_YELLOW), ellipsoid([BOLT_START - 0.03, 0.0, 0.0], [0.022, 0.02, 0.02], CLAY_POT), ball([BOLT_START - 0.03, 0.0, 0.02], 0.008, GLOW_FIRE)];
    p.extend(quarrel(Some(Element::Fire), 0, AMMO_RES));
    shaped(p)
}

/// A drum of copper behind the head, lightning glowing through slits in it
fn thunderclap_bolt() -> (Model, Vector3<f64>) {
    let mut p = vec![limb([BOLT_START - 0.05, 0.0, 0.0], [BOLT_START - 0.01, 0.0, 0.0], 0.02, 0.02, BRONZE)];
    for k in 0..4 {
        let a = k as f64 / 4.0 * TAU;
        p.push(ball([BOLT_START - 0.03, a.cos() * 0.019, a.sin() * 0.019], 0.006, GLOW_STORM));
    }
    p.extend(quarrel(Some(Element::Storm), 0, AMMO_RES));
    shaped(p)
}

/// A cluster of small lead shot in a scrap of its paper cartridge
fn scatter_shot() -> (Model, Vector3<f64>) {
    let mut p = vec![limb([-0.03, 0.0, 0.0], [-0.05, 0.0, 0.0], 0.012, 0.016, LINEN)];
    for (x, y, z) in [(0.0, 0.0, 0.0), (0.0, 0.016, 0.0), (0.0, -0.016, 0.0), (0.0, 0.0, 0.016), (0.0, 0.0, -0.016), (-0.014, 0.008, 0.008), (-0.014, -0.008, -0.008)] {
        p.push(ball([x, y, z], 0.009, DARK_STEEL));
    }
    shot(p)
}

/// Rough-cast iron, all spikes and edges
fn jagged_shot() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([0.0, 0.0, 0.0], 0.018, DARK_STEEL)];
    for (k, d) in [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.3], [0.0, -1.0, -0.3], [0.3, 0.0, 1.0], [-0.3, 0.2, -1.0]].into_iter().enumerate() {
        p.push(limb([0.0; 3], d.map(|v| v * 0.034), 0.008, 0.002, if k % 2 == 0 { STEEL } else { FUR_RED }));
    }
    shot(p)
}

/// A ball rolled in fire-salts, flames licking back off it
fn ember_shot() -> (Model, Vector3<f64>) {
    let p = vec![limb([-0.01, 0.0, 0.0], [-0.06, 0.0, 0.012], 0.014, 0.002, FLAME), ball([0.0, 0.0, 0.0], 0.02, GLOW_FIRE), ball([0.01, 0.0, 0.0], 0.016, DARK_STEEL)];
    shot(p)
}

/// Lead cast round a shard of ice that never melts, the shard showing through
fn rime_shot() -> (Model, Vector3<f64>) {
    let p = vec![limb([-0.03, 0.0, 0.0], [0.03, 0.0, 0.0], 0.01, 0.002, CRYSTAL), ball([0.0, 0.0, 0.0], 0.019, ICE), ball([0.0, 0.0, 0.0], 0.012, GLOW_FROST)];
    shot(p)
}

/// A hollow ball of soft lead, venom oozing out of a hole in it
fn venom_shot() -> (Model, Vector3<f64>) {
    let p = vec![ball([0.012, 0.0, 0.0], 0.009, GLOW_POISON), limb([-0.005, 0.0, -0.01], [-0.03, 0.0, -0.025], 0.006, 0.002, GLOW_POISON), ball([0.0, 0.0, 0.0], 0.02, MORTAR)];
    shot(p)
}

/// A small iron grenade, a bronze band round it and its fuse burning
fn bursting_shot() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([-0.035, 0.0, 0.0], 0.009, GLOW_FIRE), limb([-0.024, 0.0, 0.0], [-0.034, 0.0, 0.0], 0.004, 0.004, LINEN)];
    ring(&mut p, [0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.024, 0.005, BRONZE);
    p.push(ball([0.0, 0.0, 0.0], 0.024, DARK_STEEL));
    shot(p)
}

/// A long slug, a hardened steel core bright in its nose
fn steel_slug() -> (Model, Vector3<f64>) {
    let p = vec![ball([0.022, 0.0, 0.0], 0.01, STEEL), limb([-0.025, 0.0, 0.0], [0.02, 0.0, 0.0], 0.013, 0.013, DARK_STEEL)];
    shot(p)
}

/// A flattened ball, a disc of lead spinning
fn skipping_shot() -> (Model, Vector3<f64>) {
    let p = vec![ellipsoid([0.0, 0.0, 0.0], [0.024, 0.024, 0.008], STEEL), ellipsoid([0.0, 0.0, 0.0], [0.016, 0.016, 0.01], MORTAR)];
    shot(p)
}

/// A grenade packed with bomblets, their caps poking out all over it
fn cluster_shot() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([-0.038, 0.0, 0.0], 0.009, GLOW_FIRE)];
    for k in 0..6 {
        let a = k as f64 / 6.0 * TAU;
        p.push(ball([0.0, a.cos() * 0.024, a.sin() * 0.024], 0.009, if k % 2 == 0 { BRONZE } else { GLOW_FIRE }));
    }
    p.push(ball([0.0, 0.0, 0.0], 0.026, DARK_STEEL));
    shot(p)
}

/// A ball cast round a bead of venom, a green haze clinging to it
fn miasma_shot() -> (Model, Vector3<f64>) {
    let mut p = vec![ball([0.0, 0.0, 0.0], 0.012, GLOW_POISON)];
    for k in 0..5 {
        let a = k as f64 / 5.0 * TAU;
        p.push(ball([-0.015, a.cos() * 0.022, a.sin() * 0.022], 0.009, MOSS));
    }
    p.push(ball([0.0, 0.0, 0.0], 0.019, CLAY));
    shot(p)
}
