//! What the player's painted on their body, in the window a new character's made in, see
//! studio.rs: voxels of its pieces in colours of their own, and on the head voxels added or taken
//! away, for a beard or a crest of hair. Kept as a short list of just those voxels over the body
//! made for how they look, see body::models(), so how they look can still be changed after.
//!
//! Each voxel's kept by where it is from its piece's joint, not by where it is in the piece's
//! model, so a look that makes a piece bigger or smaller moves nothing: when skin, hair or build
//! changes after painting, everything painted is put back wherever there's still a voxel there to
//! paint, and what was added is added again, see Paint::apply(). Paint where a new look has no
//! voxel waits there, to show again should the look come back.
//!
//! What's painted in the colour of the skin or the hair on the head, or of the gloves, sleeves,
//! tunic, belt, boots or trousers on a piece drawn in them, is that colour of the look and changes
//! with it, see Colour::material(). Every other colour is a paint of its own, see PAINTS: materials
//! past the world's, from PAINT_FIRST, drawn on the player's body and arms in their own colours,
//! see PAINT in shaders\traverse.comp, so the colours the player chose their clothes in leave
//! them be.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nalgebra::Vector3;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::body::{self, BODY_RES, FOREARM, HEAD, LEG, PIECES, TORSO, UPPER_ARM};
use super::{CLOTHS, GLOVES, HAIR_COLORS, Looks, SKINS};
use crate::animals::Model;
use crate::worldgen::material::{AIR, CLOTH, LEATHER};

type V = Vector3<f64>;

/// The first of the paints' materials: far past the world's, so it'll be a long while before the
/// world's run into them. Has to match PAINT_FIRST in shaders\traverse.comp.
pub const PAINT_FIRST: u8 = 160;
// Where each set of the paints starts: the colours of every skin, hair, cloth, glove, the belt,
// boots and trousers a look can have, then the rest
const SKIN_PAINT: u8 = PAINT_FIRST;
const HAIR_PAINT: u8 = SKIN_PAINT + SKINS.len() as u8;
const CLOTH_PAINT: u8 = HAIR_PAINT + HAIR_COLORS.len() as u8;
const GLOVE_PAINT: u8 = CLOTH_PAINT + CLOTHS.len() as u8;
const BELT_PAINT: u8 = GLOVE_PAINT + GLOVES.len() as u8 - 1;
const BOOTS_PAINT: u8 = BELT_PAINT + 1;
const TROUSER_PAINT: u8 = BOOTS_PAINT + 1;
const EXTRA_PAINT: u8 = TROUSER_PAINT + CLOTHS.len() as u8;

/// The paints that aren't the colour of anything a look can have, for war paint, scars, tattoos
/// and the like, as they look, in sRGB
pub const EXTRAS: [(&str, [u8; 3]); 30] = [
    ("Soot", [16, 14, 14]),
    ("Slate", [60, 62, 70]),
    ("Grey", [122, 122, 124]),
    ("Silver", [192, 194, 198]),
    ("Chalk", [242, 240, 234]),
    ("Bone", [226, 212, 182]),
    ("Blood", [128, 14, 18]),
    ("Scarlet", [206, 38, 34]),
    ("Scar", [176, 96, 92]),
    ("Rose", [230, 142, 152]),
    ("Rust", [166, 82, 38]),
    ("Orange", [232, 128, 36]),
    ("Gold", [230, 184, 62]),
    ("Yellow", [246, 224, 92]),
    ("Lime", [150, 202, 70]),
    ("Green", [52, 138, 58]),
    ("Teal", [34, 128, 128]),
    ("Woad", [38, 70, 162]),
    ("Azure", [80, 150, 232]),
    ("Ice", [172, 218, 242]),
    ("Indigo", [58, 38, 128]),
    ("Violet", [130, 70, 182]),
    ("Magenta", [192, 58, 152]),
    ("Umber", [78, 50, 32]),
    ("Clay", [182, 120, 62]),
    ("Ash", [152, 142, 132]),
    ("Moss", [88, 100, 48]),
    ("Ink", [22, 28, 52]),
    ("Ember", [255, 168, 58]),
    ("Pale green", [178, 214, 160]),
];

/// How many paints there are. Has to match PAINT_SIZE in shaders\traverse.comp.
pub const PAINT_COUNT: usize = (EXTRA_PAINT - PAINT_FIRST) as usize + EXTRAS.len();

/// The colour of paint `m`, as it looks, in sRGB, and its name; None for what isn't a paint
pub fn paint(m: u8) -> Option<(&'static str, [u8; 3])> {
    let k = m.checked_sub(PAINT_FIRST)? as usize;
    let at = |first: u8| k - (first - PAINT_FIRST) as usize;
    Some(match m {
        _ if m < HAIR_PAINT => SKINS[at(SKIN_PAINT)],
        _ if m < CLOTH_PAINT => HAIR_COLORS[at(HAIR_PAINT)],
        _ if m < GLOVE_PAINT => CLOTHS[at(CLOTH_PAINT)],
        _ if m < BELT_PAINT => GLOVES[at(GLOVE_PAINT) + 1],
        _ if m == BELT_PAINT => ("Belt leather", body::BELT),
        _ if m == BOOTS_PAINT => ("Boot leather", body::BOOTS),
        _ if m < EXTRA_PAINT => {
            let (name, _) = CLOTHS[at(TROUSER_PAINT)];
            (name, Looks { tunic: at(TROUSER_PAINT) as u8, ..Default::default() }.trousers_rgb())
        }
        _ => *EXTRAS.get(at(EXTRA_PAINT))?,
    })
}

/// Every paint there is
pub fn paints() -> impl Iterator<Item = u8> {
    (0..PAINT_COUNT).map(|k| PAINT_FIRST + k as u8)
}

/// The colours of the look a piece can be painted in
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Skin,
    Hair,
    Tunic,
    Sleeves,
    Gloves,
    Belt,
    Boots,
    Trousers,
}

impl Role {
    pub const ALL: [Role; 8] = [Role::Skin, Role::Hair, Role::Tunic, Role::Sleeves, Role::Gloves, Role::Belt, Role::Boots, Role::Trousers];

    pub fn name(self) -> &'static str {
        match self {
            Role::Skin => "Skin",
            Role::Hair => "Hair",
            Role::Tunic => "Tunic",
            Role::Sleeves => "Sleeves",
            Role::Gloves => "Gloves",
            Role::Belt => "Belt",
            Role::Boots => "Boots",
            Role::Trousers => "Trousers",
        }
    }

    /// Its colour in `looks`, as it looks, in sRGB
    pub fn rgb(self, looks: &Looks) -> [u8; 3] {
        match self {
            Role::Skin => looks.skin_rgb(),
            Role::Hair => looks.hair_rgb(),
            Role::Tunic => looks.tunic_rgb(),
            Role::Sleeves => looks.sleeves_rgb(),
            Role::Gloves => looks.gloves_rgb(),
            Role::Belt => body::BELT,
            Role::Boots => body::BOOTS,
            Role::Trousers => looks.trousers_rgb(),
        }
    }

    /// The paint of its colour in `looks`, for a piece not drawn in it
    fn paint(self, looks: &Looks) -> u8 {
        let fit = |v: u8, n: usize| (v as usize).min(n - 1) as u8;
        match self {
            Role::Skin => SKIN_PAINT + fit(looks.skin, SKINS.len()),
            Role::Hair => HAIR_PAINT + fit(looks.hair_color, HAIR_COLORS.len()),
            Role::Tunic => CLOTH_PAINT + fit(looks.tunic, CLOTHS.len()),
            Role::Sleeves => CLOTH_PAINT + fit(looks.sleeves, CLOTHS.len()),
            Role::Gloves if looks.gloves == 0 => Role::Skin.paint(looks),
            Role::Gloves => GLOVE_PAINT + fit(looks.gloves, GLOVES.len()) - 1,
            Role::Belt => BELT_PAINT,
            Role::Boots => BOOTS_PAINT,
            Role::Trousers => TROUSER_PAINT + fit(looks.tunic, CLOTHS.len()),
        }
    }
}

/// What piece `p` has drawn in its leather and in its cloth, see body::colours()
pub fn roles(p: usize) -> (Role, Role) {
    match p {
        HEAD => (Role::Skin, Role::Hair),
        TORSO => (Role::Belt, Role::Tunic),
        _ if LEG.contains(&p) => (Role::Boots, Role::Trousers),
        _ => (Role::Gloves, Role::Sleeves),
    }
}

/// A colour to paint with: one of the look's, or a material of its own, a paint or one of the
/// world's, as the eyes' black, picked off the body
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Colour {
    Look(Role),
    Material(u8),
}

impl Colour {
    /// The material painting it on piece `p` puts there: the piece's leather or cloth where it's
    /// drawn in that colour of the look, so it follows the look, otherwise the paint of it
    pub fn material(self, p: usize, looks: &Looks) -> u8 {
        match self {
            Colour::Look(role) if roles(p).0 == role => LEATHER,
            Colour::Look(role) if roles(p).1 == role => CLOTH,
            Colour::Look(role) => role.paint(looks),
            Colour::Material(m) => m,
        }
    }

    /// What material `m` on piece `p` is, to paint with elsewhere
    pub fn of(m: u8, p: usize) -> Colour {
        match m {
            LEATHER => Colour::Look(roles(p).0),
            CLOTH => Colour::Look(roles(p).1),
            m => Colour::Material(m),
        }
    }

    /// How it looks for `looks`, in sRGB
    pub fn rgb(self, looks: &Looks) -> [u8; 3] {
        match self {
            Colour::Look(role) => role.rgb(looks),
            Colour::Material(m) => material_rgb(m, None, looks),
        }
    }

    pub fn name(self) -> String {
        match self {
            Colour::Look(role) => format!("{} (follows your looks)", role.name()),
            Colour::Material(m) => match paint(m) {
                Some((name, _)) => name.to_owned(),
                None => crate::assets::materials().get(m as usize).map_or_else(|| format!("material {m}"), |i| capitalised(&i.name)),
            },
        }
    }
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// How material `m` looks on piece `p` of the body for `looks`, in sRGB: its leather and cloth in
/// the look's colours, a paint in its own, the rest as the world has it
pub fn material_rgb(m: u8, p: Option<usize>, looks: &Looks) -> [u8; 3] {
    if let (Some(p), LEATHER | CLOTH) = (p, m) {
        return Colour::of(m, p).rgb(looks);
    }
    if let Some((_, rgb)) = paint(m) {
        return rgb;
    }
    body::palette().get(m as usize).map_or([255, 0, 255], |c| c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
}

/// One voxel painted: what it is now, and whether it's been added where there was none
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    /// Its material, AIR for one taken away
    pub voxel: u8,
    pub added: bool,
}

/// How far past the head as it's made voxels can be added, in its voxels, every way: room for a
/// crest of hair or a long beard, while the piece stays small for the shader
pub const ROOM: i32 = 6;
/// The most voxels that are kept painted, so a save stays small
pub const MOST: usize = 8192;

/// Everything painted, by piece and where it is from the piece's joint, in the piece's voxels:
/// see shift()
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Paint {
    cells: BTreeMap<(u8, [i8; 3]), Cell>,
}

/// Where voxel (0, 0, 0) of a piece's model is from its joint, in its voxels, `anchor` being
/// where the model's put from the joint, see animals::voxelize(): so voxel `i` is kept as
/// `i + shift`
pub fn shift(model: &Model, anchor: &V) -> [i32; 3] {
    [0, 1, 2].map(|a| (anchor[a] * BODY_RES - if a < 2 { model.size[a] as f64 / 2.0 } else { 0.0 } + 0.5).floor() as i32)
}

/// Where voxel `index` of the model is kept
pub fn key_of(model: &Model, anchor: &V, index: [i32; 3]) -> Option<[i8; 3]> {
    let s = shift(model, anchor);
    let k = [0, 1, 2].map(|a| index[a] + s[a]);
    k.iter().all(|&v| (i8::MIN as i32..=i8::MAX as i32).contains(&v)).then(|| k.map(|v| v as i8))
}

/// Which voxel of the model `key` is, which may be outside it
pub fn index_of(model: &Model, anchor: &V, key: [i8; 3]) -> [i32; 3] {
    let s = shift(model, anchor);
    [0, 1, 2].map(|a| key[a] as i32 - s[a])
}

/// The voxel of `model` at `index`, AIR outside it
pub fn voxel(model: &Model, index: [i32; 3]) -> u8 {
    if (0..3).any(|a| index[a] < 0 || index[a] >= model.size[a] as i32) {
        return AIR;
    }
    let [x, y, z] = index.map(|v| v as usize);
    let [sx, sy, _] = model.size.map(|v| v as usize);
    model.voxels[x + (y + z * sy) * sx]
}

/// Whether a voxel can be added at `key` to piece `p`, as it's made in `made`: on the head only,
/// and no further than ROOM past it
pub fn has_room(p: usize, made: &(Model, V), key: [i8; 3]) -> bool {
    let i = index_of(&made.0, &made.1, key);
    p == HEAD && (0..3).all(|a| i[a] >= -ROOM && i[a] < made.0.size[a] as i32 + ROOM)
}

/// The piece mirroring `p` across the middle of the body: the other arm or leg, or itself
pub fn partner(p: usize) -> usize {
    for pair in [UPPER_ARM, FOREARM, LEG] {
        if pair[0] == p {
            return pair[1];
        }
        if pair[1] == p {
            return pair[0];
        }
    }
    p
}

/// The piece and place mirroring `key` of piece `p` across the middle of the body, by `made`, the
/// body as it's made before it's painted, whose left and right are alike: the voxel nearest where
/// the middle of that one is mirrored, which is that one again mirrored back
pub fn mirrored(made: &[(Model, V)], p: usize, key: [i8; 3]) -> Option<(usize, [i8; 3])> {
    let q = partner(p);
    let (m, a) = made.get(q)?;
    // How far the middle of each voxel is across from its key, in voxels
    let off = centre(m, a, [0; 3]).y * BODY_RES - shift(m, a)[1] as f64;
    let y = (0.5 - 2.0 * off).floor() as i32 - key[1] as i32;
    i8::try_from(y).ok().map(|y| (q, [key[0], y, key[2]]))
}

/// Where the middle of voxel `index` of a piece's model is from its joint, in meters
pub fn centre(model: &Model, anchor: &V, index: [i32; 3]) -> V {
    let corner = anchor - V::new(model.size[0] as f64 / 2.0, model.size[1] as f64 / 2.0, 0.0) / BODY_RES;
    corner + V::new(index[0] as f64 + 0.5, index[1] as f64 + 0.5, index[2] as f64 + 0.5) / BODY_RES
}

impl Paint {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn get(&self, p: usize, key: [i8; 3]) -> Option<Cell> {
        self.cells.get(&(p as u8, key)).copied()
    }

    /// Sets what's painted at `key` of piece `p`, None for nothing, giving back what was
    pub fn set(&mut self, p: usize, key: [i8; 3], cell: Option<Cell>) -> Option<Cell> {
        match cell {
            Some(c) if p < PIECES && (self.cells.len() < MOST || self.cells.contains_key(&(p as u8, key))) => self.cells.insert((p as u8, key), c),
            Some(_) => self.get(p, key),
            None => self.cells.remove(&(p as u8, key)),
        }
    }

    /// Everything painted on piece `p`
    pub fn on(&self, p: usize) -> impl Iterator<Item = ([i8; 3], Cell)> + '_ {
        self.cells.range((p as u8, [i8::MIN; 3])..=(p as u8, [i8::MAX; 3])).map(|(&(_, k), &c)| (k, c))
    }

    /// Only what a hand-edited save might have that can't be: pieces that aren't, voxels added
    /// off the head, and more than MOST
    pub fn mend(&mut self) {
        self.cells.retain(|&(p, _), c| (p as usize) < PIECES && (!c.added || p as usize == HEAD) && !(c.added && c.voxel == AIR));
        while self.cells.len() > MOST {
            self.cells.pop_last();
        }
    }

    /// As kept in a save: nine hexadecimal digits a voxel, its piece with 8 added for one added,
    /// where it is, a byte each, and its material
    pub fn to_text(&self) -> String {
        let mut s = String::with_capacity(self.cells.len() * 9);
        for (&(p, k), c) in &self.cells {
            s.push_str(&format!("{:x}{:02x}{:02x}{:02x}{:02x}", p | if c.added { 8 } else { 0 }, k[0] as u8, k[1] as u8, k[2] as u8, c.voxel));
        }
        s
    }

    /// Read back from to_text(), leaving out whatever isn't right
    pub fn from_text(text: &str) -> Paint {
        let digits: Vec<u8> = text.bytes().filter(|b| b.is_ascii_hexdigit()).collect();
        let mut paint = Paint::default();
        for chunk in digits.chunks_exact(9) {
            let Ok(s) = std::str::from_utf8(chunk) else { continue };
            let Ok(v) = u64::from_str_radix(s, 16) else { continue };
            let byte = |k: u32| (v >> (8 * k)) as u8;
            let head = (v >> 32) as u8;
            let cell = Cell { voxel: byte(0), added: head & 8 != 0 };
            paint.cells.insert((head & 7, [byte(3) as i8, byte(2) as i8, byte(1) as i8]), cell);
        }
        paint.mend();
        paint
    }

    /// Paints piece `p`, `made` as body::models() makes it for the look, see the top of this file:
    /// grown to take what's added outside it
    pub fn apply_piece(&self, p: usize, made: &mut (Model, V)) {
        if self.on(p).next().is_none() {
            return;
        }
        let size = made.0.size.map(|v| v as i32);
        let (mut lo, mut hi) = ([0i32; 3], size);
        for (key, cell) in self.on(p) {
            if cell.added && has_room(p, made, key) {
                let i = index_of(&made.0, &made.1, key);
                for a in 0..3 {
                    lo[a] = lo[a].min(i[a]);
                    hi[a] = hi[a].max(i[a] + 1);
                }
            }
        }
        if lo != [0; 3] || hi != size {
            grow(made, lo, hi);
        }
        let (model, anchor) = made;
        for (key, cell) in self.on(p) {
            let i = index_of(model, anchor, key);
            if (0..3).any(|a| i[a] < 0 || i[a] >= model.size[a] as i32) {
                continue;
            }
            let [sx, sy, _] = model.size.map(|v| v as usize);
            let v = &mut model.voxels[i[0] as usize + (i[1] as usize + i[2] as usize * sy) * sx];
            if cell.added || *v != AIR {
                *v = cell.voxel;
            }
        }
    }

    /// Paints every piece of `made`
    pub fn apply(&self, made: &mut [(Model, V)]) {
        for (p, m) in made.iter_mut().enumerate().take(PIECES) {
            self.apply_piece(p, m);
        }
    }
}

/// `made` grown to run from `lo` to `hi` in its voxels as they are, staying where it was
fn grow(made: &mut (Model, V), lo: [i32; 3], hi: [i32; 3]) {
    let (model, anchor) = made;
    let old = model.size;
    let size = [0, 1, 2].map(|a| (hi[a] - lo[a]) as u32);
    let mut voxels = vec![AIR; (size[0] * size[1] * size[2]) as usize];
    for z in 0..old[2] {
        for y in 0..old[1] {
            for x in 0..old[0] {
                let to = [x as i32 - lo[0], y as i32 - lo[1], z as i32 - lo[2]].map(|v| v as u32);
                voxels[(to[0] + (to[1] + to[2] * size[1]) * size[0]) as usize] = model.voxels[(x + (y + z * old[1]) * old[0]) as usize];
            }
        }
    }
    // Where voxel (0, 0, 0) is from the joint, in voxels, then the middle of the bottom again
    let origin = [0, 1, 2].map(|a| anchor[a] * BODY_RES - if a < 2 { old[a] as f64 / 2.0 } else { 0.0 } + lo[a] as f64);
    *anchor = V::new((origin[0] + size[0] as f64 / 2.0) / BODY_RES, (origin[1] + size[1] as f64 / 2.0) / BODY_RES, origin[2] / BODY_RES);
    *model = Model { size, voxels };
}

impl Serialize for Paint {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_text())
    }
}

impl<'de> Deserialize<'de> for Paint {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Anything but text, as a hand-edited save might have, is nothing painted
        let value = serde_json::Value::deserialize(d)?;
        Ok(value.as_str().map(Paint::from_text).unwrap_or_default())
    }
}

/// The body for `looks` with `paint` on it, see body::models()
pub fn painted(looks: &Looks, paint: &Paint) -> Vec<(Model, V)> {
    let mut made = body::models(looks);
    paint.apply(&mut made);
    made
}

/// A face painted for screenshot runs, as `--set paint_demo=1` gives, as a player might at the
/// head's 32 voxels a meter: a mask of woad across the eyes, scarlet under it either side of the
/// mouth, a pointed beard in the hair's colour and a crest of it over the crown, and
/// woad on the backs of the hands
pub fn demo(looks: &Looks) -> Paint {
    let made: Vec<(Model, V)> = (0..PIECES).map(|p| body::piece(p, looks)).collect();
    let mut paint = Paint::default();
    let woad = EXTRA_PAINT + 17;
    let scarlet = EXTRA_PAINT + 7;
    let (head, anchor) = &made[HEAD];
    let hair = Colour::Look(Role::Hair).material(HEAD, looks);
    for z in -ROOM..head.size[2] as i32 + ROOM {
        for y in -ROOM..head.size[1] as i32 + ROOM {
            for x in -ROOM..head.size[0] as i32 + ROOM {
                let i = [x, y, z];
                let Some(key) = key_of(head, anchor, i) else { continue };
                let c = centre(head, anchor, i);
                let (cx, cy, cz) = (c.x, c.y.abs(), c.z);
                let here = voxel(head, i);
                if here != AIR {
                    if cx > 0.03 && here == LEATHER {
                        if (0.11..0.175).contains(&cz) {
                            paint.set(HEAD, key, Some(Cell { voxel: woad, added: false }));
                        } else if (0.075..0.11).contains(&cz) && (0.03..0.07).contains(&cy) {
                            paint.set(HEAD, key, Some(Cell { voxel: scarlet, added: false }));
                        }
                    }
                    continue;
                }
                let beard = (0.03..0.125).contains(&cx) && (-0.07..0.08).contains(&cz) && cy < 0.065 - (0.08 - cz) * 0.3;
                let crest = cy < 0.022 && (-0.12..0.075).contains(&cx) && cz > 0.24 && cz < 0.39 - 3.0 * (cx + 0.02).powi(2);
                if (beard || crest) && has_room(HEAD, &made[HEAD], key) {
                    paint.set(HEAD, key, Some(Cell { voxel: hair, added: true }));
                }
            }
        }
    }
    for (k, &p) in FOREARM.iter().enumerate() {
        let (m, a) = &made[p];
        let outward = if k == 0 { -1.0 } else { 1.0 };
        for z in 0..m.size[2] as i32 {
            for y in 0..m.size[1] as i32 {
                for x in 0..m.size[0] as i32 {
                    let i = [x, y, z];
                    let c = centre(m, a, i);
                    if voxel(m, i) != AIR && c.z < -0.22 && c.y * outward > -0.005
                        && let Some(key) = key_of(m, a, i)
                    {
                        paint.set(p, key, Some(Cell { voxel: woad, added: false }));
                    }
                }
            }
        }
    }
    paint
}
/// The arms held before the eye, `arms` right then left as weapons::effect_models() makes them, see
/// Combat::arm(), painted as the forearms of `body` are: each voxel of their gloves and sleeves the
/// paint on the forearm where it'd be on it, by how far along it is and which way round. None when
/// nothing's painted on the forearms, as the arms are drawn as they are then.
pub fn arms(paint: &Paint, looks: &Looks, body: &[(Model, V)], arms: [&(Model, V); 2]) -> Option<[Model; 2]> {
    if FOREARM.iter().all(|&p| paint.on(p).next().is_none()) || body.len() < PIECES {
        return None;
    }
    let res = crate::weapons::ARM_RES;
    Some(std::array::from_fn(|k| {
        let (arm, arm_anchor) = arms[k];
        let (piece, anchor) = &body[FOREARM[k]];
        let (made, _) = body::piece(FOREARM[k], looks);
        let lo_arm = arm_anchor - V::new(arm.size[0] as f64 / 2.0, arm.size[1] as f64 / 2.0, 0.0) / res;
        let lo = anchor - V::new(piece.size[0] as f64 / 2.0, piece.size[1] as f64 / 2.0, 0.0) / BODY_RES;
        let mut out = crate::assets::copy(arm);
        let [sx, sy, sz] = arm.size;
        for z in 0..sz {
            for y in 0..sy {
                for x in 0..sx {
                    let i = (x + (y + z * sy) * sx) as usize;
                    if arm.voxels[i] != LEATHER && arm.voxels[i] != CLOTH {
                        continue;
                    }
                    let p = lo_arm + V::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5) / res;
                    // How far along: the elbow, the end of the sleeve, the end of the cuff, the wrist
                    // and the end of the hand, on the arm held and on the forearm hanging
                    const ALONG: [(f64, f64); 5] = [(-0.34, 0.0), (-0.19, -0.17), (-0.09, -0.21), (-0.042, -0.235), (0.05, -0.32)];
                    let along = ALONG.windows(2).find(|w| p.y <= w[1].0).map_or(ALONG[4].1, |w| {
                        let t = ((p.y - w[0].0) / (w[1].0 - w[0].0)).clamp(0.0, 1.0);
                        w[0].1 + t * (w[1].1 - w[0].1)
                    });
                    // Which way round: the thumb's way ahead and the back of the hand outwards
                    let middle = if p.y < -0.09 { -0.01 + (p.y + 0.09) * 0.2 } else { 0.0 };
                    let (ahead, side) = (p.z - middle, -p.x);
                    let turn = side.atan2(ahead);
                    let way = V::new(turn.cos(), turn.sin(), 0.0);
                    // The voxel on the forearm's surface that way, at that height
                    let mut found = None;
                    for step in 0..14 {
                        let q = V::new(0.006, 0.0, along) + way * (step as f64 * 0.5 / BODY_RES);
                        let at = ((q - lo) * BODY_RES).map(|v| v.floor() as i32);
                        let at = [at.x, at.y, at.z];
                        if voxel(piece, at) != AIR {
                            found = Some(at);
                        } else if found.is_some() {
                            break;
                        }
                    }
                    if let Some(at) = found {
                        let (now, was) = (voxel(piece, at), voxel(&made, at));
                        if now != was && now != AIR {
                            out.voxels[i] = now;
                        }
                    }
                }
            }
        }
        out
    }))
}

/// Writes each piece of `made`, painted, to `dir` as a MagicaVoxel file, in the colours it's drawn
/// in for `looks`: to see or to print, as the game reads none of them back
pub fn export(made: &[(Model, V)], looks: &Looks, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (p, (model, _)) in made.iter().enumerate().take(PIECES) {
        let mut palette = crate::assets::palette();
        for (m, entry) in palette.iter_mut().enumerate().skip(1) {
            if m == LEATHER as usize || m == CLOTH as usize || paint(m as u8).is_some() {
                let rgb = material_rgb(m as u8, Some(p), looks);
                *entry = [rgb[0], rgb[1], rgb[2], 255];
            }
        }
        let grid = crate::vox::Grid { size: model.size, voxels: model.voxels.clone(), origin: [0, 0, 0] };
        let path = dir.join(format!("{}.vox", body::PIECE_NAMES[p]));
        std::fs::write(&path, crate::vox::write(&grid, &palette)).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::{Character, SECTION};
    use crate::save::{self, Mode, Save};

    fn made(looks: &Looks) -> Vec<(Model, V)> {
        (0..PIECES).map(|p| body::piece(p, looks)).collect()
    }

    /// The first solid voxel of piece `p` found looking in from its front, near its middle
    fn front_voxel(m: &(Model, V)) -> [i32; 3] {
        let [sx, sy, sz] = m.0.size.map(|v| v as i32);
        for x in (0..sx).rev() {
            let i = [x, sy / 2, sz / 2];
            if voxel(&m.0, i) != AIR {
                return i;
            }
        }
        panic!("nothing in front")
    }

    /// Painted, a voxel's the paint, added ones are there and taken ones gone, and the lot comes
    /// back the same through the whole save as it's written, in a few bytes a voxel
    #[test]
    fn edits_apply_and_come_back_from_the_save() {
        let looks = Looks::default();
        let body = made(&looks);
        let mut paint = Paint::default();
        let front = front_voxel(&body[HEAD]);
        let key = key_of(&body[HEAD].0, &body[HEAD].1, front).unwrap();
        let woad = EXTRA_PAINT + 17;
        paint.set(HEAD, key, Some(Cell { voxel: woad, added: false }));
        // Taken off the top of the torso, and added past the top of the head, so it grows
        let (t, ta) = &body[TORSO];
        let top = (0..t.size[2] as i32).rev().map(|z| [t.size[0] as i32 / 2, t.size[1] as i32 / 2, z]).find(|&i| voxel(t, i) != AIR).unwrap();
        let taken = key_of(t, ta, top).unwrap();
        paint.set(TORSO, taken, Some(Cell { voxel: AIR, added: false }));
        let over = [front[0] - 3, front[1], body[HEAD].0.size[2] as i32 + 2];
        let added = key_of(&body[HEAD].0, &body[HEAD].1, over).unwrap();
        paint.set(HEAD, added, Some(Cell { voxel: CLOTH, added: true }));

        let mut painted = made(&looks);
        paint.apply(&mut painted);
        let at = |m: &(Model, V), key| voxel(&m.0, index_of(&m.0, &m.1, key));
        assert_eq!(at(&painted[HEAD], key), woad);
        assert_eq!(at(&painted[TORSO], taken), AIR);
        assert_eq!(at(&painted[HEAD], added), CLOTH);
        assert!(painted[HEAD].0.size[2] > body[HEAD].0.size[2], "the head grows to take what's added");
        // Grown, everything else on it is where it was from the joint
        let unpainted = front_voxel(&body[HEAD]);
        let below = [unpainted[0], unpainted[1], unpainted[2] - 2];
        let k = key_of(&body[HEAD].0, &body[HEAD].1, below).unwrap();
        assert_eq!(at(&painted[HEAD], k), voxel(&body[HEAD].0, below));
        // Nothing else changes
        for p in [UPPER_ARM[0], LEG[1]] {
            assert_eq!(painted[p].0.voxels, body[p].0.voxels);
        }

        let character = Character { name: "Painted".into(), paint: paint.clone(), ..Default::default() };
        let mut s = Save { version: save::VERSION, mode: Mode::FreeRoam, ..Default::default() };
        s.set_section(SECTION, &character);
        let bytes = serde_json::to_vec_pretty(&s).unwrap();
        let back = save::parse(&bytes, Mode::FreeRoam).unwrap().section::<Character>(SECTION).unwrap();
        assert_eq!(back.paint, paint);
        assert_eq!(paint.to_text().len(), 9 * 3);
        // And a lot of it stays small
        let demo = demo(&looks);
        assert!(demo.len() > 60, "{} voxels painted", demo.len());
        assert!(demo.to_text().len() < 12 * demo.len());
        assert_eq!(Paint::from_text(&demo.to_text()), demo);
    }

    /// A save from before there was painting has nothing painted, and one with nothing painted says
    /// nothing of it; nonsense where it'd be is nothing painted too
    #[test]
    fn old_saves_load_unpainted() {
        let old = r#"{"version": 1, "mode": "free_roam", "seed": 5, "sections": {"character": {"name": "Ysolde", "looks": {"skin": 3}, "class": "mage"}}}"#;
        let s = save::parse(old.as_bytes(), Mode::FreeRoam).unwrap();
        let c = s.section::<Character>(SECTION).unwrap();
        assert_eq!(c.name, "Ysolde");
        assert!(c.paint.is_empty());
        let text = serde_json::to_string(&Character::default()).unwrap();
        assert!(!text.contains("paint"), "{text}");
        let odd: Character = serde_json::from_str(r#"{"paint": 12}"#).unwrap();
        assert!(odd.paint.is_empty());
        let mut bad = Paint::from_text("zz9ff0000a0");
        bad.mend();
        assert!(bad.len() <= 1);
        // Added off the head, it isn't kept
        let mut p = Paint::default();
        p.set(TORSO, [0, 0, 0], Some(Cell { voxel: CLOTH, added: true }));
        assert!(Paint::from_text(&p.to_text()).is_empty());
    }

    /// Painted, then the look changed, what's painted stays where it was from the joint, wherever
    /// there's still a voxel to paint; and what's in the look's colours follows them
    #[test]
    fn paint_outlasts_a_change_of_looks() {
        let looks = Looks::default();
        let paint = demo(&looks);
        let broad = Looks { build: 2, hair: 3, skin: 6, ..looks };
        let body = painted(&broad, &paint);
        let woad = EXTRA_PAINT + 17;
        assert!(body[HEAD].0.voxels.contains(&woad));
        assert!(body[FOREARM[0]].0.voxels.contains(&woad));
        // The beard's in the hair's colour, whatever it is
        assert_eq!(Colour::Look(Role::Hair).material(HEAD, &broad), CLOTH);
        assert_eq!(Colour::Look(Role::Skin).material(HEAD, &broad), LEATHER);
        assert_eq!(Colour::Look(Role::Skin).material(TORSO, &broad), SKIN_PAINT + 6);
        assert_eq!(paint_rgb(Colour::Look(Role::Skin).material(TORSO, &broad)), broad.skin_rgb());
        assert_eq!(paint_rgb(Colour::Look(Role::Trousers).material(HEAD, &broad)), broad.trousers_rgb());
        assert_eq!(paint_rgb(Colour::Look(Role::Gloves).material(HEAD, &Looks { gloves: 3, ..broad })), GLOVES[3].1);
        assert_eq!(Colour::of(LEATHER, FOREARM[1]), Colour::Look(Role::Gloves));
        // Every piece is still small enough for the shader
        for (m, _) in &body {
            assert!(m.size.iter().all(|&s| s < 255));
        }
    }

    fn paint_rgb(m: u8) -> [u8; 3] {
        paint(m).unwrap().1
    }

    /// Mirrored, a voxel on the right is the one on the left, on the same piece for the head and
    /// on the other arm for an arm, and mirrored again it's back where it was
    #[test]
    fn mirror_finds_the_other_side() {
        let looks = Looks::default();
        let body = made(&looks);
        for p in [HEAD, TORSO, FOREARM[0], LEG[1]] {
            let (m, a) = &body[p];
            let [sx, sy, sz] = m.size.map(|v| v as i32);
            let mut checked = 0;
            for z in 0..sz {
                for y in 0..sy {
                    for x in 0..sx {
                        let i = [x, y, z];
                        let key = key_of(m, a, i).unwrap();
                        let (q, back) = mirrored(&body, p, key).unwrap();
                        assert_eq!(q, partner(p));
                        let j = index_of(&body[q].0, &body[q].1, back);
                        assert_eq!([j[0], j[2]], [x, z]);
                        let (c, d) = (centre(m, a, i), centre(&body[q].0, &body[q].1, j));
                        assert!((c.y + d.y).abs() <= 0.5 / BODY_RES + 1e-9, "{} mirrored to {}", c.y, d.y);
                        assert_eq!(mirrored(&body, q, back), Some((p, key)));
                        checked += 1;
                    }
                }
            }
            assert!(checked > 100);
        }
        assert_eq!(partner(UPPER_ARM[1]), UPPER_ARM[0]);
        assert_eq!(partner(HEAD), HEAD);
    }

    /// The paints in the shader are these, in this order, after the world's materials
    #[test]
    fn the_shader_has_the_paints() {
        let text = include_str!("../../shaders/traverse.comp");
        let first: u32 = text.split("const uint PAINT_FIRST = ").nth(1).and_then(|s| s.split('u').next()).and_then(|s| s.trim().parse().ok()).unwrap();
        let size: usize = text.split("const uint PAINT_SIZE = ").nth(1).and_then(|s| s.split('u').next()).and_then(|s| s.trim().parse().ok()).unwrap();
        assert_eq!(first, PAINT_FIRST as u32);
        assert_eq!(size, PAINT_COUNT);
        assert!(crate::assets::materials().len() < PAINT_FIRST as usize, "the world's materials have run into the paints");
        let block = text.split("const vec3 PAINT[PAINT_SIZE]").nth(1).unwrap();
        let block = &block[..block.find(");").unwrap()];
        let colours: Vec<[f64; 3]> = block
            .split("vec3(")
            .skip(1)
            .map(|c| {
                let v: Vec<f64> = c[..c.find(')').unwrap()].split(',').map(|n| n.trim().parse().unwrap()).collect();
                [v[0], v[1], v[2]]
            })
            .collect();
        assert_eq!(colours.len(), PAINT_COUNT);
        for (k, m) in paints().enumerate() {
            let (name, rgb) = paint(m).unwrap();
            for c in 0..3 {
                assert!((colours[k][c] * 255.0 - rgb[c] as f64).abs() < 0.6, "paint {k}, {name}: {:?} in the shader, {rgb:?} here", colours[k]);
            }
        }
        assert!(paint(PAINT_FIRST + PAINT_COUNT as u8).is_none() && paint(PAINT_FIRST - 1).is_none());
        assert!(PAINT_FIRST as usize + PAINT_COUNT <= 256);
    }

    /// Painted on a forearm, the paint's on the arm held before the eye too, the glove's on the
    /// hand and the sleeve's on the sleeve; unpainted, the arms are left as they are
    #[test]
    fn the_arms_held_wear_the_forearms_paint() {
        let looks = Looks::default();
        let held = crate::weapons::effect_models();
        let pair = [&held[crate::weapons::ARMS], &held[crate::weapons::ARMS + 1]];
        assert!(arms(&Paint::default(), &looks, &made(&looks), pair).is_none());
        let paint = demo(&looks);
        let body = painted(&looks, &paint);
        let [right, left] = arms(&paint, &looks, &body, pair).unwrap();
        let woad = EXTRA_PAINT + 17;
        for (m, (base, _)) in [right, left].iter().zip(pair) {
            let painted = m.voxels.iter().filter(|&&v| v == woad).count();
            let gloves = base.voxels.iter().filter(|&&v| v == LEATHER).count();
            assert!(painted > 20 && painted < gloves, "{painted} of {gloves} voxels of glove painted");
            assert_eq!(m.size, base.size);
        }
    }
}
