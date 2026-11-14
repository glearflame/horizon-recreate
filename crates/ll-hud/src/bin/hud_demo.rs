//! Demo visual dos painéis HUD do Glear-Mine.

use eframe::egui;
use ll_hud::{
    graphics_ui::GraphicsSettings,
    mod_browser,
    mod_list::ModList,
    settings::{render, SettingsTab},
    theme::GlearTheme,
    hud_overlay::DebugOverlay,
};

struct HudDemo {
    theme: GlearTheme,
    tab: SettingsTab,
    graphics: GraphicsSettings,
    mods: ModList,
    show_browser: bool,
    debug: DebugOverlay,
}

impl Default for HudDemo {
    fn default() -> Self {
        Self {
            theme: GlearTheme::glear_mine(),
            tab: SettingsTab::Video,
            graphics: GraphicsSettings::default(),
            mods: ModList::default(),
            show_browser: false,
            debug: DebugOverlay::new(),
        }
    }
}

impl eframe::App for HudDemo {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.theme.apply(ctx);

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("Glear-Mine HUD Demo").size(24.0).strong());
                ui.separator();
                if ui.button("F3 Debug").clicked() {
                    self.debug.show = !self.debug.show;
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.show_browser {
                mod_browser::render(ui, &mut self.mods);
                ui.separator();
                if ui.button("← Voltar").clicked() {
                    self.show_browser = false;
                }
            } else {
                render(ui, &mut self.tab, &mut self.graphics, &mut self.mods, &mut self.show_browser);
            }
        });

        self.debug.render(ctx);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Glear-Mine HUD Demo"),
        ..Default::default()
    };
    eframe::run_native(
        "Glear-Mine HUD",
        options,
        Box::new(|_cc| Ok(Box::new(HudDemo::default()))),
    )
}
