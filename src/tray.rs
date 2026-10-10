use eframe::egui;
#[derive(Clone, Copy)]
pub enum Action {
    Show,
    Replay,
    Quit,
}
#[cfg(windows)]
pub struct Tray {
    icon: tray_icon::TrayIcon,
    rx: std::sync::mpsc::Receiver<Action>,
    status: tray_icon::menu::MenuItem,
}
#[cfg(not(windows))]
pub struct Tray;
impl Tray {
    #[cfg(windows)]
    pub fn new(ctx: &egui::Context) -> anyhow::Result<Self> {
        use tray_icon::{
            TrayIconBuilder, TrayIconEvent,
            menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
        };
        let menu = Menu::new();
        let show = MenuItem::new("Abrir AGI Brain", true, None);
        let replay = MenuItem::new("Reproduzir crescimento", true, None);
        let status = MenuItem::new("AGI Brain — carregando", false, None);
        let quit = MenuItem::new("Sair do AGI Brain", true, None);
        menu.append_items(&[
            &status,
            &PredefinedMenuItem::separator(),
            &show,
            &replay,
            &PredefinedMenuItem::separator(),
            &quit,
        ])?;
        let (tx, rx) = std::sync::mpsc::channel();
        let ids = (show.id().clone(), replay.id().clone(), quit.id().clone());
        let wake = ctx.clone();
        let menu_tx = tx.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let a = if e.id == ids.0 {
                Action::Show
            } else if e.id == ids.1 {
                Action::Replay
            } else if e.id == ids.2 {
                Action::Quit
            } else {
                return;
            };
            let _ = menu_tx.send(a);
            wake.request_repaint();
        }));
        let wake = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if matches!(e, TrayIconEvent::DoubleClick { .. }) {
                let _ = tx.send(Action::Show);
                wake.request_repaint();
            }
        }));
        let icon = crate::canvas::icon();
        let icon = tray_icon::Icon::from_rgba(icon.rgba, icon.width, icon.height)?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("AGI Brain — seu GBrain")
            .build()?;
        Ok(Self { icon, rx, status })
    }
    #[cfg(not(windows))]
    pub fn new(_ctx: &egui::Context) -> anyhow::Result<Self> {
        anyhow::bail!("Tray is Windows-only")
    }
    #[cfg(windows)]
    pub fn update(&self, pages: usize, today: usize) {
        let text = format!("{pages} páginas · {today} atualizadas hoje");
        self.status.set_text(&text);
        let _ = self.icon.set_tooltip(Some(&format!("AGI Brain — {text}")));
    }
    #[cfg(not(windows))]
    pub fn update(&self, _pages: usize, _today: usize) {}
    #[cfg(windows)]
    pub fn actions(&self) -> Vec<Action> {
        self.rx.try_iter().collect()
    }
    #[cfg(not(windows))]
    pub fn actions(&self) -> Vec<Action> {
        Vec::new()
    }
}
