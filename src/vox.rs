//! MagicaVoxel's .vox files, read and written, for editing the game's models in MagicaVoxel, see
//! src\assets.rs. A file holds one or more models of up to 256 voxels a side, each a list of
//! voxels with a colour index from 1 to 255 into its palette, placed in a scene by translations.
//! Here a file is read into one grid of voxels, its models put where the scene has them, and a grid
//! too big for one model is written as several side by side, which MagicaVoxel shows as one.
//!
//! The format: https://github.com/ephtracy/voxel-model/blob/master/MagicaVoxel-file-format-vox.txt
//! and its scene graph, https://github.com/ephtracy/voxel-model/blob/master/MagicaVoxel-file-format-vox-extension.txt

use std::collections::HashMap;

/// The most voxels a side one of MagicaVoxel's models can have
pub const MAX_SIDE: u32 = 256;

/// Voxels in a box, x + (y + z * size.y) * size.x, each a colour index, 0 for empty, with where the
/// box's first voxel is in the scene
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub size: [u32; 3],
    pub voxels: Vec<u8>,
    /// Where voxel (0, 0, 0) is in the scene's voxels
    pub origin: [i32; 3],
}

impl Grid {
    pub fn get(&self, c: [u32; 3]) -> u8 {
        self.voxels[(c[0] + (c[1] + c[2] * self.size[1]) * self.size[0]) as usize]
    }
}

/// A file as read: its voxels and its palette, colour index i's at palette[i], 0 unused
pub struct VoxFile {
    pub grid: Grid,
    pub palette: [[u8; 4]; 256],
    /// Whether a scene placed its models, so the grid's origin is where they are in it
    pub placed: bool,
}

fn chunk(id: &[u8; 4], content: &[u8], children: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + content.len() + children.len());
    out.extend_from_slice(id);
    out.extend_from_slice(&(content.len() as i32).to_le_bytes());
    out.extend_from_slice(&(children.len() as i32).to_le_bytes());
    out.extend_from_slice(content);
    out.extend_from_slice(children);
    out
}

fn put_i32(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_dict(out: &mut Vec<u8>, pairs: &[(&str, String)]) {
    put_i32(out, pairs.len() as i32);
    for (k, v) in pairs {
        put_i32(out, k.len() as i32);
        out.extend_from_slice(k.as_bytes());
        put_i32(out, v.len() as i32);
        out.extend_from_slice(v.as_bytes());
    }
}

/// `grid` as a .vox file, with `palette` (colour index i's colour at i), cut into models of up to
/// MAX_SIDE a side where it's bigger. Each model's put in the scene where its voxels are in
/// `grid.origin`'s voxels.
pub fn write(grid: &Grid, palette: &[[u8; 4]; 256]) -> Vec<u8> {
    let pieces: Vec<[u32; 3]> = {
        let counts = grid.size.map(|s| s.div_ceil(MAX_SIDE).max(1));
        let mut v = Vec::new();
        for z in 0..counts[2] {
            for y in 0..counts[1] {
                for x in 0..counts[0] {
                    v.push([x * MAX_SIDE, y * MAX_SIDE, z * MAX_SIDE]);
                }
            }
        }
        v
    };
    let mut children = Vec::new();
    // A model for each piece, however empty, so a file always has at least one
    let mut placed = Vec::new();
    for from in &pieces {
        let size = [0, 1, 2].map(|a| (grid.size[a] - from[a]).min(MAX_SIDE).max(1));
        let mut xyzi = Vec::new();
        let mut n = 0i32;
        put_i32(&mut xyzi, 0);
        for z in 0..size[2] {
            for y in 0..size[1] {
                for x in 0..size[0] {
                    let c = [from[0] + x, from[1] + y, from[2] + z];
                    if c[0] >= grid.size[0] || c[1] >= grid.size[1] || c[2] >= grid.size[2] {
                        continue;
                    }
                    let v = grid.get(c);
                    if v != 0 {
                        xyzi.extend_from_slice(&[x as u8, y as u8, z as u8, v]);
                        n += 1;
                    }
                }
            }
        }
        xyzi[..4].copy_from_slice(&n.to_le_bytes());
        let mut size_content = Vec::new();
        for s in size {
            put_i32(&mut size_content, s as i32);
        }
        children.extend(chunk(b"SIZE", &size_content, &[]));
        children.extend(chunk(b"XYZI", &xyzi, &[]));
        // MagicaVoxel puts a model's middle at its translation: voxel v at t + v - size / 2
        let t = [0, 1, 2].map(|a| grid.origin[a] + from[a] as i32 + (size[a] / 2) as i32);
        placed.push(t);
    }
    // The scene: a root transform over a group of a transform and shape for each model
    let mut node = |id: &[u8; 4], content: Vec<u8>| children.extend(chunk(id, &content, &[]));
    let mut root = Vec::new();
    put_i32(&mut root, 0);
    put_dict(&mut root, &[]);
    put_i32(&mut root, 1);
    put_i32(&mut root, -1);
    put_i32(&mut root, -1);
    put_i32(&mut root, 1);
    put_dict(&mut root, &[]);
    node(b"nTRN", root);
    let mut group = Vec::new();
    put_i32(&mut group, 1);
    put_dict(&mut group, &[]);
    put_i32(&mut group, placed.len() as i32);
    for k in 0..placed.len() {
        put_i32(&mut group, 2 + 2 * k as i32);
    }
    node(b"nGRP", group);
    for (k, t) in placed.iter().enumerate() {
        let mut trn = Vec::new();
        put_i32(&mut trn, 2 + 2 * k as i32);
        put_dict(&mut trn, &[]);
        put_i32(&mut trn, 3 + 2 * k as i32);
        put_i32(&mut trn, -1);
        put_i32(&mut trn, 0);
        put_i32(&mut trn, 1);
        put_dict(&mut trn, &[("_t", format!("{} {} {}", t[0], t[1], t[2]))]);
        node(b"nTRN", trn);
        let mut shp = Vec::new();
        put_i32(&mut shp, 3 + 2 * k as i32);
        put_dict(&mut shp, &[]);
        put_i32(&mut shp, 1);
        put_i32(&mut shp, k as i32);
        put_dict(&mut shp, &[]);
        node(b"nSHP", shp);
    }
    // Colour index i is the palette's entry i - 1
    let mut rgba = Vec::with_capacity(1024);
    for i in 0..256 {
        rgba.extend_from_slice(&palette[(i + 1) % 256]);
    }
    children.extend(chunk(b"RGBA", &rgba, &[]));
    let mut out = b"VOX ".to_vec();
    put_i32(&mut out, 150);
    out.extend(chunk(b"MAIN", &[], &children));
    out
}

/// Reads what's next in a chunk's content
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn i32(&mut self) -> Result<i32, String> {
        let b = self.bytes.get(self.at..self.at + 4).ok_or("the file ends too soon")?;
        self.at += 4;
        Ok(i32::from_le_bytes(b.try_into().unwrap()))
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let b = self.bytes.get(self.at..self.at + n).ok_or("the file ends too soon")?;
        self.at += n;
        Ok(b)
    }

    fn string(&mut self) -> Result<String, String> {
        let n = self.i32()?.max(0) as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn dict(&mut self) -> Result<HashMap<String, String>, String> {
        let n = self.i32()?.max(0);
        let mut d = HashMap::new();
        for _ in 0..n {
            let k = self.string()?;
            let v = self.string()?;
            d.insert(k, v);
        }
        Ok(d)
    }
}

enum Node {
    Transform { child: i32, t: [i32; 3] },
    Group { children: Vec<i32> },
    Shape { models: Vec<i32> },
}

/// MagicaVoxel's palette when a file has none of its own
fn default_palette() -> [[u8; 4]; 256] {
    // Its default, as its documentation gives it: a 6x6x6 cube of colours, then ramps of red,
    // green, blue and grey. Only used for files that carry no palette, which MagicaVoxel never
    // writes.
    let mut p = [[0u8; 4]; 256];
    let steps = [0xFFu8, 0xCC, 0x99, 0x66, 0x33, 0x00];
    let mut i = 1;
    for r in steps {
        for g in steps {
            for b in steps {
                if i < 256 {
                    p[i] = [r, g, b, 255];
                    i += 1;
                }
            }
        }
    }
    let ramp = [0xEEu8, 0xDD, 0xBB, 0xAA, 0x88, 0x77, 0x55, 0x44, 0x22, 0x11];
    for k in 0..4 {
        for v in ramp {
            if i < 256 {
                p[i] = match k {
                    0 => [v, 0, 0, 255],
                    1 => [0, v, 0, 255],
                    2 => [0, 0, v, 255],
                    _ => [v, v, v, 255],
                };
                i += 1;
            }
        }
    }
    p
}

/// A .vox file read into one grid: all its models put together where its scene places them, the
/// box round them all, and its palette
pub fn read(bytes: &[u8]) -> Result<VoxFile, String> {
    if bytes.len() < 8 || &bytes[..4] != b"VOX " {
        return Err("not a MagicaVoxel .vox file".into());
    }
    // Every chunk, read flat: MAIN's children follow its header
    let mut sizes: Vec<[u32; 3]> = Vec::new();
    let mut models: Vec<Vec<[u8; 4]>> = Vec::new();
    let mut palette = None;
    let mut nodes: HashMap<i32, Node> = HashMap::new();
    let mut at = 8;
    while at + 12 <= bytes.len() {
        let id: [u8; 4] = bytes[at..at + 4].try_into().unwrap();
        let content = i32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()).max(0) as usize;
        let body = bytes.get(at + 12..at + 12 + content).ok_or("a chunk runs past the end of the file")?;
        let mut r = Reader { bytes: body, at: 0 };
        match &id {
            b"SIZE" => {
                let s = [r.i32()?, r.i32()?, r.i32()?];
                sizes.push(s.map(|v| v.clamp(1, MAX_SIDE as i32) as u32));
            }
            b"XYZI" => {
                let n = r.i32()?.max(0) as usize;
                let raw = r.take(n * 4)?;
                models.push(raw.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect());
            }
            b"RGBA" => {
                let raw = r.take(1024)?;
                let mut p = [[0u8; 4]; 256];
                for i in 0..255 {
                    p[i + 1] = raw[i * 4..i * 4 + 4].try_into().unwrap();
                }
                palette = Some(p);
            }
            b"nTRN" => {
                let id = r.i32()?;
                let _attributes = r.dict()?;
                let child = r.i32()?;
                let _reserved = r.i32()?;
                let _layer = r.i32()?;
                let frames = r.i32()?.max(0);
                let mut t = [0; 3];
                for f in 0..frames {
                    let d = r.dict()?;
                    if f == 0
                        && let Some(s) = d.get("_t")
                    {
                        let v: Vec<i32> = s.split_whitespace().filter_map(|n| n.parse().ok()).collect();
                        if v.len() == 3 {
                            t = [v[0], v[1], v[2]];
                        }
                    }
                }
                nodes.insert(id, Node::Transform { child, t });
            }
            b"nGRP" => {
                let id = r.i32()?;
                let _attributes = r.dict()?;
                let n = r.i32()?.max(0);
                let children = (0..n).map(|_| r.i32()).collect::<Result<_, _>>()?;
                nodes.insert(id, Node::Group { children });
            }
            b"nSHP" => {
                let id = r.i32()?;
                let _attributes = r.dict()?;
                let n = r.i32()?.max(0);
                let mut list = Vec::new();
                for _ in 0..n {
                    list.push(r.i32()?);
                    let _model_attributes = r.dict()?;
                }
                nodes.insert(id, Node::Shape { models: list });
            }
            _ => {}
        }
        // MAIN's children come straight after it; every other chunk is skipped whole
        at += 12 + content + if &id == b"MAIN" { 0 } else { i32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap()).max(0) as usize };
    }
    if sizes.is_empty() || sizes.len() != models.len() {
        return Err("the file has no models in it".into());
    }
    // Where each model's first voxel is in the scene, by its translations: without a scene, the
    // first model alone at the origin
    let mut corners: Vec<Option<[i32; 3]>> = vec![None; sizes.len()];
    fn walk(nodes: &HashMap<i32, Node>, id: i32, t: [i32; 3], sizes: &[[u32; 3]], corners: &mut [Option<[i32; 3]>], depth: usize) {
        if depth > 64 {
            return;
        }
        match nodes.get(&id) {
            Some(Node::Transform { child, t: own }) => walk(nodes, *child, [t[0] + own[0], t[1] + own[1], t[2] + own[2]], sizes, corners, depth + 1),
            Some(Node::Group { children }) => {
                for &c in children {
                    walk(nodes, c, t, sizes, corners, depth + 1);
                }
            }
            Some(Node::Shape { models }) => {
                for &m in models {
                    if let Some(size) = sizes.get(m as usize) {
                        corners[m as usize] = Some([0, 1, 2].map(|a| t[a] - (size[a] / 2) as i32));
                    }
                }
            }
            None => {}
        }
    }
    if nodes.contains_key(&0) {
        walk(&nodes, 0, [0; 3], &sizes, &mut corners, 0);
    }
    let scene = corners.iter().any(Option::is_some);
    let placed: Vec<(usize, [i32; 3])> = if scene {
        corners.iter().enumerate().filter_map(|(m, c)| c.map(|c| (m, c))).collect()
    } else {
        vec![(0, [0; 3])]
    };
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for &(m, c) in &placed {
        for a in 0..3 {
            lo[a] = lo[a].min(c[a]);
            hi[a] = hi[a].max(c[a] + sizes[m][a] as i32);
        }
    }
    let size = [0, 1, 2].map(|a| (hi[a] - lo[a]).max(1) as u32);
    if size.iter().any(|&s| s > 4096) {
        return Err(format!("the models in it reach {}x{}x{} voxels, too far apart", size[0], size[1], size[2]));
    }
    let mut voxels = vec![0u8; (size[0] * size[1] * size[2]) as usize];
    for &(m, c) in &placed {
        for &[x, y, z, i] in &models[m] {
            let s = sizes[m];
            if (x as u32) >= s[0] || (y as u32) >= s[1] || (z as u32) >= s[2] || i == 0 {
                continue;
            }
            let p = [c[0] + x as i32 - lo[0], c[1] + y as i32 - lo[1], c[2] + z as i32 - lo[2]].map(|v| v as u32);
            voxels[(p[0] + (p[1] + p[2] * size[1]) * size[0]) as usize] = i;
        }
    }
    Ok(VoxFile { grid: Grid { size, voxels, origin: lo }, palette: palette.unwrap_or_else(default_palette), placed: scene })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> [[u8; 4]; 256] {
        std::array::from_fn(|i| [i as u8, 255 - i as u8, (i * 7) as u8, 255])
    }

    /// Written and read back, a grid comes out exactly as it went in, its place in the scene too,
    /// one too big for a single model as well
    #[test]
    fn grids_come_back_as_they_went() {
        for size in [[3u32, 5, 7], [300, 20, 9], [10, 260, 513]] {
            let mut voxels = vec![0u8; (size[0] * size[1] * size[2]) as usize];
            for (i, v) in voxels.iter_mut().enumerate() {
                if i % 7 == 0 || i % 11 == 3 {
                    *v = (i % 255 + 1) as u8;
                }
            }
            // Something in both far corners, so the box read back is the whole of it
            *voxels.first_mut().unwrap() = 9;
            *voxels.last_mut().unwrap() = 200;
            let grid = Grid { size, voxels, origin: [-17, 4, -3] };
            let back = read(&write(&grid, &palette())).unwrap();
            assert_eq!(back.grid, grid, "{size:?}");
            assert_eq!(back.palette[1..], palette()[1..]);
        }
    }
}
