use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use eframe::egui;

use crate::dlss5::{self, Dlss5State};
use crate::fg::{self, FgStatus};
use crate::game::{self, GameInfo};
use crate::gpu::{self, Gpu, GpuClass};
use crate::nr::{self, LogFacts, NrSettings};
use crate::steam::{self, SteamGame};
use crate::update::{self, Release};

enum Msg {
    Log(String),
    Analyzed(Box<Result<GameInfo, String>>),
    Done,
}

enum UpdateMsg {
    Checked(Result<Option<Release>, String>),
    Failed(String),
}

enum UpdateState {
    Unknown,
    UpToDate,
    Available(Release),
    Working,
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Keep,
    Install,
    Remove,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Apply { fg: Op, dlss5: Op },
    Check,
}

struct Row {
    op_if_ticked: Op,
    enabled: bool,
    reason: Option<String>,
}

pub struct App {
    games: Vec<SteamGame>,
    steam_error: Option<String>,
    filter: String,
    selected: Option<PathBuf>,
    info: Option<Result<GameInfo, String>>,
    gpu: Result<Gpu, String>,
    fg_tick: bool,
    dlss5_tick: bool,
    legacy: bool,
    route: String,
    log: Vec<String>,
    busy: bool,
    rx: Option<Receiver<Msg>>,
    fg_status: Option<FgStatus>,
    dlss5_status: Option<Dlss5State>,
    nr: Option<NrSettings>,
    nr_facts: Option<LogFacts>,
    nr_dirty: bool,
    update: UpdateState,
    urx: Option<Receiver<UpdateMsg>>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(14.0, 6.0);
        style.spacing.indent = 24.0;
        cc.egui_ctx.set_style(style);
        let (games, steam_error) = match steam::scan() {
            Ok(g) => (g, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let gpu = gpu::detect();
        let mut app = Self {
            games,
            steam_error,
            filter: String::new(),
            selected: None,
            info: None,
            gpu,
            fg_tick: false,
            dlss5_tick: false,
            legacy: false,
            route: "native".into(),
            log: Vec::new(),
            busy: false,
            rx: None,
            fg_status: None,
            dlss5_status: None,
            nr: None,
            nr_facts: None,
            nr_dirty: false,
            update: UpdateState::Unknown,
            urx: None,
        };
        update::cleanup_old();
        app.push(format!("rtx-unlock {}", update::current_version()));
        app.start_update_check();
        match &app.gpu {
            Ok(g) => app.push(format!("GPU: {} | driver {} | {}", g.name, g.driver, g.class.label())),
            Err(e) => app.push(format!("GPU: {e}")),
        }
        if let Some(e) = &app.steam_error {
            app.push(format!("Steam: {e}"));
        } else {
            app.push(format!("Steam: {} games found", app.games.len()));
        }
        if let Some(p) = dlss5::find_autopilot() {
            app.push(format!("DLSS 5 Autopilot: {}", p.display()));
        } else {
            app.push("DLSS 5 Autopilot: not present, downloaded on the first DLSS 5 install".into());
        }
        if let Some(dir) = std::env::args().nth(1) {
            let p = PathBuf::from(dir);
            if p.is_dir() {
                app.select(p);
            }
        }
        app
    }

    fn push(&mut self, s: String) {
        self.log.push(s);
    }

    fn start_update_check(&mut self) {
        let (tx, rx) = channel();
        self.urx = Some(rx);
        thread::spawn(move || {
            let _ = tx.send(UpdateMsg::Checked(update::check()));
        });
    }

    fn start_update(&mut self) {
        let UpdateState::Available(rel) = &self.update else { return };
        let rel = rel.clone();
        self.update = UpdateState::Working;
        let (tx, rx) = channel();
        self.urx = Some(rx);
        let (ltx, lrx) = channel();
        self.rx = Some(lrx);
        self.busy = true;
        thread::spawn(move || {
            let log = move |s: String| {
                let _ = ltx.send(Msg::Log(s));
            };
            if let Err(e) = update::apply(&rel, &log) {
                let _ = tx.send(UpdateMsg::Failed(e));
            }
        });
    }

    fn poll_update(&mut self) {
        let mut msg = None;
        if let Some(rx) = &self.urx {
            if let Ok(m) = rx.try_recv() {
                msg = Some(m);
            }
        }
        let Some(m) = msg else { return };
        self.urx = None;
        match m {
            UpdateMsg::Checked(Ok(Some(rel))) => {
                self.push(format!("version {} is available", rel.version));
                self.update = UpdateState::Available(rel);
            }
            UpdateMsg::Checked(Ok(None)) => self.update = UpdateState::UpToDate,
            UpdateMsg::Checked(Err(e)) => {
                self.push(format!("update check failed: {e}"));
                self.update = UpdateState::Failed(e);
            }
            UpdateMsg::Failed(e) => {
                self.push(format!("ERROR [update]: {e}"));
                self.update = UpdateState::Failed(e);
                self.busy = false;
                self.rx = None;
            }
        }
    }

    fn fg_install_allowed(&self) -> Result<(), String> {
        let gpu = self.gpu.as_ref().map_err(|e| e.clone())?;
        match gpu.class {
            GpuClass::Sm75 | GpuClass::Sm86 => {}
            GpuClass::NativeFg => return Err("RTX 40/50 already run DLSS Frame Generation".into()),
            GpuClass::Unsupported => return Err("unsupported GPU".into()),
        }
        match &self.info {
            Some(Ok(i)) if i.dlssg => Ok(()),
            Some(Ok(_)) => Err("the game ships no DLSS-G DLL, nothing to unlock".into()),
            Some(Err(e)) => Err(e.clone()),
            None => Err("no game selected".into()),
        }
    }

    fn fg_row(&self) -> Row {
        match &self.fg_status {
            Some(FgStatus::Installed(_)) => Row { op_if_ticked: Op::Remove, enabled: true, reason: None },
            Some(FgStatus::Manual(_)) => Row {
                op_if_ticked: Op::Keep,
                enabled: false,
                reason: Some("installed by hand, remove it by hand".into()),
            },
            _ => match self.fg_install_allowed() {
                Ok(()) => Row { op_if_ticked: Op::Install, enabled: true, reason: None },
                Err(e) => Row { op_if_ticked: Op::Keep, enabled: false, reason: Some(e) },
            },
        }
    }

    fn dlss5_row(&self) -> Row {
        if self.dlss5_status.is_some() {
            Row { op_if_ticked: Op::Remove, enabled: true, reason: None }
        } else {
            Row { op_if_ticked: Op::Install, enabled: true, reason: None }
        }
    }

    fn select(&mut self, dir: PathBuf) {
        if self.busy {
            return;
        }
        self.selected = Some(dir.clone());
        self.info = None;
        self.fg_status = None;
        self.dlss5_status = None;
        self.nr = None;
        self.nr_facts = None;
        self.nr_dirty = false;
        self.push(format!("--- {}", dir.display()));
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.busy = true;
        thread::spawn(move || {
            let r = game::analyze(&dir);
            let _ = tx.send(Msg::Analyzed(Box::new(r)));
            let _ = tx.send(Msg::Done);
        });
    }

    fn refresh_status(&mut self) {
        if let Some(Ok(i)) = &self.info {
            self.fg_status = Some(fg::status(i));
            self.dlss5_status = dlss5::status(&i.proxy_dir);
            self.nr = if self.dlss5_status.is_some() {
                Some(nr::load(&i.proxy_dir).unwrap_or_default())
            } else {
                None
            };
            self.nr_facts = nr::read_log(&i.proxy_dir);
            self.nr_dirty = false;
        }
        let fg = self.fg_row();
        let d5 = self.dlss5_row();
        self.fg_tick = fg.enabled && fg.op_if_ticked == Op::Install;
        self.dlss5_tick = d5.enabled && d5.op_if_ticked == Op::Install;
    }

    fn save_nr(&mut self) {
        let (Some(Ok(i)), Some(s)) = (&self.info, &self.nr) else { return };
        match nr::save(&i.proxy_dir, s) {
            Ok(()) => {
                self.nr_dirty = false;
                self.push(format!(
                    "NR settings written (paper-white {}, style {}). If the game is running, quit and relaunch it: ReShade rewrites the ini on exit.",
                    s.paper_white,
                    nr::STYLE_NAMES.get(s.style as usize).copied().unwrap_or("?")
                ));
            }
            Err(e) => self.push(format!("ERROR [NR settings]: {e}")),
        }
    }

    fn log_nr_facts(log: &dyn Fn(String), engine: game::Engine, facts: Option<&LogFacts>, settings: Option<&NrSettings>) {
        if let Some(s) = settings {
            log(format!(
                "NR settings: enabled={} style={} intensity={} paper-white={}",
                s.enabled,
                nr::STYLE_NAMES.get(s.style as usize).copied().unwrap_or("?"),
                s.intensity,
                s.paper_white
            ));
        }
        let Some(f) = facts else { return };
        if let Some(d) = &f.driver {
            if nr::driver_faults_new_addon(d) {
                log(format!("driver {d}: renodx-dlss5 4.6/4.7 fault on every frame with this driver, so 4.55 is used. The automatic colour bridge (4.7) needs driver 616.56."));
            }
        }
        if f.hdr_codec {
            log("NR HDR codec active: the game hands DLSS a scene-linear (pre-tonemap) buffer, so paper-white decides how the frame looks.".into());
            if let Some(s) = settings {
                if s.paper_white <= 1.0 {
                    log(format!(
                        "paper-white {} is too low and produces a grey frame; start at {} (measured on RE Engine).",
                        s.paper_white,
                        nr::RE_ENGINE_PAPER_WHITE
                    ));
                }
            }
        } else if engine == game::Engine::ReEngine {
            log("RE Engine: NR has not run yet (no codec line in the log); paper-white will matter on the first run.".into());
        }
        if f.nr_evaluated {
            log("NR processed at least one frame (ReShade.log).".into());
        }
        if let Some(d) = &f.device_lost {
            log(format!("GPU device lost in the last session: {d}. NR plus DLSS FG can hit the TDR limit together; drop FG to 2X or try with NR off."));
        }
    }

    fn start(&mut self, action: Action) {
        let Some(Ok(info)) = self.info.clone() else { return };
        let gpu = self.gpu.clone();
        let legacy = self.legacy;
        let route = self.route.clone();
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.busy = true;
        thread::spawn(move || {
            let tx2: Sender<Msg> = tx.clone();
            let log = move |s: String| {
                let _ = tx2.send(Msg::Log(s));
            };
            let step = |name: &str, r: Result<(), String>| match r {
                Ok(()) => true,
                Err(e) => {
                    log(format!("ERROR [{name}]: {e}"));
                    false
                }
            };
            match action {
                Action::Apply { fg: fg_op, dlss5: d5_op } => {
                    let mut ok = true;
                    match fg_op {
                        Op::Install => match &gpu {
                            Ok(g) => ok = step("FG unlock", fg::install(&info, g, legacy, &log)),
                            Err(e) => ok = step("FG unlock", Err(e.clone())),
                        },
                        Op::Remove => {
                            step("FG unlock", fg::remove(&info, &log));
                        }
                        Op::Keep => {}
                    }
                    match d5_op {
                        Op::Install if ok => {
                            let r = dlss5::ensure_autopilot(&log)
                                .and_then(|exe| dlss5::install(&exe, &info.root, &route, &log));
                            if step("DLSS 5", r) && info.engine == game::Engine::ReEngine {
                                let pw = nr::RE_ENGINE_PAPER_WHITE.to_string();
                                match nr::set_if_absent(&info.proxy_dir, "NRPaperWhiteScale", &pw) {
                                    Ok(true) => log(format!(
                                        "RE Engine: NR paper-white preset to {pw} (scene-linear buffer; 1.0 gives a grey frame)."
                                    )),
                                    Ok(false) => {}
                                    Err(e) => log(format!("cannot write NR paper-white: {e}")),
                                }
                            }
                        }
                        Op::Install => log("DLSS 5 skipped because the FG unlock failed".into()),
                        Op::Remove => {
                            let r = dlss5::ensure_autopilot(&log)
                                .and_then(|exe| dlss5::remove(&exe, &info.root, &log));
                            step("DLSS 5", r);
                        }
                        Op::Keep => {}
                    }
                    if fg_op == Op::Keep && d5_op == Op::Keep {
                        log("nothing ticked".into());
                    }
                }
                Action::Check => {
                    log(format!("FG unlock: {}", fg::status(&info).label()));
                    match dlss5::status(&info.proxy_dir) {
                        Some(s) => {
                            log(format!("DLSS 5: {}", s.label()));
                            let settings = nr::load(&info.proxy_dir);
                            let facts = nr::read_log(&info.proxy_dir);
                            Self::log_nr_facts(&log, info.engine, facts.as_ref(), settings.as_ref());
                        }
                        None => log("DLSS 5: not installed".into()),
                    }
                    if let Some(exe) = dlss5::find_autopilot() {
                        let _ = dlss5::check(&exe, &info.root, &log);
                    }
                }
            }
            let _ = tx.send(Msg::Done);
        });
    }

    fn poll(&mut self) {
        let mut done = false;
        let mut analyzed = None;
        let mut logs = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(m) = rx.try_recv() {
                match m {
                    Msg::Log(s) => logs.push(s),
                    Msg::Analyzed(r) => analyzed = Some(*r),
                    Msg::Done => done = true,
                }
            }
        }
        for s in logs {
            self.push(s);
        }
        if let Some(r) = analyzed {
            match &r {
                Ok(i) => {
                    self.push(format!("exe: {}", i.exe.display()));
                    self.push(format!("engine: {}", i.engine.label()));
                    self.push(format!("DLSS-G DLL: {}", if i.dlssg { "present" } else { "missing" }));
                }
                Err(e) => self.push(format!("analysis failed: {e}")),
            }
            self.info = Some(r);
        }
        if done {
            self.busy = false;
            self.rx = None;
            self.refresh_status();
        }
    }

    fn component_row(ui: &mut egui::Ui, title: &str, status: &str, row: &Row, tick: &mut bool) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.strong(title);
            ui.label(status);
        });
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let label = match row.op_if_ticked {
                Op::Install => "Install",
                Op::Remove => "Remove",
                Op::Keep => "Keep",
            };
            changed = ui.add_enabled(row.enabled, egui::Checkbox::new(tick, label)).changed();
            if let Some(r) = &row.reason {
                ui.colored_label(egui::Color32::GRAY, r);
            }
        });
        changed
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.poll_update();
        if self.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let mut pick: Option<PathBuf> = None;
        let margin = egui::Margin::same(14);

        let mut do_update = false;
        let banner = match &self.update {
            UpdateState::Available(r) => Some((format!("Version {} is available.", r.version), true)),
            UpdateState::Working => Some(("Updating, the app restarts by itself.".to_string(), false)),
            UpdateState::Failed(e) => Some((format!("Update failed: {e}"), false)),
            _ => None,
        };
        if let Some((text, offer)) = banner {
            egui::TopBottomPanel::top("update")
                .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(14, 8)))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(text);
                        if offer && ui.add_enabled(!self.busy, egui::Button::new("Update now")).clicked() {
                            do_update = true;
                        }
                    });
                });
        }

        egui::SidePanel::left("games")
            .min_width(280.0)
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(margin))
            .show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut self.filter);
            });
            if ui.add_enabled(!self.busy, egui::Button::new("Choose folder...")).clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    pick = Some(d);
                }
            }
            ui.separator();
            let f = self.filter.to_lowercase();
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.games.is_empty() {
                    ui.label(match &self.steam_error {
                        Some(e) => format!("{e}. Use \"Choose folder\"."),
                        None => "No Steam games found. Use \"Choose folder\".".into(),
                    });
                }
                for g in &self.games {
                    if !f.is_empty() && !g.name.to_lowercase().contains(&f) {
                        continue;
                    }
                    let sel = self.selected.as_ref() == Some(&g.dir);
                    if ui.selectable_label(sel, &g.name).clicked() && !sel {
                        pick = Some(g.dir.clone());
                    }
                }
            });
        });

        egui::TopBottomPanel::bottom("log")
            .min_height(180.0)
            .resizable(true)
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(margin))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Log");
                if ui.small_button("clear").clicked() {
                    self.log.clear();
                }
            });
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                for l in &self.log {
                    ui.monospace(l);
                }
            });
        });

        let mut action: Option<Action> = None;
        let mut nr_save = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(margin))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("rtx-unlock");
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::GRAY, env!("CARGO_PKG_VERSION"));
            });
            match &self.gpu {
                Ok(g) => {
                    ui.label(format!("GPU: {} | driver {} | {}", g.name, g.driver, g.class.label()));
                }
                Err(e) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                }
            }
            ui.separator();

            match &self.info {
                None => {
                    ui.label(if self.busy { "Inspecting the game..." } else { "Pick a game on the left." });
                }
                Some(Err(e)) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("Cannot inspect this folder: {e}"));
                    ui.label("Choose the folder that holds the game's own executable and try again.");
                }
                Some(Ok(i)) => {
                    ui.label(format!("exe: {}", i.exe.display()));
                    ui.label(format!("engine: {}", i.engine.label()));
                    ui.label(format!(
                        "DLSS-G DLL: {}",
                        if i.dlssg { "present" } else { "missing (the game has no DLSS Frame Generation of its own)" }
                    ));
                    let imported: Vec<&str> = fg::PROXY_ORDER
                        .iter()
                        .copied()
                        .filter(|p| i.imports.iter().any(|x| x == p))
                        .collect();
                    ui.label(format!("proxy names the exe loads: {}", imported.join(", ")));
                    if i.present.is_empty() {
                        ui.label("proxies already in the folder: none");
                    } else {
                        let taken: Vec<String> =
                            i.present.iter().map(|(n, o)| format!("{n} ({o})")).collect();
                        ui.label(format!("proxies already in the folder: {}", taken.join(", ")));
                    }
                    if let Some(f) = &self.nr_facts {
                        if let Some(d) = &f.device_lost {
                            ui.colored_label(egui::Color32::LIGHT_RED, format!("GPU device lost in the last session: {d}"));
                        }
                    }
                    ui.separator();

                    let fg_row = self.fg_row();
                    let d5_row = self.dlss5_row();
                    let fg_status = self.fg_status.as_ref().map(|s| s.label()).unwrap_or_default();
                    let d5_status = self.dlss5_status.as_ref().map(|s| s.label()).unwrap_or("not installed".into());

                    Self::component_row(ui, "Frame Generation unlock", &fg_status, &fg_row, &mut self.fg_tick);
                    if self.fg_tick && fg_row.op_if_ticked == Op::Install {
                        ui.horizontal(|ui| {
                            ui.add_space(40.0);
                            ui.checkbox(&mut self.legacy, "use the legacy 0.1.0 build (some games black-screen on 0.2.x)");
                        });
                    }
                    ui.add_space(6.0);
                    Self::component_row(ui, "DLSS 5 neural rendering", &d5_status, &d5_row, &mut self.dlss5_tick);
                    if self.dlss5_tick && d5_row.op_if_ticked == Op::Install {
                        ui.horizontal(|ui| {
                            ui.add_space(40.0);
                            ui.label("route:");
                            egui::ComboBox::from_id_salt("route")
                                .selected_text(&self.route)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.route, "native".into(), "native");
                                    ui.selectable_value(&mut self.route, "upstream".into(), "upstream");
                                });
                        });
                    }
                    ui.add_space(8.0);

                    let fg_op = if self.fg_tick && fg_row.enabled { fg_row.op_if_ticked } else { Op::Keep };
                    let d5_op = if self.dlss5_tick && d5_row.enabled { d5_row.op_if_ticked } else { Op::Keep };
                    let installs = [fg_op, d5_op].iter().filter(|o| **o == Op::Install).count();
                    let removes = [fg_op, d5_op].iter().filter(|o| **o == Op::Remove).count();
                    let apply_label = match (installs, removes) {
                        (0, 0) => "Nothing ticked".to_string(),
                        (n, 0) => format!("Install {n}"),
                        (0, n) => format!("Remove {n}"),
                        (a, b) => format!("Install {a}, remove {b}"),
                    };
                    ui.horizontal(|ui| {
                        let can_apply = !self.busy && (installs + removes) > 0;
                        if ui.add_enabled(can_apply, egui::Button::new(&apply_label)).clicked() {
                            action = Some(Action::Apply { fg: fg_op, dlss5: d5_op });
                        }
                        if ui.add_enabled(!self.busy, egui::Button::new("Check")).clicked() {
                            action = Some(Action::Check);
                        }
                        if self.busy {
                            ui.spinner();
                            ui.label("working...");
                        }
                    });
                    if removes > 0 {
                        ui.colored_label(egui::Color32::GRAY, "Remove deletes only the files listed in this tool's manifest.");
                    }
                    ui.separator();

                    if let Some(s) = &mut self.nr {
                        let hdr = self.nr_facts.as_ref().map_or(false, |f| f.hdr_codec);
                        ui.collapsing("DLSS 5 NR settings (ReShade.ini)", |ui| {
                            let mut changed = false;
                            changed |= ui.checkbox(&mut s.enabled, "neural rendering enabled").changed();
                            ui.horizontal(|ui| {
                                ui.label("style:");
                                egui::ComboBox::from_id_salt("nr_style")
                                    .selected_text(nr::STYLE_NAMES.get(s.style as usize).copied().unwrap_or("?"))
                                    .show_ui(ui, |ui| {
                                        for (i, n) in nr::STYLE_NAMES.iter().enumerate() {
                                            changed |= ui.selectable_value(&mut s.style, i as i32, *n).changed();
                                        }
                                    });
                                ui.label("preset:");
                                egui::ComboBox::from_id_salt("nr_preset")
                                    .selected_text(nr::PRESET_NAMES.get(s.preset as usize).copied().unwrap_or("?"))
                                    .show_ui(ui, |ui| {
                                        for (i, n) in nr::PRESET_NAMES.iter().enumerate() {
                                            changed |= ui.selectable_value(&mut s.preset, i as i32, *n).changed();
                                        }
                                    });
                            });
                            changed |= ui.add(egui::Slider::new(&mut s.intensity, 0.0..=1.0).text("intensity")).changed();
                            changed |= ui.add(egui::Slider::new(&mut s.local_tone, 0.0..=1.0).text("local tone")).changed();
                            changed |= ui.add(egui::Slider::new(&mut s.local_structure, 0.0..=1.0).text("local structure")).changed();
                            changed |= ui.add(egui::Slider::new(&mut s.skin_structure, -1.0..=1.0).text("skin structure (-1 = follow local)")).changed();
                            changed |= ui.add(egui::Slider::new(&mut s.paper_white, 1.0..=64.0).logarithmic(true).text("paper-white (scene-linear HDR)")).changed();
                            ui.horizontal(|ui| {
                                changed |= ui.checkbox(&mut s.auto_mask, "automatic skin mask").changed();
                                changed |= ui.checkbox(&mut s.ui_correction, "UI correction").changed();
                            });
                            if hdr && s.paper_white <= 1.0 {
                                ui.colored_label(
                                    egui::Color32::YELLOW,
                                    format!("This game hands DLSS a pre-tonemap linear buffer; paper-white 1 gives a grey frame. Start at {}.", nr::RE_ENGINE_PAPER_WHITE),
                                );
                            } else if hdr {
                                ui.colored_label(egui::Color32::GRAY, "HDR codec active: tune paper-white between 8 and 32 to the scene exposure; lower for dark scenes, higher for bright ones.");
                            }
                            if changed {
                                self.nr_dirty = true;
                            }
                            ui.horizontal(|ui| {
                                let can_save = self.nr_dirty && !self.busy;
                                if ui.add_enabled(can_save, egui::Button::new("Save NR settings")).clicked() {
                                    nr_save = true;
                                }
                                ui.colored_label(egui::Color32::GRAY, "save while the game is closed");
                            });
                        });
                        if let Some(f) = &self.nr_facts {
                            if let Some(d) = &f.driver {
                                if nr::driver_faults_new_addon(d) {
                                    ui.colored_label(
                                        egui::Color32::GRAY,
                                        format!("driver {d}: add-on 4.55 (manual paper-white). The 4.7 build with the automatic colour bridge only runs on driver 616.56."),
                                    );
                                }
                            }
                        }
                        ui.separator();
                    }

                    ui.colored_label(
                        egui::Color32::GRAY,
                        "Online games with anti-cheat can ban for this. Antivirus may quarantine the DLLs; exclude the game folder.",
                    );
                    if i.engine == game::Engine::ReEngine {
                        ui.colored_label(
                            egui::Color32::GRAY,
                            "RE Engine: the FG unlock goes into reframework\\plugins. If REFramework is missing, the nightly dinput8.dll is installed for you.",
                        );
                    }
                }
            }
        });

        if do_update {
            self.start_update();
        }
        if let Some(d) = pick {
            self.select(d);
        }
        if nr_save {
            self.save_nr();
        }
        if let Some(a) = action {
            self.start(a);
        }
    }
}
