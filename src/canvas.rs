use crate::{
    appearance::{Appearance, GraphStyle},
    graph::Graph,
    layout::Point,
};
use eframe::egui::{self, Color32, Mesh, Pos2, Rect, Stroke, TextureHandle, Vec2};
use std::collections::BTreeSet;

pub const BG: Color32 = Color32::from_rgb(11, 13, 18);
pub const ACCENT: Color32 = Color32::from_rgb(124, 92, 255);
pub const MUTED: Color32 = Color32::from_rgb(125, 137, 158);
pub fn type_color(kind: &str) -> Color32 {
    match kind {
        "person" => Color32::from_rgb(82, 214, 138),
        "company" => Color32::from_rgb(255, 158, 79),
        "concept" => Color32::from_rgb(230, 199, 79),
        "note" => Color32::from_rgb(224, 89, 115),
        "atom" => Color32::from_rgb(158, 140, 255),
        "source" => Color32::from_rgb(91, 210, 199),
        "extract_receipt" => Color32::from_rgb(79, 194, 194),
        "original" => Color32::from_rgb(217, 140, 255),
        "meeting" => Color32::from_rgb(79, 168, 255),
        _ => {
            // Stable, saturated colors also cover new/custom types without a gray fallback.
            let hue = (crate::storage::stable_hash(kind) % 360) as f32 / 360.;
            egui::ecolor::Hsva::new(hue, 0.70, 0.95, 1.).into()
        }
    }
}
pub fn icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../assets/agi-brain-icon.png"))
        .expect("bundled AGI Brain icon must be a valid PNG")
        .into_rgba8();
    egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

#[derive(Default)]
pub struct Camera {
    pub center: Vec2,
    pub zoom: f32,
    target: Option<(Vec2, f32)>,
}
impl Camera {
    pub fn new() -> Self {
        Self {
            zoom: 1.,
            ..Self::default()
        }
    }
    pub fn screen(&self, p: Point, rect: Rect) -> Pos2 {
        rect.center() + (Vec2::new(p.x, p.y) - self.center) * self.zoom
    }
    pub fn fit(&mut self, positions: &[Point], visible: &[bool], rect: Rect, instant: bool) {
        let mut lo = Vec2::splat(f32::INFINITY);
        let mut hi = Vec2::splat(f32::NEG_INFINITY);
        for (p, on) in positions.iter().zip(visible) {
            if *on {
                lo = lo.min(Vec2::new(p.x, p.y));
                hi = hi.max(Vec2::new(p.x, p.y));
            }
        }
        if !lo.is_finite() {
            return;
        }
        let size = (hi - lo).max(Vec2::splat(40.));
        let usable = (rect.size() - Vec2::new(430., 150.)).max(Vec2::splat(180.));
        let zoom = (usable.x / size.x).min(usable.y / size.y).clamp(0.025, 12.);
        self.fly((lo + hi) * 0.5, zoom, instant);
    }
    pub fn fly(&mut self, center: Vec2, zoom: f32, instant: bool) {
        if instant {
            self.center = center;
            self.zoom = zoom;
            self.target = None;
        } else {
            self.target = Some((center, zoom));
        }
    }
    pub fn animate(&mut self, dt: f32) -> bool {
        if let Some((c, z)) = self.target {
            let t = 1. - (-dt.clamp(0., 0.05) * 10.).exp();
            self.center += (c - self.center) * t;
            self.zoom += (z - self.zoom) * t;
            if (self.center - c).length() < 0.08 && (self.zoom - z).abs() < 0.002 {
                self.center = c;
                self.zoom = z;
                self.target = None;
            }
        }
        self.target.is_some()
    }
    pub fn zoom_at(&mut self, pointer: Pos2, rect: Rect, factor: f32) {
        self.target = None;
        let before = self.center + (pointer - rect.center()) / self.zoom;
        self.zoom = (self.zoom * factor).clamp(0.01, 35.);
        self.center = before - (pointer - rect.center()) / self.zoom;
    }
    pub fn pan(&mut self, delta: Vec2) {
        self.target = None;
        self.center -= delta / self.zoom;
    }
}
pub struct Canvas {
    texture: TextureHandle,
    pub camera: Camera,
    pub rect: Rect,
    pub hovered: Option<usize>,
    pub drawn_nodes: usize,
    pub drawn_edges: usize,
    pub drawn_labels: usize,
}
pub struct Scene<'a> {
    pub graph: &'a Graph,
    pub positions: &'a [Point],
    pub visible: &'a [bool],
    pub selected: Option<usize>,
    pub stars: &'a BTreeSet<String>,
    pub labels: bool,
    pub appearance: &'a Appearance,
    pub bloom: &'a std::collections::HashMap<String, f64>,
    pub time: f64,
}
impl Canvas {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut pixels = Vec::new();
        let size = 64;
        for y in 0..size {
            for x in 0..size {
                let dx = (x as f32 + 0.5 - size as f32 / 2.) / (size as f32 / 2.);
                let dy = (y as f32 + 0.5 - size as f32 / 2.) / (size as f32 / 2.);
                let d = (dx * dx + dy * dy).sqrt();
                let core = ((0.43 - d) * 30.).clamp(0., 1.);
                let glow = (-d * d * 8.).exp() * 0.28 * (1. - d).max(0.);
                let a = (core + glow).clamp(0., 1.);
                let highlight = ((0.3 - (dx + 0.1).hypot(dy + 0.16)) * 3.).clamp(0., 1.);
                let value = (0.75 + highlight * 0.25) * 255.;
                pixels.push(Color32::from_rgba_unmultiplied(
                    value as u8,
                    value as u8,
                    255,
                    (a * 255.) as u8,
                ));
            }
        }
        let texture = ctx.load_texture(
            "glia-orb",
            egui::ColorImage::new([size, size], pixels),
            egui::TextureOptions::LINEAR,
        );
        Self {
            texture,
            camera: Camera::new(),
            rect: Rect::NOTHING,
            hovered: None,
            drawn_nodes: 0,
            drawn_edges: 0,
            drawn_labels: 0,
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, scene: Scene<'_>) -> Option<usize> {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        self.rect = rect;
        let painter = ui.painter_at(rect);
        let appearance = scene.appearance;
        let solid = appearance.style == GraphStyle::Gephi;
        painter.rect_filled(rect, 0, appearance.style.background());
        self.drawn_nodes = 0;
        self.drawn_edges = 0;
        self.drawn_labels = 0;
        self.hovered = None;
        if response.dragged() {
            self.camera.pan(ui.input(|i| i.pointer.delta()));
        }
        if response.hovered() {
            let (scroll, multi_zoom, pointer) = ui.input(|i| {
                (
                    i.smooth_scroll_delta.y,
                    i.zoom_delta(),
                    i.pointer.hover_pos(),
                )
            });
            if let Some(pointer) = pointer {
                if scroll != 0. {
                    self.camera.zoom_at(pointer, rect, (scroll * 0.002).exp());
                }
                if multi_zoom != 1. {
                    self.camera.zoom_at(pointer, rect, multi_zoom);
                }
            }
        }
        let g = scene.graph;
        let positions = scene.positions;
        if g.nodes.is_empty() || positions.len() != g.nodes.len() {
            return None;
        }
        let mut focused = vec![true; g.nodes.len()];
        if let Some(i) = scene.selected {
            focused.fill(false);
            focused[i] = true;
            for j in &g.neighbors[i] {
                focused[*j] = true;
            }
        }
        let screen: Vec<_> = positions
            .iter()
            .map(|p| self.camera.screen(*p, rect))
            .collect();
        let birth: Vec<_> = g
            .nodes
            .iter()
            .map(|n| {
                scene
                    .bloom
                    .get(&n.key())
                    .map(|t| ((scene.time - t) / 0.7).clamp(0., 1.) as f32)
                    .unwrap_or(1.)
            })
            .collect();
        // Edges are batched in one GPU mesh. At distant zoom, deterministically
        // sample dense ambient edges; focus always retains every adjacent edge.
        let stride = if self.camera.zoom < 0.45 {
            g.pairs.len().div_ceil(18000).max(1)
        } else {
            1
        };
        let mut edges = Mesh::default();
        self.drawn_edges = 0;
        for (i, &(s, t)) in g.pairs.iter().enumerate() {
            if appearance.edge_opacity == 0. {
                break;
            }
            if !scene.visible[s] || !scene.visible[t] {
                continue;
            }
            // Both endpoints must have visibly bloomed before their line appears.
            let edge_birth = ((birth[s].min(birth[t]) - 0.35) / 0.65).clamp(0., 1.);
            if edge_birth == 0. {
                continue;
            }
            let active = scene.selected.is_some_and(|j| s == j || t == j);
            if !active && i % stride != 0 {
                continue;
            }
            let a = screen[s];
            let b = screen[t];
            if !Rect::from_two_pos(a, b).intersects(rect) {
                continue;
            }
            let color = if active {
                Color32::from_rgba_unmultiplied(151, 136, 226, 145)
            } else if scene.selected.is_some() {
                Color32::from_rgba_unmultiplied(105, 116, 141, 12)
            } else {
                Color32::from_rgba_unmultiplied(111, 127, 161, 48)
            };
            let opacity = if scene.selected.is_some() && !active {
                0.06
            } else if active {
                1.8
            } else {
                1.
            };
            let color_at = |node: usize| {
                if solid {
                    let base = if appearance.colored_edges {
                        type_color(&g.nodes[node].kind)
                    } else {
                        Color32::from_rgb(105, 116, 141)
                    };
                    Color32::from_rgba_unmultiplied(
                        base.r(),
                        base.g(),
                        base.b(),
                        (255. * appearance.edge_opacity * opacity * edge_birth) as u8,
                    )
                } else {
                    color.linear_multiply(appearance.edge_opacity / 0.32 * edge_birth)
                }
            };
            let delta = b - a;
            let normal = Vec2::new(-delta.y, delta.x).normalized()
                * appearance.edge_width
                * if active { 0.95 } else { 0.5 };
            let v = edges.vertices.len() as u32;
            for (p, node) in [
                (a + normal, s),
                (a - normal, s),
                (b - normal, t),
                (b + normal, t),
            ] {
                edges.colored_vertex(p, color_at(node));
            }
            edges.add_triangle(v, v + 1, v + 2);
            edges.add_triangle(v, v + 2, v + 3);
            self.drawn_edges += 1;
        }
        painter.add(egui::Shape::mesh(edges));
        let mut nodes = Mesh::with_texture(self.texture.id());
        self.hovered = None;
        self.drawn_nodes = 0;
        let pointer = if response.hovered() {
            ui.input(|i| i.pointer.hover_pos())
        } else {
            None
        };
        let mut closest = f32::INFINITY;
        let mut label_candidates = Vec::new();
        for (i, n) in g.nodes.iter().enumerate() {
            if !scene.visible[i] || birth[i] == 0. {
                continue;
            }
            let p = screen[i];
            let radius = appearance.radius(g.degree[i], self.camera.zoom);
            if !rect.expand(radius * 3.).contains(p) {
                continue;
            }
            let birth = birth[i];
            let alpha = if focused[i] { 1. } else { 0.13 };
            let color = type_color(&n.kind).linear_multiply(alpha * birth);
            if solid {
                let r = radius * (0.35 + 0.65 * birth);
                painter.circle(
                    p,
                    r,
                    color,
                    Stroke::new(
                        0.45_f32,
                        Color32::from_black_alpha((150. * alpha * birth) as u8),
                    ),
                );
            } else {
                let r = radius * (0.35 + 0.65 * birth) * 2.4;
                nodes.add_rect_with_uv(
                    Rect::from_center_size(p, Vec2::splat(r * 2.)),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                    color,
                );
            }
            self.drawn_nodes += 1;
            if let Some(pointer) = pointer {
                let d = p.distance(pointer);
                if d < radius + 6. && d < closest {
                    closest = d;
                    self.hovered = Some(i);
                }
            }
            if birth >= 0.35 && scene.stars.contains(&n.key()) && focused[i] {
                painter.circle_stroke(
                    p,
                    radius + 3.,
                    Stroke::new(0.8_f32, Color32::from_rgb(239, 204, 113)),
                );
            }
            if birth >= 0.35 && scene.selected == Some(i) {
                painter.circle_stroke(p, radius + 5., Stroke::new(1.5_f32, ACCENT));
            }
            if scene.labels
                && birth >= 0.35
                && focused[i]
                && ((solid || self.camera.zoom > 0.7) && g.degree[i] > 2
                    || scene.selected == Some(i))
            {
                label_candidates.push((
                    scene.selected != Some(i),
                    std::cmp::Reverse(g.degree[i]),
                    i,
                    p,
                    radius,
                ));
            }
        }
        painter.add(egui::Shape::mesh(nodes));
        label_candidates.sort_unstable_by_key(|(selected, d, i, _, _)| (*selected, *d, *i));
        let mut occupied: Vec<Rect> = Vec::new();
        for (_, _, i, p, r) in label_candidates
            .into_iter()
            .take(appearance.label_limit * 4)
        {
            let galley = painter.layout_no_wrap(
                g.nodes[i].display_title(),
                egui::FontId::proportional(appearance.label_size),
                appearance.style.text_color(),
            );
            let pos = p + Vec2::new(r + 7., -galley.size().y / 2.);
            let bounds = Rect::from_min_size(pos, galley.size());
            if !rect.contains_rect(bounds)
                || occupied.iter().any(|b| b.intersects(bounds.expand(4.)))
            {
                continue;
            }
            occupied.push(bounds);
            painter.galley(pos, galley, Color32::WHITE);
            self.drawn_labels += 1;
            if occupied.len() >= appearance.label_limit {
                break;
            }
        }
        if let Some(i) = self.hovered {
            let p = screen[i];
            painter.circle_stroke(
                p,
                appearance.radius(g.degree[i], self.camera.zoom) + 3.,
                Stroke::new(1.0_f32, appearance.style.text_color()),
            );
            response.clone().on_hover_text(format!(
                "{}\n{} · {} conexões",
                g.nodes[i].display_title(),
                crate::locale::type_label(&g.nodes[i].kind),
                g.degree[i]
            ));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.double_clicked()
            && let Some(i) = self.hovered
        {
            self.camera
                .fly(Vec2::new(positions[i].x, positions[i].y), 4., false);
            return Some(i);
        }
        if response.clicked() {
            if let Some(i) = self.hovered {
                return Some(i);
            }
            return Some(usize::MAX);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_dots_are_visible_before_their_edges_in_both_styles() {
        let graph = Graph::parse(br#"{"nodes":[{"id":1,"slug":"old"},{"id":2,"slug":"new"}],"links":[{"source":1,"target":2}]}"#).unwrap();
        let positions = [Point::new(-50., 0.), Point::new(50., 0.)];
        let ctx = egui::Context::default();
        let mut canvas = Canvas::new(&ctx);
        let bloom = std::collections::HashMap::from([(graph.nodes[1].key(), 10.)]);
        for style in [GraphStyle::Gephi, GraphStyle::Constellation] {
            let appearance = Appearance {
                style,
                ..Default::default()
            };
            for (time, dots, lines) in [(10., 1, 0), (10.15, 2, 0), (10.7, 2, 1)] {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            canvas.show(
                                ui,
                                Scene {
                                    graph: &graph,
                                    positions: &positions,
                                    visible: &[true; 2],
                                    selected: None,
                                    stars: &BTreeSet::new(),
                                    labels: true,
                                    appearance: &appearance,
                                    bloom: &bloom,
                                    time,
                                },
                            );
                        });
                    },
                );
                assert_eq!((canvas.drawn_nodes, canvas.drawn_edges), (dots, lines));
            }
        }
    }
    #[test]
    fn styles_preserve_graph_and_filters_while_controls_change_rendering() {
        let graph = Graph::parse(br#"{"nodes":[{"id":1,"slug":"hub","type":"concept"},{"id":2,"slug":"left","type":"note"},{"id":3,"slug":"right","type":"person"},{"id":4,"slug":"bottom","type":"custom"}],"links":[{"source":1,"target":2},{"source":1,"target":3},{"source":1,"target":4}]}"#).unwrap();
        let positions = [
            Point::new(0., 0.),
            Point::new(-100., 0.),
            Point::new(100., 0.),
            Point::new(0., 100.),
        ];
        let ctx = egui::Context::default();
        let mut canvas = Canvas::new(&ctx);
        let mut appearance = Appearance::default();
        assert!(appearance.radius(100, 1.) > appearance.radius(1, 1.));
        appearance.size_by_degree = false;
        assert_eq!(appearance.radius(100, 1.), appearance.radius(1, 1.));
        let stars = BTreeSet::from([graph.nodes[0].key()]);
        let bloom = Default::default();
        let render =
            |canvas: &mut Canvas, appearance: &Appearance, visible: &[bool], labels: bool| {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200., 800.))),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            canvas.show(
                                ui,
                                Scene {
                                    graph: &graph,
                                    positions: &positions,
                                    visible,
                                    selected: Some(0),
                                    stars: &stars,
                                    labels,
                                    appearance,
                                    bloom: &bloom,
                                    time: 1.,
                                },
                            );
                        });
                    },
                );
            };
        for style in [GraphStyle::Gephi, GraphStyle::Constellation] {
            appearance.style = style;
            render(&mut canvas, &appearance, &[true; 4], true);
            assert_eq!(canvas.drawn_nodes, 4);
            assert_eq!(canvas.drawn_edges, 3);
            assert!(canvas.drawn_labels > 0);
        }
        appearance.edge_opacity = 0.;
        render(&mut canvas, &appearance, &[true, true, true, false], false);
        assert_eq!(canvas.drawn_nodes, 3);
        assert_eq!(canvas.drawn_edges, 0);
        assert_eq!(canvas.drawn_labels, 0);
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.pairs.len(), 3);
        assert_eq!(positions[1].x, -100.);
        assert!(stars.contains(&graph.nodes[0].key()));
    }
    #[test]
    fn custom_types_have_stable_saturated_colors_before_selection() {
        for kind in [
            "analysis",
            "architecture",
            "archive",
            "biblio",
            "conversation",
            "daily",
            "project",
            "new-custom-type",
            "",
        ] {
            let color = type_color(kind);
            assert_eq!(color, type_color(kind));
            let [r, g, b, a] = color.to_array();
            assert_eq!(a, 255);
            let channels = [r, g, b];
            assert!(*channels.iter().max().unwrap() - *channels.iter().min().unwrap() > 80);
            // WCAG contrast against the actual dark panel background.
            let luminance = |c: Color32| {
                let linear = |v: u8| {
                    let v = v as f32 / 255.;
                    if v <= 0.04045 {
                        v / 12.92
                    } else {
                        ((v + 0.055) / 1.055).powf(2.4)
                    }
                };
                linear(c.r()) * 0.2126 + linear(c.g()) * 0.7152 + linear(c.b()) * 0.0722
            };
            assert!(
                (luminance(color) + 0.05) / (luminance(Color32::from_rgb(23, 26, 35)) + 0.05)
                    >= 4.5
            );
        }
    }
    #[test]
    fn zoom_anchors_pointer_and_fit_handles_empty_or_single_graph() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200., 800.));
        let mut c = Camera::new();
        let p = Point::new(50., 70.);
        let pointer = c.screen(p, rect);
        c.zoom_at(pointer, rect, 2.);
        assert!(c.screen(p, rect).distance(pointer) < 0.001);
        c.fit(&[p], &[true], rect, true);
        assert!(c.zoom.is_finite());
        let z = c.zoom;
        c.fit(&[], &[], rect, true);
        assert_eq!(c.zoom, z);
    }
}
