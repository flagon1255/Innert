//! The land being built, as the loading screen shows it: the real lie of the land round where the
//! player's going to stand, a few hundred columns of it, rising out of a dark plate as the world's
//! built, the woods springing up on it as the grass grows and the seas filling in last, turning
//! slowly as if walked round. In greys, as a print cut in wood, see theme.rs.
//!
//! Its heights are the landscape's own, read off worldgen::seen_from_above() on another thread in a
//! few milliseconds, see around(), so it's the same hills, valleys and shores the player then
//! stands among, only without the rivers cut into it. Drawn with the GUI as a few thousand flat
//! shapes, and only while loading and behind the menu the game starts on.
//!
//! The columns answer the mouse, see touch.rs: hit() finds the one drawn under it, which rises
//! and whitens with those round it, and the waves it stirs lift the rest and ripple the sea.

use std::f32::consts::{FRAC_1_SQRT_2, SQRT_2, TAU};

use egui_winit_vulkano::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

use super::touch::Touch;
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

    /// `done` of the way from this to all built
    pub fn towards_done(self, done: f32) -> Phase {
        Phase { rise: self.rise + (1.0 - self.rise) * done, grow: self.grow + (1.0 - self.grow) * done, fill: self.fill + (1.0 - self.fill) * done }
    }
}

/// How the land's seen: where its middle's drawn, how big a column is, in points, and how it's
/// turned and looked down on, see Land::view()
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Where the middle of the plate's drawn
    pub middle: Pos2,
    /// How many points across a column is
    pub scale: f32,
    sin_t: f32,
    cos_t: f32,
    sin_e: f32,
    cos_e: f32,
    /// How much its heights are stretched, so gentle hills still read
    stretch: f32,
}

impl View {
    /// Where the spot `x` and `y` columns from the middle, `z` columns over the floor, is drawn
    pub fn project(&self, x: f32, y: f32, z: f32) -> Pos2 {
        let (u, v) = (x * self.cos_t - y * self.sin_t, x * self.sin_t + y * self.cos_t);
        pos2(self.middle.x + u * self.scale, self.middle.y + (v * self.sin_e - z * self.cos_e) * self.scale)
    }

    /// The spot drawn at `p`, `z` columns over the floor, in columns from the middle
    pub fn unproject(&self, p: Pos2, z: f32) -> (f32, f32) {
        let u = (p.x - self.middle.x) / self.scale;
        let v = ((p.y - self.middle.y) / self.scale + z * self.cos_e) / self.sin_e;
        (u * self.cos_t + v * self.sin_t, v * self.cos_t - u * self.sin_t)
    }

    /// How near the viewer the spot `x` and `y` columns from the middle is: what's nearer's drawn
    /// over what's further
    fn nearness(&self, x: f32, y: f32) -> f32 {
        x * self.sin_t + y * self.cos_t
    }

    /// Whether the side facing `nx` and `ny` faces the viewer
    fn facing(&self, nx: f32, ny: f32) -> bool {
        nx * self.sin_t + ny * self.cos_t > 0.0
    }

    /// `along` of the way from this to `other`, as the land glides over from the loading screen to
    /// behind the menu
    pub fn lerp(&self, other: &View, along: f32) -> View {
        View { middle: self.middle.lerp(other.middle, along), scale: self.scale + (other.scale - self.scale) * along, ..*other }
    }
}

/// How high a column may be lifted by the mouse, in columns, which the land's fitted with room for
pub const HEADROOM: f32 = 1.3;

/// The column drawn at a point, see Land::hit(): which it is, across and up, and the spot on its
/// top under the point, in columns from the middle
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub column: (i32, i32),
    pub at: (f32, f32),
}

/// Columns across each way, for the field the mouse makes waves in, see touch.rs
pub const COLUMNS: usize = ACROSS;

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

    /// Level land `h` meters over the sea, for tests
    #[cfg(test)]
    pub fn flat_for_tests(h: f32) -> Land {
        Land::from_heights(vec![h; ACROSS * ACROSS], vec![0.0; ACROSS * ACROSS])
    }

    fn height(&self, i: i32, j: i32) -> Option<f32> {
        let n = ACROSS as i32;
        if i < 0 || j < 0 || i >= n || j >= n {
            return None;
        }
        Some(self.heights[(i + j * n) as usize]).filter(|h| h.is_finite())
    }

    /// Whether column `k`, row after row from the south-west corner, is there, rather than the open
    /// air between sky isles
    pub fn solid(&self, k: usize) -> bool {
        self.heights.get(k).is_some_and(|h| h.is_finite())
    }

    /// How it's seen fitted into `area`, `t` seconds after it was first shown: as big as fits
    /// whichever way it's turned, with room over it for a column to be lifted by the mouse
    pub fn view(&self, area: Rect, t: f32) -> View {
        let (sin_e, cos_e) = ELEVATION.sin_cos();
        let (span, stretch) = self.stretch();
        let reach = ACROSS as f32 * SQRT_2;
        let scale = (area.width() / reach).min(area.height() / (reach * sin_e + (span * stretch + HEADROOM) * cos_e));
        let middle = pos2(area.center().x, area.bottom() - (reach * 0.5 * sin_e) * scale);
        self.turned(middle, scale, t)
    }

    /// How many columns high from the floor to the highest, and how much that's stretched so gentle
    /// hills still read, unless that'd stand them taller than the land's wide
    fn stretch(&self) -> (f32, f32) {
        let span = (self.top - self.floor).max(1.0) / CELL_M;
        (span, (0.5 * ACROSS as f32 / span).min(1.7))
    }

    /// Seen with its middle at `middle`, `scale` points a column, turned as it is `t` seconds after
    /// it was first shown
    fn turned(&self, middle: Pos2, scale: f32, t: f32) -> View {
        let (sin_t, cos_t) = (0.65 + t * TURN_SPEED).sin_cos();
        let (sin_e, cos_e) = ELEVATION.sin_cos();
        View { middle, scale, sin_t, cos_t, sin_e, cos_e, stretch: self.stretch().1 }
    }

    /// How far column `i`, `j` has come up, 0 to 1, `phase` of the way built: from the middle out,
    /// a little ragged, a little past where it stops on the way
    fn rises(&self, i: i32, j: i32, phase: Phase) -> f32 {
        let half = ACROSS as f32 * 0.5;
        let d = ((i as f32 + 0.5 - half).hypot(j as f32 + 0.5 - half) / (half * SQRT_2)).min(1.0);
        let start = d * 0.78 + hash(i as u32, j as u32, 1) * 0.2;
        ease_out_back(((phase.rise * 1.25 - start) / 0.25).clamp(0.0, 1.0))
    }

    /// Meters over the sea to how high it's drawn, in columns over the floor
    fn lift(&self, view: &View, h: f32) -> f32 {
        (h - self.floor) / CELL_M * view.stretch
    }

    /// How low column `k` is drawn down to, in columns over the floor
    fn bottom(&self, view: &View, k: usize) -> f32 {
        self.lift(view, self.undersides[k]).max(0.0)
    }

    /// How high column `i`, `j`'s ground is drawn, in columns over the floor, as far as it's come
    /// up, lifted by the mouse as `touch` has it
    fn ground(&self, view: &View, phase: Phase, i: i32, j: i32, touch: Option<&Touch>) -> Option<f32> {
        let h = self.height(i, j)?;
        let k = (i + j * ACROSS as i32) as usize;
        let r = self.rises(i, j, phase);
        let bottom = self.bottom(view, k);
        let lifted = touch.map_or(0.0, |t| t.lift(k)) * r;
        Some((bottom + (self.lift(view, h) - bottom) * r + lifted).max(bottom))
    }

    /// How high the sea's filled, in columns over the floor: none round sky isles
    fn sea(&self, view: &View, phase: Phase) -> f32 {
        if self.floating { 0.0 } else { self.lift(view, 0.0) * phase.fill }
    }

    /// Which column's drawn at `p`, seen as `view`, `phase` of the way built, and the spot on its
    /// top under `p`: of all those drawn there, the one nearest the viewer, as the rest are drawn
    /// behind it. As the land stands without the mouse's lifting, so a column lifted doesn't move
    /// out from under it. None off the land.
    pub fn hit(&self, view: &View, phase: Phase, p: Pos2) -> Option<Spot> {
        let n = ACROSS as i32;
        let half = n as f32 * 0.5;
        let sea = self.sea(view, phase);
        let mut best: Option<(f32, i32, i32, f32)> = None;
        for j in 0..n {
            for i in 0..n {
                let (x, y) = (i as f32 + 0.5 - half, j as f32 + 0.5 - half);
                let near = view.nearness(x, y);
                if best.is_some_and(|b| b.0 >= near) {
                    continue;
                }
                // Too far across to be under it, whatever its height
                if (view.project(x, y, 0.0).x - p.x).abs() > view.scale * FRAC_1_SQRT_2 + 1.0 {
                    continue;
                }
                if self.rises(i, j, phase) <= 0.0 {
                    continue;
                }
                let Some(ground) = self.ground(view, phase, i, j, None) else { continue };
                let bottom = self.bottom(view, (i + j * n) as usize);
                if covers(view, (x, y), ground.max(sea), bottom, p) {
                    best = Some((near, i, j, ground.max(sea)));
                }
            }
        }
        let (_, i, j, z) = best?;
        let (cx, cy) = (i as f32 + 0.5 - half, j as f32 + 0.5 - half);
        let (x, y) = view.unproject(p, z);
        Some(Spot { column: (i, j), at: (x.clamp(cx - 0.5, cx + 0.5), y.clamp(cy - 0.5, cy + 0.5)) })
    }

    /// Draws it seen as `view`, `t` seconds after it was first shown, `phase` of the way built,
    /// `alpha` of the way opaque and `bright` as bright as on the loading screen, its columns
    /// lifted and lit by the mouse as `touch` has them
    #[allow(clippy::too_many_arguments)]
    pub fn paint(&self, painter: &Painter, view: &View, t: f32, phase: Phase, alpha: f32, bright: f32, touch: Option<&Touch>) {
        let n = ACROSS as i32;
        let half = n as f32 * 0.5;
        let (sin_t, cos_t, scale) = (view.sin_t, view.cos_t, view.scale);
        let middle = view.middle;
        let project = |x: f32, y: f32, z: f32| view.project(x, y, z);
        let fade = |c: Color32| c.gamma_multiply(alpha);
        let grey = |v: f32| {
            let v = (v * bright).round().clamp(0.0, 255.0) as u8;
            fade(Color32::from_rgb(v, v, v))
        };
        let sea = self.sea(view, phase);
        // The sea over column `k`, rippled where the mouse has stirred it
        let water = |k: usize| if self.floating { 0.0 } else { (sea + WATER_RIPPLE * touch.map_or(0.0, |t| t.ripple(k)) * phase.fill).max(0.0) };

        // The plate the land rises out of, unless it's sky isles hanging over nothing
        let mut mesh = Mesh::default();
        if !self.floating {
            let corners = [(-half, -half), (half, -half), (half, half), (-half, half)].map(|(x, y)| project(x, y, 0.0));
            quad(&mut mesh, corners, grey(24.0));
            let below = corners.map(|p| p + vec2(0.0, 0.6 * scale));
            // Its front edges, the two facing the viewer
            for k in 0..4 {
                let (a, b) = (k, (k + 1) % 4);
                if corners[a].y + corners[b].y > 2.0 * middle.y {
                    quad(&mut mesh, [corners[a], corners[b], below[b], below[a]], grey(14.0));
                }
            }
        }

        // Back to front, as the view's turned
        let mut order: Vec<(f32, i32, i32)> = (0..n * n)
            .map(|k| {
                let (i, j) = (k % n, k / n);
                (view.nearness(i as f32 + 0.5 - half, j as f32 + 0.5 - half), i, j)
            })
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        // The light from the north-west, its sides lit by how much they face it
        let light = [-FRAC_1_SQRT_2, FRAC_1_SQRT_2];
        let outline = scale > 7.0;
        let turned = (sin_t, cos_t);
        // How high each column's drawn now, sea and all, to know how much of its neighbours'
        // sides show
        let surface = |i: i32, j: i32| self.ground(view, phase, i, j, touch).map(|z| z.max(water((i + j * n) as usize)));
        for &(_, i, j) in &order {
            let Some(h) = self.height(i, j) else { continue };
            let r = self.rises(i, j, phase);
            if r <= 0.0 {
                continue;
            }
            let k = (i + j * n) as usize;
            let (x, y) = (i as f32 + 0.5 - half, j as f32 + 0.5 - half);
            let ground = self.ground(view, phase, i, j, touch).unwrap();
            let bottom = self.bottom(view, k);
            let wet = !self.floating && water(k) > ground + 1e-3;
            let z = if wet { water(k) } else { ground };
            // How much the mouse has lit it, whitening it as a block fresh cut
            let lit_up = touch.map_or(0.0, |t| t.glow(k)) * r;
            // Higher is whiter, and what's only just come up whiter still, fresh from the plate
            let height = ((h - self.floor) / (self.top - self.floor).max(1.0)).clamp(0.0, 1.0);
            let speck = (hash(i as u32, j as u32, 2) - 0.5) * 14.0;
            let fresh = (1.0 - r).max(0.0) * 60.0;
            let under_sea = if h < 0.0 { 0.72 } else { 1.0 };
            let land_top = ((132.0 + 104.0 * height + speck) * under_sea + fresh).min(250.0);
            let land_top = land_top + (250.0 - land_top).max(0.0) * lit_up;
            // A ripple going through lighter on its crests and darker in its troughs, so its rings
            // read in black and white
            let swell = touch.map_or(0.0, |t| t.ripple(k)) * r;
            let land_top = (land_top + swell.clamp(-1.0, 1.0) * if swell > 0.0 { 170.0 } else { 140.0 }).clamp(40.0, 252.0);
            let shimmer = 7.0 * (t * 1.6 + i as f32 * 0.7 + j as f32 * 0.45).sin();
            let top = if wet { (58.0 + shimmer + 110.0 * lit_up + 200.0 * swell).clamp(24.0, 210.0) } else { land_top };

            // The sides that face the viewer, down to where the next column's top hides them
            for (f, &(nx, ny)) in FACES.iter().enumerate() {
                if !view.facing(nx, ny) {
                    continue;
                }
                let (di, dj) = (nx as i32, ny as i32);
                let next = surface(i + di, j + dj).unwrap_or(if self.floating { bottom } else { 0.0 });
                let lit = 0.36 + 0.3 * (nx * light[0] + ny * light[1]).max(0.0) + if f < 2 { 0.04 } else { 0.0 };
                let (ax, ay, bx, by) = if nx != 0.0 { (x + nx * 0.5, y - 0.5, x + nx * 0.5, y + 0.5) } else { (x - 0.5, y + ny * 0.5, x + 0.5, y + ny * 0.5) };
                let mut side = |from: f32, to: f32, shade: f32| {
                    if to < from - 1e-3 {
                        quad(&mut mesh, [project(ax, ay, from), project(bx, by, from), project(bx, by, to), project(ax, ay, to)], grey(shade));
                    }
                };
                // Under water, as seen through the side of a tank: the water, then the ground
                if wet {
                    side(z, next.max(ground), 44.0 + lit * 20.0);
                }
                // Lit too where the mouse has lifted it, so what's under it lights up whichever
                // face it's on
                side(ground, next.max(bottom), land_top * lit + (236.0 - land_top * lit).max(0.0) * lit_up * 0.75);
            }
            let corners = [project(x - 0.5, y - 0.5, z), project(x + 0.5, y - 0.5, z), project(x + 0.5, y + 0.5, z), project(x - 0.5, y + 0.5, z)];
            if outline && !wet {
                // Edged in a darker line, as cut into the block
                quad(&mut mesh, corners, grey(top * 0.62));
                let middle = corners.iter().fold(vec2(0.0, 0.0), |sum, p| sum + p.to_vec2()) / 4.0;
                quad(&mut mesh, corners.map(|p| p + (middle.to_pos2() - p).normalized() * 1.3), grey(top));
            } else {
                quad(&mut mesh, corners, grey(top));
            }

            // A tree on it, in the woods, once the grass is growing
            let woods = self.woods[k];
            if !wet && r >= 1.0 && woods > 0.0 && hash(i as u32, j as u32, 3) < woods * 0.55 {
                let size = ease_out_back(((phase.grow * 1.25 - hash(i as u32, j as u32, 4)) / 0.25).clamp(0.0, 1.0));
                if size > 0.0 {
                    let side = 0.3 * size;
                    let (ox, oy) = ((hash(i as u32, j as u32, 5) - 0.5) * 0.3, (hash(i as u32, j as u32, 6) - 0.5) * 0.3);
                    let (cx, cy) = (x + ox, y + oy);
                    let trunk = z + 0.45 * size;
                    let crown = trunk + 0.75 * size;
                    // The trunk, then the crown, a block of dark leaves
                    block(&mut mesh, &project, turned, (cx, cy), 0.06 * size, z, trunk, grey(40.0), &|_| grey(30.0));
                    block(&mut mesh, &project, turned, (cx, cy), side, trunk, crown, grey(96.0 + speck + 80.0 * lit_up), &|lit| grey(40.0 + 34.0 * lit + 40.0 * lit_up));
                }
            }

            // And the beasts, once it's all there, wandering about each on its own column
            if let Some(b) = self.beasts.iter().position(|&b| b == (i, j))
                && !wet
                && r >= 1.0
            {
                let size = ease_out_back(((phase.fill * 1.3 - b as f32 / self.beasts.len() as f32 * 0.3) / 0.3).clamp(0.0, 1.0));
                if size > 0.0 {
                    let (speed, start) = (0.4 + hash(i as u32, j as u32, 7) * 0.5, hash(i as u32, j as u32, 8) * TAU);
                    let a = start + t * speed;
                    let (cx, cy) = (x + a.cos() * 0.28, y + (a * 1.3).sin() * 0.28);
                    block(&mut mesh, &project, turned, (cx, cy), 0.11 * size, z, z + 0.2 * size, grey(236.0), &|lit| grey(120.0 + 70.0 * lit));
                }
            }
        }
        painter.add(Shape::mesh(mesh));

        // Where the player's to stand, in the middle: a white mark over it, bobbing
        let (c, k) = (n / 2, ((n / 2) * (n + 1)) as usize);
        if let Some(ground) = self.ground(view, phase, c, c, touch) {
            let shown = self.rises(c, c, phase) * phase.rise.clamp(0.0, 1.0);
            if shown > 0.0 {
                let at = c as f32 + 0.5 - half;
                let foot = project(at, at, ground.max(water(k)));
                let bob = (t * 2.2).sin() * 0.12 * scale;
                let tip = foot + vec2(0.0, -2.2 * scale + bob);
                painter.line_segment([foot, tip], Stroke::new(1.0f32, fade(Color32::from_white_alpha((200.0 * shown) as u8))));
                crate::theme::diamond(painter, tip, 3.5 + 0.08 * scale, grey(240.0).gamma_multiply(shown));
            }
        }
    }
}

/// A column's four sides, which way each faces: those along x, and along y
const FACES: [(f32, f32); 4] = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)];

/// How much of a ripple shows in the sea over a column, against how much a column's lifted
const WATER_RIPPLE: f32 = 0.35;

/// Whether the column `x` and `y` columns from the middle, its top `z` and its foot `bottom`
/// columns over the floor, is drawn over `p`, seen as `view`: its top or a side facing the viewer
fn covers(view: &View, (x, y): (f32, f32), z: f32, bottom: f32, p: Pos2) -> bool {
    let top = [view.project(x - 0.5, y - 0.5, z), view.project(x + 0.5, y - 0.5, z), view.project(x + 0.5, y + 0.5, z), view.project(x - 0.5, y + 0.5, z)];
    if inside(&top, p) {
        return true;
    }
    if z - bottom < 1e-3 {
        return false;
    }
    FACES.iter().filter(|&&(nx, ny)| view.facing(nx, ny)).any(|&(nx, ny)| {
        let (ax, ay, bx, by) = if nx != 0.0 { (x + nx * 0.5, y - 0.5, x + nx * 0.5, y + 0.5) } else { (x - 0.5, y + ny * 0.5, x + 0.5, y + ny * 0.5) };
        inside(&[view.project(ax, ay, z), view.project(bx, by, z), view.project(bx, by, bottom), view.project(ax, ay, bottom)], p)
    })
}

/// Whether `p` is inside the four-sided shape with `corners`, which goes round one way or the other
fn inside(corners: &[Pos2; 4], p: Pos2) -> bool {
    let mut way = 0.0f32;
    for k in 0..4 {
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if cross.abs() < 1e-6 {
            continue;
        }
        if way == 0.0 {
            way = cross.signum();
        } else if cross.signum() != way {
            return false;
        }
    }
    way != 0.0
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

    /// Where it's seen on the loading screen in a window `w` by `h`, `t` seconds in
    fn seen(land: &Land, [w, h]: [f32; 2], t: f32) -> View {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h));
        land.view(super::super::land_area(screen, super::super::innert_below(screen)).expect("room for the land"), t)
    }

    /// Where the middle of column `i`, `j`'s top is drawn
    fn top_of(land: &Land, view: &View, (i, j): (i32, i32)) -> Pos2 {
        let half = ACROSS as f32 * 0.5;
        let z = land.ground(view, Phase::DONE, i, j, None).unwrap().max(land.sea(view, Phase::DONE));
        view.project(i as f32 + 0.5 - half, j as f32 + 0.5 - half, z)
    }

    const WINDOWS: [[f32; 2]; 4] = [[1280.0, 600.0], [1548.0, 872.0], [1920.0, 1080.0], [3840.0, 2160.0]];

    #[test]
    fn the_mouse_finds_the_column_under_it() {
        let land = flat(6.0);
        let half = ACROSS as f32 * 0.5;
        for size in WINDOWS {
            for t in [0.0, 5.0, 13.0, 29.0, 41.0] {
                let view = seen(&land, size, t);
                for i in 0..ACROSS as i32 {
                    for j in 0..ACROSS as i32 {
                        // On its top, a little off its middle
                        let p = top_of(&land, &view, (i, j)) + vec2(0.2, -0.15) * view.scale;
                        let spot = land.hit(&view, Phase::DONE, p).unwrap_or_else(|| panic!("nothing at {p:?} for {i}, {j} at {size:?}"));
                        assert_eq!(spot.column, (i, j), "at {size:?}, {t} s");
                        // And where on it
                        let z = land.ground(&view, Phase::DONE, i, j, None).unwrap();
                        let back = view.project(spot.at.0, spot.at.1, z);
                        assert!((back - p).length() < 0.05, "{back:?} against {p:?}");
                        assert!((spot.at.0 - (i as f32 + 0.5 - half)).abs() <= 0.5 && (spot.at.1 - (j as f32 + 0.5 - half)).abs() <= 0.5);
                    }
                }
                // Over the bare page, nothing
                assert_eq!(land.hit(&view, Phase::DONE, pos2(4.0, 4.0)), None);
            }
        }
    }

    #[test]
    fn the_sides_at_the_front_are_the_columns_they_belong_to() {
        let land = flat(6.0);
        let half = ACROSS as f32 * 0.5;
        let n = ACROSS as i32;
        for size in WINDOWS {
            let view = seen(&land, size, 7.0);
            let mut sides = 0;
            for i in 0..n {
                for j in 0..n {
                    let (x, y) = (i as f32 + 0.5 - half, j as f32 + 0.5 - half);
                    let z = land.ground(&view, Phase::DONE, i, j, None).unwrap();
                    for (nx, ny) in FACES {
                        let (a, b) = (i + nx as i32, j + ny as i32);
                        // A side at the land's edge, facing the viewer: halfway down its middle
                        if view.facing(nx, ny) && (a < 0 || b < 0 || a >= n || b >= n) {
                            let p = view.project(x + nx * 0.5, y + ny * 0.5, z * 0.5);
                            assert_eq!(land.hit(&view, Phase::DONE, p).map(|s| s.column), Some((i, j)), "at {size:?}");
                            sides += 1;
                        }
                    }
                }
            }
            assert!(sides >= ACROSS);
        }
    }

    #[test]
    fn a_column_in_front_hides_those_behind_it() {
        let mut heights = vec![6.0; ACROSS * ACROSS];
        let tower = (17, 17);
        heights[(tower.0 + tower.1 * ACROSS as i32) as usize] = 60.0;
        let land = Land::from_heights(heights, vec![0.0; ACROSS * ACROSS]);
        let half = ACROSS as f32 * 0.5;
        for t in [0.0, 9.0, 23.0] {
            let view = seen(&land, [1548.0, 872.0], t);
            let (tx, ty) = (tower.0 as f32 + 0.5 - half, tower.1 as f32 + 0.5 - half);
            let tall = land.ground(&view, Phase::DONE, tower.0, tower.1, None).unwrap();
            let mut hidden = 0;
            for i in 0..ACROSS as i32 {
                for j in 0..ACROSS as i32 {
                    let p = top_of(&land, &view, (i, j));
                    let (x, y) = (i as f32 + 0.5 - half, j as f32 + 0.5 - half);
                    // The tops behind it that it's drawn over are its
                    if (i, j) != tower && view.nearness(x, y) < view.nearness(tx, ty) && covers(&view, (tx, ty), tall, 0.0, p) {
                        assert_eq!(land.hit(&view, Phase::DONE, p).unwrap().column, tower);
                        hidden += 1;
                    }
                }
            }
            assert!(hidden > 3, "{hidden} hidden at {t} s");
        }
    }

    #[test]
    fn what_is_drawn_where_can_be_found_again() {
        let land = flat(3.0);
        let view = seen(&land, [1280.0, 600.0], 11.0);
        for (x, y, z) in [(0.0, 0.0, 0.0), (-7.3, 4.1, 2.0), (12.0, -16.5, 9.5)] {
            let (a, b) = view.unproject(view.project(x, y, z), z);
            assert!((a - x).abs() < 1e-3 && (b - y).abs() < 1e-3, "{a}, {b} against {x}, {y}");
        }
    }

    #[test]
    fn lifted_columns_stay_inside_the_room_left_for_them() {
        // The land's fitted with room over its highest column for one lifted by the mouse
        let land = flat(6.0);
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 600.0));
        let area = super::super::land_area(screen, super::super::innert_below(screen)).unwrap();
        let half = ACROSS as f32 * 0.5;
        for t in [0.0, 6.0, 17.0] {
            let view = land.view(area, t);
            for i in 0..ACROSS as i32 {
                for j in 0..ACROSS as i32 {
                    let (x, y) = (i as f32 + 0.5 - half, j as f32 + 0.5 - half);
                    let z = land.ground(&view, Phase::DONE, i, j, None).unwrap() + HEADROOM;
                    for (a, b) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                        let top = view.project(x + a, y + b, z);
                        assert!(top.y >= area.top() - 0.5, "{top:?} over {area:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn nothing_grows_on_the_cinder_crown() {
        let realm = crate::realms::REALMS.iter().find(|r| worldgen::land_of(r.seed) == worldgen::Land::Dragon).unwrap();
        let settings = TerrainSettings { width_m: 224, height_m: 128, voxels_per_meter: 4, seed: realm.seed, grass_tufts: false };
        let land = Land::around(&settings, [195.0, 169.0]);
        assert!(land.woods.iter().all(|&w| w == 0.0));
    }
}
