//! Launcher/hub visual do Glear-Mine — UI nativa em egui (eframe).
//!
//! Telas no estilo glearflame/horizon (MIT):
//! - Title Screen (menu principal)
//! - Opções (vídeo, controles, etc.)
//! - Vídeo (configurações completas)
//! - Jogar (lançar Minecraft)
//!
//! Créditos: estrutura baseada em glearflame/horizon
//! Licença: MIT (veja LICENSE)

mod app;

use eframe::egui;
use ll_hud::{
    title_screen::{TitleButton, TitleScreen},
    video_settings::VideoSettings,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const HUB_NAME: &str = "𝕷&𝕷-𝕸𝖎𝖓𝖊";
const LAUNCHER_VERSION: &str = "Launcher Offline 1.5.0";
const DEFAULT_GAME_VERSION: &str = "v0.2.971";

/// Telas do launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Title,
    Options,
    Video,
    Launch,
}

impl Default for Screen {
    fn default() -> Self {
        Self::Title
    }
}

/// Estado principal do launcher.
struct LauncherApp {
    screen: Screen,
    title_screen: TitleScreen,
    video_settings: VideoSettings,
    launch_state: Arc<Mutex<app::launch_button::LaunchState>>,
    java_path: PathBuf,
    install_path: PathBuf,
    installing: bool,
}

impl Default for LauncherApp {
    fn default() -> Self {
        let install_path = dirs::data_dir()
            .map(|d| d.join("Glear-Mine").join("versions").join("26.3"))
            .unwrap_or_else(|| PathBuf::from("versions/26.3"));

        let java_path = PathBuf::from("/usr/bin/java");

        let mut launch_state = app::launch_button::LaunchState::new();
        app::launch_button::detect_version_info(&mut launch_state, &install_path);

        Self {
            screen: Screen::Title,
            title_screen: TitleScreen::new(),
            video_settings: VideoSettings::new(),
            launch_state: Arc::new(Mutex::new(launch_state)),
            java_path,
            install_path,
            installing: false,
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Repaint sempre para manter a UI responsiva
        ctx.request_repaint();

        match self.screen {
            Screen::Title => self.render_title(ctx),
            Screen::Options => self.render_options(ctx),
            Screen::Video => self.render_video(ctx),
            Screen::Launch => self.render_launch(ctx),
        }
    }
}

impl LauncherApp {
    /// Tela de título (menu principal do Horizon).
    fn render_title(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let clicked = self.title_screen.render(ui);

            match clicked {
                Some(TitleButton::Jogar) => self.screen = Screen::Launch,
                Some(TitleButton::Multijogador) => {
                    // TODO: tela de servidores/multiplayer
                    self.screen = Screen::Title;
                }
                Some(TitleButton::Opcoes) => self.screen = Screen::Options,
                Some(TitleButton::Sair) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                None => {}
            }
        });
    }

    /// Tela de opções (menu lateral esquerdo + conteúdo direito).
    fn render_options(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("options_sidebar").default_width(200.0).show(ctx, |ui| {
            ui.heading(egui::RichText::new("Opções").strong().color(egui::Color32::from_rgb(220, 220, 220)));
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            let btn_style = egui::Button::new(
                egui::RichText::new("Vídeo").strong().color(egui::Color32::from_rgb(255, 255, 255))
            ).min_size(egui::Vec2::new(160.0, 40.0))
             .fill(egui::Color32::from_rgb(60, 60, 70));

            if ui.add(btn_style).clicked() {
                self.screen = Screen::Video;
            }
            ui.add_space(8.0);

            let btn_controles = egui::Button::new(
                egui::RichText::new("Controles").strong().color(egui::Color32::from_rgb(255, 255, 255))
            ).min_size(egui::Vec2::new(160.0, 40.0))
             .fill(egui::Color32::from_rgb(40, 40, 50));

            if ui.add(btn_controles).clicked() {
                // TODO: tela de controles
            }
            ui.add_space(8.0);

            let btn_sons = egui::Button::new(
                egui::RichText::new("Música e Sons").strong().color(egui::Color32::from_rgb(255, 255, 255))
            ).min_size(egui::Vec2::new(160.0, 40.0))
             .fill(egui::Color32::from_rgb(40, 40, 50));

            if ui.add(btn_sons).clicked() {
                // TODO: tela de sons
            }
            ui.add_space(8.0);

            ui.add_space(40.0);

            if ui.button("Voltar").clicked() {
                self.screen = Screen::Title;
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(egui::RichText::new("Opções").size(24.0).strong().color(egui::Color32::from_rgb(220, 220, 220)));
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Selecione uma categoria à esquerda.").color(egui::Color32::from_gray(150)));
        });
    }

    /// Tela de configurações de vídeo (estilo Horizon).
    fn render_video(&mut self, ctx: &egui::Context) {
        // Sidebar esquerda com abas de vídeo
        egui::SidePanel::left("video_sidebar").default_width(180.0).show(ctx, |ui| {
            ui.heading(egui::RichText::new("Vídeo").strong().color(egui::Color32::from_rgb(220, 220, 220)));
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            let tabs = [("Display", "Exibição"), ("Quality & Performance", "Qualidade"), ("Preferences", "Preferências")];

            for (_id, label) in tabs.iter() {
                let btn = egui::Button::new(
                    egui::RichText::new(*label).strong().color(egui::Color32::from_rgb(255, 255, 255))
                ).min_size(egui::Vec2::new(150.0, 40.0))
                 .fill(egui::Color32::from_rgb(50, 50, 60));

                if ui.add(btn).clicked() {
                    // TODO: mudar aba interna do vídeo
                }
                ui.add_space(8.0);
            }

            ui.add_space(40.0);

            if ui.button("Voltar").clicked() {
                self.screen = Screen::Options;
            }
        });

        // Conteúdo: delega ao video_settings
        egui::CentralPanel::default().show(ctx, |ui| {
            self.video_settings.render(ui);
        });
    }

    /// Tela de lançamento (Instalar / Jogar).
    fn render_launch(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(egui::RichText::new("Jogar").size(24.0).strong().color(egui::Color32::from_rgb(220, 220, 220)));
            ui.add_space(20.0);

            egui::Frame::group(ui.style()).inner_margin(20.0).show(ui, |ui| {
                ui.label(egui::RichText::new("Versão 26.3").strong());

                // Info da versão
                let version_info = {
                    let s = self.launch_state.lock().unwrap_or_else(|e| e.into_inner());
                    s.version_info.clone()
                };
                if let Some(info) = &version_info {
                    ui.label(egui::RichText::new(info).small().color(egui::Color32::from_gray(150)));
                    ui.add_space(10.0);
                }

                // Memória e usuário
                let mut mem_mb: u32;
                let mut username: String;
                {
                    let s = self.launch_state.lock().unwrap();
                    mem_mb = s.mem_mb;
                    username = s.username.clone();
                }

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Memória (MB):").color(egui::Color32::from_rgb(180, 180, 180)));
                    ui.add(egui::DragValue::new(&mut mem_mb).range(512..=16384));
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Usuário:").color(egui::Color32::from_rgb(180, 180, 180)));
                    ui.text_edit_singleline(&mut username);
                });

                // Salva de volta
                {
                    let mut s = self.launch_state.lock().unwrap();
                    s.mem_mb = mem_mb;
                    s.username = username;
                }

                ui.add_space(20.0);

                // Botões
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.installing, egui::Button::new("📥 Instalar").min_size(egui::Vec2::new(120.0, 40.0))).clicked() {
                        self.installing = true;
                        let path = self.install_path.clone();
                        let state = self.launch_state.clone();
                        std::thread::spawn(move || {
                            app::launch_button::do_install(state, &path);
                        });
                    }
                    ui.add_space(10.0);
                    if ui.button("📁 Abrir pasta").clicked() {
                        let game_dir = dirs::data_local_dir()
                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                            .join("Glear-Mine/game");
                        let _ = std::fs::create_dir_all(&game_dir);
                        let _ = std::process::Command::new("xdg-open").arg(&game_dir).spawn();
                    }
                });

                ui.add_space(20.0);

                // Botão Jogar
                let is_launching = {
                    let s = self.launch_state.lock().unwrap_or_else(|e| e.into_inner());
                    s.is_launching
                };

                let play_btn = egui::Button::new(
                    if is_launching {
                        "🔄 Iniciando Minecraft..."
                    } else {
                        "▶ Jogar Glear-Mine"
                    }
                ).min_size(egui::Vec2::new(200.0, 50.0))
                 .fill(if is_launching {
                     egui::Color32::from_rgb(100, 100, 100)
                 } else {
                     egui::Color32::from_rgb(50, 150, 50)
                 });

                if ui.add_enabled(!is_launching && !self.installing, play_btn).clicked() {
                    let path = self.install_path.clone();
                    let state = self.launch_state.clone();
                    let java_path = self.java_path.clone();
                    let mem_mb = {
                        let s = self.launch_state.lock().unwrap();
                        s.mem_mb
                    };
                    let username = {
                        let s = self.launch_state.lock().unwrap();
                        s.username.clone()
                    };
                    std::thread::spawn(move || {
                        app::launch_button::launch_simple(state, &path, &java_path, mem_mb, username);
                    });
                }

                // Erro
                let error = {
                    let s = self.launch_state.lock().unwrap();
                    s.error.clone()
                };
                if let Some(err) = &error {
                    ui.add_space(10.0);
                    ui.colored_label(egui::Color32::from_rgb(255, 80, 80), format!("Erro: {err}"));
                }

                ui.add_space(20.0);

                if ui.button("Voltar").clicked() {
                    self.screen = Screen::Title;
                }
            });
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title(format!("{} - Launcher", HUB_NAME))
            .with_resizable(true),
        ..Default::default()
    };
    eframe::run_native(
        HUB_NAME,
        options,
        Box::new(|_cc| Ok(Box::new(LauncherApp::default()))),
    )
}
