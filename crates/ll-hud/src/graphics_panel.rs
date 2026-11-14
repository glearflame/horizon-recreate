//! Painel de configurações gráficas no estilo Horizon.
//!
//! Recria o visual do menu General / Quality / Performance / Advanced / HDR
//! com a marca 𝕷&𝕷-𝕸𝖎𝖓𝖊.

use egui::Ui;

/// Abas do painel de configurações gráficas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsTab {
    General,
    Quality,
    Performance,
    Advanced,
    Hdr,
}

impl Default for GraphicsTab {
    fn default() -> Self {
        Self::General
    }
}

impl GraphicsTab {
    pub fn all() -> [Self; 5] {
        [
            Self::General,
            Self::Quality,
            Self::Performance,
            Self::Advanced,
            Self::Hdr,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Quality => "Quality",
            Self::Performance => "Performance",
            Self::Advanced => "Advanced",
            Self::Hdr => "HDR",
        }
    }
}

/// Estado do painel de gráficos.
#[derive(Debug, Clone, Default)]
pub struct GraphicsPanel {
    pub tab: GraphicsTab,
    pub preset: String,
    pub render_distance: i32,
    pub lod_distance: String,
    pub far_detail: String,
    pub find_bluemap: bool,
}

impl GraphicsPanel {
    pub fn new() -> Self {
        Self {
            tab: GraphicsTab::General,
            preset: "Custom".to_string(),
            render_distance: 12,
            lod_distance: "Off".to_string(),
            far_detail: "Full".to_string(),
            find_bluemap: false,
        }
    }

    /// Renderiza o painel completo (sidebar + painel direito).
    pub fn render(&mut self, ui: &mut Ui) {
        ui.heading(egui::RichText::new("𝕷&𝕷-𝕸𝖎𝖓𝖊 Graphics").size(24.0).strong());

        ui.columns(2, |columns| {
            // Sidebar esquerda.
            columns[0].vertical(|ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    ui.label(egui::RichText::new("Search options").small().color(egui::Color32::from_gray(150)));
                    ui.separator();

                    for tab in GraphicsTab::all() {
                        let selected = self.tab == tab;
                        let text = egui::RichText::new(tab.label())
                            .strong()
                            .color(if selected {
                                egui::Color32::from_rgb(255, 255, 255)
                            } else {
                                egui::Color32::from_gray(200)
                            });

                        let button = egui::SelectableLabel::new(selected, text);
                        if ui.add(button).clicked() {
                            self.tab = tab;
                        }
                    }
                });
            });

            // Painel direito.
            columns[1].vertical(|ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    ui.heading(egui::RichText::new("General").strong());
                    ui.separator();

                    ui.label("Preset");
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Preset").small().color(egui::Color32::from_gray(150)));
                        egui::ComboBox::new("preset_combo", "")
                            .selected_text(&format!("<{}>", self.preset))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.preset, "Custom".to_string(), "Custom");
                                ui.selectable_value(&mut self.preset, "Fast".to_string(), "Fast");
                                ui.selectable_value(&mut self.preset, "Fancy".to_string(), "Fancy");
                            });
                        ui.label(egui::RichText::new("<Custom>").small());
                    });

                    ui.separator();
                    ui.label(egui::RichText::new("Distance").strong());
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Render Distance");
                        ui.add(egui::Slider::new(&mut self.render_distance, 2..=32));
                        ui.label(format!("{} Chunks", self.render_distance));
                    });

                    ui.horizontal(|ui| {
                        ui.label("LOD Distance");
                        egui::ComboBox::new("lod_combo", "")
                            .selected_text(&self.lod_distance)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.lod_distance, "Off".to_string(), "Off");
                                ui.selectable_value(&mut self.lod_distance, "Fast".to_string(), "Fast");
                                ui.selectable_value(&mut self.lod_distance, "Balanced".to_string(), "Balanced");
                                ui.selectable_value(&mut self.lod_distance, "Quality".to_string(), "Quality");
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Far Detail");
                        egui::ComboBox::new("far_detail_combo", "")
                            .selected_text(&self.far_detail)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.far_detail, "Full".to_string(), "Full");
                                ui.selectable_value(&mut self.far_detail, "Partial".to_string(), "Partial");
                                ui.selectable_value(&mut self.far_detail, "None".to_string(), "None");
                            });
                    });

                    ui.checkbox(&mut self.find_bluemap, "Find BlueMap");

                    ui.separator();

                    // Caixa de aviso estilo Horizon.
                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(139, 92, 0).linear_multiply(0.15))
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(139, 92, 0)))
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("■").color(egui::Color32::from_rgb(139, 92, 0)));
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("Render Distance · High cost").strong());
                                    ui.label(
                                        egui::RichText::new(
                                            "Full-detail terrain range. LOD Distance can extend local worlds farther.",
                                        )
                                        .small()
                                        .color(egui::Color32::from_gray(200)),
                                    );
                                });
                            });
                        });

                    ui.separator();

                    if ui.button("Done").clicked() {
                        // fecha/aplica
                    }
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let panel = GraphicsPanel::new();
        assert_eq!(panel.tab, GraphicsTab::General);
        assert_eq!(panel.render_distance, 12);
        assert_eq!(panel.lod_distance, "Off");
        assert_eq!(panel.far_detail, "Full");
        assert!(!panel.find_bluemap);
    }
}
