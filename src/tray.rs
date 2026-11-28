use std::sync::Arc;
use std::sync::mpsc::Sender;

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ToggleWindow,
    SetPower(bool),
    Refresh,
    // Only routed through the enum on Linux; Windows/macOS exit directly.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Quit,
}

/// Executes device actions (power / refresh) straight from the tray handler.
///
/// These must not depend on `eframe::App::update`: on Windows a hidden window
/// receives no `WM_PAINT`, so `update` never runs while the app lives in the tray.
pub type TrayCommandHandler = Arc<dyn Fn(TrayAction) + Send + Sync>;

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
        handler: TrayCommandHandler,
        state: TrayState,
        icon_pixmap: Vec<ksni::Icon>,
    }

    impl GreeTray {
        fn send(&mut self, action: TrayAction) {
            match action {
                TrayAction::SetPower(on) => {
                    (self.handler)(action);
                    self.state.power = Some(on);
                    return;
                }
                TrayAction::Refresh => {
                    (self.handler)(action);
                    return;
                }
                TrayAction::Quit => std::process::exit(0),
                TrayAction::ToggleWindow => {}
            }

            let _ = self.tx.send(action);
            let on_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
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
                    activate: Box::new(|this: &mut Self| {
                        if let Some(p) = this.state.power {
                            this.send(TrayAction::SetPower(!p));
                        }
                    }),
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
        pub fn spawn(
            cc: &eframe::CreationContext<'_>,
            tx: Sender<TrayAction>,
            handler: TrayCommandHandler,
        ) -> Option<Self> {
            let tray = GreeTray {
                tx,
                ctx: cc.egui_ctx.clone(),
                handler,
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
        Mutex,
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

    /// Native handle of the main window (Windows only). Used to show the window
    /// again directly, because `update` does not run while it is hidden.
    #[derive(Clone, Copy)]
    struct NativeWindow(#[cfg_attr(not(target_os = "windows"), allow(dead_code))] Option<isize>);

    impl NativeWindow {
        #[cfg(target_os = "windows")]
        fn from_cc(cc: &eframe::CreationContext<'_>) -> Self {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let hwnd = cc.window_handle().ok().and_then(|h| match h.as_raw() {
                RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
                _ => None,
            });
            Self(hwnd)
        }

        #[cfg(not(target_os = "windows"))]
        fn from_cc(_cc: &eframe::CreationContext<'_>) -> Self {
            Self(None)
        }

        #[cfg(target_os = "windows")]
        #[allow(unsafe_code)]
        fn show(self) {
            use windows_sys::Win32::Foundation::HWND;
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                IsIconic, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
            };
            let Some(raw) = self.0 else { return };
            let hwnd = raw as HWND;
            // SAFETY: `hwnd` is the eframe root window, which lives for the whole
            // process; tray events are dispatched on the UI thread that owns it.
            unsafe {
                let cmd = if IsIconic(hwnd) != 0 { SW_RESTORE } else { SW_SHOW };
                ShowWindow(hwnd, cmd);
                SetForegroundWindow(hwnd);
            }
        }

        #[cfg(not(target_os = "windows"))]
        fn show(self) {}
    }

    fn toggle_window(
        ctx: &egui::Context,
        native: NativeWindow,
        visible: &AtomicBool,
        tx: &Mutex<Sender<TrayAction>>,
    ) {
        let show = !visible.load(Ordering::Relaxed);
        if show {
            // Show natively first so the window gets WM_PAINT and egui resumes;
            // the viewport command below keeps winit's visibility flag in sync.
            native.show();
        }
        visible.store(show, Ordering::Relaxed);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(show));
        if show {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if let Ok(guard) = tx.lock() {
            let _ = guard.send(TrayAction::ToggleWindow);
        }
        ctx.request_repaint();
    }

    pub struct TrayController {
        tray: TrayIcon,
        show_item: MenuItem,
        power_on_item: MenuItem,
        power_off_item: MenuItem,
        window_visible: Arc<AtomicBool>,
    }

    impl TrayController {
        pub fn spawn(
            cc: &eframe::CreationContext<'_>,
            tx: Sender<TrayAction>,
            handler: TrayCommandHandler,
        ) -> Option<Self> {
            let ctx = cc.egui_ctx.clone();
            let native = NativeWindow::from_cc(cc);

            let show_item = MenuItem::new("Ocultar ventana", true, None);
            let power_on_item = MenuItem::new("Encender aire", false, None);
            let power_off_item = MenuItem::new("Apagar aire", false, None);
            let refresh_item = MenuItem::new("Actualizar estado", true, None);
            let separator = PredefinedMenuItem::separator();
            let separator2 = PredefinedMenuItem::separator();
            let quit_item = MenuItem::new("Salir", true, None);

            let menu = Menu::new();
            let _ = menu.append_items(&[
                &show_item,
                &separator,
                &power_on_item,
                &power_off_item,
                &refresh_item,
                &separator2,
                &quit_item,
            ]);

            let show_id = show_item.id().clone();
            let power_on_id = power_on_item.id().clone();
            let power_off_id = power_off_item.id().clone();
            let refresh_id = refresh_item.id().clone();
            let quit_id = quit_item.id().clone();

            let tx = Arc::new(Mutex::new(tx));
            let window_visible = Arc::new(AtomicBool::new(true));

            // Menu click handler: runs outside `App::update`, so every action here
            // must work even while the window is hidden.
            let tx_menu = Arc::clone(&tx);
            let vis_menu = Arc::clone(&window_visible);
            let ctx_menu = ctx.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                if event.id == show_id {
                    toggle_window(&ctx_menu, native, &vis_menu, &tx_menu);
                } else if event.id == power_on_id {
                    handler(TrayAction::SetPower(true));
                } else if event.id == power_off_id {
                    handler(TrayAction::SetPower(false));
                } else if event.id == refresh_id {
                    handler(TrayAction::Refresh);
                } else if event.id == quit_id {
                    std::process::exit(0);
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
                    toggle_window(&ctx_click, native, &vis_click, &tx_click);
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
                    power_on_item,
                    power_off_item,
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

            // Fixed labels for power: they can't go stale while the window is
            // hidden (the menu can only be relabelled from `update`).
            let has_status = new_state.power.is_some();
            self.power_on_item.set_enabled(has_status);
            self.power_off_item.set_enabled(has_status);

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
        pub fn spawn(
            _cc: &eframe::CreationContext<'_>,
            _tx: Sender<TrayAction>,
            _handler: TrayCommandHandler,
        ) -> Option<Self> {
            None
        }

        pub fn update(&self, _new_state: TrayState) {}
    }
}

pub use platform::TrayController;
