use crate::{
    appearance::{Appearance, GraphStyle},
    canvas::{self, ACCENT, BG, Canvas, MUTED, Scene},
    context::{self, Bundle, Scope},
    graph::Graph,
    layout::Point,
    source::{self, Event, Worker},
    storage::{self, Settings},
    tray::{Action, Tray},
};
use eframe::egui::{self, Align2, Color32, FontId, Key, Modifiers, RichText, Stroke, Vec2};
use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Launch {
    pub demo: bool,
    pub path: Option<PathBuf>,
    pub snapshot: Option<PathBuf>,
    pub live: Option<String>,
}
pub struct GliaApp {
    pub graph: Arc<Graph>,
    pub positions: Vec<Point>,
    pub selected: Option<usize>,
    pub settings: Settings,
    config_path: PathBuf,
    can_save: bool,
    path: Option<PathBuf>,
    namespace: String,
    worker: Option<Worker>,
    canvas: Canvas,
    enabled_types: BTreeSet<String>,
    enabled_sources: BTreeSet<String>,
    known_types: BTreeSet<String>,
    known_sources: BTreeSet<String>,
    visible: Vec<bool>,
    show_collection: bool,
    births: HashMap<String, f64>,
    layout_from: Vec<Point>,
    layout_target: Vec<Point>,
    layout_started: f64,
    settling: bool,
    fit_pending: bool,
    palette: bool,
    query: String,
    highlight: usize,
    focus_search: bool,
    settings_open: bool,
    appearance_open: bool,
    help_open: bool,
    export_open: bool,
    scope: Scope,
    budget: usize,
    bundle: Option<Bundle>,
    export_rx: Option<mpsc::Receiver<Bundle>>,
    markdown: Option<String>,
    markdown_error: Option<String>,
    notice: Option<String>,
    load_error: Option<String>,
    replay: Option<f32>,
    playing: bool,
    replay_speed: f32,
    filters_open: bool,
    tray: Option<Tray>,
    quit: bool,
    started: Instant,
    last_time: f64,
    snapshot: Option<PathBuf>,
    snapshot_requested: bool,
    snapshot_ready: f64,
    snapshot_configured: bool,
    online: Option<String>,
    checking: bool,
    map_updates: usize,
}
fn panel() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(23, 26, 35))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(43, 47, 63)))
        .corner_radius(12)
        .inner_margin(14)
}
pub fn theme(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/seguisym.ttf") {
        fonts.font_data.insert(
            "windows-symbols".into(),
            Arc::new(egui::FontData::from_owned(bytes)),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("windows-symbols".into());
        }
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = Color32::from_rgb(23, 26, 35);
    style.visuals.extreme_bg_color = Color32::from_rgb(13, 16, 23);
    style.visuals.selection.bg_fill = ACCENT.linear_multiply(0.6);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(34, 38, 51);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(51, 45, 77);
    style.visuals.widgets.active.bg_fill = ACCENT.linear_multiply(0.6);
    style.spacing.item_spacing = Vec2::new(9., 8.);
    style.spacing.button_padding = Vec2::new(10., 6.);
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.));
    ctx.set_style(style);
}
impl GliaApp {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self {
        Self::create(&cc.egui_ctx, launch, true)
    }
    pub fn create(ctx: &egui::Context, launch: Launch, native_tray: bool) -> Self {
        theme(ctx);
        let config_path = storage::state_dir().join("settings.json");
        let (mut settings, can_save, notice) = match Settings::load(&config_path) {
            Ok(s) => (s, true, None),
            Err(e) => (Settings::default(), false, Some(e.to_string())),
        };
        if let Some(host) = launch.live {
            settings.live = true;
            settings.ssh_host = host;
        }
        if launch.demo || launch.path.is_some() {
            settings.live = false;
        }
        let path = if launch.demo {
            None
        } else {
            Some(storage::resolve_graph(
                &launch
                    .path
                    .or_else(|| std::env::var_os("GLIA_DATA").map(Into::into))
                    .or_else(|| settings.graph_path.clone())
                    .unwrap_or_else(storage::default_graph),
            ))
        };
        let namespace = if settings.live {
            format!("aws:{}", settings.ssh_host)
        } else {
            storage::namespace(path.as_deref())
        };
        let tray = if native_tray {
            Tray::new(ctx).ok()
        } else {
            None
        };
        let mut app = Self {
            graph: Arc::new(Graph::default()),
            positions: Vec::new(),
            selected: None,
            settings,
            config_path,
            can_save,
            path,
            namespace,
            worker: None,
            canvas: Canvas::new(ctx),
            enabled_types: BTreeSet::new(),
            enabled_sources: BTreeSet::new(),
            known_types: BTreeSet::new(),
            known_sources: BTreeSet::new(),
            visible: Vec::new(),
            show_collection: false,
            births: HashMap::new(),
            layout_from: Vec::new(),
            layout_target: Vec::new(),
            layout_started: 0.,
            settling: true,
            fit_pending: true,
            palette: false,
            query: String::new(),
            highlight: 0,
            focus_search: false,
            settings_open: false,
            appearance_open: false,
            help_open: false,
            export_open: false,
            scope: Scope::Collection,
            budget: 24000,
            bundle: None,
            export_rx: None,
            markdown: None,
            markdown_error: None,
            notice,
            load_error: None,
            replay: None,
            playing: false,
            replay_speed: 1.,
            filters_open: true,
            tray,
            quit: false,
            started: Instant::now(),
            last_time: 0.,
            snapshot: launch.snapshot,
            snapshot_requested: false,
            snapshot_ready: 0.,
            snapshot_configured: false,
            online: None,
            checking: true,
            map_updates: 0,
        };
        app.start_worker(ctx);
        app
    }
    fn stars(&self) -> BTreeSet<String> {
        self.settings
            .stars
            .get(&self.namespace)
            .cloned()
            .unwrap_or_default()
    }
    fn save_settings(&mut self) {
        if !self.can_save {
            return;
        }
        if let Err(e) = self.settings.save(&self.config_path) {
            self.notice = Some(e.to_string());
        }
    }
    fn cache_path(&self) -> PathBuf {
        storage::state_dir().join("layouts").join(format!(
            "{:016x}.json",
            storage::stable_hash(&self.namespace)
        ))
    }
    fn start_worker(&mut self, ctx: &egui::Context) {
        self.worker.take();
        self.online = None;
        let wake = ctx.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || wake.request_repaint());
        self.worker = Some(if self.settings.live {
            source::spawn_live(
                self.settings.ssh_host.clone(),
                self.settings.poll_seconds,
                self.cache_path(),
                wake,
            )
        } else {
            source::spawn(self.path.clone(), self.cache_path(), wake)
        });
        self.settling = true;
        self.load_error = None;
        self.checking = true;
    }
    fn refresh_map(&mut self) {
        if let Some(worker) = &self.worker {
            self.checking = true;
            worker.refresh();
        }
    }
    fn switch_source(&mut self, path: Option<PathBuf>, ctx: &egui::Context) {
        self.settings.live = false;
        self.path = path.map(|p| storage::resolve_graph(&p));
        self.namespace = storage::namespace(self.path.as_deref());
        self.graph = Arc::new(Graph::default());
        self.positions.clear();
        self.selected = None;
        self.markdown = None;
        self.markdown_error = None;
        self.settings.graph_path = self.path.clone();
        self.save_settings();
        self.replay = None;
        self.playing = false;
        self.known_types.clear();
        self.known_sources.clear();
        self.enabled_types.clear();
        self.enabled_sources.clear();
        self.births.clear();
        self.layout_target.clear();
        self.layout_from.clear();
        self.show_collection = false;
        self.fit_pending = true;
        self.start_worker(ctx);
    }
    fn choose_file(&mut self, ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Abrir arquivo do mapa")
            .add_filter("Mapa de páginas", &["json"])
            .pick_file()
        {
            self.switch_source(Some(p), ctx);
        }
    }
    fn choose_folder(&mut self, ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Escolher pasta do GBrain (contém viz/graph.json)")
            .pick_folder()
        {
            self.switch_source(Some(p), ctx);
        }
    }
    fn receive(&mut self, ctx: &egui::Context, time: f64) {
        let events = self
            .worker
            .as_ref()
            .map(|w| w.rx.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Checking => self.checking = true,
                Event::Current => {
                    self.checking = false;
                    self.load_error = None;
                }
                Event::Warning(message) => self.notice = Some(message),
                Event::Loaded {
                    graph,
                    positions,
                    new_keys,
                } => {
                    let selected_key = self.selected.map(|i| self.graph.nodes[i].key());
                    let first = self.graph.nodes.is_empty();
                    self.map_updates += 1;
                    for key in new_keys {
                        self.births.insert(key, time);
                    }
                    for kind in graph.types.keys() {
                        if !self.known_types.contains(kind) {
                            self.enabled_types.insert(kind.clone());
                        }
                    }
                    for source in graph.sources.keys() {
                        if !self.known_sources.contains(source) {
                            self.enabled_sources.insert(source.clone());
                        }
                    }
                    self.known_types.extend(graph.types.keys().cloned());
                    self.known_sources.extend(graph.sources.keys().cloned());
                    self.graph = graph;
                    self.positions = positions;
                    self.layout_target.clear();
                    self.layout_from.clear();
                    self.selected = selected_key
                        .and_then(|k| self.graph.nodes.iter().position(|n| n.key() == k));
                    self.load_error = None;
                    self.settling = true;
                    if self.selected.is_some() {
                        self.refresh_markdown();
                    }
                    if first {
                        self.fit_pending = true;
                    }
                    self.bundle = None;
                    self.export_rx = None;
                }
                Event::Settled { positions, warning } => {
                    self.layout_from = self.positions.clone();
                    self.layout_target = positions;
                    self.layout_started = time;
                    self.settling = false;
                    if !self.settings.live {
                        self.checking = false;
                    }
                    if let Some(w) = warning {
                        self.notice = Some(w);
                    }
                    self.snapshot_ready = time + 1.2;
                    if let Some(tray) = &self.tray {
                        tray.update(self.graph.nodes.len(), self.updated_today());
                    }
                }
                Event::Error(e) => {
                    self.load_error = Some(e);
                    self.checking = false;
                    self.settling = false;
                    self.snapshot_ready = time + 1.;
                }
                Event::Online(time) => {
                    self.online = Some(time);
                    self.checking = false;
                    self.load_error = None;
                    self.save_settings();
                }
                Event::Page { key, result } => {
                    if self
                        .selected
                        .is_some_and(|i| self.graph.nodes[i].key() == key)
                    {
                        match result {
                            Ok(body) => {
                                self.markdown = Some(body);
                                self.markdown_error = None;
                            }
                            Err(e) => self.markdown_error = Some(e),
                        }
                    }
                }
            }
        }
        if let Some(rx) = &self.export_rx
            && let Ok(bundle) = rx.try_recv()
        {
            self.bundle = Some(bundle);
            self.export_rx = None;
            ctx.request_repaint();
        }
    }
    fn updated_today(&self) -> usize {
        let today = chrono::Local::now().date_naive();
        self.graph
            .nodes
            .iter()
            .filter(|n| crate::graph::parse_day(&n.updated) == Some(today))
            .count()
    }
    fn refresh_markdown(&mut self) {
        self.markdown = None;
        self.markdown_error = None;
        if let Some(i) = self.selected {
            if self.settings.live {
                if let Some(w) = &self.worker {
                    w.page(self.graph.nodes[i].clone());
                }
            } else if self.path.is_none() {
                self.markdown = Some(format!(
                    "# {}\n\nPágina de exemplo da demonstração do AGI Brain.\n\nExplore suas conexões, adicione aos favoritos ou reproduza o crescimento. Conecte ao seu GBrain para ler e exportar suas próprias notas.",
                    self.graph.nodes[i].display_title()
                ));
            } else {
                match context::read_page(&self.settings, self.path.as_deref(), &self.graph.nodes[i])
                {
                    Ok(s) => self.markdown = Some(s),
                    Err(e) => self.markdown_error = Some(e.to_string()),
                }
            }
        }
    }
    pub fn select(&mut self, i: usize, fly: bool) {
        if i >= self.graph.nodes.len() {
            self.selected = None;
            self.markdown = None;
            return;
        }
        self.selected = Some(i);
        self.refresh_markdown();
        if fly && i < self.positions.len() {
            let p = self.positions[i];
            self.canvas.camera.fly(
                Vec2::new(p.x, p.y),
                self.canvas.camera.zoom.max(2.5),
                self.settings.reduce_motion,
            );
        }
    }
    pub fn toggle_star(&mut self) {
        if let Some(i) = self.selected {
            let key = self.graph.nodes[i].key();
            let stars = self
                .settings
                .stars
                .entry(self.namespace.clone())
                .or_default();
            if !stars.remove(&key) {
                stars.insert(key);
            }
            self.bundle = None;
            self.save_settings();
        }
    }
    fn update_visibility(&mut self) {
        let stars = self.stars();
        let day = self.replay.and_then(|p| self.graph.cursor_day(p));
        self.visible = self
            .graph
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                self.enabled_types.contains(&n.kind)
                    && self.enabled_sources.contains(&n.source)
                    && (!self.settings.hide_unlinked || self.graph.degree[i] > 0)
                    && (!self.show_collection || stars.contains(&n.key()))
                    && day.is_none_or(|cursor| n.day().is_none_or(|d| d <= cursor))
            })
            .collect();
    }
    pub fn start_replay(&mut self) {
        self.selected = None;
        self.replay = Some(0.);
        self.playing = true;
    }
    fn open_search(&mut self) {
        self.palette = true;
        self.focus_search = true;
        self.highlight = 0;
    }
    fn commit_search(&mut self) {
        let hits = self.graph.search(&self.query, 60);
        if let Some(&i) = hits.get(self.highlight) {
            let n = &self.graph.nodes[i];
            self.enabled_types.insert(n.kind.clone());
            self.enabled_sources.insert(n.source.clone());
            self.settings.hide_unlinked = false;
            self.show_collection = false;
            self.replay = None;
            self.playing = false;
            self.select(i, true);
            self.palette = false;
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| {
            i.consume_key(Modifiers::CTRL, Key::K) || i.consume_key(Modifiers::CTRL, Key::F)
        }) {
            self.open_search();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::O)) {
            self.choose_file(ctx);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::D)) {
            self.toggle_star();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Comma)) {
            self.settings_open = true;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if self.palette {
                self.palette = false;
            } else if self.export_open {
                self.export_open = false;
            } else if self.settings_open {
                self.settings_open = false;
            } else {
                self.selected = None;
            }
        }
        if !ctx.wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Home)) {
                self.fit_pending = true;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) {
                if self.replay.is_none() {
                    self.start_replay();
                } else {
                    self.playing = !self.playing;
                }
            }
        }
    }
    fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("◉").size(22.).color(ACCENT));
                    ui.label(RichText::new(crate::locale::APP_NAME).size(19.).strong());
                    if self.settings.live {
                        ui.label(
                            RichText::new(format!(
                                "AWS · {}",
                                if self.load_error.is_some() {
                                    "sem conexão"
                                } else if self.checking {
                                    "verificando mudanças"
                                } else {
                                    self.online.as_deref().unwrap_or("conectando")
                                }
                            ))
                            .small()
                            .color(
                                if self.online.is_some() && self.load_error.is_none() {
                                    Color32::from_rgb(99, 210, 154)
                                } else {
                                    MUTED
                                },
                            ),
                        );
                    } else if self.path.is_none()
                        && ui
                            .add(egui::Button::new(
                                RichText::new("DEMO ×")
                                    .size(10.)
                                    .color(Color32::from_rgb(239, 187, 100)),
                            ))
                            .clicked()
                    {
                        self.switch_source(Some(storage::default_graph()), ctx);
                    }
                    ui.add_space(15.);
                    if !self.graph.nodes.is_empty() && ctx.content_rect().width() >= 1050. {
                        for (count, label) in [
                            (self.graph.nodes.len(), "páginas"),
                            (self.graph.links.len(), "conexões"),
                            (self.updated_today(), "hoje"),
                            (self.visible.iter().filter(|v| **v).count(), "visíveis"),
                        ] {
                            ui.label(RichText::new(count.to_string()).strong());
                            ui.label(RichText::new(label).color(MUTED).size(11.));
                            ui.add_space(5.);
                        }
                    }
                    if self.settling {
                        ui.spinner();
                        ui.label(RichText::new("organizando").small().color(MUTED));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Buscar   Ctrl K").clicked() {
                            self.open_search();
                        }
                        if ui
                            .button("⋯")
                            .on_hover_text("Configurações e atalhos")
                            .clicked()
                        {
                            self.settings_open = true;
                        }
                        if ui.button("Exportar contexto").clicked() {
                            self.export_open = true;
                            if self.stars().is_empty() {
                                self.scope = Scope::Identity;
                            }
                            self.build_export(ctx);
                        }
                        if ui
                            .button("Aparência")
                            .on_hover_text("Estilo Gephi ou Constelação e ajustes do mapa")
                            .clicked()
                        {
                            self.appearance_open = true;
                        }
                        ui.menu_button("Abrir", |ui| {
                            if ui.button("Conectar ao GBrain na AWS…").clicked() {
                                ui.close();
                                self.settings_open = true;
                            }
                            if ui.button("Arquivo do mapa…   Ctrl O").clicked() {
                                ui.close();
                                self.choose_file(ctx);
                            }
                            if ui.button("Pasta do GBrain…").clicked() {
                                ui.close();
                                self.choose_folder(ctx);
                            }
                            if ui.button("Explorar demonstração").clicked() {
                                ui.close();
                                self.switch_source(None, ctx);
                            }
                            if ui.button("Recarregar").clicked() {
                                ui.close();
                                self.start_worker(ctx);
                            }
                        });
                    });
                });
            });
    }
    fn filters(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        egui::Area::new("filters".into())
            .default_size(Vec2::new(235., (screen.height() - 150.).max(220.)))
            .fixed_pos(egui::pos2(
                screen.left() + 16.,
                self.canvas.rect.top() + 16.,
            ))
            .show(ctx, |ui| {
                // Refresh the available height even when an older panel size was persisted.
                ui.set_max_height((screen.height() - 150.).max(220.));
                panel().show(ui, |ui| {
                    ui.set_width(205.);
                    if ui
                        .add(
                            egui::Button::new(if self.filters_open {
                                "▾  Filtros"
                            } else {
                                "▸  Filtros"
                            })
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.filters_open = !self.filters_open;
                    }
                    if self.filters_open {
                        let max_height = (screen.height() - 250.).max(120.);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(max_height)
                            .show(ui, |ui| {
                                if ui
                                    .checkbox(
                                        &mut self.settings.hide_unlinked,
                                        "Ocultar sem conexões",
                                    )
                                    .changed()
                                {
                                    self.save_settings();
                                }
                                let star_count = self.stars().len();
                                ui.checkbox(
                                    &mut self.show_collection,
                                    format!("Só favoritos ({star_count})"),
                                );
                                ui.separator();
                                if self.graph.sources.len() > 1 {
                                    ui.label(RichText::new("FONTES").color(MUTED).size(9.));
                                    let sources = self.graph.sources.clone();
                                    for source in sources.keys() {
                                        let mut on = self.enabled_sources.contains(source);
                                        if ui.checkbox(&mut on, source).changed() {
                                            if on {
                                                self.enabled_sources.insert(source.clone());
                                            } else {
                                                self.enabled_sources.remove(source);
                                            }
                                        }
                                    }
                                    ui.separator();
                                }
                                ui.label(RichText::new("TIPOS DE PÁGINA").color(MUTED).size(9.));
                                let types = self.graph.types.clone();
                                for (kind, count) in types {
                                    let mut on = self.enabled_types.contains(&kind);
                                    ui.horizontal(|ui| {
                                        let color = canvas::type_color(&kind);
                                        let (swatch, _) = ui.allocate_exact_size(
                                            Vec2::splat(8.),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().circle_filled(swatch.center(), 4., color);
                                        if ui
                                            .checkbox(
                                                &mut on,
                                                RichText::new(crate::locale::type_label(&kind))
                                                    .color(color),
                                            )
                                            .on_hover_text(&kind)
                                            .changed()
                                        {
                                            if on {
                                                self.enabled_types.insert(kind.clone());
                                            } else {
                                                self.enabled_types.remove(&kind);
                                            }
                                        }
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(count.to_string())
                                                        .size(10.)
                                                        .color(MUTED),
                                                );
                                            },
                                        );
                                    });
                                }
                                ui.separator();
                                if ui.button("Ajustar mapa   Home").clicked() {
                                    self.fit_pending = true;
                                }
                                if ui.button("Mostrar tudo").clicked() {
                                    self.enabled_types = self.known_types.clone();
                                    self.enabled_sources = self.known_sources.clone();
                                    self.show_collection = false;
                                    self.settings.hide_unlinked = false;
                                    self.fit_pending = true;
                                }
                            });
                    }
                });
            });
    }
    fn inspector(&mut self, ctx: &egui::Context) {
        let Some(i) = self.selected else {
            return;
        };
        let node = self.graph.nodes[i].clone();
        let stars = self.stars();
        let neighbors = self.graph.neighbors[i].clone();
        let screen = ctx.content_rect();
        egui::Area::new("inspector".into())
            .anchor(
                Align2::RIGHT_TOP,
                Vec2::new(-16., self.canvas.rect.top() - screen.top() + 16.),
            )
            .show(ctx, |ui| {
                panel().show(ui, |ui| {
                    ui.set_width(284.);
                    ui.horizontal_top(|ui| {
                        ui.add_sized(
                            [220., 42.],
                            egui::Label::new(
                                RichText::new(node.display_title()).strong().size(15.),
                            )
                            .wrap(),
                        );
                        if ui
                            .button(if stars.contains(&node.key()) {
                                "★"
                            } else {
                                "☆"
                            })
                            .on_hover_text("Favoritar página   Ctrl D")
                            .clicked()
                        {
                            self.toggle_star();
                        }
                        if ui.small_button("×").clicked() {
                            self.selected = None;
                        }
                    });
                    ui.label(
                        RichText::new(&node.slug)
                            .monospace()
                            .size(10.)
                            .color(ACCENT),
                    );
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(crate::locale::type_label(&node.kind))
                                .color(canvas::type_color(&node.kind)),
                        );
                        ui.label(RichText::new(&node.source).small().color(MUTED));
                    });
                    egui::Grid::new("node-metadata")
                        .spacing([16., 5.])
                        .show(ui, |ui| {
                            for (k, v) in [
                                ("criada em", node.created.as_str()),
                                (
                                    "atualizada em",
                                    node.updated.get(..10).unwrap_or(&node.updated),
                                ),
                            ] {
                                ui.label(RichText::new(k).color(MUTED).size(11.));
                                ui.label(RichText::new(v).size(11.));
                                ui.end_row();
                            }
                            ui.label(RichText::new("conexões").color(MUTED).size(11.));
                            ui.label(self.graph.degree[i].to_string());
                            ui.end_row();
                        });
                    egui::ScrollArea::vertical()
                        .id_salt("inspector-scroll")
                        .max_height((screen.height() - 310.).max(150.))
                        .show(ui, |ui| {
                            ui.separator();
                            ui.label(RichText::new("PÁGINA").size(9.).color(MUTED));
                            if let Some(markdown) = &self.markdown {
                                render_markdown(ui, markdown);
                            } else {
                                ui.label(
                                    RichText::new(
                                        if self.settings.live && self.markdown_error.is_none() {
                                            "Lendo página da AWS…"
                                        } else {
                                            "Página indisponível."
                                        },
                                    )
                                    .small()
                                    .color(MUTED),
                                );
                                if let Some(e) = &self.markdown_error {
                                    ui.label(RichText::new(e).size(10.).color(MUTED));
                                }
                            }
                            if !neighbors.is_empty() {
                                ui.add_space(6.);
                                ui.separator();
                                ui.label(RichText::new("CONECTADAS").size(9.).color(MUTED));
                                for j in neighbors.iter().take(100) {
                                    let n = &self.graph.nodes[*j];
                                    let kind = n.kind.clone();
                                    let title = n.display_title();
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(format!("●  {title}"))
                                                    .size(12.)
                                                    .color(canvas::type_color(&kind)),
                                            )
                                            .frame(false)
                                            .wrap(),
                                        )
                                        .clicked()
                                    {
                                        self.select(*j, true);
                                    }
                                }
                                if neighbors.len() > 100 {
                                    ui.label(
                                        RichText::new(format!(
                                            "mais {} — use a busca para explorar",
                                            neighbors.len() - 100
                                        ))
                                        .small()
                                        .color(MUTED),
                                    );
                                }
                            }
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if self.path.is_some() && ui.button("Abrir no Obsidian ↗").clicked() {
                            ctx.open_url(egui::OpenUrl::new_tab(context::obsidian_url(
                                &self.settings.vault,
                                &node.slug,
                            )));
                        }
                        if ui.small_button("Copiar identificador").clicked() {
                            ctx.copy_text(node.slug.clone());
                        }
                    });
                });
            });
    }
    fn replay_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("replay-bar")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if self.settings.live {
                            "AWS · AO VIVO · SOMENTE LEITURA"
                        } else if self.path.is_none() {
                            "DEMONSTRAÇÃO"
                        } else {
                            "LOCAL · ATUALIZAÇÃO AO VIVO"
                        })
                        .size(9.)
                        .color(MUTED),
                    );
                    if self.path.is_some() || self.settings.live {
                        if ui
                            .add_enabled(
                                !self.checking,
                                egui::Button::new("Detectar mudanças e atualizar o mapa"),
                            )
                            .clicked()
                        {
                            self.refresh_map();
                        }
                        if self.checking {
                            ui.spinner();
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("?").clicked() {
                            self.help_open = true;
                        }
                    });
                });
                if self.graph.date_range().is_none() {
                    return;
                }
                ui.horizontal(|ui| {
                    if let Some(mut progress) = self.replay {
                        if ui
                            .button(if self.playing {
                                "Ⅱ  Pausar"
                            } else {
                                "▶  Reproduzir"
                            })
                            .clicked()
                        {
                            if progress >= 1. {
                                progress = 0.;
                            }
                            self.playing = !self.playing;
                        }
                        let width = (ui.available_width() - 280.).max(120.);
                        let (r, response) = ui.allocate_exact_size(
                            Vec2::new(width, 32.),
                            egui::Sense::click_and_drag(),
                        );
                        let counts = self.graph.histogram(70);
                        let peak = counts.iter().max().copied().unwrap_or(1).max(1) as f32;
                        let painter = ui.painter_at(r);
                        for (i, count) in counts.iter().enumerate() {
                            let x = r.left() + i as f32 * r.width() / 70.;
                            let h = 2. + (*count as f32 / peak) * 25.;
                            let on = i as f32 / 69. <= progress;
                            painter.rect_filled(
                                egui::Rect::from_min_max(
                                    egui::pos2(x, r.bottom() - h),
                                    egui::pos2(x + r.width() / 70. - 1., r.bottom()),
                                ),
                                1.,
                                if on {
                                    ACCENT
                                } else {
                                    Color32::from_rgb(50, 55, 70)
                                },
                            );
                        }
                        let x = r.left() + r.width() * progress;
                        painter.line_segment(
                            [egui::pos2(x, r.top()), egui::pos2(x, r.bottom())],
                            Stroke::new(2.0_f32, Color32::WHITE),
                        );
                        if (response.dragged() || response.clicked())
                            && let Some(p) = response.interact_pointer_pos()
                        {
                            progress = ((p.x - r.left()) / r.width()).clamp(0., 1.);
                            self.playing = false;
                        }
                        // The standard slider supplies keyboard and accessibility support.
                        if ui
                            .add(
                                egui::Slider::new(&mut progress, 0.0..=1.0)
                                    .show_value(false)
                                    .trailing_fill(true),
                            )
                            .changed()
                        {
                            self.playing = false;
                        }
                        if let Some(day) = self.graph.cursor_day(progress) {
                            ui.label(
                                RichText::new(day.format("%d/%m/%Y").to_string())
                                    .monospace()
                                    .size(11.),
                            );
                        }
                        if ui.button("Ao vivo ×").clicked() {
                            self.replay = None;
                            self.playing = false;
                        } else {
                            self.replay = Some(progress);
                        }
                    } else {
                        let available = ui.available_width();
                        ui.add_space((available / 2. - 80.).max(0.));
                        if ui.button("▶  Reproduzir crescimento").clicked() {
                            self.start_replay();
                        }
                    }
                });
            });
    }
    fn search_palette(&mut self, ctx: &egui::Context) {
        if !self.palette {
            return;
        }
        let mut open = true;
        egui::Window::new("Buscar no GBrain")
            .id("palette".into())
            .anchor(Align2::CENTER_TOP, Vec2::new(0., 100.))
            .collapsible(false)
            .resizable(false)
            .default_width(500.)
            .open(&mut open)
            .show(ctx, |ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Título, identificador, tipo, fonte…")
                        .desired_width(f32::INFINITY),
                );
                if self.focus_search {
                    field.request_focus();
                    self.focus_search = false;
                }
                if field.changed() {
                    self.highlight = 0;
                }
                let hits = self.graph.search(&self.query, 60);
                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                    self.highlight = (self.highlight + 1).min(hits.len().saturating_sub(1));
                }
                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                    self.highlight = self.highlight.saturating_sub(1);
                }
                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                    self.commit_search();
                }
                ui.separator();
                if hits.is_empty() {
                    ui.label(RichText::new("Nenhuma página encontrada.").color(MUTED));
                }
                egui::ScrollArea::vertical()
                    .max_height(330.)
                    .show(ui, |ui| {
                        for (rank, &i) in hits.iter().enumerate() {
                            let n = &self.graph.nodes[i];
                            let response = ui.selectable_label(
                                rank == self.highlight,
                                RichText::new(format!(
                                    "●  {}\n     {} · {}",
                                    n.display_title(),
                                    n.slug,
                                    crate::locale::type_label(&n.kind)
                                ))
                                .color(canvas::type_color(&n.kind)),
                            );
                            if rank == self.highlight {
                                response.scroll_to_me(Some(egui::Align::Center));
                            }
                            if response.clicked() {
                                self.highlight = rank;
                                self.commit_search();
                            }
                        }
                    });
                ui.separator();
                ui.label(
                    RichText::new("↑ ↓ escolher   ·   Enter ir à página   ·   Esc fechar")
                        .small()
                        .color(MUTED),
                );
            });
        if !open {
            self.palette = false;
        }
    }
    fn build_export(&mut self, ctx: &egui::Context) {
        let g = self.graph.clone();
        let settings = self.settings.clone();
        let path = self.path.clone();
        let scope = self.scope;
        let selected = self.selected;
        let stars = self.stars();
        let budget = self.budget;
        let (tx, rx) = mpsc::channel();
        self.export_rx = Some(rx);
        self.bundle = None;
        let wake = ctx.clone();
        std::thread::spawn(move || {
            let b = context::build(
                &g,
                &settings,
                path.as_deref(),
                scope,
                selected,
                &stars,
                budget,
            );
            let _ = tx.send(b);
            wake.request_repaint();
        });
    }
    fn export_window(&mut self, ctx: &egui::Context) {
        if !self.export_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Exportar contexto do GBrain").open(&mut open).default_width(600.).default_height(540.).show(ctx,|ui|{
            ui.label("Escolha as páginas para compartilhar com um agente. O conteúdo vem do GBrain conectado ou das suas pastas locais.");
            let previous=self.scope;
            egui::ComboBox::from_label("Páginas").selected_text(self.scope.label()).show_ui(ui,|ui|{for scope in Scope::ALL{ui.selectable_value(&mut self.scope,scope,scope.label());}});
            let changed=ui.add(egui::DragValue::new(&mut self.budget).range(100..=250000).speed(100.).prefix("Limite aproximado de tokens: ")).changed();
            if previous!=self.scope||changed{self.build_export(ctx);}
            if self.export_rx.is_some(){ui.spinner();ui.label("Lendo páginas…");}
            if let Some(b)=&self.bundle{
                if let Some(error)=&b.error{ui.colored_label(Color32::YELLOW,error);}
                ui.label(RichText::new(format!("{} páginas · cerca de {} tokens · {} indisponíveis{}",b.pages,b.tokens(),b.missing,if b.capped{" · limite atingido"}else{""})).color(MUTED));
                if b.pages==0{ui.label("Conecte ao GBrain ou escolha as pastas de páginas em Configurações para exportar conteúdo.");}
                egui::ScrollArea::vertical().max_height(330.).show(ui,|ui|{ui.add(egui::Label::new(RichText::new(&b.text).monospace().size(11.)).wrap().selectable(true));});
                ui.separator();let can_export=b.pages>0;let text=b.text.clone();
                ui.horizontal(|ui|{
                    if ui.add_enabled(can_export,egui::Button::new("Copiar contexto")).clicked(){ctx.copy_text(text.clone());self.notice=Some("Contexto copiado.".into());}
                    if ui.add_enabled(can_export,egui::Button::new("Salvar como…")).clicked()&& let Some(path)=rfd::FileDialog::new().set_file_name("contexto-agi-brain.md").add_filter("Markdown",&["md"]).save_file(){self.notice=Some(match storage::atomic_write(&path,text.as_bytes()){Ok(())=>format!("Salvo em {}",path.display()),Err(e)=>e.to_string()});}
                    if ui.add_enabled(can_export,egui::Button::new("Exportar identidade para MCP")).clicked(){let path=storage::psyche_path();self.notice=Some(match storage::atomic_write(&path,text.as_bytes()){Ok(())=>format!("Identidade exportada para {}",path.display()),Err(e)=>e.to_string()});}
                });ui.label(RichText::new("O agi-brain-context.exe compartilha a identidade que você exportar. A conexão com outros agentes é configurada por você.").small().color(MUTED));
            }
        });
        self.export_open = open;
    }
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut dirty = false;
        egui::Window::new("Configurações").id("settings".into()).open(&mut open).default_width(510.).show(ctx,|ui|{
            ui.heading("GBrain na AWS ao vivo");
            ui.label("Usa o acesso SSH já configurado no Windows para ler páginas e conexões de forma privada.");
            ui.horizontal(|ui|{ui.label("Servidor SSH");dirty|=ui.text_edit_singleline(&mut self.settings.ssh_host).changed();});
            dirty|=ui.add(egui::DragValue::new(&mut self.settings.poll_seconds).range(5..=300).prefix("Atualizar a cada (segundos): ")).changed();
            if ui.add_enabled(crate::remote::valid_host(&self.settings.ssh_host),egui::Button::new("Conectar ao vivo")).clicked(){
                let host=self.settings.ssh_host.clone();self.switch_source(Some(storage::default_graph()),ctx);self.settings.live=true;self.namespace=format!("aws:{host}");self.start_worker(ctx);
            }
            ui.separator();
            ui.heading("Seu GBrain");
            ui.label(RichText::new(self.path.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||"Demonstração incluída".into())).monospace().size(11.).color(MUTED));
            ui.horizontal(|ui|{if ui.button("Escolher mapa…").clicked(){self.choose_file(ctx);}
if ui.button("Escolher pasta…").clicked(){self.choose_folder(ctx);}});
            ui.separator();ui.label("Pastas locais de páginas (Markdown, somente leitura)");
            let sources:Vec<_>=self.graph.sources.keys().cloned().collect();
            for source in sources{
                ui.horizontal(|ui|{ui.label(&source);let path=context::mirror(&self.settings,self.path.as_deref(),&source);
                    ui.label(RichText::new(path.map(|p|p.display().to_string()).unwrap_or_else(||"Não configurado".into())).size(10.).color(MUTED));
                    if ui.small_button("Escolher…").clicked()&& let Some(p)=rfd::FileDialog::new().set_title(format!("Pasta de páginas da fonte {source}")).pick_folder(){self.settings.mirrors.insert(source.clone(),p);dirty=true;self.refresh_markdown();}
                });
            }
            ui.separator();egui::Grid::new("settings-fields").min_col_width(220.).show(ui,|ui|{
                ui.label("Pasta do Obsidian");dirty|=ui.text_edit_singleline(&mut self.settings.vault).changed();ui.end_row();
                ui.label("Identificador da sua página de identidade");dirty|=ui.text_edit_singleline(&mut self.settings.self_slug).changed();ui.end_row();
            });
            ui.separator();dirty|=ui.checkbox(&mut self.settings.reduce_motion,"Reduzir animações").changed();dirty|=ui.checkbox(&mut self.settings.show_labels,"Mostrar títulos no mapa").changed();
            if self.tray.is_some(){dirty|=ui.checkbox(&mut self.settings.minimize_to_tray,"Ao fechar, manter na área de notificações").changed();}
            ui.separator();ui.label(RichText::new(format!("Configurações e posições do mapa: {}",storage::state_dir().display())).small().color(MUTED));
            if !self.can_save{ui.colored_label(Color32::YELLOW,"Não foi possível ler as configurações existentes. O arquivo foi preservado.");if ui.button("Substituir o arquivo inválido por estas configurações").clicked(){self.can_save=true;dirty=true;}}
            ui.horizontal(|ui|{if ui.button("Atalhos de teclado").clicked(){self.help_open=true;}
if ui.button("Sair do AGI Brain").clicked(){self.quit=true;ctx.send_viewport_cmd(egui::ViewportCommand::Close);}});
            ui.hyperlink_to("Glia original, por David Raphael · MIT","https://github.com/draphael89/glia");
        });
        self.settings_open = open;
        if dirty {
            self.save_settings();
        }
    }
    fn appearance_window(&mut self, ctx: &egui::Context) {
        if !self.appearance_open {
            return;
        }
        let mut open = true;
        let mut dirty = false;
        egui::Window::new("Aparência do mapa").id("appearance".into()).open(&mut open).resizable(false).show(ctx, |ui| {
            ui.set_width(340.);
            let a = &mut self.settings.appearance;
            ui.horizontal(|ui| {
                ui.label("Estilo");
                for style in [GraphStyle::Gephi, GraphStyle::Constellation] {
                    dirty |= ui.selectable_value(&mut a.style, style, style.label()).changed();
                }
            });
            ui.label("As cores identificam os tipos de página em todos os estilos.");
            ui.separator();
            dirty |= ui.add(egui::Slider::new(&mut a.node_size, 0.5..=3.).text("Tamanho dos pontos")).changed();
            dirty |= ui.checkbox(&mut a.size_by_degree, "Páginas com mais conexões ficam maiores").changed();
            ui.separator();
            dirty |= ui.add(egui::Slider::new(&mut a.edge_opacity, 0.0..=1.).text("Visibilidade das conexões")).changed();
            dirty |= ui.add(egui::Slider::new(&mut a.edge_width, 0.2..=3.).text("Espessura das conexões")).changed();
            ui.add_enabled_ui(a.style == GraphStyle::Gephi, |ui| {
                dirty |= ui.checkbox(&mut a.colored_edges, "Conexões com as cores das páginas").changed();
            });
            ui.separator();
            dirty |= ui.checkbox(&mut self.settings.show_labels, "Mostrar títulos no mapa").changed();
            ui.add_enabled_ui(self.settings.show_labels, |ui| {
                dirty |= ui.add(egui::Slider::new(&mut a.label_size, 8.0..=18.).text("Tamanho dos títulos")).changed();
                dirty |= ui.add(egui::Slider::new(&mut a.label_limit, 10..=150).text("Limite de títulos")).changed();
            });
            ui.label(RichText::new("Títulos de páginas mais conectadas têm prioridade. A aproximação e os filtros ajudam a ler o mapa.").small().color(MUTED));
            if ui.button("Restaurar aparência deste estilo").clicked() {
                *a = Appearance { style: a.style, ..Appearance::default() };
                self.settings.show_labels = true;
                dirty = true;
            }
        });
        self.appearance_open = open;
        if dirty {
            self.save_settings();
        }
    }
    fn messages(&mut self, ctx: &egui::Context) {
        if self.graph.nodes.is_empty() && !self.settling {
            egui::Area::new("empty".into())
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    panel().show(ui, |ui| {
                        ui.set_width(480.);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("◉").size(54.).color(ACCENT));
                            ui.heading(if self.load_error.is_some() {
                                "Nenhum GBrain conectado"
                            } else {
                                "Este GBrain ainda não tem páginas"
                            });
                            ui.label("Explore tudo o que seu agente sabe.");
                            ui.add_space(8.);
                            if let Some(path) = &self.path {
                                ui.label(
                                    RichText::new(path.display().to_string())
                                        .monospace()
                                        .color(MUTED)
                                        .size(11.),
                                );
                            }
                            ui.add_space(10.);
                            ui.horizontal(|ui| {
                                if ui.button("Escolher pasta do GBrain…").clicked() {
                                    self.choose_folder(ctx);
                                }
                                if ui.button("Explorar demonstração").clicked() {
                                    self.switch_source(None, ctx);
                                }
                                if ui.button("Tentar novamente").clicked() {
                                    self.start_worker(ctx);
                                }
                            });
                            if let Some(e) = &self.load_error {
                                ui.add_space(8.);
                                ui.label(RichText::new(e).size(10.).color(MUTED));
                            }
                        });
                    });
                });
        } else if let Some(error) = self.load_error.clone() {
            egui::Area::new("load-warning".into())
                .anchor(Align2::CENTER_BOTTOM, Vec2::new(0., -95.))
                .show(ctx, |ui| {
                    panel().show(ui, |ui| {
                        ui.set_max_width(560.);
                        ui.colored_label(
                            Color32::YELLOW,
                            "A atualização falhou. Exibindo o último mapa válido.",
                        );
                        ui.label(RichText::new(error).size(11.));
                        if ui.button("Tentar novamente agora").clicked() {
                            self.refresh_map();
                        }
                    });
                });
        }
        if self.settings.live && self.checking && self.online.is_none() && self.load_error.is_none()
        {
            egui::Area::new("connecting".into())
                .anchor(Align2::CENTER_BOTTOM, Vec2::new(0., -95.))
                .show(ctx, |ui| {
                    panel().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(if self.graph.nodes.is_empty() {
                                "Conectando ao GBrain na AWS…"
                            } else {
                                "Conectando à AWS. Exibindo o último mapa salvo enquanto verifico mudanças."
                            });
                        });
                    });
                });
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if let Some(message) = self.notice.clone() {
            egui::Area::new("notice".into())
                .anchor(Align2::CENTER_BOTTOM, Vec2::new(0., -125.))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    panel().show(ui, |ui| {
                        ui.set_max_width(550.);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(message);
                            if ui.small_button("×").clicked() {
                                self.notice = None;
                            }
                        });
                    });
                });
        }
        if self.help_open {
            egui::Window::new("Teclado e mouse")
                .default_width(540.)
                .min_width(540.)
                .open(&mut self.help_open)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.set_width(540.);
                    egui::Grid::new("shortcuts")
                        .spacing([26., 8.])
                        .show(ui, |ui| {
                            for (key, action) in [
                                ("Ctrl K / Ctrl F", "Buscar páginas"),
                                ("↑ ↓ / Enter", "Escolher resultado e ir à página"),
                                ("Ctrl O", "Abrir arquivo do mapa"),
                                ("Ctrl D", "Favoritar página selecionada"),
                                ("Ctrl ,", "Configurações"),
                                ("Home", "Ajustar mapa visível"),
                                ("Space", "Reproduzir / pausar"),
                                ("Esc", "Fechar painel ou limpar seleção"),
                                ("Roda / pinça", "Ampliar ao redor do ponteiro"),
                                ("Arrastar fundo", "Mover o mapa"),
                                ("Clique / clique duplo", "Destacar conexões / aproximar"),
                            ] {
                                ui.monospace(key);
                                ui.label(action);
                                ui.end_row();
                            }
                        });
                });
        }
    }
    fn snapshot(&mut self, ctx: &egui::Context, time: f64) {
        let Some(path) = self.snapshot.clone() else {
            return;
        };
        let shots = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });
        for image in shots {
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let saved = image::save_buffer(
                &path,
                &bytes,
                image.size[0] as u32,
                image.size[1] as u32,
                image::ColorType::Rgba8,
            );
            if let Err(ref e) = saved {
                self.notice = Some(format!("Falha na captura: {e}"));
            }
            let report = serde_json::json!({"app_name":crate::locale::APP_NAME,"language":crate::locale::LANGUAGE,"appearance":self.settings.appearance,"appearance_open":self.appearance_open,"drawn_labels":self.canvas.drawn_labels,"pages":self.graph.nodes.len(),"links":self.graph.links.len(),"visible":self.visible.iter().filter(|b|**b).count(),"drawn_nodes":self.canvas.drawn_nodes,"drawn_edges":self.canvas.drawn_edges,"selected":self.selected.map(|i|self.graph.nodes[i].slug.clone()),"replay":self.replay,"settling":self.settling,"error":self.load_error,"screenshot_saved":saved.is_ok(),"live":self.settings.live,"last_sync":self.online,"checking":self.checking,"map_updates":self.map_updates,"page_chars":self.markdown.as_ref().map(|s|s.chars().count()).unwrap_or(0),"page_error":self.markdown_error,"palette":self.palette,"export_pages":self.bundle.as_ref().map(|b|b.pages),"export_error":self.bundle.as_ref().and_then(|b|b.error.clone()),"tray":self.tray.is_some()});
            let _ = storage::atomic_write(
                &path.with_extension("json"),
                &serde_json::to_vec_pretty(&report).unwrap(),
            );
            self.quit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if !self.settling && !self.snapshot_configured && !self.graph.nodes.is_empty() {
            self.snapshot_configured = true;
            match std::env::var("GLIA_SNAPSHOT_PANEL").as_deref() {
                Ok("settings") => self.settings_open = true,
                Ok("help") => self.help_open = true,
                Ok("appearance") => self.appearance_open = true,
                _ => {}
            }
            if let Ok(style) = std::env::var("GLIA_SNAPSHOT_STYLE")
                && let Ok(style) = serde_json::from_value(serde_json::Value::String(style))
            {
                self.settings.appearance.style = style;
            }
            if let Ok(slug) = std::env::var("GLIA_SNAPSHOT_FOCUS")
                && let Some(i) = self.graph.nodes.iter().position(|n| n.slug == slug)
            {
                self.select(i, false);
            }
            if let Ok(p) = std::env::var("GLIA_SNAPSHOT_REPLAY") {
                self.replay = p.parse::<f32>().ok().map(|p| p.clamp(0., 1.));
                self.playing = false;
            }
            if let Ok(query) = std::env::var("GLIA_SNAPSHOT_SEARCH") {
                self.query = query;
                self.open_search();
            }
            if std::env::var_os("GLIA_SNAPSHOT_EXPORT").is_some() {
                self.scope = Scope::Selection;
                self.export_open = true;
                self.build_export(ctx);
            }
            if std::env::var_os("GLIA_SNAPSHOT_REFRESH").is_some() {
                self.refresh_map();
            }
            self.snapshot_ready = time + 1.2;
        }
        if !self.settling
            && !self.checking
            && time > self.snapshot_ready
            && self.snapshot_ready > 0.
            && !self.snapshot_requested
            && self.export_rx.is_none()
            && (!self.settings.live || self.online.is_some() || time > 40.)
            && (!(self.settings.live && self.selected.is_some())
                || self.markdown.is_some()
                || self.markdown_error.is_some()
                || time > 40.)
        {
            self.snapshot_requested = true;
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }
    pub fn draw(&mut self, ctx: &egui::Context) {
        let time = self.started.elapsed().as_secs_f64();
        let dt = (time - self.last_time).clamp(0., 0.1) as f32;
        self.last_time = time;
        self.receive(ctx, time);
        if let Some(tray) = &self.tray {
            for action in tray.actions() {
                match action {
                    Action::Show => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    Action::Replay => {
                        self.start_replay();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    }
                    Action::Quit => {
                        self.quit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quit
            && self.settings.minimize_to_tray
            && self.tray.is_some()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        self.shortcuts(ctx);
        if self.playing {
            if let Some(p) = &mut self.replay {
                *p = (*p + dt * self.replay_speed / 25.).min(1.);
                if *p >= 1. {
                    self.playing = false;
                }
            }
            ctx.request_repaint();
        }
        let mut animation = false;
        if !self.layout_target.is_empty() {
            let t = if self.settings.reduce_motion {
                1.
            } else {
                ((time - self.layout_started) / 0.7).clamp(0., 1.) as f32
            };
            let ease = t * t * (3. - 2. * t);
            self.positions = self
                .layout_from
                .iter()
                .zip(&self.layout_target)
                .map(|(a, b)| Point::new(a.x + (b.x - a.x) * ease, a.y + (b.y - a.y) * ease))
                .collect();
            if t >= 1. {
                self.layout_target.clear();
                self.layout_from.clear();
            } else {
                animation = true;
            }
        }
        if self.settings.reduce_motion {
            self.births.clear();
        }
        self.births.retain(|_, t| time - *t < 0.8);
        if !self.births.is_empty() && !self.settings.reduce_motion {
            animation = true;
        }
        let previously_visible = self.visible.clone();
        self.update_visibility();
        if self.replay.is_some() && !self.settings.reduce_motion {
            for (i, on) in self.visible.iter().enumerate() {
                if *on && !previously_visible.get(i).copied().unwrap_or(false) {
                    self.births.insert(self.graph.nodes[i].key(), time);
                    animation = true;
                }
            }
        }
        self.toolbar(ctx);
        self.replay_bar(ctx);
        let stars = self.stars();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                if self.fit_pending
                    && self.positions.len() == self.graph.nodes.len()
                    && !self.settling
                    && self.layout_target.is_empty()
                {
                    self.canvas.camera.fit(
                        &self.positions,
                        &self.visible,
                        ui.available_rect_before_wrap(),
                        self.settings.reduce_motion || self.snapshot.is_some(),
                    );
                    self.fit_pending = false;
                }
                animation |= self.canvas.camera.animate(dt);
                if let Some(i) = self.canvas.show(
                    ui,
                    Scene {
                        graph: &self.graph,
                        positions: &self.positions,
                        visible: &self.visible,
                        selected: self.selected,
                        stars: &stars,
                        labels: self.settings.show_labels,
                        appearance: &self.settings.appearance,
                        bloom: &self.births,
                        time,
                    },
                ) {
                    self.select(i, false);
                }
            });
        if !self.graph.nodes.is_empty() {
            self.filters(ctx);
            self.inspector(ctx);
        }
        self.messages(ctx);
        self.search_palette(ctx);
        self.settings_window(ctx);
        self.appearance_window(ctx);
        self.export_window(ctx);
        self.snapshot(ctx, time);
        if animation {
            ctx.request_repaint();
        }
    }
}
impl eframe::App for GliaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save_settings();
    }
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }
}

/// Render Markdown without interpreting HTML or activating remote resources.
fn render_markdown(ui: &mut egui::Ui, text: &str) {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut paragraph = String::new();
    let mut heading = false;
    let mut code = false;
    let flush = |ui: &mut egui::Ui, paragraph: &mut String, heading: bool, code: bool| {
        if paragraph.trim().is_empty() {
            paragraph.clear();
            return;
        }
        let mut rich = RichText::new(std::mem::take(paragraph));
        if heading {
            rich = rich.strong().size(14.);
        } else if code {
            rich = rich.monospace().size(11.);
        } else {
            rich = rich.size(12.).color(Color32::from_rgb(188, 195, 210));
        }
        ui.add(egui::Label::new(rich).wrap().selectable(true));
    };
    for e in Parser::new(text) {
        match e {
            Event::Start(Tag::Heading { .. }) => heading = true,
            Event::Start(Tag::CodeBlock(_)) => code = true,
            Event::Start(Tag::Item) => paragraph.push_str("• "),
            Event::End(
                TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item | TagEnd::CodeBlock,
            ) => {
                flush(ui, &mut paragraph, heading, code);
                heading = false;
                code = false;
            }
            Event::Text(t) | Event::Code(t) => paragraph.push_str(&t),
            Event::SoftBreak | Event::HardBreak => paragraph.push('\n'),
            Event::Rule => {
                ui.separator();
            }
            _ => {}
        }
    }
    flush(ui, &mut paragraph, heading, code);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_file_updates_preserve_selection_camera_favorites_and_disabled_types() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("graph.json");
        let initial = br#"{"nodes":[{"id":1,"slug":"old","type":"note"},{"id":2,"slug":"hidden","type":"concept"}],"links":[{"source":1,"target":2}]}"#;
        std::fs::write(&path, initial).unwrap();
        let ctx = egui::Context::default();
        let mut app = GliaApp::create(
            &ctx,
            Launch {
                demo: true,
                ..Default::default()
            },
            false,
        );
        app.can_save = false;
        app.settings.live = false;
        app.settings.reduce_motion = true;
        app.settings.hide_unlinked = false;
        app.worker.take();
        app.worker = Some(source::spawn(
            Some(path.clone()),
            d.path().join("layout/cache.json"),
            Arc::new(|| {}),
        ));
        let drive = |app: &mut GliaApp, updates: usize| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(1200., 800.),
                        )),
                        ..Default::default()
                    },
                    |ctx| app.draw(ctx),
                );
                assert!(app.load_error.is_none());
                if app.map_updates >= updates && !app.settling && app.layout_target.is_empty() {
                    break;
                }
                assert!(Instant::now() < deadline, "map update did not reach the UI");
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        drive(&mut app, 1);
        app.enabled_types.remove("concept");
        app.selected = Some(0);
        app.settings.stars.insert(
            app.namespace.clone(),
            BTreeSet::from(["default:old".into()]),
        );
        app.canvas.camera.pan(Vec2::new(40., 30.));
        let camera = app.canvas.camera.center;
        let old_position = app.positions[0];
        crate::storage::atomic_write(&path, br#"{"nodes":[{"id":3,"slug":"new","type":"person"},{"id":1,"slug":"old","type":"note"}],"links":[{"source":1,"target":3}]}"#).unwrap();
        drive(&mut app, 2);
        assert_eq!(app.graph.nodes[app.selected.unwrap()].slug, "old");
        assert_eq!(app.positions[1], old_position);
        assert_eq!(app.canvas.camera.center, camera);
        assert!(app.stars().contains("default:old"));
        assert!(app.enabled_types.contains("person"));
        crate::storage::atomic_write(&path, initial).unwrap();
        drive(&mut app, 3);
        assert!(
            !app.visible[1],
            "a returning type must retain its disabled filter"
        );
        assert_eq!(app.graph.nodes[app.selected.unwrap()].slug, "old");
        assert!(!app.checking);
    }
    #[test]
    fn native_ui_search_filter_focus_replay_and_star_work_together() {
        let ctx = egui::Context::default();
        let mut app = GliaApp::create(
            &ctx,
            Launch {
                demo: true,
                ..Default::default()
            },
            false,
        );
        app.can_save = false;
        app.settings.stars.clear();
        app.worker.take();
        app.graph = Arc::new(Graph::demo());
        app.positions = crate::layout::seed(&app.graph, &Default::default());
        app.settling = false;
        app.known_types = app.graph.types.keys().cloned().collect();
        app.known_sources = app.graph.sources.keys().cloned().collect();
        app.enabled_types = app.known_types.clone();
        app.enabled_sources = app.known_sources.clone();
        app.query = "Ada Lovelace".into();
        app.commit_search();
        assert_eq!(
            app.graph.nodes[app.selected.unwrap()].slug,
            "people/ada-lovelace"
        );
        app.toggle_star();
        assert_eq!(app.stars().len(), 1);
        app.show_collection = true;
        app.update_visibility();
        assert_eq!(app.visible.iter().filter(|b| **b).count(), 1);
        app.show_collection = false;
        app.start_replay();
        app.update_visibility();
        assert!(app.visible.iter().filter(|b| **b).count() < app.graph.nodes.len());
        app.replay = Some(1.);
        app.update_visibility();
        assert!(app.visible.iter().all(|b| *b));
        for _ in 0..3 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1360., 900.),
                    )),
                    ..Default::default()
                },
                |ctx| app.draw(ctx),
            );
        }
        assert!(app.canvas.drawn_nodes > 0);
        app.query = "nothing-matches-this".into();
        app.commit_search();
    }
}
