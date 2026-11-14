//! Painel de configurações de vídeo/gráficos do Glear-Mine.

use egui::Ui;
use serde::{Deserialize, Serialize};

/// Configurações gráficas do Glear-Mine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicsSettings {
    pub render_distance: u32,
    pub max_fps: u32,
    pub vsync: bool,
    pub gui_scale: u32,
    pub brightness: f32,
    pub fov: u32,
    pub particles: bool,
    pub shadows: bool,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            render_distance: 8,
            max_fps: 60,
            vsync: true,
            gui_scale: 2,
            brightness: 1.0,
            fov: 70,
            particles: true,
            shadows: true,
        }
    }
}

/// Renderiza o painel de vídeo/gráficos.
pub fn render(ui: &mut Ui, settings: &mut GraphicsSettings, theme_name: &str) {
    ui.heading(format!("⚙ Vídeo — {theme_name}"));
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Distância de renderização:");
        ui.add(egui::Slider::new(&mut settings.render_distance, 2..=32));
    });
    ui.horizontal(|ui| {
        ui.label("FPS máximo:");
        ui.add(egui::Slider::new(&mut settings.max_fps, 30..=240));
    });
    ui.horizontal(|ui| {
        ui.label("FOV:");
        ui.add(egui::Slider::new(&mut settings.fov, 30..=110));
    });
    ui.horizontal(|ui| {
        ui.label("Brilho:");
        ui.add(egui::Slider::new(&mut settings.brightness, 0.0..=2.0));
    });
    ui.horizontal(|ui| {
        ui.label("Escala da GUI:");
        ui.add(egui::Slider::new(&mut settings.gui_scale, 1..=4));
    });
    ui.checkbox(&mut settings.vsync, "VSync");
    ui.checkbox(&mut settings.particles, "Partículas");
    ui.checkbox(&mut settings.shadows, "Sombras");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings() {
        let s = GraphicsSettings::default();
        assert_eq!(s.render_distance, 8);
        assert!(s.vsync);
    }
}
