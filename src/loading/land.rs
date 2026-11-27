//! The land being built, as the loading screen shows it: the real lie of the land round where the
//! player's going to stand, a few hundred columns of it, rising out of a dark plate as the world's
//! built, the woods springing up on it as the grass grows and the seas filling in last, turning
//! slowly as if walked round. In greys, as a print cut in wood, see theme.rs.
//!
//! Its heights are the landscape's own, read off worldgen::seen_from_above() on another thread in a
//! few milliseconds, see around(), so it's the same hills, valleys and shores the player then
//! stands among, only without the rivers cut into it. Drawn with the GUI as a few thousand flat
//! shapes, and only while loading.

use std::f32::consts::{FRAC_1_SQRT_2, SQRT_2, TAU};

use egui_winit_vulkano::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

use crate::worldgen::{self, TerrainSettings};

/// Columns across each way
const ACROSS: usize = 34;
/// How many meters of the landscape each column is across
const CELL_M: f32 = 3.0;
/// How far down under the lowest column the plate the land rises out of is, in meters
const PLATE_M: f32 = 3.0;
/// How far under its top a sky isle goes on down, in meters
const ISLE_DEPTH_M: f32 = 9.0;
/// How far the view looks down on it from, in radians over the level
const ELEVATION: f32 = 0.56;
/// How fast it turns, in radians a second: once round in a little under a minute
const TURN_SPEED: f32 = 0.11;
/// How many beasts wander about on it once it's built
const BEASTS: usize = 9;

/// The heights of the land round a spot, see around()
pub struct Land {
    /// Each column's ground over the sea, in meters, row after row from the south-west corner, or
    /// NaN in the open air between the sky isles
    heights: Vec<f32>,
    /// How thick with trees each is, 0 to 1, see worldgen::woods_at()
    woods: Vec<f32>,
    /// How low the plate is, and how high the highest column, in meters over the sea
    floor: f32,
    top: f32,
    /// Whether it's sky isles, which hang in the air rather than standing on the plate
    floating: bool,
    /// How low each column goes, in meters over the sea: down to the plate, or a sky isle's
    /// underside, deeper the further in from its edge
    undersides: Vec<f32>,
    /// The columns a few beasts wander about on, on dry land
    beasts: Vec<(i32, i32)>,
}

/// How far each part of building the land's got, 0 to 1: the land rising, the trees growing and
/// the seas filling
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Phase {
    pub rise: f32,
    pub grow: f32,
    pub fill: f32,
}

impl Phase {
    #[cfg(test)]
    pub const DONE: Phase = Phase { rise: 1.0, grow: 1.0, fill: 1.0 };
}

impl Land {
    /// The land round `middle`, in meters from the corner of the whole landscape, as `settings`
    /// build it
    pub fn around(settings: &TerrainSettings, middle: [f32; 2]) -> Land {
        let n = ACROSS;
        let spot = |i: usize, j: usize| [middle[0] + (i as f32 + 0.5 - n as f32 * 0.5) * CELL_M, middle[1] + (j as f32 + 0.5 - n as f32 * 0.5) * CELL_M];
        let heights: Vec<f32> = (0..n * n)
            .map(|k| {
                let [x, y] = spot(k % n, k / n);
                worldgen::seen_from_above(settings, x, y).unwrap_or(f32::NAN)
            })
            .collect();
        let woods = (0..n * n)
            .map(|k| {
                let [x, y] = spot(k % n, k / n);
                if heights[k] > 0.8 { worldgen::woods_at(settings, x, y) } else { 0.0 }
            })
            .collect();
        Land::from_heights(heights, woods)
    }

    /// The land with these heights and woods, ACROSS columns each way
    fn from_heights(heights: Vec<f32>, woods: Vec<f32>) -> Land {
        let solid = || heights.iter().copied().filter(|h| h.is_finite());
        let floating = heights.iter().any(|h| h.is_nan());
        let low = solid().fold(f32::INFINITY, f32::min);
        let high = solid().fold(f32::NEG_INFINITY, f32::max);
        let (low, high) = if low.is_finite() { (low, high) } else { (0.0, 0.0) };
        let n = ACROSS as i32;
        // A sky isle comes to a point underneath, as far down as its middle's in from its edge
        let mut inland: Vec<f32> = heights.iter().map(|h| if h.is_finite() { 6.0 } else { 0.0 }).collect();
        for _ in 0..6 {
            for k in 0..(n * n) as usize {
                let (i, j) = (k as i32 % n, k as i32 / n);
                let near = [(1, 0), (-1, 0), (0, 1), (0, -1)].map(|(di, dj)| {
                    let (a, b) = (i + di, j + dj);
                    if a < 0 || b < 0 || a >= n || b >= n { 6.0 } else { inland[(a + b * n) as usize] }
                });
                inland[k] = inland[k].min(near.iter().fold(f32::INFINITY, |m, &v| m.min(v)) + 1.0);
            }
        }
        let undersides: Vec<f32> = heights.iter().zip(&inland).map(|(h, d)| h - ISLE_DEPTH_M * (0.3 + 0.7 * (d / 4.0).min(1.0))).collect();
        // A little under the lowest ground, so where that's under the sea there's sea to fill, and
        // land high over it isn't stood on a tall block. Under sky isles, their undersides.
        let floor = if floating { undersides.iter().copied().filter(|v| v.is_finite()).fold(low, f32::min) } else { low - PLATE_M };
        let undersides = if floating { undersides } else { vec![floor; heights.len()] };
        // A few dry columns, scattered, for the beasts
        let mut beasts: Vec<(i32, i32)> = (0..n * n).map(|k| (k % n, k / n)).filter(|&(i, j)| heights[(i + j * n) as usize] > 1.0 && (i - n / 2).abs().max((j - n / 2).abs()) > 2).collect();
        beasts.sort_by(|a, b| hash(a.0 as u32, a.1 as u32, 9).total_cmp(&hash(b.0 as u32, b.1 as u32, 9)));
        beasts.truncate(BEASTS);
        Land { heights, woods, floor, top: high.max(floor + 4.0), floating, undersides, beasts }
    }

    fn height(&self, i: i32, j: i32) -> Option<f32> {
        let n = ACROSS as i32;
        if i < 0 || j < 0 || i >= n || j >= n {
            return None;
        }
        Some(self.heights[(i + j * n) as usize]).filter(|h| h.is_finite())
    }

    /// Draws it into `area`, `t` seconds after it was first shown, `phase` of the way built,
    /// `alpha` of the way opaque
    pub fn paint(&self, painter: &Painter, area: Rect, t: f32, phase: Phase, alpha: f32) {
        let n = ACROSS as i32;
        let half = n as f32 * 0.5;
        let turn = 0.65 + t * TURN_SPEED;
        let (sin_t, cos_t) = turn.sin_cos();
        let (sin_e, cos_e) = ELEVATION.sin_cos();
        // Heights stretched a little, so gentle hills still read, unless that'd stand them taller
        // than the land's wide
        let span = (self.top - self.floor).max(1.0) / CELL_M;
        let stretch = (0.5 * n as f32 / span).min(1.7);
        let tall = span * stretch;
        // As big as fits whichever way it's turned
        let reach = n as f32 * SQRT_2;
        let scale = (area.width() / reach).min(area.height() / (reach * sin_e + tall * cos_e));
        let middle = pos2(area.center().x, area.bottom() - (reach * 0.5 * sin_e) * scale);
        let project = |x: f32, y: f32, z: f32| -> Pos2 {
            let (u, v) = (x * cos_t - y * sin_t, x * sin_t + y * cos_t);
            pos2(middle.x + u * scale, middle.y + (v * sin_e - z * cos_e) * scale)
        };
        // Meters to how high it's drawn, in columns over the floor
        let lift = |h: f32| (h - self.floor) / CELL_M * stretch;
        let fade = |c: Color32| c.gamma_multiply(alpha);
        let grey = |v: f32, a: f32| fade(Color32::from_rgba_unmultiplied(v.round().clamp(0.0, 255.0) as u8, v.round().clamp(0.0, 255.0) as u8, v.round().clamp(0.0, 255.0) as u8, (a * 255.0) as u8));
        let water_level = lift(0.0) * phase.fill;

        // The plate the land rises out of, unless it's sky isles hanging over nothing
        let mut mesh = Mesh::default();
        if !self.floating {
            let corners = [(-half, -half), (half, -half), (half, half), (-half, half)].map(|(x, y)| project(x, y, 0.0));
            quad(&mut mesh, corners, grey(24.0, 1.0));
            let below = corners.map(|p| p + vec2(0.0, 0.6 * scale));
            // Its front edges, the two facing the viewer
            for k in 0..4 {
                let (a, b) = (k, (k + 1) % 4);
                if corners[a].y + corners[b].y > 2.0 * middle.y {
                    quad(&mut mesh, [corners[a], corners[b], below[b], below[a]], grey(14.0, 1.0));
                }
            }
        }

        // When each column comes up: from the middle out, a little ragged
        let rises = |i: i32, j: i32| {
            let d = ((i as f32 + 0.5 - half).hypot(j as f32 + 0.5 - half) / (half * SQRT_2)).min(1.0);
            let start = d * 0.78 + hash(i as u32, j as u32, 1) * 0.2;
            ease_out_back(((phase.rise * 1.25 - start) / 0.25).clamp(0.0, 1.0))
        };
        // Back to front, as the view's turned
        let mut order: Vec<(f32, i32, i32)> = (0..n * n)
            .map(|k| {
                let (i, j) = (k % n, k / n);
                let (x, y) = (i as f32 + 0.5 - half, j as f32 + 0.5 - half);
                (x * sin_t + y * cos_t, i, j)
            })
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Which of a column's four sides face the viewer: those along x, and along y
        let faces = [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)];
        // The light from the north-west, its sides lit by how much they face it
        let light = [-FRAC_1_SQRT_2, FRAC_1_SQRT_2];
        let outline = scale > 7.0;
        let turned = (sin_t, cos_t);
        // How high each column's drawn now, to know how much of its neighbours' sides show
        let drawn = |i: i32, j: i32| -> Option<f32> {
            let h = self.height(i, j)?;
            let r = rises(i, j);
            let bottom = lift(self.undersides[(i + j * n) as usize]).max(0.0);
            Some(bottom + (lift(h) - bottom) * r)
        };
        let surface = |i: i32, j: i32| drawn(i, j).map(|z| z.max(if self.floating { 0.0 } else { water_level }));
        for &(_, i, j) in &order {
            let Some(h) = self.height(i, j) else { continue };
            let r = rises(i, j);
            if r <= 0.0 {
                continue;
            }
            let (x, y) = (i as f32 + 0.5 - half, j as f32 + 0.5 - half);
            let ground = drawn(i, j).unwrap();
            let bottom = lift(self.undersides[(i + j * n) as usize]).max(0.0);
            let wet = !self.floating && water_level > ground + 1e-3;
            let z = if wet { water_level } else { ground };
            // Higher is whiter, and what's only just come up whiter still, fresh from the plate
            let height = ((h - self.floor) / (self.top - self.floor).max(1.0)).clamp(0.0, 1.0);
            let speck = (hash(i as u32, j as u32, 2) - 0.5) * 14.0;
            let fresh = (1.0 - r).max(0.0) * 60.0;
            let under_sea = if h < 0.0 { 0.72 } else { 1.0 };
            let land_top = ((132.0 + 104.0 * height + speck) * under_sea + fresh).min(250.0);
            let shimmer = 7.0 * (t * 1.6 + i as f32 * 0.7 + j as f32 * 0.45).sin();
            let top = if wet { 58.0 + shimmer } else { land_top };

            // The sides that face the viewer, down to where the next column's top hides them
            for (k, &(nx, ny)) in faces.iter().enumerate() {
                if nx * sin_t + ny * cos_t <= 0.0 {
                    continue;
                }
                let (di, dj) = (nx as i32, ny as i32);
                let next = surface(i + di, j + dj).unwrap_or(if self.floating { bottom } else { 0.0 });
                let lit = 0.36 + 0.3 * (nx * light[0] + ny * light[1]).max(0.0) + if k < 2 { 0.04 } else { 0.0 };
                let (ax, ay, bx, by) = if nx != 0.0 { (x + nx * 0.5, y - 0.5, x + nx * 0.5, y + 0.5) } else { (x - 0.5, y + ny * 0.5, x + 0.5, y + ny * 0.5) };
                let mut side = |from: f32, to: f32, shade: f32| {
                    if to < from - 1e-3 {
                        quad(&mut mesh, [project(ax, ay, from), project(bx, by, from), project(bx, by, to), project(ax, ay, to)], grey(shade, 1.0));
                    }
                };
                // Under water, as seen through the side of a tank: the water, then the ground
                if wet {
                    side(z, next.max(ground), 44.0 + lit * 20.0);
                }
                side(ground, next.max(bottom), land_top * lit);
            }
            let corners = [project(x - 0.5, y - 0.5, z), project(x + 0.5, y - 0.5, z), project(x + 0.5, y + 0.5, z), project(x - 0.5, y + 0.5, z)];
            if outline && !wet {
                // Edged in a darker line, as cut into the block
                quad(&mut mesh, corners, grey(top * 0.62, 1.0));
                let middle = corners.iter().fold(vec2(0.0, 0.0), |sum, p| sum + p.to_vec2()) / 4.0;
                quad(&mut mesh, corners.map(|p| p + (middle.to_pos2() - p).normalized() * 1.3), grey(top, 1.0));
            } else {
                quad(&mut mesh, corners, grey(top, 1.0));
            }

            // A tree on it, in the woods, once the grass is growing
            let woods = self.woods[(i + j * n) as usize];
            if !wet && r >= 1.0 && woods > 0.0 && hash(i as u32, j as u32, 3) < woods * 0.55 {
                let size = ease_out_back(((phase.grow * 1.25 - hash(i as u32, j as u32, 4)) / 0.25).clamp(0.0, 1.0));
                if size > 0.0 {
                    let side = 0.3 * size;
                    let (ox, oy) = ((hash(i as u32, j as u32, 5) - 0.5) * 0.3, (hash(i as u32, j as u32, 6) - 0.5) * 0.3);
                    let (cx, cy) = (x + ox, y + oy);
                    let trunk = z + 0.45 * size;
                    let crown = trunk + 0.75 * size;
                    // The trunk, then the crown, a block of dark leaves
                    block(&mut mesh, &project, turned, (cx, cy), 0.06 * size, z, trunk, grey(40.0, 1.0), &|_| grey(30.0, 1.0));
                    block(&mut mesh, &project, turned, (cx, cy), side, trunk, crown, grey(96.0 + speck, 1.0), &|lit| grey(40.0 + 34.0 * lit, 1.0));
                }
            }

            // And the beasts, once it's all there, wandering about each on its own column
            if let Some(k) = self.beasts.iter().position(|&b| b == (i, j))
                && !wet
                && r >= 1.0
            {
                let size = ease_out_back(((phase.fill * 1.3 - k as f32 / self.beasts.len() as f32 * 0.3) / 0.3).clamp(0.0, 1.0));
                if size > 0.0 {
                    let (speed, start) = (0.4 + hash(i as u32, j as u32, 7) * 0.5, hash(i as u32, j as u32, 8) * TAU);
                    let a = start + t * speed;
                    let (cx, cy) = (x + a.cos() * 0.28, y + (a * 1.3).sin() * 0.28);
                    block(&mut mesh, &project, turned, (cx, cy), 0.11 * size, z, z + 0.2 * size, grey(236.0, 1.0), &|lit| grey(120.0 + 70.0 * lit, 1.0));
                }
            }
        }
        painter.add(Shape::mesh(mesh));

        // Where the player's to stand, in the middle: a white mark over it, bobbing
        if let Some(h) = self.height(n / 2, n / 2) {
            let ground = drawn(n / 2, n / 2).unwrap_or(0.0).max(if self.floating { 0.0 } else { water_level });
            let shown = rises(n / 2, n / 2) * phase.rise.clamp(0.0, 1.0);
            if shown > 0.0 && h.is_finite() {
                let c = (n / 2) as f32 + 0.5 - half;
                let foot = project(c, c, ground);
                let bob = (t * 2.2).sin() * 0.12 * scale;
                let tip = foot + vec2(0.0, -2.2 * scale + bob);
                painter.line_segment([foot, tip], Stroke::new(1.0f32, fade(Color32::from_white_alpha((200.0 * shown) as u8))));
                crate::theme::diamond(painter, tip, 3.5 + 0.08 * scale, fade(Color32::from_gray(240)).gamma_multiply(shown));
            }
        }
    }
}

/// Adds a block standing on the land, as a tree's crown or a beast, the view turned as `turned`
/// says, its sine and cosine: `half` a column across each way from `center`, from `from` up to
/// `to` columns over the floor, its top `top` and its sides `side` by how much they face the light
#[allow(clippy::too_many_arguments)]
fn block(mesh: &mut Mesh, project: &impl Fn(f32, f32, f32) -> Pos2, turned: (f32, f32), center: (f32, f32), half: f32, from: f32, to: f32, top: Color32, side: &dyn Fn(f32) -> Color32) {
    let (sin_t, cos_t) = turned;
    let (cx, cy) = center;
    let light = [-FRAC_1_SQRT_2, FRAC_1_SQRT_2];
    for (nx, ny) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        if nx * sin_t + ny * cos_t <= 0.0 {
            continue;
        }
        let lit = (nx * light[0] + ny * light[1]).max(0.0);
        let (ax, ay, bx, by) = if nx != 0.0 { (cx + nx * half, cy - half, cx + nx * half, cy + half) } else { (cx - half, cy + ny * half, cx + half, cy + ny * half) };
        quad(mesh, [project(ax, ay, to), project(bx, by, to), project(bx, by, from), project(ax, ay, from)], side(lit));
    }
    quad(mesh, [project(cx - half, cy - half, to), project(cx + half, cy - half, to), project(cx + half, cy + half, to), project(cx - half, cy + half, to)], top);
}

/// Adds the four-sided shape with `corners` in `color`
fn quad(mesh: &mut Mesh, corners: [Pos2; 4], color: Color32) {
    let first = mesh.vertices.len() as u32;
    for p in corners {
        mesh.colored_vertex(p, color);
    }
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
}

/// Coming up from 0 to 1 a little past it, and settling back
fn ease_out_back(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.0 {
        return 0.0;
    }
    let c = 1.4f32;
    1.0 + (c + 1.0) * (x - 1.0).powi(3) + c * (x - 1.0).powi(2)
}

/// A number from 0 to 1 that's always the same for `i`, `j` and `k`
fn hash(i: u32, j: u32, k: u32) -> f32 {
    super::hash(i.wrapping_mul(0x27d4_eb2d) ^ j, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(h: f32) -> Land {
        Land::from_heights(vec![h; ACROSS * ACROSS], vec![0.0; ACROSS * ACROSS])
    }

    #[test]
    fn the_plate_is_under_the_land_and_the_sea() {
        let land = flat(12.0);
        assert!(!land.floating);
        assert!(land.floor < 12.0 && land.floor > 0.0, "high land isn't stood on a tall block: {}", land.floor);
        assert!(land.top >= 12.0);
        let under = flat(-6.0);
        assert!(under.floor < -6.0, "the sea's level, 0, is over the plate to fill to: {}", under.floor);
    }

    #[test]
    fn sky_isles_hang_in_the_air() {
        let mut heights = vec![f32::NAN; ACROSS * ACROSS];
        for k in 0..ACROSS * 4 {
            heights[k] = 60.0;
        }
        let land = Land::from_heights(heights, vec![0.0; ACROSS * ACROSS]);
        assert!(land.floating);
        assert_eq!(land.height(0, 0), Some(60.0));
        assert_eq!(land.height(0, ACROSS as i32 - 1), None);
        // Its underside, not the far-off plate
        assert!((land.floor - (60.0 - ISLE_DEPTH_M)).abs() < 1e-3);
    }

    #[test]
    fn columns_come_up_and_settle() {
        assert_eq!(ease_out_back(0.0), 0.0);
        assert!((ease_out_back(1.0) - 1.0).abs() < 1e-5);
        // Past the top on the way
        assert!((0..100).map(|k| ease_out_back(k as f32 / 100.0)).any(|v| v > 1.0));
    }

    #[test]
    fn it_reads_the_real_land() {
        let settings = TerrainSettings { width_m: 224, height_m: 128, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let started = std::time::Instant::now();
        let land = Land::around(&settings, [195.0, 169.0]);
        // Quick enough to be there by the time the loading screen's drawn
        assert!(started.elapsed().as_millis() < 500, "took {:?}", started.elapsed());
        let middle = land.height(ACROSS as i32 / 2, ACROSS as i32 / 2).unwrap();
        let there = worldgen::seen_from_above(&settings, 195.0 + 1.5, 169.0 + 1.5).unwrap();
        assert!((middle - there).abs() < 1e-3, "{middle} against {there}");
        // Not all one height
        assert!(land.top - land.floor > 4.0);
        // Woods somewhere about, as there are round the lake where Innert starts
        assert!(land.woods.iter().any(|&w| w > 0.3));
    }

    #[test]
    fn nothing_grows_on_the_cinder_crown() {
        let realm = crate::realms::REALMS.iter().find(|r| worldgen::land_of(r.seed) == worldgen::Land::Dragon).unwrap();
        let settings = TerrainSettings { width_m: 224, height_m: 128, voxels_per_meter: 4, seed: realm.seed, grass_tufts: false };
        let land = Land::around(&settings, [195.0, 169.0]);
        assert!(land.woods.iter().all(|&w| w == 0.0));
    }
}
