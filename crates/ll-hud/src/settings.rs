//! Painel geral de configurações do Glear-Mine (atalhos pra outros painéis).

use egui::Ui;

/// Abas do painel de configurações.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Video,
    Mods,
    Geral,
}

/// Renderiza o painel mestre de configurações com abas.
pub fn render(
    ui: &mut Ui,
    tab: &mut SettingsTab,
    graphics: &mut crate::graphics_ui::GraphicsSettings,
    mods: &mut crate::mod_list::ModList,
    show_browser: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.selectable_value(tab, SettingsTab::Video, "🖥 Vídeo");
        ui.selectable_value(tab, SettingsTab::Mods, "📦 Mods");
        ui.selectable_value(tab, SettingsTab::Geral, "⚙ Geral");
    });
    ui.separator();

    match tab {
        SettingsTab::Video => crate::graphics_ui::render(ui, graphics, "Glear-Mine"),
        SettingsTab::Mods => {
            ui.horizontal(|ui| {
                if ui.button("🔍 Navegador de mods").clicked() {
                    *show_browser = true;
                }
            });
            ui.separator();
            mods.render(ui);
        }
        SettingsTab::Geral => {
            ui.heading("⚙ Geral — Glear-Mine");
            ui.label("Idioma: Português (BR)");
            ui.label("Tema do HUD: Glear-Mine Dark");
            ui.label("Atalho debug: F3");
        }
    }
}
