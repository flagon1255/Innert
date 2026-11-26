//! The look of the whole interface, set in one place, see apply(): black and white, as a print
//! cut in wood. Near-black panels a little see-through, edged in grey with a fine second rule
//! inside and a white line along their tops, white and greys for words, edges, buttons and what's
//! picked. Colour's kept only for what it tells in play: rarities, the bars of health, hunger and
//! mana, better and worse, gold, hurt and warnings, and it stands out the more for it.
//!
//! The name on the menu and the biggest titles are in UnifrakturCook, a blackletter, the other
//! headings in Cinzel, see assets\fonts\CREDITS.txt, the rest in egui's own font: blackletter's
//! hard to read small, so it's only ever big.
//!
//! And what the HUD and the windows draw by hand with, to look the same: panels, bars, slots, the
//! white line, and the title, rule, rows and bullets of an item's card. All of it's a few shapes,
//! no pictures, so it costs next to nothing each frame.

use std::sync::Arc;

use egui_winit_vulkano::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Margin, Painter, Pos2, Rect, RichText, Sense, Shadow, Stroke, StrokeKind, TextStyle,
    style::WidgetVisuals, text::LayoutJob, text::TextFormat, pos2, vec2,
};

/// Words
pub const TEXT: Color32 = Color32::from_gray(226);
/// Words that matter less: what's said about something, how to do it
pub const WEAK: Color32 = Color32::from_gray(156);
/// Headings and titles
pub const HEADING: Color32 = Color32::from_gray(250);
/// The white of the line along a panel's top, the rules and ornaments, and what's picked
pub const ACCENT: Color32 = Color32::from_gray(238);
/// The grey of panels' edges and the lines across them
pub const EDGE: Color32 = Color32::from_gray(118);
/// Better, and worse
pub const GOOD: Color32 = Color32::from_rgb(128, 226, 112);
pub const BAD: Color32 = Color32::from_rgb(240, 100, 88);
/// Something to know, as points waiting to be spent
pub const INFO: Color32 = Color32::from_rgb(140, 212, 250);
/// The fill of a button that goes on, as Start: white, its words in black, see primary()
pub const PRIMARY: Color32 = Color32::from_gray(226);
/// The words on it
pub const PRIMARY_INK: Color32 = Color32::from_gray(12);

/// How big windows' titles are, in blackletter
const HEADING_SIZE: f32 = 23.0;

/// The names the headings' fonts go by, see display() and blackletter()
const DISPLAY: &str = "display";
const BLACKLETTER: &str = "blackletter";

/// The fill of windows and panels, near black and a little see-through
pub fn panel_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(9, 9, 9, 236)
}

/// The fill of the HUD's panels, more see-through so the world shows, dark enough that what's on
/// them reads over snow at noon
pub fn hud_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(6, 6, 6, 160)
}

/// The headings' font at `size`
pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DISPLAY.into()))
}

/// The blackletter at `size`, for the name on the menu and the biggest titles only: under about 20
/// it's hard to read
pub fn blackletter(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BLACKLETTER.into()))
}

/// A button that goes on, as Start: white with `text` on it in black, in the headings' font at
/// `size`
pub fn primary(text: &str, size: f32) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).font(display(size)).color(PRIMARY_INK)).fill(PRIMARY)
}

/// `c` a little greyer, `t` of the way to a grey as light as it is, so a bar's colour sits in the
/// black and white without losing which it is
pub fn muted(c: Color32, t: f32) -> Color32 {
    let grey = (0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32) as u8;
    c.lerp_to_gamma(Color32::from_rgba_premultiplied(grey, grey, grey, c.a()), t)
}

/// `c` lighter, `t` of the way to white
pub fn lighter(c: Color32, t: f32) -> Color32 {
    c.lerp_to_gamma(Color32::WHITE, t)
}

/// `c` darker, its light times `k`, as see-through as it was
pub fn darker(c: Color32, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8, c.a())
}

/// The fonts: egui's own, Cinzel for headings and UnifrakturCook for the biggest titles, each
/// falling back on the ones after it for anything it hasn't
fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("Cinzel".to_owned(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/Cinzel-Bold.ttf"))));
    fonts.font_data.insert("UnifrakturCook".to_owned(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/UnifrakturCook-Bold.ttf"))));
    let proportional = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut display = vec!["Cinzel".to_owned()];
    display.extend(proportional.iter().cloned());
    let mut blackletter = vec!["UnifrakturCook".to_owned()];
    blackletter.extend(display.iter().cloned());
    fonts.families.insert(FontFamily::Name(DISPLAY.into()), display);
    fonts.families.insert(FontFamily::Name(BLACKLETTER.into()), blackletter);
    fonts
}

/// The look of a widget: its fill, its edge, the colour of what's on it, and how round it is
fn widget(fill: Color32, edge: Stroke, ink: Stroke, expansion: f32) -> WidgetVisuals {
    WidgetVisuals { bg_fill: fill, weak_bg_fill: fill, bg_stroke: edge, corner_radius: CornerRadius::same(RADIUS), fg_stroke: ink, expansion }
}

/// How round widgets' corners are, a little: squarer than egui's, as a printed frame
const RADIUS: u8 = 3;

/// Sets the theme on everything egui draws: the fonts, the sizes of words, the spacing, and the
/// colours of windows, widgets and what's picked. Called once, as the GUI's made.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    // Dark whatever Windows is set to
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(style);
}

/// The theme, on `style`
pub fn style(style: &mut egui::Style) {
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(10.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.5)),
        // Window titles, in blackletter
        (TextStyle::Heading, blackletter(HEADING_SIZE)),
        (TextStyle::Monospace, FontId::monospace(12.5)),
    ]
    .into();

    let spacing = &mut style.spacing;
    // A little more room inside windows and buttons than egui gives, not so much that what fit
    // before doesn't now
    spacing.item_spacing = vec2(8.0, 3.0);
    spacing.button_padding = vec2(7.0, 2.0);
    spacing.window_margin = Margin::same(10);
    spacing.menu_margin = Margin::same(9);

    let v = &mut style.visuals;
    v.dark_mode = true;
    v.override_text_color = None;
    v.window_fill = panel_fill();
    v.panel_fill = panel_fill();
    v.window_stroke = Stroke::new(1.0f32, EDGE);
    v.window_highlight_topmost = false;
    // Squarer than egui's, as a printed frame
    v.window_corner_radius = CornerRadius::same(RADIUS + 1);
    v.menu_corner_radius = CornerRadius::same(RADIUS);
    v.window_shadow = Shadow { offset: [0, 8], blur: 22, spread: 0, color: Color32::from_black_alpha(150) };
    v.popup_shadow = Shadow { offset: [0, 6], blur: 14, spread: 0, color: Color32::from_black_alpha(140) };
    v.extreme_bg_color = Color32::from_gray(5);
    v.faint_bg_color = Color32::from_white_alpha(6);
    v.code_bg_color = Color32::from_gray(34);
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = Color32::from_rgb(255, 170, 90);
    v.error_fg_color = BAD;
    // What's picked a grey under white words: light enough to tell from what isn't, dark enough
    // that words selected in a text box over it still read
    v.selection.bg_fill = Color32::from_gray(86);
    v.selection.stroke = Stroke::new(1.0f32, Color32::WHITE);
    v.slider_trailing_fill = true;

    let w = &mut v.widgets;
    // Labels and the lines across windows; weak words are halfway from TEXT to its weak_bg_fill
    w.noninteractive = WidgetVisuals {
        bg_fill: panel_fill(),
        weak_bg_fill: Color32::from_gray(92),
        bg_stroke: Stroke::new(1.0f32, EDGE.gamma_multiply(0.75)),
        corner_radius: CornerRadius::same(RADIUS),
        fg_stroke: Stroke::new(1.0f32, TEXT),
        expansion: 0.0,
    };
    // Buttons dark grey edged in grey, lit white round their edge when the pointer's on them
    w.inactive = widget(Color32::from_gray(30), Stroke::new(1.0f32, Color32::from_gray(86)), Stroke::new(1.0f32, TEXT), 0.0);
    w.hovered = widget(Color32::from_gray(50), Stroke::new(1.0f32, Color32::from_gray(232)), Stroke::new(1.5f32, Color32::WHITE), 1.0);
    w.active = widget(Color32::from_gray(76), Stroke::new(1.0f32, Color32::WHITE), Stroke::new(2.0f32, Color32::WHITE), 1.0);
    w.open = widget(Color32::from_gray(40), Stroke::new(1.0f32, Color32::from_gray(132)), Stroke::new(1.0f32, TEXT), 0.0);
}

/// A rect shaded from `top` at its top to `bottom` at its foot, square-cornered
pub fn gradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// A line `thick` high from `from` to `to` across, at `y`, in `color` at the middle fading out to
/// nothing at both ends
fn faded_line(painter: &Painter, from: f32, to: f32, y: f32, thick: f32, color: Color32) {
    let mut mesh = egui::Mesh::default();
    let middle = (from + to) * 0.5;
    for (x, c) in [(from, Color32::TRANSPARENT), (middle, color), (to, Color32::TRANSPARENT)] {
        mesh.colored_vertex(pos2(x, y), c);
        mesh.colored_vertex(pos2(x, y + thick), c);
    }
    for k in [0, 2] {
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k + 1, k + 3, k + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// The thin white line along the top of panel `rect`, brightest in the middle, `strength` how
/// bright
pub fn accent(painter: &Painter, rect: Rect, strength: f32) {
    let inset = (rect.width() * 0.08).clamp(6.0, 28.0);
    faded_line(painter, rect.left() + inset, rect.right() - inset, rect.top() + 0.5, 1.5, ACCENT.gamma_multiply(strength));
}

/// The fine second rule inside a panel's edge, `inset` in from it, as round a printed page: in
/// white, `strength` how bright, `radius` the panel's roundness
pub fn inner_rule(painter: &Painter, rect: Rect, inset: f32, radius: f32, strength: f32) {
    let rule = rect.shrink(inset);
    painter.rect_stroke(rule, (radius - inset).max(0.0), Stroke::new(1.0f32, Color32::from_white_alpha((46.0 * strength) as u8)), StrokeKind::Inside);
}

/// A little diamond, `r` from its middle to its points, ringed in dark
pub fn diamond(painter: &Painter, at: Pos2, r: f32, color: Color32) {
    let points = vec![at + vec2(0.0, -r), at + vec2(r, 0.0), at + vec2(0.0, r), at + vec2(-r, 0.0)];
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::new(1.0f32, Color32::from_black_alpha(160))));
}

/// An ornament under a title, as a printer's: a white diamond at `at` between two small ones,
/// with lines fading out either side of it, `half` across each way
pub fn flourish(painter: &Painter, at: Pos2, half: f32) {
    for side in [-1.0, 1.0f32] {
        let (a, b) = (at.x + side * 15.0, at.x + side * half);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(pos2(a, at.y - 0.6), ACCENT);
        mesh.colored_vertex(pos2(a, at.y + 0.6), ACCENT);
        mesh.colored_vertex(pos2(b, at.y - 0.6), Color32::TRANSPARENT);
        mesh.colored_vertex(pos2(b, at.y + 0.6), Color32::TRANSPARENT);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 3, 2);
        painter.add(egui::Shape::mesh(mesh));
        diamond(painter, at + vec2(side * 9.5, 0.0), 2.0, ACCENT);
    }
    diamond(painter, at, 4.5, ACCENT);
}

/// The frame of a window drawn by hand, as the menu's: near black, edged in grey, `margin` inside
/// and `radius` round
pub fn panel(margin: f32, radius: f32) -> egui::Frame {
    egui::Frame::new()
        .fill(panel_fill())
        .stroke(Stroke::new(1.0f32, EDGE))
        .corner_radius(radius)
        .inner_margin(margin)
        .shadow(Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(140) })
}

/// Shows `frame` round what `add` puts in it, with the second rule inside its edge and the white
/// line along its top
pub fn framed<R>(ui: &mut egui::Ui, frame: egui::Frame, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let radius = frame.corner_radius.nw as f32;
    let shown = frame.show(ui, add);
    inner_rule(ui.painter(), shown.response.rect, 4.0, radius, 1.0);
    accent(ui.painter(), shown.response.rect, 1.0);
    shown
}

/// A panel of the HUD at `rect`, `radius` round: see-through, edged in grey, the white line along
/// its top
pub fn hud_panel(painter: &Painter, rect: Rect, radius: f32) {
    painter.rect(rect, radius, hud_fill(), Stroke::new(1.0f32, EDGE.gamma_multiply(0.8)), StrokeKind::Inside);
    accent(painter, rect, 0.8);
}

/// `text` with a shadow under it, so it reads over the brightest of what's behind
pub fn shadowed(painter: &Painter, at: Pos2, align: Align2, text: &str, font: FontId, color: Color32) {
    let shade = Color32::from_black_alpha((color.a() as f32 * 0.7) as u8);
    painter.text(at + vec2(1.0, 1.0), align, text, font.clone(), shade);
    painter.text(at, align, text, font, color);
}

/// How much greyer a bar's colour is than it's given, see muted(): enough to sit in the black and
/// white, not so much that health, hunger and mana aren't told apart at a glance
pub const BAR_MUTED: f32 = 0.14;

/// A bar at `rect` `fraction` full of `fill`, a little greyed: on a black backing in a grey frame,
/// shaded from light at its top to darker at its foot, a glint along its top, and `text` across it
/// at the right
pub fn bar(painter: &Painter, rect: Rect, fraction: f32, fill: Color32, text: Option<&str>) {
    painter.rect(rect.expand(1.0), 2.0, Color32::from_black_alpha(170), Stroke::new(1.0f32, EDGE.gamma_multiply(0.7)), StrokeKind::Outside);
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let fill = muted(fill, BAR_MUTED);
        let full = Rect::from_min_size(rect.min, vec2(rect.width() * fraction, rect.height()));
        gradient(painter, full, lighter(fill, 0.2), darker(fill, 0.62));
        painter.line_segment([full.left_top() + vec2(0.0, 0.5), full.right_top() + vec2(0.0, 0.5)], Stroke::new(1.0f32, Color32::from_white_alpha(70)));
    }
    if let Some(text) = text {
        let size = (rect.height() - 1.0).clamp(8.5, 14.0);
        shadowed(painter, pos2(rect.right() - 4.0, rect.center().y), Align2::RIGHT_CENTER, text, FontId::proportional(size), Color32::from_gray(250));
    }
}

/// How round a slot's corners are
pub const SLOT_RADIUS: f32 = 4.0;

/// A slot for something carried, at `rect`: a dark well, tinted and edged in `tint`, its rarity's
/// colour, with a band of it along the foot; ringed in white when it's `picked`, the one in hand
pub fn slot(painter: &Painter, rect: Rect, tint: Option<Color32>, picked: bool) {
    let radius = SLOT_RADIUS;
    painter.rect_filled(rect, radius, Color32::from_black_alpha(if picked { 160 } else { 125 }));
    match tint {
        Some(c) => {
            gradient(painter, rect.shrink2(vec2(2.0, 3.0)), c.gamma_multiply(if picked { 0.12 } else { 0.05 }), c.gamma_multiply(if picked { 0.38 } else { 0.24 }));
            let band = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 3.0), rect.max);
            painter.rect_filled(band, CornerRadius { nw: 0, ne: 0, sw: radius as u8, se: radius as u8 }, c.gamma_multiply(0.9));
            painter.rect_stroke(rect, radius, Stroke::new(1.0f32, c.gamma_multiply(0.85)), StrokeKind::Inside);
        }
        None => {
            painter.rect_filled(rect, radius, Color32::from_white_alpha(if picked { 26 } else { 10 }));
            painter.rect_stroke(rect, radius, Stroke::new(1.0f32, EDGE.gamma_multiply(0.7)), StrokeKind::Inside);
        }
    }
    if picked {
        painter.rect_stroke(rect.expand(2.0), radius + 2.0, Stroke::new(2.0f32, Color32::WHITE), StrokeKind::Inside);
    }
}

/// The ring round a slot whose card is shown, as the one picked in the inventory: white, with a
/// fine second ring outside it, to tell it from the one in hand
pub fn inspected(painter: &Painter, rect: Rect) {
    painter.rect_stroke(rect.expand(2.0), SLOT_RADIUS + 2.0, Stroke::new(2.0f32, ACCENT), StrokeKind::Inside);
    painter.rect_stroke(rect.expand(4.5), SLOT_RADIUS + 4.0, Stroke::new(1.0f32, ACCENT.gamma_multiply(0.55)), StrokeKind::Inside);
}

/// The number of the key that picks a slot, on a little dark tab in its top left corner
pub fn key_number(painter: &Painter, rect: Rect, key: &str) {
    let tab = Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(11.0, 12.0));
    painter.rect_filled(tab, CornerRadius { nw: 3, ne: 1, sw: 1, se: 3 }, Color32::from_black_alpha(160));
    painter.text(tab.center() + vec2(0.0, 0.5), Align2::CENTER_CENTER, key, FontId::proportional(9.5), Color32::from_white_alpha(220));
}

/// A heading `size` big, in the headings' font and colour
pub fn heading(ui: &mut egui::Ui, text: &str, size: f32) {
    ui.label(RichText::new(text).font(display(size)).color(HEADING));
}

/// The name at the head of an item's card, in its rarity's `color`
pub fn card_title(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).font(display(18.0)).color(color));
}

/// A line across a card under its title, in `color` at the left fading out to the right
pub fn rule(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 7.0), Sense::hover());
    let y = rect.center().y - 0.5;
    let mut mesh = egui::Mesh::default();
    for (x, c) in [(rect.left(), color.gamma_multiply(0.9)), (rect.right(), Color32::TRANSPARENT)] {
        mesh.colored_vertex(pos2(x, y), c);
        mesh.colored_vertex(pos2(x, y + 1.0), c);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// A line of a card that's a perk or an effect: `mark` in `accent` before it, `name` in
/// `name_color`, then what it does in `about_color`, wrapping under
pub fn bullet(ui: &mut egui::Ui, mark: &str, accent: Color32, name: &str, name_color: Color32, about: &str, about_color: Color32) {
    let font = TextStyle::Body.resolve(ui.style());
    let mut job = LayoutJob::default();
    job.append(mark, 0.0, TextFormat::simple(font.clone(), accent));
    job.append(name, 5.0, TextFormat::simple(font.clone(), name_color));
    if !about.is_empty() {
        job.append(&format!(": {about}"), 0.0, TextFormat::simple(font, about_color));
    }
    ui.label(job);
}

/// A row of a grid on a card: what it is, weak, then its value
pub fn row(ui: &mut egui::Ui, label: &str, value: RichText) {
    ui.label(RichText::new(label).color(WEAK));
    ui.label(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How light a colour looks, as WCAG reckons it
    fn luminance(c: Color32) -> f64 {
        let f = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    /// How far apart two colours stand, 1 to 21
    fn contrast(a: Color32, b: Color32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// The theme set on a style
    fn themed() -> egui::Style {
        let mut s = egui::Style::default();
        style(&mut s);
        s
    }

    /// Words, weak words, headings, better and worse all read on the panels, over the darkest
    /// and the lightest of what shows through them
    #[test]
    fn words_read_on_the_panels() {
        let fill = panel_fill().to_opaque();
        for (name, c) in [("text", TEXT), ("weak", WEAK), ("heading", HEADING), ("accent", ACCENT), ("good", GOOD), ("bad", BAD), ("info", INFO)] {
            let k = contrast(c, fill);
            assert!(k >= 4.5, "{name} stands {k:.1} to 1 from the panel");
        }
        // The panel's see-through: over white behind, it's still dark enough
        let [r, g, b, a] = panel_fill().to_srgba_unmultiplied();
        let over_white = |v: u8| ((v as f64 * a as f64 + 255.0 * (255.0 - a as f64)) / 255.0) as u8;
        let lit = Color32::from_rgb(over_white(r), over_white(g), over_white(b));
        assert!(contrast(TEXT, lit) >= 4.5, "text over a panel over white stands {:.1} to 1", contrast(TEXT, lit));
        // And a button's words on a button, as it is and hovered, what's picked, and the white
        // button that goes on
        let style = themed();
        for w in [&style.visuals.widgets.inactive, &style.visuals.widgets.hovered, &style.visuals.widgets.active] {
            assert!(contrast(w.fg_stroke.color, w.weak_bg_fill) >= 4.5);
        }
        let picked = &style.visuals.selection;
        assert!(contrast(picked.stroke.color, picked.bg_fill) >= 4.5, "what's picked stands {:.1} to 1", contrast(picked.stroke.color, picked.bg_fill));
        assert!(contrast(TEXT, picked.bg_fill) >= 4.5, "words selected in a text box stand {:.1} to 1", contrast(TEXT, picked.bg_fill));
        assert!(contrast(PRIMARY_INK, PRIMARY) >= 7.0);
    }

    /// Panels' and buttons' edges stand out from them as much as anything to be seen on screen
    /// should, 3 to 1, and a button's edge lights up under the pointer
    #[test]
    fn edges_show() {
        let fill = panel_fill().to_opaque();
        assert!(contrast(EDGE, fill) >= 3.0, "a panel's edge stands {:.1} to 1", contrast(EDGE, fill));
        let w = &themed().visuals.widgets;
        assert!(contrast(w.hovered.bg_stroke.color, fill) > 2.0 * contrast(w.inactive.bg_stroke.color, fill));
    }

    /// It's black and white: the words, edges, panels, buttons and what's picked are all greys,
    /// the colours left being those that say something in play
    #[test]
    fn the_look_is_black_and_white() {
        let grey = |c: Color32| c.r() == c.g() && c.g() == c.b();
        for (name, c) in [("text", TEXT), ("weak", WEAK), ("heading", HEADING), ("accent", ACCENT), ("edge", EDGE), ("primary", PRIMARY), ("ink", PRIMARY_INK), ("panel", panel_fill()), ("hud", hud_fill())] {
            assert!(grey(c), "{name} is {c:?}");
        }
        let v = themed().visuals;
        let w = &v.widgets;
        for (name, c) in [
            ("window edge", v.window_stroke.color),
            ("picked", v.selection.bg_fill),
            ("faint", v.faint_bg_color),
            ("button", w.inactive.bg_fill),
            ("button edge", w.inactive.bg_stroke.color),
            ("hovered", w.hovered.bg_fill),
            ("hovered edge", w.hovered.bg_stroke.color),
            ("pressed", w.active.bg_fill),
            ("open", w.open.bg_fill),
            ("line", w.noninteractive.bg_stroke.color),
        ] {
            assert!(grey(c), "{name} is {c:?}");
        }
        // A bar's colour greyed a little is still the colour it was, health red and mana blue
        let red = muted(Color32::from_rgb(225, 55, 60), BAR_MUTED);
        let blue = muted(Color32::from_rgb(80, 140, 255), BAR_MUTED);
        let [r, b] = [red, blue].map(|c| [c.r() as u16, c.g() as u16, c.b() as u16]);
        assert!(r[0] > 2 * r[1] && r[0] > 2 * r[2], "health is {red:?}");
        assert!(b[2] > 2 * b[0] && b[2] > b[1] + 60, "mana is {blue:?}");
    }

    /// Set on a context, the headings are in Cinzel and the window titles in the blackletter, each
    /// with every letter and number, each of its own, and the rest's as big as it's meant to be
    #[test]
    fn the_theme_sets_the_fonts() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| ui.heading("Innert"));
        });
        assert_eq!(ctx.style().text_styles[&TextStyle::Heading], blackletter(HEADING_SIZE));
        assert_eq!(ctx.style().text_styles[&TextStyle::Body].size, 13.0);
        let letters: String = ('A'..='Z').chain('a'..='z').chain('0'..='9').collect();
        assert!(ctx.fonts(|f| f.has_glyphs(&display(18.0), &letters)));
        assert!(ctx.fonts(|f| f.has_glyphs(&blackletter(18.0), &letters)));
        // Their own: a heading's letters aren't as wide as the body font's, nor the blackletter's
        // as either
        let wide = |text: &str, font: FontId| ctx.fonts(|f| f.layout_no_wrap(text.to_owned(), font, Color32::WHITE).size().x);
        assert_ne!(wide("THE INNERT GATE", display(18.0)), wide("THE INNERT GATE", FontId::proportional(18.0)));
        for other in [display(40.0), FontId::proportional(40.0)] {
            assert_ne!(wide("Innert", blackletter(40.0)), wide("Innert", other));
        }
    }
}
