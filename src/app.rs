use std::net::Ipv4Addr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Datelike, Duration as ChronoDuration, Local, Timelike};
use eframe::egui::{self, CornerRadius, Margin, RichText, Stroke, vec2};

use crate::gree::client;
use crate::gree::protocol::{
    AcMode, AcStatus, DeviceInfo, HorizontalSwing, VerticalSwing,
};
use crate::gree::storage;
use crate::tray::{TrayAction, TrayController, TrayState};
use crate::ui::icons::{Icon, draw_icon};
use crate::ui::theme::{
    self, BG_DEEP, BORDER, CARD_BG, CARD_INNER, ERROR, POWER_ON, TEXT_MUTED, TEXT_PRIMARY,
};
use crate::ui::widgets;

#[derive(Debug, Clone)]
struct ActiveTimer {
    target_time_label: String,
    deadline: Instant,
    power_on: bool,
    on_device: bool,
    hour: u32,
    minute: u32,
    weekday_sun0: usize,
}

enum WorkerCmd {
    ProbeCached(DeviceInfo),
    Scan(Option<Ipv4Addr>),
    Refresh(DeviceInfo),
    SetParams {
        device: DeviceInfo,
        params: Vec<(String, i64)>,
    },
    SetTimer {
        device: DeviceInfo,
        power_on: bool,
        hour: u32,
        minute: u32,
        weekday_sun0: usize,
        minutes_from_now: u64,
        target_label: String,
    },
    CancelTimer {
        device: DeviceInfo,
        power_on: bool,
        hour: u32,
        minute: u32,
        weekday_sun0: usize,
    },
}

enum WorkerEvent {
    FallbackScanning(String),
    ScanResult(Result<Vec<DeviceInfo>, String>),
    StatusResult(String, Result<AcStatus, String>),
    CommandResult(Result<(), String>),
    TimerSetResult {
        timer: ActiveTimer,
        message: String,
    },
    TimerCancelled(String),
}

pub struct GreeRemoteApp {
    cmd_tx: Sender<WorkerCmd>,
    evt_rx: Receiver<WorkerEvent>,
    tray_rx: Receiver<TrayAction>,
    tray: Option<TrayController>,

    devices: Vec<DeviceInfo>,
    selected_idx: usize,
    status: Option<AcStatus>,

    manual_ip: String,
    show_settings: bool,
    show_timer: bool,
    timer_minutes: u32,
    timer_power_on: bool,
    active_timer: Option<ActiveTimer>,

    minimize_to_tray: bool,
    window_visible: bool,
    allow_close: bool,

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
        let (tray_tx, tray_rx) = mpsc::channel::<TrayAction>();
        let tray = TrayController::spawn(tray_tx, cc.egui_ctx.clone());
        let ctx = cc.egui_ctx.clone();

        thread::spawn(move || {
            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    WorkerCmd::ProbeCached(dev) => {
                        let id = dev.id.clone();
                        match client::get_status(&dev) {
                            Ok(st) => {
                                let _ = evt_tx.send(WorkerEvent::StatusResult(id, Ok(st)));
                                ctx.request_repaint();
                            }
                            Err(_) => {
                                // Cached IP/key didn't respond — fall back to full network scan
                                let _ = evt_tx.send(WorkerEvent::FallbackScanning(format!(
                                    "{} no respondió; escaneando red...",
                                    dev.ip
                                )));
                                ctx.request_repaint();
                                let res = client::scan_and_bind(None);
                                if let Ok(ref devs) = res {
                                    let _ = storage::save_devices(devs);
                                }
                                let _ = evt_tx.send(WorkerEvent::ScanResult(res));
                                ctx.request_repaint();
                            }
                        }
                    }
                    WorkerCmd::Scan(manual_ip) => {
                        let res = client::scan_and_bind(manual_ip);
                        if let Ok(ref devs) = res {
                            let _ = storage::save_devices(devs);
                        }
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
                    WorkerCmd::SetTimer {
                        device,
                        power_on,
                        hour,
                        minute,
                        weekday_sun0,
                        minutes_from_now,
                        target_label,
                    } => {
                        let native_ok = client::set_device_timer(
                            &device,
                            true,
                            power_on,
                            hour,
                            minute,
                            weekday_sun0,
                        )
                        .is_ok();
                        let action_txt = if power_on { "Encender" } else { "Apagar" };
                        let message = if native_ok {
                            format!("{action_txt} programado en el aire a las {target_label}")
                        } else {
                            format!(
                                "{action_txt} programado en la app (bandeja) a las {target_label}"
                            )
                        };
                        let timer = ActiveTimer {
                            target_time_label: target_label,
                            deadline: Instant::now() + Duration::from_secs(minutes_from_now * 60),
                            power_on,
                            on_device: native_ok,
                            hour,
                            minute,
                            weekday_sun0,
                        };
                        let _ = evt_tx.send(WorkerEvent::TimerSetResult { timer, message });
                        ctx.request_repaint();
                    }
                    WorkerCmd::CancelTimer {
                        device,
                        power_on,
                        hour,
                        minute,
                        weekday_sun0,
                    } => {
                        let _ = client::set_device_timer(
                            &device,
                            false,
                            power_on,
                            hour,
                            minute,
                            weekday_sun0,
                        );
                        let _ = evt_tx.send(WorkerEvent::TimerCancelled(
                            "Temporizador cancelado".to_string(),
                        ));
                        ctx.request_repaint();
                    }
                }
            }
        });

        let cached_devices = storage::load_devices();
        let initial_msg = if let Some(first) = cached_devices.first() {
            format!("Conectando a {}...", first.ip)
        } else {
            "Escaneando la red local...".to_string()
        };

        let app = Self {
            cmd_tx,
            evt_rx,
            tray_rx,
            tray,
            devices: cached_devices,
            selected_idx: 0,
            status: None,
            manual_ip: String::new(),
            show_settings: false,
            show_timer: false,
            timer_minutes: 60,
            timer_power_on: false,
            active_timer: None,
            minimize_to_tray: true,
            window_visible: true,
            allow_close: false,
            busy: true,
            auto_refresh: true,
            last_refresh: Instant::now(),
            status_message: initial_msg,
            is_error: false,
        };
        app.sync_tray();
        if let Some(first) = app.devices.first().cloned() {
            let _ = app.cmd_tx.send(WorkerCmd::ProbeCached(first));
        } else {
            let _ = app.cmd_tx.send(WorkerCmd::Scan(None));
        }
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

    fn sync_tray(&self) {
        if let Some(ref tray) = self.tray {
            let device_name = self
                .selected_device()
                .map(|d| d.display_name())
                .unwrap_or_else(|| "Sin equipo conectado".to_string());
            tray.update(TrayState {
                power: self.status.as_ref().map(|s| s.power),
                set_temp: self.status.as_ref().map(|s| s.set_temp),
                room_temp: self.status.as_ref().and_then(|s| s.current_temp),
                device_name,
                window_visible: self.window_visible,
            });
        }
    }

    fn trigger_set_timer(&mut self, minutes: u32, power_on: bool) {
        if let Some(dev) = self.selected_device().cloned() {
            let mins = minutes.clamp(1, 1440);
            let target = Local::now() + ChronoDuration::minutes(i64::from(mins));
            let hour = target.hour();
            let minute = target.minute();
            let weekday_sun0 = target.weekday().num_days_from_sunday() as usize;
            let target_label = format!("{hour:02}:{minute:02}");

            self.busy = true;
            self.is_error = false;
            self.status_message = format!("Programando temporizador para las {target_label}...");
            let _ = self.cmd_tx.send(WorkerCmd::SetTimer {
                device: dev,
                power_on,
                hour,
                minute,
                weekday_sun0,
                minutes_from_now: u64::from(mins),
                target_label,
            });
        }
    }

    fn trigger_cancel_timer(&mut self) {
        if let Some(timer) = self.active_timer.take() {
            if timer.on_device
                && let Some(dev) = self.selected_device().cloned()
            {
                self.busy = true;
                self.status_message = "Cancelando temporizador...".to_string();
                let _ = self.cmd_tx.send(WorkerCmd::CancelTimer {
                    device: dev,
                    power_on: timer.power_on,
                    hour: timer.hour,
                    minute: timer.minute,
                    weekday_sun0: timer.weekday_sun0,
                });
            } else {
                self.is_error = false;
                self.status_message = "Temporizador cancelado".to_string();
            }
        }
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        while let Ok(action) = self.tray_rx.try_recv() {
            match action {
                TrayAction::ToggleWindow => {
                    self.window_visible = !self.window_visible;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.window_visible));
                    if self.window_visible {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    self.sync_tray();
                }
                TrayAction::TogglePower => {
                    if let Some(mut st) = self.status.clone() {
                        st.power = !st.power;
                        self.status = Some(st.clone());
                        self.send_cmd(vec![("Pow", i64::from(st.power))]);
                        self.sync_tray();
                    }
                }
                TrayAction::Refresh => {
                    self.trigger_refresh();
                }
                TrayAction::Quit => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn check_active_timer(&mut self) {
        let expired = self
            .active_timer
            .as_ref()
            .is_some_and(|t| Instant::now() >= t.deadline);
        if expired && let Some(timer) = self.active_timer.take() {
            if !timer.on_device {
                // App-side fallback timer executes power command now
                self.send_cmd(vec![("Pow", i64::from(timer.power_on))]);
            } else {
                // Device-side timer should have triggered; refresh status
                self.trigger_refresh();
            }
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
                WorkerEvent::FallbackScanning(msg) => {
                    self.busy = true;
                    self.is_error = false;
                    self.status_message = msg;
                }
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
                        self.sync_tray();
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
                WorkerEvent::TimerSetResult { timer, message } => {
                    self.busy = false;
                    self.is_error = false;
                    self.active_timer = Some(timer);
                    self.status_message = message;
                }
                WorkerEvent::TimerCancelled(msg) => {
                    self.busy = false;
                    self.is_error = false;
                    self.active_timer = None;
                    self.status_message = msg;
                }
            }
        }
    }
}

impl eframe::App for GreeRemoteApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested())
            && self.tray.is_some()
            && self.minimize_to_tray
            && !self.allow_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.window_visible = false;
            self.sync_tray();
            return;
        }

        self.poll_tray(ctx);
        self.poll_events();
        self.check_active_timer();

        if self.auto_refresh
            && !self.busy
            && self.selected_device().is_some()
            && self.last_refresh.elapsed() >= Duration::from_secs(10)
        {
            self.trigger_refresh();
        }
        if (self.auto_refresh && self.selected_device().is_some()) || self.active_timer.is_some() {
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
                            Icon::Timer,
                            "Temporizador de encendido / apagado",
                            self.selected_device().is_some(),
                            self.show_timer || self.active_timer.is_some(),
                            accent,
                        ) {
                            self.show_timer = !self.show_timer;
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
                        if let Some(ref timer) = self.active_timer {
                            let rem_secs = timer
                                .deadline
                                .saturating_duration_since(Instant::now())
                                .as_secs();
                            let rem_h = rem_secs / 3600;
                            let rem_m = (rem_secs % 3600) / 60;
                            let action = if timer.power_on { "Encender" } else { "Apagar" };
                            let where_str = if timer.on_device { "equipo" } else { "app" };
                            format!(
                                "{action} a las {} ({rem_h}h {rem_m:02}m · {where_str})",
                                timer.target_time_label
                            )
                        } else if let Some(dev) = self.selected_device() {
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
                                if self.tray.is_some() {
                                    ui.checkbox(&mut self.minimize_to_tray, "Minimizar a bandeja");
                                }
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

                    // Collapsible Timer / Schedule Panel
                    if self.show_timer && self.selected_device().is_some() {
                        theme::card_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (ir, _) =
                                    ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
                                draw_icon(ui.painter(), ir.center(), 7.5, Icon::Timer, accent);
                                ui.label(
                                    RichText::new("Temporizador")
                                        .size(14.0)
                                        .strong()
                                        .color(TEXT_PRIMARY),
                                );
                            });
                            ui.add_space(6.0);

                            if let Some(timer) = self.active_timer.clone() {
                                let rem_secs = timer
                                    .deadline
                                    .saturating_duration_since(Instant::now())
                                    .as_secs();
                                let rem_h = rem_secs / 3600;
                                let rem_m = (rem_secs % 3600) / 60;
                                let action = if timer.power_on { "Encender" } else { "Apagar" };
                                let mode_str = if timer.on_device {
                                    "guardado en el aire"
                                } else {
                                    "activo en segundo plano"
                                };
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{action} a las {} (en {rem_h}h {rem_m:02}m · {mode_str})",
                                            timer.target_time_label
                                        ))
                                        .color(accent),
                                    );
                                    if ui
                                        .add_enabled(!self.busy, egui::Button::new("Cancelar"))
                                        .clicked()
                                    {
                                        self.trigger_cancel_timer();
                                    }
                                });
                            } else {
                                ui.horizontal_wrapped(|ui| {
                                    ui.selectable_value(&mut self.timer_power_on, false, "Apagar");
                                    ui.selectable_value(&mut self.timer_power_on, true, "Encender");
                                    ui.separator();
                                    for (mins, lbl) in
                                        [(30u32, "30m"), (60, "1h"), (120, "2h"), (180, "3h")]
                                    {
                                        if ui
                                            .selectable_label(self.timer_minutes == mins, lbl)
                                            .clicked()
                                        {
                                            self.timer_minutes = mins;
                                        }
                                    }
                                    ui.add(
                                        egui::DragValue::new(&mut self.timer_minutes)
                                            .range(1..=1440)
                                            .suffix(" min"),
                                    );
                                    if ui
                                        .add_enabled(!self.busy, egui::Button::new("Programar"))
                                        .clicked()
                                    {
                                        self.trigger_set_timer(
                                            self.timer_minutes,
                                            self.timer_power_on,
                                        );
                                    }
                                });
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

                    // 1. Compact Split 2-Column Hero Card (Mini-Dial + Power / Ambient / Stepper)
                    theme::card_frame().show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        let prev_temp = st.set_temp;
                        let (new_temp, power_toggled) = widgets::compact_thermostat_hero(
                            ui,
                            &mut st.set_temp,
                            st.current_temp,
                            st.power,
                            accent,
                            !self.busy,
                        );
                        if let Some(t) = new_temp {
                            self.status = Some(st.clone());
                            self.send_cmd(vec![("TemUn", 0), ("SetTem", t)]);
                        } else if st.set_temp != prev_temp {
                            self.status = Some(st.clone());
                        }
                        if power_toggled {
                            st.power = !st.power;
                            self.status = Some(st.clone());
                            self.send_cmd(vec![("Pow", i64::from(st.power))]);
                        }
                    });

                    ui.add_space(8.0);

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
