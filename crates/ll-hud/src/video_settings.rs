//! Painel de configurações de vídeo — estilo horizon-rust/horizon (MIT).
//!
//! Recria o menu "Video Settings" completo com aparência autêntica do Minecraft:
//! painéis escuros, texto branco, botões cinza, sliders com trilho.
//!
//! Créditos: baseado na estrutura pública do horizon-rust/horizon
//! (https://github.com/horizon-rust/horizon), licença MIT.

use egui::Ui;

/// Aba do painel de vídeo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoTab {
    Display,
    QualityPerformance,
    Preferences,
}

impl Default for VideoTab {
    fn default() -> Self {
        Self::Display
    }
}

impl VideoTab {
    pub fn all() -> [Self; 3] {
        [Self::Display, Self::QualityPerformance, Self::Preferences]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Display => "Display",
            Self::QualityPerformance => "Quality & Performance",
            Self::Preferences => "Preferences",
        }
    }
}

/// Estado do painel de vídeo.
#[derive(Debug, Clone, Default)]
pub struct VideoSettings {
    pub tab: VideoTab,

    // --- Display ---
    pub exclusive_fullscreen: String,
    pub max_framerate: i32,
    pub vsync: String,
    pub reduce_fps_when: String,
    pub gui_scale: i32,
    pub fullscreen: bool,
    pub exclusive_fullscreen_mode: bool,
    pub brightness: String,
    pub graphics_api: String,

    // --- Quality & Performance ---
    pub preset: String,
    pub biome_blend: String,
    pub render_distance: i32,
    pub chunk_builder: String,
    pub simulation_distance: i32,
    pub smooth_lighting: String,
    pub clouds: String,
    pub particles: String,
    pub mipmap_levels: i32,
    pub entity_shadows: bool,
    pub entity_distance: String,
    pub menu_background_blur: i32,
    pub cloud_distance: i32,
    pub see_through_leaves: bool,
    pub improved_transparency: bool,
    pub texture_filtering: String,
    pub anisotropic_filtering: String,
    pub weather_effect_radius: i32,

    // --- Preferences ---
    pub autosave_indicator: bool,
    pub show_vignette: bool,
    pub attack_indicator: String,
    pub chunk_fade: String,
}

impl VideoSettings {
    pub fn new() -> Self {
        Self {
            tab: VideoTab::Display,

            // Display defaults.
            exclusive_fullscreen: "Current".to_string(),
            max_framerate: 250,
            vsync: "OFF".to_string(),
            reduce_fps_when: "Minimized".to_string(),
            gui_scale: 2,
            fullscreen: true,
            exclusive_fullscreen_mode: false,
            brightness: "Default".to_string(),
            graphics_api: "Prefer Vulkan".to_string(),

            // Quality & Performance defaults.
            preset: "Custom".to_string(),
            biome_blend: "3x3 (Fast)".to_string(),
            render_distance: 32,
            chunk_builder: "Threaded".to_string(),
            simulation_distance: 6,
            smooth_lighting: "OFF".to_string(),
            clouds: "Fast".to_string(),
            particles: "Decreased".to_string(),
            mipmap_levels: 2,
            entity_shadows: false,
            entity_distance: "75%".to_string(),
            menu_background_blur: 2,
            cloud_distance: 32,
            see_through_leaves: false,
            improved_transparency: false,
            texture_filtering: "None".to_string(),
            anisotropic_filtering: "2x".to_string(),
            weather_effect_radius: 5,

            // Preferences defaults.
            autosave_indicator: true,
            show_vignette: true,
            attack_indicator: "Crosshair".to_string(),
            chunk_fade: "0.75 seconds".to_string(),
        }
    }

    /// Renderiza o painel completo (sidebar + conteúdo da aba selecionada).
    pub fn render(&mut self, ui: &mut egui::Ui) {
        // Fundo escuro do painel
        let rect = ui.max_rect();
        ui.painter().rect_filled(
            rect,
            0.0,
            egui::Color32::from_rgb(20, 20, 25),
        );

        ui.heading(egui::RichText::new("Video Settings").size(22.0).strong().color(egui::Color32::from_rgb(220, 220, 220)));

        ui.columns(2, |columns| {
            // Sidebar esquerda com as abas.
            columns[0].vertical(|ui| {
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    for tab in VideoTab::all() {
                        let selected = self.tab == tab;

                        let text_color = if selected {
                            egui::Color32::from_rgb(255, 255, 255)
                        } else {
                            egui::Color32::from_rgb(150, 150, 150)
                        };

                        let text = egui::RichText::new(tab.label())
                            .strong()
                            .color(text_color)
                            .size(16.0);

                        let button = egui::SelectableLabel::new(selected, text);

                        if ui.add(button).clicked() {
                            self.tab = tab;
                        }
                        ui.add_space(4.0);
                    }
                });
            });

            // Painel direito com o conteúdo da aba.
            columns[1].vertical(|ui| {
                ui.add_space(10.0);
                match self.tab {
                    VideoTab::Display => self.render_display(ui),
                    VideoTab::QualityPerformance => self.render_quality_performance(ui),
                    VideoTab::Preferences => self.render_preferences(ui),
                }
            });
        });
    }

    /// Renderiza a aba Display.
    fn render_display(&mut self, ui: &mut egui::Ui) {
        ui.heading(egui::RichText::new("Display").strong().color(egui::Color32::from_rgb(200, 200, 200)));
        ui.add_space(8.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Exclusive Fullscreen Mode", &self.exclusive_fullscreen);
        ui.add_space(6.0);
        ui.separator();

        // Max Framerate slider
        let mut framerate = self.max_framerate;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Max Framerate").color(egui::Color32::from_rgb(180, 180, 180)));
            ui.add(egui::Slider::new(&mut framerate, 30..=1000));
            ui.label(egui::RichText::new(format!("{} fps", framerate)).color(egui::Color32::from_rgb(120, 120, 120)));
        });
        self.max_framerate = framerate;
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "VSync", &self.vsync);
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Reduce fps when:", &self.reduce_fps_when);
        ui.add_space(6.0);
        ui.separator();

        let mut scale = self.gui_scale;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("GUI Scale").color(egui::Color32::from_rgb(180, 180, 180)));
            ui.add(egui::Slider::new(&mut scale, 1..=4));
            ui.label(egui::RichText::new(format!("{}", scale)).color(egui::Color32::from_rgb(120, 120, 120)));
        });
        self.gui_scale = scale;
        ui.add_space(6.0);
        ui.separator();

        ui.horizontal(|ui| {
            let mut fs = self.fullscreen;
            ui.checkbox(&mut fs, "Fullscreen: ON");
            if !fs {
                ui.label(egui::RichText::new("Fullscreen: OFF").color(egui::Color32::from_rgb(150, 150, 150)));
            }
            self.fullscreen = fs;
        });
        ui.add_space(6.0);
        ui.separator();

        ui.horizontal(|ui| {
            let mut efs = self.exclusive_fullscreen_mode;
            ui.checkbox(&mut efs, "Exclusive Fullscreen: ON");
            if !efs {
                ui.label(egui::RichText::new("Exclusive Fullscreen: OFF").color(egui::Color32::from_rgb(150, 150, 150)));
            }
            self.exclusive_fullscreen_mode = efs;
        });
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Brightness", &self.brightness);
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Graphics API", &self.graphics_api);
    }

    /// Renderiza a aba Quality & Performance.
    fn render_quality_performance(&mut self, ui: &mut egui::Ui) {
        ui.heading(egui::RichText::new("Quality & Performance").strong().color(egui::Color32::from_rgb(200, 200, 200)));
        ui.add_space(8.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Preset", &self.preset);
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Biome Blend", &self.biome_blend);
        ui.add_space(6.0);
        ui.separator();

        let mut rd = self.render_distance;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Render Distance").color(egui::Color32::from_rgb(180, 180, 180)));
            ui.add(egui::Slider::new(&mut rd, 2..=64));
            ui.label(egui::RichText::new(format!("{} Chunks", rd)).color(egui::Color32::from_rgb(120, 120, 120)));
        });
        self.render_distance = rd;
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Chunk Builder", &self.chunk_builder);
        ui.add_space(6.0);
        ui.separator();

        let mut sd = self.simulation_distance;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Simulation Distance").color(egui::Color32::from_rgb(180, 180, 180)));
            ui.add(egui::Slider::new(&mut sd, 2..=16));
            ui.label(egui::RichText::new(format!("{} Chunks", sd)).color(egui::Color32::from_rgb(120, 120, 120)));
        });
        self.simulation_distance = sd;
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Smooth Lighting", &self.smooth_lighting);
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Clouds", &self.clouds);
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Particles", &self.particles);
        ui.add_space(6.0);
        ui.separator();

        let mut ml = self.mipmap_levels;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Mipmap Levels").color(egui::Color32::from_rgb(180, 180, 180)));
            ui.add(egui::Slider::new(&mut ml, 0..=4));
            ui.label(egui::RichText::new(format!("{}", ml)).color(egui::Color32::from_rgb(120, 120, 120)));
        });
        self.mipmap_levels = ml;
        ui.add_space(6.0);
        ui.separator();

        ui.horizontal(|ui| {
            let mut es = self.entity_shadows;
            ui.checkbox(&mut es, "Entity Shadows: ON");
            if !es {
                ui.label(egui::RichText::new("Entity Shadows: OFF").color(egui::Color32::from_rgb(150, 150, 150)));
            }
            self.entity_shadows = es;
        });
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Entity Distance", &self.entity_distance);
        ui.add_space(6.0);
        ui.separator();

        let mut mb = self.menu_background_blur;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Menu Background Blur").color(egui::Color32::from_rgb(180, 180, 180)));
            ui.add(egui::Slider::new(&mut mb, 0..=8));
            ui.label(egui::RichText::new(format!("{}", mb)).color(egui::Color32::from_rgb(120, 120, 120)));
        });
        self.menu_background_blur = mb;
        ui.add_space(6.0);
        ui.separator();

        let mut cd = self.cloud_distance;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Cloud Distance").color(egui::Color32::from_rgb(180, 180, 180)));
            ui.add(egui::Slider::new(&mut cd, 2..=64));
            ui.label(egui::RichText::new(format!("{} Chunks", cd)).color(egui::Color32::from_rgb(120, 120, 120)));
        });
        self.cloud_distance = cd;
        ui.add_space(6.0);
        ui.separator();

        ui.horizontal(|ui| {
            let mut stl = self.see_through_leaves;
            ui.checkbox(&mut stl, "See-Through Leaves: ON");
            if !stl {
                ui.label(egui::RichText::new("See-Through Leaves: OFF").color(egui::Color32::from_rgb(150, 150, 150)));
            }
            self.see_through_leaves = stl;
        });
        ui.add_space(6.0);
        ui.separator();

        ui.horizontal(|ui| {
            let mut it = self.improved_transparency;
            ui.checkbox(&mut it, "Improved Transparency: ON");
            if !it {
                ui.label(egui::RichText::new("Improved Transparency: OFF").color(egui::Color32::from_rgb(150, 150, 150)));
            }
            self.improved_transparency = it;
        });
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Texture Filtering", &self.texture_filtering);
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Anisotropic Filtering", &self.anisotropic_filtering);
        ui.add_space(6.0);
        ui.separator();

        let mut wer = self.weather_effect_radius;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Weather Effect Radius").color(egui::Color32::from_rgb(180, 180, 180)));
            ui.add(egui::Slider::new(&mut wer, 1..=20));
            ui.label(egui::RichText::new(format!("{} Blocks", wer)).color(egui::Color32::from_rgb(120, 120, 120)));
        });
        self.weather_effect_radius = wer;

        // Caixa de aviso estilo Horizon (bottom left)
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);
        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_rgb(40, 30, 20).linear_multiply(0.8))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(120, 60, 0)))
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("■").color(egui::Color32::from_rgb(200, 80, 0)));
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Render Distance · High cost").strong().color(egui::Color32::from_rgb(220, 220, 220)));
                        ui.label(
                            egui::RichText::new(
                                            "Full-detail terrain range. LOD Distance can extend local worlds farther.",
                        )
                        .small()
                        .color(egui::Color32::from_rgb(150, 150, 150)),
                    );
                    });
                });
            });
    }

    /// Renderiza a aba Preferences.
    fn render_preferences(&mut self, ui: &mut egui::Ui) {
        ui.heading(egui::RichText::new("Preferences").strong().color(egui::Color32::from_rgb(200, 200, 200)));
        ui.add_space(8.0);
        ui.separator();

        ui.horizontal(|ui| {
            let mut ai = self.autosave_indicator;
            ui.checkbox(&mut ai, "Autosave Indicator: ON");
            if !ai {
                ui.label(egui::RichText::new("Autosave Indicator: OFF").color(egui::Color32::from_rgb(150, 150, 150)));
            }
            self.autosave_indicator = ai;
        });
        ui.add_space(6.0);
        ui.separator();

        ui.horizontal(|ui| {
            let mut sv = self.show_vignette;
            ui.checkbox(&mut sv, "Show Vignette: ON");
            if !sv {
                ui.label(egui::RichText::new("Show Vignette: OFF").color(egui::Color32::from_rgb(150, 150, 150)));
            }
            self.show_vignette = sv;
        });
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Attack Indicator", &self.attack_indicator);
        ui.add_space(6.0);
        ui.separator();

        VideoSettings::setting_row(ui, "Chunk Fade", &self.chunk_fade);
    }

    /// Renderiza uma linha de configuração simples (label + valor).
    fn setting_row(ui: &mut egui::Ui, label: &str, value: &str) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(egui::Color32::from_rgb(180, 180, 180)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(value).monospace().color(egui::Color32::from_rgb(120, 120, 120)));
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let settings = VideoSettings::new();
        assert_eq!(settings.tab, VideoTab::Display);
        assert_eq!(settings.max_framerate, 250);
        assert_eq!(settings.vsync, "OFF");
        assert_eq!(settings.render_distance, 32);
        assert_eq!(settings.preset, "Custom");
        assert_eq!(settings.weather_effect_radius, 5);
        assert!(settings.fullscreen);
        assert!(!settings.entity_shadows);
        assert!(settings.autosave_indicator);
    }

    #[test]
    fn tabs_count() {
        assert_eq!(VideoTab::all().len(), 3);
    }
}
