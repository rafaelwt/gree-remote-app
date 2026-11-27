use std::net::Ipv4Addr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::gree::client;
use crate::gree::protocol::{
    AcMode, AcStatus, DeviceInfo, FanSpeed, HorizontalSwing, VerticalSwing,
};

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
    busy: bool,
    auto_refresh: bool,
    last_refresh: Instant,
    status_message: String,
    is_error: bool,
}

impl GreeRemoteApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
            busy: true,
            auto_refresh: true,
            last_refresh: Instant::now(),
            status_message: "Escaneando la red en busca de equipos Gree...".to_string(),
            is_error: false,
        };
        let _ = app.cmd_tx.send(WorkerCmd::Scan(None));
        app
    }

    fn selected_device(&self) -> Option<&DeviceInfo> {
        self.devices.get(self.selected_idx)
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
                        self.status_message =
                            "No se encontraron equipos Gree. Prueba ingresar la IP manualmente."
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
                        self.status_message = "Estado actualizado.".to_string();
                    }
                }
                WorkerEvent::StatusResult(_, Err(e)) => {
                    self.busy = false;
                    self.is_error = true;
                    self.status_message = format!("Error leyendo estado: {e}");
                }
                WorkerEvent::CommandResult(Ok(())) => {
                    self.is_error = false;
                    self.status_message = "Comando aplicado.".to_string();
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

        // Top toolbar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.heading("❄ Gree Remote");
                ui.separator();

                if ui
                    .add_enabled(!self.busy, egui::Button::new("🔍 Escanear red"))
                    .clicked()
                {
                    self.trigger_scan(None);
                }

                ui.label("IP:");
                let ip_resp = ui.add(
                    egui::TextEdit::singleline(&mut self.manual_ip)
                        .desired_width(110.0)
                        .hint_text("192.168.1.100"),
                );
                let connect_clicked = ui
                    .add_enabled(!self.busy, egui::Button::new("Conectar"))
                    .clicked()
                    || (ip_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));

                if connect_clicked {
                    match self.manual_ip.trim().parse::<Ipv4Addr>() {
                        Ok(ip) => self.trigger_scan(Some(ip)),
                        Err(_) => {
                            self.is_error = true;
                            self.status_message = "IP inválida (ej: 192.168.1.100)".to_string();
                        }
                    }
                }

                ui.separator();
                if ui
                    .add_enabled(
                        !self.busy && self.selected_device().is_some(),
                        egui::Button::new("⟳ Actualizar"),
                    )
                    .clicked()
                {
                    self.trigger_refresh();
                }
                ui.checkbox(&mut self.auto_refresh, "Auto (10s)");
            });
            ui.add_space(4.0);
        });

        // Bottom status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.busy {
                    ui.spinner();
                }
                let color = if self.is_error {
                    egui::Color32::from_rgb(240, 100, 100)
                } else {
                    ui.visuals().text_color()
                };
                ui.colored_label(color, &self.status_message);
            });
        });

        // Main remote control panel
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.devices.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        if self.busy {
                            ui.spinner();
                            ui.label("Buscando aires acondicionados Gree en tu red local...");
                        } else {
                            ui.label("No hay equipos conectados.");
                            ui.label("Pulsa 'Escanear red' o ingresa la IP de tu aire arriba.");
                        }
                    });
                    return;
                }

                // Device selector + info
                let mut changed_device = false;
                ui.group(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Equipo:");
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
                                        changed_device = true;
                                    }
                                }
                            });

                        let dev = &self.devices[self.selected_idx];
                        ui.separator();
                        ui.small(format!(
                            "MAC: {} | {} | {}",
                            dev.id, dev.ver, dev.encryption
                        ));
                    });
                });

                if changed_device {
                    self.status = None;
                    self.trigger_refresh();
                }

                ui.add_space(8.0);

                let Some(mut st) = self.status.clone() else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Leyendo estado del aire acondicionado...");
                    });
                    return;
                };

                // Power + Temperature Card
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (btn_text, btn_color) = if st.power {
                            ("ENCENDIDO", egui::Color32::from_rgb(34, 153, 84))
                        } else {
                            ("APAGADO", egui::Color32::from_rgb(160, 50, 50))
                        };
                        let power_clicked = draw_power_button(ui, !self.busy, btn_text, btn_color);

                        if power_clicked {
                            st.power = !st.power;
                            self.status = Some(st.clone());
                            self.send_cmd(vec![("Pow", i64::from(st.power))]);
                        }

                        ui.add_space(16.0);

                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{} °C", st.set_temp))
                                        .size(34.0)
                                        .strong(),
                                );
                                if let Some(room) = st.current_temp {
                                    ui.add_space(12.0);
                                    ui.label(
                                        egui::RichText::new(format!("Ambiente: {room} °C"))
                                            .size(16.0)
                                            .weak(),
                                    );
                                }
                            });

                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        !self.busy && st.set_temp > 16,
                                        egui::Button::new("  −  "),
                                    )
                                    .clicked()
                                {
                                    st.set_temp -= 1;
                                    self.status = Some(st.clone());
                                    self.send_cmd(vec![("TemUn", 0), ("SetTem", st.set_temp)]);
                                }

                                let slider = ui.add_enabled(
                                    !self.busy,
                                    egui::Slider::new(&mut st.set_temp, 16..=30).show_value(false),
                                );
                                if slider.drag_stopped() {
                                    self.status = Some(st.clone());
                                    self.send_cmd(vec![("TemUn", 0), ("SetTem", st.set_temp)]);
                                } else if slider.changed() {
                                    self.status = Some(st.clone());
                                }

                                if ui
                                    .add_enabled(
                                        !self.busy && st.set_temp < 30,
                                        egui::Button::new("  +  "),
                                    )
                                    .clicked()
                                {
                                    st.set_temp += 1;
                                    self.status = Some(st.clone());
                                    self.send_cmd(vec![("TemUn", 0), ("SetTem", st.set_temp)]);
                                }
                            });
                        });
                    });
                });

                ui.add_space(8.0);

                // Mode selector
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new("Modo de operación").strong());
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for mode in AcMode::ALL {
                            let selected = st.mode == mode;
                            if ui
                                .add_enabled(
                                    !self.busy,
                                    egui::SelectableLabel::new(selected, mode.label()),
                                )
                                .clicked()
                                && !selected
                            {
                                st.mode = mode;
                                self.status = Some(st.clone());
                                self.send_cmd(vec![("Mod", mode as i64)]);
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                // Fan speed selector
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new("Velocidad del ventilador").strong());
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for speed in FanSpeed::ALL {
                            let selected = st.fan_speed == speed;
                            if ui
                                .add_enabled(
                                    !self.busy,
                                    egui::SelectableLabel::new(selected, speed.label()),
                                )
                                .clicked()
                                && !selected
                            {
                                st.fan_speed = speed;
                                self.status = Some(st.clone());
                                self.send_cmd(vec![("WdSpd", speed as i64)]);
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                // Swing controls
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new("Oscilación (Swing)").strong());
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Vertical:");
                        egui::ComboBox::from_id_salt("v_swing")
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
                                        self.send_cmd(vec![("SwUpDn", swing as i64)]);
                                    }
                                }
                            });

                        ui.add_space(12.0);
                        ui.label("Horizontal:");
                        egui::ComboBox::from_id_salt("h_swing")
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
                                        self.send_cmd(vec![("SwingLfRig", swing as i64)]);
                                    }
                                }
                            });
                    });
                });

                ui.add_space(8.0);

                // Extra feature toggles
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new("Funciones adicionales").strong());
                    ui.add_space(4.0);

                    let mut cmd_to_send: Option<(&str, i64)> = None;
                    egui::Grid::new("toggles_grid")
                        .num_columns(3)
                        .spacing([24.0, 8.0])
                        .show(ui, |ui| {
                            if ui.checkbox(&mut st.light, "💡 Luz / Display").changed() {
                                cmd_to_send = Some(("Lig", i64::from(st.light)));
                            }
                            if ui.checkbox(&mut st.turbo, "⚡ Turbo").changed() {
                                cmd_to_send = Some(("Tur", i64::from(st.turbo)));
                            }
                            if ui.checkbox(&mut st.quiet, "🔇 Silencio (Quiet)").changed() {
                                cmd_to_send = Some(("Quiet", i64::from(st.quiet)));
                            }
                            ui.end_row();

                            if ui.checkbox(&mut st.sleep, "🌙 Dormir (Sleep)").changed() {
                                cmd_to_send = Some(("SwhSlp", i64::from(st.sleep)));
                            }
                            if ui.checkbox(&mut st.xfan, "💨 X-Fan (Blow)").changed() {
                                cmd_to_send = Some(("Blo", i64::from(st.xfan)));
                            }
                            if ui.checkbox(&mut st.health, "🛡 Salud (Health)").changed() {
                                cmd_to_send = Some(("Health", i64::from(st.health)));
                            }
                            ui.end_row();

                            if ui.checkbox(&mut st.saving, "🌱 Ahorro (Saving)").changed() {
                                cmd_to_send = Some(("SvSt", i64::from(st.saving)));
                            }
                            if ui.checkbox(&mut st.air, "🍃 Aire fresco").changed() {
                                cmd_to_send = Some(("Air", i64::from(st.air)));
                            }
                            if ui.checkbox(&mut st.freeze_guard, "❄ 8°C Anti-hielo").changed() {
                                cmd_to_send = Some(("StHt", i64::from(st.freeze_guard)));
                            }
                            ui.end_row();
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

fn draw_power_button(
    ui: &mut egui::Ui,
    enabled: bool,
    label: &str,
    base_color: egui::Color32,
) -> bool {
    let desired_size = egui::vec2(165.0, 54.0);
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(desired_size, sense);

    if ui.is_rect_visible(rect) {
        let mut fill = base_color;
        if !enabled {
            fill = fill.gamma_multiply(0.6);
        } else if response.is_pointer_button_down_on() {
            fill = fill.gamma_multiply(0.85);
        } else if response.hovered() {
            fill = fill.gamma_multiply(1.1);
        }

        let painter = ui.painter();
        painter.rect_filled(rect, egui::CornerRadius::same(6), fill);

        let fg = egui::Color32::WHITE;
        let icon_center = egui::pos2(rect.left() + 26.0, rect.center().y);
        let radius = 8.5_f32;
        let stroke = egui::Stroke::new(2.2_f32, fg);

        // Draw open circle arc (~280 degrees, gap at the top)
        let segments = 24;
        let start_angle = -std::f32::consts::FRAC_PI_2 + 0.55;
        let end_angle = start_angle + (std::f32::consts::TAU - 1.10);
        let points: Vec<egui::Pos2> = (0..=segments)
            .map(|i| {
                let t = i as f32 / segments as f32;
                let angle = start_angle + t * (end_angle - start_angle);
                egui::pos2(
                    icon_center.x + radius * angle.cos(),
                    icon_center.y + radius * angle.sin(),
                )
            })
            .collect();
        painter.add(egui::Shape::line(points, stroke));

        // Draw top vertical stem of the power icon
        painter.line_segment(
            [
                egui::pos2(icon_center.x, icon_center.y - radius - 2.0),
                egui::pos2(icon_center.x, icon_center.y - 1.5),
            ],
            stroke,
        );

        // Draw button label
        painter.text(
            egui::pos2(rect.left() + 46.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(17.0),
            fg,
        );
    }

    enabled && response.clicked()
}
