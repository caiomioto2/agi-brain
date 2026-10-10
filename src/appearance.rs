use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphStyle {
    Gephi,
    #[default]
    Constellation,
}
impl GraphStyle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gephi => "Gephi",
            Self::Constellation => "Constelação",
        }
    }
    pub fn background(self) -> Color32 {
        match self {
            Self::Gephi => Color32::WHITE,
            Self::Constellation => crate::canvas::BG,
        }
    }
    pub fn text_color(self) -> Color32 {
        match self {
            Self::Gephi => Color32::from_rgb(35, 39, 48),
            Self::Constellation => Color32::from_rgb(172, 181, 200),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub style: GraphStyle,
    pub node_size: f32,
    pub edge_opacity: f32,
    pub edge_width: f32,
    pub label_size: f32,
    pub label_limit: usize,
    pub size_by_degree: bool,
    pub colored_edges: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            style: GraphStyle::Constellation,
            node_size: 1.,
            edge_opacity: 0.32,
            edge_width: 0.7,
            label_size: 10.,
            label_limit: 60,
            size_by_degree: true,
            colored_edges: true,
        }
    }
}
impl Appearance {
    pub fn normalize(&mut self) {
        fn bounded(value: f32, low: f32, high: f32, default: f32) -> f32 {
            if value.is_finite() {
                value.clamp(low, high)
            } else {
                default
            }
        }
        self.node_size = bounded(self.node_size, 0.5, 3., 1.);
        self.edge_opacity = bounded(self.edge_opacity, 0., 1., 0.32);
        self.edge_width = bounded(self.edge_width, 0.2, 3., 0.7);
        self.label_size = bounded(self.label_size, 8., 18., 10.);
        self.label_limit = self.label_limit.clamp(10, 150);
    }
    pub fn radius(&self, degree: usize, zoom: f32) -> f32 {
        let degree = if self.size_by_degree {
            degree as f32
        } else {
            0.
        };
        let base = 2.5 + degree.sqrt().min(12.) * 0.6;
        (base * zoom.sqrt() * self.node_size).clamp(1.25, 32.)
    }
}
