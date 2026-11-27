use std::net::Ipv4Addr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, CornerRadius, Margin, RichText, Stroke, vec2};

use crate::gree::client;
use crate::gree::protocol::{
    AcMode, AcStatus, DeviceInfo, HorizontalSwing, VerticalSwing,
};
use crate::ui::icons::{Icon, draw_icon};
use crate::ui::theme::{
    self, BG_DEEP, BORDER, CARD_BG, CARD_INNER, ERROR, POWER_ON, TEXT_MUTED, TEXT_PRIMARY,
};
use crate::ui::widgets;

enum WorkerCmd {
    Scan(Option<Ipv4Addr>),
    Refresh(DeviceInfo),
    SetParams {
        device: DeviceInfo,
        params: Vec<(String, i64)>,
    },
}

enum WorkerEvent {
    ScanResult(Result<Vec<DeviceInfo>, String>),
    StatusResult(String, Result<AcStatus, String>),
    CommandResult(Result<(), String>),
}

pub struct GreeRemoteApp {
    cmd_tx: Sender<WorkerCmd>,
    evt_rx: Receiver<WorkerEvent>,

    devices: Vec<DeviceInfo>,
    selected_idx: usize,
    status: Option<AcStatus>,

    manual_ip: String,
    show_settings: bool,
    busy: bool,
    auto_refresh: bool,
    last_refresh: Instant,
    status_message: String,
    is_error: bool,
}

impl GreeRemoteApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply_theme(&cc.egui_ctx);

        let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCmd>();
        let (evt_tx, evt_rx) = mpsc::channel::<WorkerEvent>();
        let ctx = cc.egui_ctx.clone();

        thread::spawn(move || {
            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    WorkerCmd::Scan(manual_ip) => {
                        let res = client::scan_and_bind(manual_ip);
                        let _ = evt_tx.send(WorkerEvent::ScanResult(res));
                        ctx.request_repaint();
                    }
                    WorkerCmd::Refresh(dev) => {
                        let id = dev.id.clone();
                        let res = client::get_status(&dev);
                        let _ = evt_tx.send(WorkerEvent::StatusResult(id, res));
                        ctx.request_repaint();
                    }
                    WorkerCmd::SetParams { device, params } => {
                        let pairs: Vec<(&str, i64)> =
                            params.iter().map(|(k, v)| (k.as_str(), *v)).collect();
                        let cmd_res = client::set_params(&device, &pairs);
                        let _ = evt_tx.send(WorkerEvent::CommandResult(cmd_res.clone()));
                        if cmd_res.is_ok() {
                            let id = device.id.clone();
                            let st = client::get_status(&device);
                            let _ = evt_tx.send(WorkerEvent::StatusResult(id, st));
                        }
                        ctx.request_repaint();
                    }
                }
            }
        });

        let app = Self {
            cmd_tx,
            evt_rx,
            devices: Vec::new(),
            selected_idx: 0,
            status: None,
            manual_ip: String::new(),
            show_settings: false,
            busy: true,
            auto_refresh: true,
            last_refresh: Instant::now(),
            status_message: "Escaneando la red local...".to_string(),
            is_error: false,
        };
        let _ = app.cmd_tx.send(WorkerCmd::Scan(None));
        app
    }

    fn selected_device(&self) -> Option<&DeviceInfo> {
        self.devices.get(self.selected_idx)
    }

    fn current_accent(&self) -> egui::Color32 {
        let mode = self.status.as_ref().map(|s| s.mode).unwrap_or(AcMode::Cool);
        theme::mode_accent(mode)
    }

    fn trigger_scan(&mut self, manual_ip: Option<Ipv4Addr>) {
        self.busy = true;
        self.is_error = false;
        self.status_message = match manual_ip {
            Some(ip) => format!("Conectando a {ip}..."),
            None => "Escaneando la red local...".to_string(),
        };
        let _ = self.cmd_tx.send(WorkerCmd::Scan(manual_ip));
    }

    fn trigger_refresh(&mut self) {
        if let Some(dev) = self.selected_device().cloned() {
            self.last_refresh = Instant::now();
            let _ = self.cmd_tx.send(WorkerCmd::Refresh(dev));
        }
    }

    fn send_cmd(&mut self, params: Vec<(&str, i64)>) {
        if let Some(dev) = self.selected_device().cloned() {
            self.busy = true;
            self.is_error = false;
            self.status_message = "Enviando comando...".to_string();
            let owned = params
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
            let _ = self.cmd_tx.send(WorkerCmd::SetParams {
                device: dev,
                params: owned,
            });
        }
    }

    fn poll_events(&mut self) {
        while let Ok(evt) = self.evt_rx.try_recv() {
            match evt {
                WorkerEvent::ScanResult(Ok(devs)) => {
                    self.busy = false;
                    if devs.is_empty() {
                        self.is_error = true;
                        self.show_settings = true;
                        self.status_message =
                            "No se encontraron equipos Gree. Ingresa la IP manualmente."
                                .to_string();
                    } else {
                        self.devices = devs;
                        self.selected_idx = 0;
                        self.is_error = false;
                        self.status_message =
                            format!("{} equipo(s) encontrado(s).", self.devices.len());
                        self.trigger_refresh();
                    }
                }
                WorkerEvent::ScanResult(Err(e)) => {
                    self.busy = false;
                    self.is_error = true;
                    self.status_message = format!("Error al escanear: {e}");
                }
                WorkerEvent::StatusResult(id, Ok(st)) => {
                    self.busy = false;
                    if self.selected_device().map(|d| d.id.as_str()) == Some(id.as_str()) {
                        self.status = Some(st);
                        self.last_refresh = Instant::now();
                        self.is_error = false;
                        self.status_message = "Conectado".to_string();
                    }
                }
                WorkerEvent::StatusResult(_, Err(e)) => {
                    self.busy = false;
                    self.is_error = true;
                    self.status_message = format!("Error leyendo estado: {e}");
                }
                WorkerEvent::CommandResult(Ok(())) => {
                    self.is_error = false;
                    self.status_message = "Comando aplicado".to_string();
                }
                WorkerEvent::CommandResult(Err(e)) => {
                    self.busy = false;
                    self.is_error = true;
                    self.status_message = format!("Error enviando comando: {e}");
                }
            }
        }
    }
}

impl eframe::App for GreeRemoteApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();

        if self.auto_refresh
            && !self.busy
            && self.selected_device().is_some()
            && self.last_refresh.elapsed() >= Duration::from_secs(10)
        {
            self.trigger_refresh();
        }
        if self.auto_refresh && self.selected_device().is_some() {
            ctx.request_repaint_after(Duration::from_secs(2));
        }

        let accent = self.current_accent();

        // Compact Top Header
        egui::TopBottomPanel::top("top_header")
            .frame(
                egui::Frame::new()
                    .fill(BG_DEEP)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(Margin::symmetric(16, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // App vector logo
                    let (logo_rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
                    if ui.is_rect_visible(logo_rect) {
                        draw_icon(ui.painter(), logo_rect.center(), 13.5, Icon::Snowflake, accent);
                    }
                    ui.add_space(4.0);

                    // Title + device pill
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(
                            RichText::new("Gree Remote")
                                .size(17.5)
                                .strong()
                                .color(TEXT_PRIMARY),
                        );
                        if let Some(dev) = self.selected_device() {
                            egui::Frame::new()
                                .fill(CARD_BG)
                                .stroke(Stroke::new(1.0_f32, BORDER))
                                .corner_radius(CornerRadius::same(10))
                                .inner_margin(Margin::symmetric(8, 2))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let (dot_rect, _) = ui.allocate_exact_size(
                                            vec2(7.0, 7.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().circle_filled(
                                            dot_rect.center(),
                                            3.2,
                                            if dev.key.is_some() { POWER_ON } else { ERROR },
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "{} {} · {}",
                                                dev.brand.to_uppercase(),
                                                dev.model_type,
                                                dev.ip
                                            ))
                                            .size(11.5)
                                            .color(TEXT_MUTED),
                                        );
                                    });
                                });
                        } else {
                            ui.label(
                                RichText::new("Sin equipo conectado")
                                    .size(11.5)
                                    .color(TEXT_MUTED),
                            );
                        }
                    });

                    // Right action icon buttons
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::header_icon_button(
                            ui,
                            Icon::Settings,
                            "Conexión manual y ajustes",
                            true,
                            self.show_settings,
                            accent,
                        ) {
                            self.show_settings = !self.show_settings;
                        }
                        if widgets::header_icon_button(
                            ui,
                            Icon::Scan,
                            "Escanear red local",
                            !self.busy,
                            false,
                            accent,
                        ) {
                            self.trigger_scan(None);
                        }
                        if widgets::header_icon_button(
                            ui,
                            Icon::Refresh,
                            "Actualizar estado",
                            !self.busy && self.selected_device().is_some(),
                            false,
                            accent,
                        ) {
                            self.trigger_refresh();
                        }
                    });
                });
            });

        // Minimalist Footer Status Bar
        egui::TopBottomPanel::bottom("footer_bar")
            .frame(
                egui::Frame::new()
                    .fill(BG_DEEP)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(Margin::symmetric(16, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.busy {
                        ui.add(egui::Spinner::new().size(13.0));
                    } else {
                        let (dot_rect, _) =
                            ui.allocate_exact_size(vec2(8.0, 8.0), egui::Sense::hover());
                        let dot_color = if self.is_error {
                            ERROR
                        } else if self.selected_device().is_some() {
                            POWER_ON
                        } else {
                            TEXT_MUTED
                        };
                        ui.painter().circle_filled(dot_rect.center(), 3.5, dot_color);
                    }

                    let footer_text = if !self.is_error && !self.busy {
                        if let Some(dev) = self.selected_device() {
                            let auto_str = if self.auto_refresh { " · Auto 10s" } else { "" };
                            format!(
                                "{} · {} · {}{}",
                                self.status_message, dev.ver, dev.encryption, auto_str
                            )
                        } else {
                            self.status_message.clone()
                        }
                    } else {
                        self.status_message.clone()
                    };

                    ui.label(
                        RichText::new(footer_text)
                            .size(12.0)
                            .color(if self.is_error { ERROR } else { TEXT_MUTED }),
                    );
                });
            });

        // Main Content Area
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG_DEEP).inner_margin(Margin::same(14)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Collapsible Settings / Manual IP Panel
                    if self.show_settings {
                        theme::card_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("IP manual:").color(TEXT_MUTED));
                                let ip_resp = ui.add(
                                    egui::TextEdit::singleline(&mut self.manual_ip)
                                        .desired_width(120.0)
                                        .hint_text("192.168.1.100"),
                                );
                                let connect = ui
                                    .add_enabled(!self.busy, egui::Button::new("Conectar"))
                                    .clicked()
                                    || (ip_resp.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter)));

                                if connect {
                                    match self.manual_ip.trim().parse::<Ipv4Addr>() {
                                        Ok(ip) => self.trigger_scan(Some(ip)),
                                        Err(_) => {
                                            self.is_error = true;
                                            self.status_message =
                                                "IP inválida (ej: 192.168.1.100)".to_string();
                                        }
                                    }
                                }

                                ui.separator();
                                ui.checkbox(&mut self.auto_refresh, "Auto-actualizar (10s)");
                            });

                            if self.devices.len() > 1 {
                                ui.add_space(6.0);
                                let mut changed = false;
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Equipo:").color(TEXT_MUTED));
                                    egui::ComboBox::from_id_salt("device_selector")
                                        .selected_text(self.devices[self.selected_idx].display_name())
                                        .show_ui(ui, |ui| {
                                            for (idx, dev) in self.devices.iter().enumerate() {
                                                if ui
                                                    .selectable_value(
                                                        &mut self.selected_idx,
                                                        idx,
                                                        dev.display_name(),
                                                    )
                                                    .changed()
                                                {
                                                    changed = true;
                                                }
                                            }
                                        });
                                });
                                if changed {
                                    self.status = None;
                                    self.trigger_refresh();
                                }
                            }
                        });
                        ui.add_space(10.0);
                    }

                    if self.devices.is_empty() {
                        theme::card_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                ui.add_space(40.0);
                                if self.busy {
                                    ui.spinner();
                                    ui.add_space(8.0);
                                    ui.label("Buscando aires acondicionados Gree en tu red...");
                                } else {
                                    ui.label(
                                        RichText::new("No hay equipos conectados")
                                            .size(15.0)
                                            .strong(),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(
                                            "Usa el botón de escaneo o ingresa la IP arriba.",
                                        )
                                        .color(TEXT_MUTED),
                                    );
                                }
                                ui.add_space(40.0);
                            });
                        });
                        return;
                    }

                    let Some(mut st) = self.status.clone() else {
                        theme::card_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Leyendo estado del aire acondicionado...");
                            });
                        });
                        return;
                    };

                    // 1. Hero Thermostat Card (Circular Dial + Power Pill)
                    theme::card_frame().show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        let prev_temp = st.set_temp;
                        if let Some(new_temp) = widgets::thermostat_dial(
                            ui,
                            &mut st.set_temp,
                            st.current_temp,
                            st.power,
                            accent,
                            !self.busy,
                        ) {
                            self.status = Some(st.clone());
                            self.send_cmd(vec![("TemUn", 0), ("SetTem", new_temp)]);
                        } else if st.set_temp != prev_temp {
                            self.status = Some(st.clone());
                        }

                        ui.add_space(4.0);
                        if widgets::power_pill_button(ui, st.power, !self.busy) {
                            st.power = !st.power;
                            self.status = Some(st.clone());
                            self.send_cmd(vec![("Pow", i64::from(st.power))]);
                        }
                    });

                    ui.add_space(10.0);

                    // 2. Mode Segmented Pill Card
                    theme::card_frame().show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        if let Some(new_mode) =
                            widgets::mode_segmented_bar(ui, st.mode, accent, !self.busy)
                        {
                            st.mode = new_mode;
                            self.status = Some(st.clone());
                            self.send_cmd(vec![("Mod", new_mode as i64)]);
                        }
                    });

                    ui.add_space(10.0);

                    // 3. Fan Speed 6-Step Bar Card
                    theme::card_frame().show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        if let Some(new_speed) =
                            widgets::fan_speed_bar(ui, st.fan_speed, accent, !self.busy)
                        {
                            st.fan_speed = new_speed;
                            self.status = Some(st.clone());
                            self.send_cmd(vec![("WdSpd", new_speed as i64)]);
                        }
                    });

                    ui.add_space(10.0);

                    // 4. Airflow / Swing Card
                    theme::card_frame().show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        let half_w = ((ui.available_width() - 12.0) * 0.5).max(160.0);
                        ui.horizontal(|ui| {
                            // Vertical swing
                            egui::Frame::new()
                                .fill(CARD_INNER)
                                .stroke(Stroke::new(1.0_f32, BORDER))
                                .corner_radius(CornerRadius::same(10))
                                .inner_margin(Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.set_width(half_w - 22.0);
                                    ui.horizontal(|ui| {
                                        let (ir, _) = ui.allocate_exact_size(
                                            vec2(18.0, 20.0),
                                            egui::Sense::hover(),
                                        );
                                        draw_icon(
                                            ui.painter(),
                                            ir.center(),
                                            7.5,
                                            Icon::SwingV,
                                            TEXT_MUTED,
                                        );
                                        egui::ComboBox::from_id_salt("v_swing")
                                            .width(ui.available_width())
                                            .selected_text(st.vertical_swing.label())
                                            .show_ui(ui, |ui| {
                                                for swing in VerticalSwing::ALL {
                                                    if ui
                                                        .selectable_value(
                                                            &mut st.vertical_swing,
                                                            swing,
                                                            swing.label(),
                                                        )
                                                        .changed()
                                                    {
                                                        self.status = Some(st.clone());
                                                        self.send_cmd(vec![(
                                                            "SwUpDn",
                                                            swing as i64,
                                                        )]);
                                                    }
                                                }
                                            });
                                    });
                                });

                            ui.add_space(4.0);

                            // Horizontal swing
                            egui::Frame::new()
                                .fill(CARD_INNER)
                                .stroke(Stroke::new(1.0_f32, BORDER))
                                .corner_radius(CornerRadius::same(10))
                                .inner_margin(Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.set_width(half_w - 22.0);
                                    ui.horizontal(|ui| {
                                        let (ir, _) = ui.allocate_exact_size(
                                            vec2(18.0, 20.0),
                                            egui::Sense::hover(),
                                        );
                                        draw_icon(
                                            ui.painter(),
                                            ir.center(),
                                            7.5,
                                            Icon::SwingH,
                                            TEXT_MUTED,
                                        );
                                        egui::ComboBox::from_id_salt("h_swing")
                                            .width(ui.available_width())
                                            .selected_text(st.horizontal_swing.label())
                                            .show_ui(ui, |ui| {
                                                for swing in HorizontalSwing::ALL {
                                                    if ui
                                                        .selectable_value(
                                                            &mut st.horizontal_swing,
                                                            swing,
                                                            swing.label(),
                                                        )
                                                        .changed()
                                                    {
                                                        self.status = Some(st.clone());
                                                        self.send_cmd(vec![(
                                                            "SwingLfRig",
                                                            swing as i64,
                                                        )]);
                                                    }
                                                }
                                            });
                                    });
                                });
                        });
                    });

                    ui.add_space(10.0);

                    // 5. Quick Features 3x3 Glowing Tiles Card
                    theme::card_frame().show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        let gap = 8.0;
                        let tile_w = ((ui.available_width() - gap * 2.0) / 3.0).max(110.0);
                        let mut cmd_to_send: Option<(&str, i64)> = None;

                        ui.horizontal(|ui| {
                            if widgets::feature_tile(
                                ui,
                                Icon::Bulb,
                                "Luz",
                                st.light,
                                accent,
                                tile_w,
                                !self.busy,
                            ) {
                                st.light = !st.light;
                                cmd_to_send = Some(("Lig", i64::from(st.light)));
                            }
                            if widgets::feature_tile(
                                ui,
                                Icon::Turbo,
                                "Turbo",
                                st.turbo,
                                accent,
                                tile_w,
                                !self.busy,
                            ) {
                                st.turbo = !st.turbo;
                                cmd_to_send = Some(("Tur", i64::from(st.turbo)));
                            }
                            if widgets::feature_tile(
                                ui,
                                Icon::Quiet,
                                "Silencio",
                                st.quiet,
                                accent,
                                tile_w,
                                !self.busy,
                            ) {
                                st.quiet = !st.quiet;
                                cmd_to_send = Some(("Quiet", i64::from(st.quiet)));
                            }
                        });

                        ui.horizontal(|ui| {
                            if widgets::feature_tile(
                                ui,
                                Icon::Sleep,
                                "Dormir",
                                st.sleep,
                                accent,
                                tile_w,
                                !self.busy,
                            ) {
                                st.sleep = !st.sleep;
                                cmd_to_send = Some(("SwhSlp", i64::from(st.sleep)));
                            }
                            if widgets::feature_tile(
                                ui,
                                Icon::Fan,
                                "X-Fan",
                                st.xfan,
                                accent,
                                tile_w,
                                !self.busy,
                            ) {
                                st.xfan = !st.xfan;
                                cmd_to_send = Some(("Blo", i64::from(st.xfan)));
                            }
                            if widgets::feature_tile(
                                ui,
                                Icon::Health,
                                "Salud",
                                st.health,
                                accent,
                                tile_w,
                                !self.busy,
                            ) {
                                st.health = !st.health;
                                cmd_to_send = Some(("Health", i64::from(st.health)));
                            }
                        });

                        ui.horizontal(|ui| {
                            if widgets::feature_tile(
                                ui,
                                Icon::Saving,
                                "Ahorro",
                                st.saving,
                                accent,
                                tile_w,
                                !self.busy,
                            ) {
                                st.saving = !st.saving;
                                cmd_to_send = Some(("SvSt", i64::from(st.saving)));
                            }
                            if widgets::feature_tile(
                                ui,
                                Icon::Air,
                                "Aire",
                                st.air,
                                accent,
                                tile_w,
                                !self.busy,
                            ) {
                                st.air = !st.air;
                                cmd_to_send = Some(("Air", i64::from(st.air)));
                            }
                            if widgets::feature_tile(
                                ui,
                                Icon::Freeze,
                                "8°C",
                                st.freeze_guard,
                                accent,
                                tile_w,
                                !self.busy,
                            ) {
                                st.freeze_guard = !st.freeze_guard;
                                cmd_to_send = Some(("StHt", i64::from(st.freeze_guard)));
                            }
                        });

                        if let Some(pair) = cmd_to_send {
                            self.status = Some(st);
                            self.send_cmd(vec![pair]);
                        }
                    });
                });
            });
    }
}
