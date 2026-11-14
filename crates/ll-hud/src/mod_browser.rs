//! Navegador de mods do Glear-Mine (estilo loja/curadoria).

use crate::mod_list::{ModEntry, ModList};

/// Mod disponível no catálogo Glear-Mine.
#[derive(Debug, Clone)]
pub struct CatalogMod {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub downloads: u64,
}

/// Catálogo de mods disponíveis.
pub fn catalog() -> Vec<CatalogMod> {
    vec![
        CatalogMod {
            id: "glear-hud".into(),
            name: "Glear HUD".into(),
            version: "1.0.0".into(),
            description: "HUD customizado do Glear-Mine com FPS, coords e coordenadas.".into(),
            downloads: 15234,
        },
        CatalogMod {
            id: "glear-zoom".into(),
            name: "Glear Zoom".into(),
            version: "2.1.0".into(),
            description: "Zoom estilo OptiFine com tecla C.".into(),
            downloads: 8932,
        },
        CatalogMod {
            id: "glear-fps".into(),
            name: "Glear FPS Boost".into(),
            version: "1.3.2".into(),
            description: "Otimizações de renderização para FPS alto.".into(),
            downloads: 22100,
        },
    ]
}

/// Renderiza o navegador de mods com botão de instalar.
pub fn render(ui: &mut egui::Ui, installed: &mut ModList) {
    ui.heading("🔍 Navegador de Mods — Glear-Mine");
    ui.separator();

    for cat in catalog() {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&cat.name).strong());
                ui.label(egui::RichText::new(&cat.description).small().color(egui::Color32::from_gray(150)));
                ui.label(format!("v{} • {} downloads", cat.version, cat.downloads));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let already = installed.mods.iter().any(|m| m.id == cat.id);
                if ui.add_enabled(!already, egui::Button::new(if already { "✓ Instalado" } else { "Instalar" })).clicked() {
                    installed.add(ModEntry::new(&cat.id, &cat.name, &cat.version));
                }
            });
        });
        ui.separator();
    }
}
