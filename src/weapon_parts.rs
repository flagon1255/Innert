//! Weapons' parts as model files, see src\assets.rs. A weapon's built by code out of the parts its
//! family's slots were rolled, see weapons::build_look(): every shape the builders make is tagged
//! with the slot it's drawn for, see slot(), so each voxel of a weapon's model is known to be its
//! blade's, its hilt's and so on. `innert --export-models` writes each part of every family, the
//! tools' too, as built on that family's reference weapon to weapons\<family>\<part>.vox, and a
//! part's file, once it's changed, is drawn in place of the code's shape wherever that part's on a
//! weapon of that family: where the code puts the part on that weapon, so it still joins the rest
//! as it did, and where it's held, its tip and the rest stay the code's. Which parts a weapon's
//! rolled, their rarities and colours stay the code's too.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use nalgebra::Vector3;

use crate::animals::{self, Model, Part};
use crate::assets::{self, Grow, Sidecar};
use crate::weapons::{Library, WeaponLook};
use crate::worldgen::material::AIR;

/// What's done to each frame of a weapon as it's voxelized: given the frame, its voxels a meter,
/// its model and anchor, and each voxel's tag
type Change = Box<dyn FnMut(usize, f64, &mut Model, &mut Vector3<f64>, &[u8])>;

thread_local! {
    /// The slot what's made now is drawn for: 1 + its index among the family's slots, 0 for none
    static SLOT: Cell<u8> = const { Cell::new(0) };
    /// The weapon being built, if one is
    static BUILDING: RefCell<Option<Building>> = const { RefCell::new(None) };
    /// How many times finer than its usual voxels what's made now is voxelized, see finer()
    static FINE: Cell<f64> = const { Cell::new(1.0) };
}

/// How many times finer than usual the weapon being made now is voxelized: 1 but while finer()
/// makes one, for the builders to add what only shows at a finer grid
pub fn fine() -> f64 {
    FINE.get()
}

/// What `f` makes, a weapon's look, voxelized `k` times finer than usual: its shapes the same and
/// where it's held, its tip and the rest where they were, only its voxels smaller, `k` times as
/// many a meter. The look's `res` says so.
pub fn finer(k: f64, f: impl FnOnce() -> WeaponLook) -> WeaponLook {
    let mut look = at_fine(k, f);
    look.res *= k;
    look
}

/// What `f` makes with weapons voxelized `k` times finer than usual, see fine()
pub fn at_fine<R>(k: f64, f: impl FnOnce() -> R) -> R {
    let before = FINE.replace(k);
    let r = f();
    FINE.set(before);
    r
}

struct Building {
    /// Its family's slots
    slots: Vec<String>,
    /// How many of its frames have been voxelized so far
    frame: usize,
    /// Each frame's tags, a voxel each, kept when asked for
    tags: Option<Vec<Vec<u8>>>,
    change: Option<Change>,
}

/// From here on, what the builders make is drawn for slot `name` of the weapon being built, until
/// the next call; "" for what's no one slot's
pub fn slot(name: &str) {
    let tag = BUILDING.with_borrow(|b| b.as_ref().and_then(|b| b.slots.iter().position(|s| s == name)).map_or(0, |i| i as u8 + 1));
    SLOT.set(tag);
}

/// Runs `f`, which builds a weapon of a family with `slots`, its frames each changed by `change`
/// as they're voxelized; with `keep_tags`, also returns each frame's tags
fn building<R>(slots: &[String], keep_tags: bool, change: Option<Change>, f: impl FnOnce() -> R) -> (R, Vec<Vec<u8>>) {
    let before = BUILDING.replace(Some(Building { slots: slots.to_vec(), frame: 0, tags: keep_tags.then(Vec::new), change }));
    let slot_before = SLOT.replace(0);
    let r = f();
    let done = BUILDING.replace(before);
    SLOT.set(slot_before);
    (r, done.and_then(|b| b.tags).unwrap_or_default())
}

fn tag(p: Part) -> Part {
    p.tagged(SLOT.get())
}

pub fn ball(c: [f64; 3], r: f64, material: u8) -> Part {
    tag(animals::ball(c, r, material))
}

pub fn ellipsoid(c: [f64; 3], radii: [f64; 3], material: u8) -> Part {
    tag(animals::ellipsoid(c, radii, material))
}

pub fn limb(a: [f64; 3], b: [f64; 3], ra: f64, rb: f64, material: u8) -> Part {
    tag(animals::limb(a, b, ra, rb, material))
}

pub fn cuboid(lo: [f64; 3], hi: [f64; 3], material: u8) -> Part {
    tag(animals::cuboid(lo, hi, material))
}

/// animals::voxelize(), for a weapon being built: each frame's voxels tagged with their slots, and
/// changed as asked, see building()
pub fn voxelize(parts: &[Part], res: f64) -> (Model, Vector3<f64>) {
    let res = res * fine();
    if BUILDING.with_borrow(|b| b.is_none()) {
        return animals::voxelize(parts, res);
    }
    let (mut model, mut anchor, tags) = animals::voxelize_tagged(parts, res);
    // Taken out while it's changed, which may build another weapon to see how a part was
    let mut b = BUILDING.take().expect("a weapon's being built");
    let frame = b.frame;
    b.frame += 1;
    if let Some(change) = &mut b.change {
        change(frame, res, &mut model, &mut anchor, &tags);
    }
    if let Some(kept) = &mut b.tags {
        kept.push(tags);
    }
    BUILDING.set(Some(b));
    (model, anchor)
}

/// A weapon of `family` made of `parts` as the code builds it, unscaled, and each frame's tags
pub fn tagged_look(library: &Library, family: usize, parts: &[usize]) -> (WeaponLook, Vec<Vec<u8>>) {
    building(&library.families[family].slots, true, None, || crate::weapons::look_of(library, family, parts))
}

/// The voxels of `model` tagged `tag`, as a model of just them in the box round them, and where
/// that box starts in `model`: None if there are none
pub fn part_of(model: &Model, tags: &[u8], tag: u8) -> Option<(Model, [i32; 3])> {
    let [sx, sy, sz] = model.size;
    let (mut lo, mut hi) = ([u32::MAX; 3], [0u32; 3]);
    for z in 0..sz {
        for y in 0..sy {
            for x in 0..sx {
                let i = (x + (y + z * sy) * sx) as usize;
                if tags[i] == tag && model.voxels[i] != AIR {
                    for (a, c) in [x, y, z].into_iter().enumerate() {
                        lo[a] = lo[a].min(c);
                        hi[a] = hi[a].max(c + 1);
                    }
                }
            }
        }
    }
    if lo[0] == u32::MAX {
        return None;
    }
    let size = [0, 1, 2].map(|a| hi[a] - lo[a]);
    let mut out = assets::air(size);
    for z in 0..size[2] {
        for y in 0..size[1] {
            for x in 0..size[0] {
                let i = (x + lo[0] + (y + lo[1] + (z + lo[2]) * sy) * sx) as usize;
                if tags[i] == tag {
                    out.voxels[(x + (y + z * size[1]) * size[0]) as usize] = model.voxels[i];
                }
            }
        }
    }
    Some((out, lo.map(|v| v as i32)))
}

/// What part `c` is called in the files: its id, and for the few that share an id with one
/// listed before them, which of those it is
pub fn part_name(library: &Library, c: usize) -> String {
    let id = &library.components[c].id;
    let before = library.components[..c].iter().filter(|o| &o.id == id).count();
    if before == 0 { id.clone() } else { format!("{id}_{}", before + 1) }
}

/// What part `component` of `family` is called in the files in frame `frame` of the weapon, the
/// first frame's being the part's in every frame it's made the same in
pub fn part_file(family: &str, component: &str, frame: usize) -> String {
    if frame == 0 { format!("weapons/{family}/{component}") } else { format!("weapons/{family}/{component}_frame_{}", frame + 1) }
}

/// Where the weapon's own middle, its foot or pommel, is in the voxels of a frame `size` big with
/// its anchor at `anchor`, from the corner `lo`: the pivot of a part whose box starts at `lo`
pub fn part_pivot(size: [u32; 3], anchor: Vector3<f64>, res: f64, lo: [i32; 3]) -> [f64; 3] {
    let p = assets::pivot_of(size, Some(anchor), res);
    [p[0] - lo[0] as f64, p[1] - lo[1] as f64, p[2] - lo[2] as f64]
}

/// Where a weapon is held, its tip and the rest, as the code has them, for a part's .json
fn points(look: &WeaponLook) -> BTreeMap<String, [f64; 3]> {
    let v = |p: &Vector3<f64>| [p.x, p.y, p.z];
    let mut out = BTreeMap::new();
    out.insert("grip".into(), v(&look.grip));
    out.insert("tip".into(), v(&look.tip));
    for (k, n) in look.nocks.iter().enumerate() {
        out.insert(format!("nock_{}", k + 1), v(n));
    }
    if let Some(m) = &look.mover {
        out.insert("mover_pivot".into(), v(&m.pivot));
        out.insert("mover_handle".into(), v(&m.handle));
    }
    if let Some((p, _)) = &look.support {
        out.insert("support".into(), v(p));
    }
    if let Some(l) = &look.lash {
        out.insert("lash_root".into(), v(&l.root));
    }
    out
}

/// Every part of every family of weapons and tools, a job each
fn part_jobs(library: &Library) -> Vec<(usize, usize, usize)> {
    let mut jobs = Vec::new();
    for f in 0..library.families.len() {
        for (s, slot) in library.families[f].slots.iter().enumerate() {
            for c in library.fitting(f, slot) {
                jobs.push((f, s, c));
            }
        }
    }
    jobs
}

/// Runs `each` on every job, on every core at once, each thread with a library of its own
fn on_every_core<T: Send>(jobs: &[(usize, usize, usize)], each: impl Fn(&Library, (usize, usize, usize)) -> T + Sync) -> Vec<T> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
    // As fine on every core as here, see finer()
    let k = fine();
    let mut done: Vec<(usize, T)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    FINE.set(k);
                    let library = Library::load();
                    let mut out = Vec::new();
                    loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&job) = jobs.get(k) else { break };
                        out.push((k, each(&library, job)));
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("a weapon's part failed")).collect()
    });
    done.sort_by_key(|(k, _)| *k);
    done.into_iter().map(|(_, t)| t).collect()
}

/// Writes every part of every family of weapons and tools, see the module's comment: each built
/// on its family's reference weapon, and a part made differently in some frame again for that frame
pub fn export(dir: &Path, library: &Library, lines: &mut Vec<String>) -> Result<(), String> {
    for r in on_every_core(&part_jobs(library), |library, (f, s, c)| export_part(dir, library, f, s, c)) {
        lines.extend(r?);
    }
    Ok(())
}

/// A part of the weapon `parts` of family `f` builds, its frame `j`'s, and the file it goes in:
/// None where it's not in that frame, or is made the same there as in the first
fn part_frames(library: &Library, f: usize, s: usize, c: usize) -> (WeaponLook, Vec<Option<(Model, [i32; 3], String)>>) {
    let fam = &library.families[f];
    let mut parts = library.reference_parts(f);
    parts[s] = c;
    let (look, tags) = tagged_look(library, f, &parts);
    let mut first: Option<Model> = None;
    let mut out = Vec::new();
    for (j, (model, _)) in look.frames.iter().enumerate() {
        let part = part_of(model, &tags[j], s as u8 + 1);
        let same = part.as_ref().is_some_and(|(p, _)| first.as_ref().is_some_and(|m| m.size == p.size && m.voxels == p.voxels));
        if j == 0 {
            first = part.as_ref().map(|(p, _)| assets::copy(p));
        }
        out.push(part.filter(|_| !same).map(|(p, lo)| (p, lo, part_file(&fam.id, &part_name(library, c), j))));
    }
    (look, out)
}

pub fn export_part(dir: &Path, library: &Library, f: usize, s: usize, c: usize) -> Result<Vec<String>, String> {
    let fam = &library.families[f];
    let comp = &library.components[c];
    let (look, frames) = part_frames(library, f, s, c);
    // Exported finer, its voxels are that many more a meter, see finer()
    let res = look.res * fine();
    let mut out = Vec::new();
    for (j, frame) in frames.into_iter().enumerate() {
        let Some((part, lo, name)) = frame else { continue };
        let (model, anchor) = &look.frames[j];
        let pivot = part_pivot(model.size, *anchor, res, lo);
        let n = look.frames.len();
        let what = format!("{}, {}: {}{}", fam.display_name, fam.slots[s], comp.display_name, if j > 0 { format!(", frame {} of {n}", j + 1) } else { String::new() });
        let sidecar = Sidecar {
            what: what.clone(),
            voxels_per_meter: res,
            size: part.size,
            pivot,
            anchor: None,
            origin: assets::origin_of(pivot),
            points: points(&look),
            written: None,
            note: "One part of a weapon. The scene's middle is the weapon's foot or pommel, as the part sits on its family's \
                   reference weapon; on any weapon it's drawn as it is here from where the code puts the part. The points are \
                   the code's, and stay as they are whatever the part's shape."
                .into(),
        };
        assets::write_sidecar(dir, &name, &part, sidecar)?;
        out.push(format!("{name}.vox: {what}"));
    }
    Ok(out)
}

/// A part's file, as it changes the part in a frame of the weapon it was written from: the code's
/// voxels and the file's, each from the corner of the box round the code's
struct FrameEdit {
    code: Vec<([i32; 3], u8)>,
    edited: Vec<([i32; 3], u8)>,
}

pub struct PartEdit {
    frames: Vec<Option<FrameEdit>>,
}

/// The weapons' files there are, and what each part's files change, worked out once each
#[derive(Default)]
struct Known {
    files: Option<Arc<HashSet<String>>>,
    /// By family, part and how much finer, see fine()
    parts: HashMap<(usize, usize, u64), Option<Arc<PartEdit>>>,
}

fn known() -> std::sync::MutexGuard<'static, Known> {
    static KNOWN: OnceLock<Mutex<Known>> = OnceLock::new();
    KNOWN.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// The weapons' files, by name
fn files() -> Arc<HashSet<String>> {
    known()
        .files
        .get_or_insert_with(|| {
            let dir = assets::root().join("weapons");
            Arc::new(assets::scan(&dir).into_iter().map(|f| format!("weapons/{f}")).collect())
        })
        .clone()
}

/// Forgets what the files were, as when they've changed
pub fn forget() {
    *known() = Known::default();
}

/// Each solid voxel of `m`, sorted
fn solid(m: &Model) -> Vec<([i32; 3], u8)> {
    let mut out = Vec::new();
    for z in 0..m.size[2] {
        for y in 0..m.size[1] {
            for x in 0..m.size[0] {
                let v = m.voxels[(x + (y + z * m.size[1]) * m.size[0]) as usize];
                if v != AIR {
                    out.push(([x as i32, y as i32, z as i32], v));
                }
            }
        }
    }
    out.sort_unstable();
    out
}

/// What the files of part `c` in slot `s` of family `f` change, read from `dir`, if they're there
/// and change it from what the code made
fn read_part(dir: &Path, files: &HashSet<String>, library: &Library, f: usize, s: usize, c: usize) -> Option<PartEdit> {
    let (fam, comp) = (&library.families[f].id, &part_name(library, c));
    let base = part_file(fam, comp, 0);
    // Only files edited since they were exported, see assets::unchanged()
    let frame_files = format!("{base}_frame_");
    let ours: Vec<&String> = files.iter().filter(|n| **n == base || n.starts_with(&frame_files)).collect();
    if ours.iter().all(|n| assets::unchanged(&assets::file_of(dir, n))) {
        return None;
    }
    let mut parts = library.reference_parts(f);
    parts[s] = c;
    let (look, tags) = tagged_look(library, f, &parts);
    // Made finer, as the weapon in hand is, the code's part is too, see finer()
    let res = look.res * fine();
    let mut first: Option<Model> = None;
    let mut frames = Vec::new();
    for (j, (model, anchor)) in look.frames.iter().enumerate() {
        let Some((part, lo)) = part_of(model, &tags[j], s as u8 + 1) else {
            frames.push(None);
            continue;
        };
        // A frame's own file, or the first's where it's made the same as in the first
        let own = part_file(fam, comp, j);
        let same = j == 0 || first.as_ref().is_some_and(|m| m.size == part.size && m.voxels == part.voxels);
        if j == 0 {
            first = Some(assets::copy(&part));
        }
        let name = if files.contains(&own) { Some(own) } else if same && files.contains(&base) { Some(base.clone()) } else { None };
        let Some(name) = name else {
            frames.push(None);
            continue;
        };
        let code = solid(&part);
        match assets::read_edit(&assets::file_of(dir, &name), part_pivot(model.size, *anchor, res, lo), res) {
            Ok(e) => {
                let mut edited = e.solid();
                edited.sort_unstable();
                frames.push((edited != code).then_some(FrameEdit { code, edited }));
            }
            Err(e) => {
                assets::log(format!("{name}.vox: {e}"));
                frames.push(None);
            }
        }
    }
    frames.iter().any(Option::is_some).then_some(PartEdit { frames })
}

fn part_edit(library: &Library, f: usize, s: usize, c: usize) -> Option<Arc<PartEdit>> {
    let key = (f, c, fine().to_bits());
    if let Some(e) = known().parts.get(&key) {
        return e.clone();
    }
    let edit = read_part(&assets::root(), &files(), library, f, s, c).map(Arc::new);
    known().parts.insert(key, edit.clone());
    edit
}

/// The look of a weapon of `family` made of `parts`, as `build` makes it, with each of its parts
/// whose file's been changed drawn from it, see the module's comment
pub fn build(library: &Library, family: usize, parts: &[usize], build: impl FnOnce() -> WeaponLook) -> WeaponLook {
    if !assets::enabled() {
        return build();
    }
    let prefix = format!("weapons/{}/", library.families[family].id);
    if !files().iter().any(|n| n.starts_with(&prefix)) {
        return build();
    }
    let edits: Vec<Option<Arc<PartEdit>>> = parts.iter().enumerate().map(|(s, &c)| part_edit(library, family, s, c)).collect();
    if edits.iter().all(Option::is_none) {
        return build();
    }
    with_edits(&library.families[family].slots, edits, build)
}

fn with_edits(slots: &[String], edits: Vec<Option<Arc<PartEdit>>>, build: impl FnOnce() -> WeaponLook) -> WeaponLook {
    let change: Change = Box::new(move |frame, res, model, anchor, tags| apply(&edits, frame, res, model, anchor, tags));
    building(slots, false, Some(change), build).0
}

/// Frame `frame` of a weapon, `model` at `res` voxels a meter with its anchor at `anchor` and its
/// voxels tagged `tags`, with each part whose file's been changed drawn as the file has it: from
/// where the code put the part on this weapon, its colours swapped for this weapon's where the
/// code drew them otherwise on the file's weapon, and never over the other parts
fn apply(edits: &[Option<Arc<PartEdit>>], frame: usize, res: f64, model: &mut Model, anchor: &mut Vector3<f64>, tags: &[u8]) {
    let replaced: Vec<u8> = (0..edits.len()).filter(|&s| edits[s].as_ref().is_some_and(|e| e.frames.get(frame).is_some_and(Option::is_some))).map(|s| s as u8 + 1).collect();
    if replaced.is_empty() {
        return;
    }
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for &tag in &replaced {
        let fe = edits[tag as usize - 1].as_ref().unwrap().frames[frame].as_ref().unwrap();
        let Some((here, lo)) = part_of(model, tags, tag) else { continue };
        let mut pairs: HashMap<(u8, u8), usize> = HashMap::new();
        for (p, m) in &fe.code {
            let h = assets::at(&here, *p);
            if h != AIR {
                *pairs.entry((*m, h)).or_default() += 1;
            }
        }
        let mut map: [u8; 256] = std::array::from_fn(|i| i as u8);
        let mut best = [0usize; 256];
        for ((from, to), n) in pairs {
            if n > best[from as usize] || (n == best[from as usize] && to == from) {
                best[from as usize] = n;
                map[from as usize] = to;
            }
        }
        for z in 0..model.size[2] as i32 {
            for y in 0..model.size[1] as i32 {
                for x in 0..model.size[0] as i32 {
                    if tags[(x as u32 + (y as u32 + z as u32 * model.size[1]) * model.size[0]) as usize] == tag {
                        removed.push(([x, y, z], AIR));
                    }
                }
            }
        }
        added.extend(fe.edited.iter().map(|(p, m)| ([lo[0] + p[0], lo[1] + p[1], lo[2] + p[2]], map[*m as usize])));
    }
    let free = |p: [i32; 3]| {
        if (0..3).any(|a| p[a] < 0 || p[a] >= model.size[a] as i32) {
            return true;
        }
        let i = (p[0] as u32 + (p[1] as u32 + p[2] as u32 * model.size[1]) * model.size[0]) as usize;
        model.voxels[i] == AIR || replaced.contains(&tags[i])
    };
    added.retain(|(p, _)| free(*p));
    removed.extend(added);
    let (m, lo) = assets::place(model, &removed, Grow::Free);
    *anchor = assets::moved_anchor(*anchor, model.size, lo, m.size, res);
    *model = m;
}

/// The parts of every weapon there is, for the editor: each family's slots with what fits each
pub fn slots_of(library: &Library, family: usize) -> Vec<(String, Vec<usize>)> {
    library.families[family].slots.iter().map(|s| (s.clone(), library.fitting(family, s))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every voxel of a weapon belongs to one of its parts, nearly: the builders tag what they make
    #[test]
    fn weapons_are_made_of_their_parts() {
        let library = Library::load();
        for f in 0..library.families.len() {
            let (look, tags) = tagged_look(&library, f, &library.reference_parts(f));
            let (mut solid, mut untagged) = (0, 0);
            for (j, (m, _)) in look.frames.iter().enumerate() {
                for (i, &v) in m.voxels.iter().enumerate() {
                    if v != AIR {
                        solid += 1;
                        untagged += (tags[j][i] == 0) as usize;
                    }
                }
            }
            assert!(untagged * 50 <= solid, "{}: {untagged} of {solid} voxels belong to no part", library.families[f].id);
        }
    }

    /// Every weapon part exported and read back as the game reads it is the same as the code made
    /// it, so every weapon is built voxel for voxel as before; and a part changed in its file is
    /// drawn changed, joined to the rest where it was
    #[test]
    fn weapon_parts_round_trip() {
        let dir = std::env::temp_dir().join(format!("innert-parts-test-{}", std::process::id()));
        let library = Library::load();
        let mut lines = Vec::new();
        export(&dir, &library, &mut lines).unwrap();
        assert!(lines.len() >= part_jobs(&library).len(), "a file for every part");
        let files: HashSet<String> = assets::scan(&dir.join("weapons")).into_iter().map(|f| format!("weapons/{f}")).collect();
        // Read back, none of them changes anything
        let changed = on_every_core(&part_jobs(&library), |library, (f, s, c)| read_part(&dir, &files, library, f, s, c).is_some().then(|| format!("{f} {s} {c}")));
        let changed: Vec<String> = changed.into_iter().flatten().collect();
        assert!(changed.is_empty(), "read back changed {changed:?}");

        // A sword's blade with a voxel taken off its tip and one added beside it, then put on a
        // weapon of the same family with another hilt: the hilt's as the code made it, the blade's
        // changed
        let f = library.family("sword").unwrap_or(0);
        let s = library.families[f].slots.iter().position(|x| x == "blade").unwrap_or(0);
        let c = library.reference_parts(f)[s];
        let name = part_file(&library.families[f].id, &part_name(&library, c), 0);
        let path = assets::file_of(&dir, &name);
        let mut file = crate::vox::read(&std::fs::read(&path).unwrap()).unwrap();
        let g = &mut file.grid;
        let top = (0..g.voxels.len()).rev().find(|&i| g.voxels[i] != 0).unwrap();
        g.voxels[top] = 0;
        std::fs::write(&path, crate::vox::write(g, &assets::palette())).unwrap();
        let edit = read_part(&dir, &files, &library, f, s, c).expect("the blade's changed");
        let edit = Arc::new(edit);
        let slots = library.families[f].slots.clone();
        let solid = |m: &Model| m.voxels.iter().filter(|v| **v != AIR).count();
        // On the weapon it was written from, exactly the voxel fewer
        let reference = library.reference_parts(f);
        let mut edits: Vec<Option<Arc<PartEdit>>> = reference.iter().map(|_| None).collect();
        edits[s] = Some(edit.clone());
        let code = crate::weapons::look_of(&library, f, &reference);
        let changed = with_edits(&slots, edits.clone(), || crate::weapons::look_of(&library, f, &reference));
        assert_eq!(solid(&changed.frames[0].0) + 1, solid(&code.frames[0].0), "one voxel fewer");
        assert_eq!(changed.frames[0].1, code.frames[0].1, "anchored where it was");
        // On one with another of the next slot's parts, that part's as the code made it
        let mut parts = reference.clone();
        let next = (s + 1) % parts.len();
        if let Some(&h) = library.fitting(f, &slots[next]).last() {
            parts[next] = h;
        }
        let (code, tags) = tagged_look(&library, f, &parts);
        let changed = with_edits(&slots, edits, || crate::weapons::look_of(&library, f, &parts));
        let (a, b) = (&code.frames[0], &changed.frames[0]);
        let mine = |m: &Model, anchor: Vector3<f64>, tag: u8| -> Vec<[i64; 3]> {
            // Where each voxel of part `tag` is in the weapon's own voxels, wherever its box is
            let corner = anchor * code.res - Vector3::new(m.size[0] as f64 * 0.5, m.size[1] as f64 * 0.5, 0.0);
            let mut out = Vec::new();
            for z in 0..m.size[2] {
                for y in 0..m.size[1] {
                    for x in 0..m.size[0] {
                        let i = (x + (y + z * m.size[1]) * m.size[0]) as usize;
                        if tags[0].get(i) == Some(&tag) && m.voxels[i] != AIR {
                            out.push([(corner.x + x as f64).round() as i64, (corner.y + y as f64).round() as i64, (corner.z + z as f64).round() as i64]);
                        }
                    }
                }
            }
            out
        };
        let hilt = mine(&a.0, a.1, next as u8 + 1);
        assert!(!hilt.is_empty());
        let blade_before = mine(&a.0, a.1, s as u8 + 1).len();
        assert!((solid(&b.0) as i64 - solid(&a.0) as i64).unsigned_abs() as usize <= blade_before / 20 + 2, "the blade about as big");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
