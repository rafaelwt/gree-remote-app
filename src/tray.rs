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
            if matches!(action, TrayAction::ToggleWindow) {
                let show = !self.state.window_visible;
                self.ctx
                    .send_viewport_cmd(egui::ViewportCommand::Visible(show));
                if show {
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            } else if matches!(action, TrayAction::Quit) {
                self.ctx
                    .send_viewport_cmd(egui::ViewportCommand::Visible(true));
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

#[cfg(not(target_os = "linux"))]
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
