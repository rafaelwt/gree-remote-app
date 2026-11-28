use std::sync::mpsc::Sender;

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ToggleWindow,
    TogglePower,
    Refresh,
    Quit,
}

#[derive(Debug, Clone)]
pub struct TrayState {
    pub power: Option<bool>,
    pub set_temp: Option<i64>,
    pub room_temp: Option<i64>,
    pub device_name: String,
    pub window_visible: bool,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            power: None,
            set_temp: None,
            room_temp: None,
            device_name: "Gree Remote".to_string(),
            window_visible: true,
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use ksni::blocking::TrayMethods;
    use ksni::menu::StandardItem;

    struct GreeTray {
        tx: Sender<TrayAction>,
        ctx: egui::Context,
        state: TrayState,
        icon_pixmap: Vec<ksni::Icon>,
    }

    impl GreeTray {
        fn send(&self, action: TrayAction) {
            let _ = self.tx.send(action);
            let on_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
            if matches!(action, TrayAction::ToggleWindow) {
                let show = !self.state.window_visible;
                self.ctx
                    .send_viewport_cmd(egui::ViewportCommand::Visible(show));
                if on_wayland {
                    self.ctx
                        .send_viewport_cmd(egui::ViewportCommand::Minimized(!show));
                }
                if show {
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            } else if matches!(action, TrayAction::Quit) {
                self.ctx
                    .send_viewport_cmd(egui::ViewportCommand::Visible(true));
                if on_wayland {
                    self.ctx
                        .send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                }
            }
            self.ctx.request_repaint();
        }
    }

    impl ksni::Tray for GreeTray {
        fn id(&self) -> String {
            "gree-remote-app".to_string()
        }

        fn title(&self) -> String {
            match (self.state.power, self.state.set_temp) {
                (Some(true), Some(t)) => format!("Gree Remote · Encendido · {t}°C"),
                (Some(false), _) => "Gree Remote · Apagado".to_string(),
                _ => "Gree Remote".to_string(),
            }
        }

        fn icon_name(&self) -> String {
            "gree-remote-app".to_string()
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            self.icon_pixmap.clone()
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            let mut desc = self.state.device_name.clone();
            if let Some(r) = self.state.room_temp {
                desc.push_str(&format!(" · Ambiente {r}°C"));
            }
            ksni::ToolTip {
                title: self.title(),
                description: desc,
                ..Default::default()
            }
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            self.send(TrayAction::ToggleWindow);
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            let show_label = if self.state.window_visible {
                "Ocultar ventana"
            } else {
                "Mostrar ventana"
            };
            let power_label = match self.state.power {
                Some(true) => "Apagar aire",
                Some(false) => "Encender aire",
                None => "Encender / Apagar aire",
            };

            vec![
                StandardItem {
                    label: show_label.to_string(),
                    activate: Box::new(|this: &mut Self| this.send(TrayAction::ToggleWindow)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: power_label.to_string(),
                    enabled: self.state.power.is_some(),
                    activate: Box::new(|this: &mut Self| this.send(TrayAction::TogglePower)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Actualizar estado".to_string(),
                    activate: Box::new(|this: &mut Self| this.send(TrayAction::Refresh)),
                    ..Default::default()
                }
                .into(),
                ksni::MenuItem::Separator,
                StandardItem {
                    label: "Salir".to_string(),
                    activate: Box::new(|this: &mut Self| this.send(TrayAction::Quit)),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    fn load_argb32_icon() -> Vec<ksni::Icon> {
        let Ok(img) = image::load_from_memory(include_bytes!("../assets/icon.png")) else {
            return Vec::new();
        };
        let resized = img
            .resize_exact(64, 64, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let (w, h) = resized.dimensions();
        let mut argb = Vec::with_capacity((w * h * 4) as usize);
        for pixel in resized.pixels() {
            // D-Bus StatusNotifierItem expects network byte order ARGB32
            argb.push(pixel[3]);
            argb.push(pixel[0]);
            argb.push(pixel[1]);
            argb.push(pixel[2]);
        }
        vec![ksni::Icon {
            width: w as i32,
            height: h as i32,
            data: argb,
        }]
    }

    pub struct TrayController {
        handle: ksni::blocking::Handle<GreeTray>,
    }

    impl TrayController {
        pub fn spawn(tx: Sender<TrayAction>, ctx: egui::Context) -> Option<Self> {
            let tray = GreeTray {
                tx,
                ctx,
                state: TrayState::default(),
                icon_pixmap: load_argb32_icon(),
            };
            match tray.spawn() {
                Ok(handle) => Some(Self { handle }),
                Err(e) => {
                    eprintln!("System tray unavailable: {e}");
                    None
                }
            }
        }

        pub fn update(&self, new_state: TrayState) {
            self.handle.update(move |tray| {
                tray.state = new_state;
            });
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod platform {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
        menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    };

    use super::*;

    fn load_rgba_icon() -> Option<Icon> {
        let img = image::load_from_memory(include_bytes!("../assets/icon.png")).ok()?;
        let resized = img
            .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let (w, h) = resized.dimensions();
        Icon::from_rgba(resized.into_raw(), w, h).ok()
    }

    pub struct TrayController {
        tray: TrayIcon,
        show_item: MenuItem,
        power_item: MenuItem,
        window_visible: Arc<AtomicBool>,
    }

    impl TrayController {
        pub fn spawn(tx: Sender<TrayAction>, ctx: egui::Context) -> Option<Self> {
            let show_item = MenuItem::new("Ocultar ventana", true, None);
            let power_item = MenuItem::new("Encender / Apagar aire", false, None);
            let refresh_item = MenuItem::new("Actualizar estado", true, None);
            let separator = PredefinedMenuItem::separator();
            let quit_item = MenuItem::new("Salir", true, None);

            let menu = Menu::new();
            let _ = menu.append_items(&[
                &show_item,
                &power_item,
                &refresh_item,
                &separator,
                &quit_item,
            ]);

            let show_id = show_item.id().clone();
            let power_id = power_item.id().clone();
            let refresh_id = refresh_item.id().clone();
            let quit_id = quit_item.id().clone();

            let tx = Arc::new(Mutex::new(tx));
            let window_visible = Arc::new(AtomicBool::new(true));

            // Menu click handler (Windows & macOS)
            let tx_menu = Arc::clone(&tx);
            let vis_menu = Arc::clone(&window_visible);
            let ctx_menu = ctx.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                let action = if event.id == show_id {
                    let show = !vis_menu.load(Ordering::Relaxed);
                    ctx_menu.send_viewport_cmd(egui::ViewportCommand::Visible(show));
                    if show {
                        ctx_menu.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    Some(TrayAction::ToggleWindow)
                } else if event.id == power_id {
                    Some(TrayAction::TogglePower)
                } else if event.id == refresh_id {
                    Some(TrayAction::Refresh)
                } else if event.id == quit_id {
                    ctx_menu.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    Some(TrayAction::Quit)
                } else {
                    None
                };

                if let Some(act) = action {
                    if let Ok(guard) = tx_menu.lock() {
                        let _ = guard.send(act);
                    }
                    ctx_menu.request_repaint();
                }
            }));

            // Left-click tray icon toggles window on Windows (macOS opens menu on left-click)
            let tx_click = Arc::clone(&tx);
            let vis_click = Arc::clone(&window_visible);
            let ctx_click = ctx;
            TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                if cfg!(target_os = "windows")
                    && let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                {
                    let show = !vis_click.load(Ordering::Relaxed);
                    ctx_click.send_viewport_cmd(egui::ViewportCommand::Visible(show));
                    if show {
                        ctx_click.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    if let Ok(guard) = tx_click.lock() {
                        let _ = guard.send(TrayAction::ToggleWindow);
                    }
                    ctx_click.request_repaint();
                }
            }));

            let mut builder = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("Gree Remote")
                .with_menu_on_left_click(cfg!(target_os = "macos"));

            if let Some(icon) = load_rgba_icon() {
                builder = builder.with_icon(icon);
            }

            match builder.build() {
                Ok(tray) => Some(Self {
                    tray,
                    show_item,
                    power_item,
                    window_visible,
                }),
                Err(e) => {
                    eprintln!("System tray unavailable: {e}");
                    None
                }
            }
        }

        pub fn update(&self, new_state: TrayState) {
            self.window_visible
                .store(new_state.window_visible, Ordering::Relaxed);

            let show_label = if new_state.window_visible {
                "Ocultar ventana"
            } else {
                "Mostrar ventana"
            };
            self.show_item.set_text(show_label);

            let power_label = match new_state.power {
                Some(true) => "Apagar aire",
                Some(false) => "Encender aire",
                None => "Encender / Apagar aire",
            };
            self.power_item.set_text(power_label);
            self.power_item.set_enabled(new_state.power.is_some());

            let mut tooltip = match (new_state.power, new_state.set_temp) {
                (Some(true), Some(t)) => format!("Gree Remote · Encendido · {t}°C"),
                (Some(false), _) => "Gree Remote · Apagado".to_string(),
                _ => "Gree Remote".to_string(),
            };
            if let Some(r) = new_state.room_temp {
                tooltip.push_str(&format!(" (Ambiente {r}°C)"));
            }
            let _ = self.tray.set_tooltip(Some(tooltip));
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod platform {
    use super::*;

    pub struct TrayController;

    impl TrayController {
        pub fn spawn(_tx: Sender<TrayAction>, _ctx: egui::Context) -> Option<Self> {
            None
        }

        pub fn update(&self, _new_state: TrayState) {}
    }
}

pub use platform::TrayController;
