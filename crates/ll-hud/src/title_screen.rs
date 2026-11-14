//! Tela de título (Title Screen) — estilo horizon-rust/horizon (MIT).
//!
//! Recria o menu principal do Horizon/Minecraft com fundo escuro, logo,
//! botões centrais e versão no canto.
//! Marca: 𝕷&𝕷-𝕸𝖎𝖓𝖊

use eframe::egui;

/// Estado da tela de título.
#[derive(Debug, Clone, Default)]
pub struct TitleScreen {
    pub hovered_button: Option<TitleButton>,
}

/// Botões do menu principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleButton {
    Jogar,
    Multijogador,
    Opcoes,
    Sair,
}

impl TitleScreen {
    pub fn new() -> Self {
        Self::default()
    }

    /// Renderiza a tela de título completa.
    pub fn render(&mut self, ui: &mut egui::Ui) -> Option<TitleButton> {
        let mut clicked = None;

        // Fundo escuro com gradiente sutil
        let rect = ui.max_rect();
        ui.painter().rect_filled(
            rect,
            0.0,
            egui::Color32::from_rgb(10, 12, 18),
        );

        // Linha decorativa superior
        ui.painter().line_segment(
            [
                egui::Pos2::new(rect.left(), rect.top() + 60.0),
                egui::Pos2::new(rect.right(), rect.top() + 60.0),
            ],
            egui::Stroke::new(2.0, egui::Color32::from_rgb(130, 80, 255)),
        );

        ui.columns(2, |columns| {
            // Coluna esquerda: logo e versão
            columns[0].vertical(|ui| {
                ui.add_space(100.0);

                // Logo 𝕷&𝕷-𝕸𝖎𝖓𝖊
                ui.heading(
                    egui::RichText::new("𝕷&𝕷-𝕸𝖎𝖓𝖊")
                        .size(48.0)
                        .strong()
                        .color(egui::Color32::from_rgb(255, 255, 255)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Launcher Offline 1.5.0")
                        .size(14.0)
                        .color(egui::Color32::from_rgb(150, 150, 150)),
                );
                ui.add_space(20.0);

                // Versão no canto inferior esquerdo
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.label(
                        egui::RichText::new("Versão 26.3")
                            .size(12.0)
                            .color(egui::Color32::from_gray(120)),
                    );
                    ui.label(
                        egui::RichText::new("Recriação do zero — baseada em horizon-rust/horizon (MIT)")
                            .size(10.0)
                            .color(egui::Color32::from_gray(80)),
                    );
                });
            });

            // Coluna direita: botões centralizados
            columns[1].vertical_centered(|ui| {
                ui.add_space(120.0);

                // Botão Jogar
                if self.render_button(
                    ui,
                    TitleButton::Jogar,
                    "▶ Jogar",
                    "Iniciar o Glear-Mine",
                ).clicked() {
                    clicked = Some(TitleButton::Jogar);
                }
                ui.add_space(16.0);

                // Botão Multijogador
                if self.render_button(
                    ui,
                    TitleButton::Multijogador,
                    "🌐 Multijogador",
                    "Conectar a um servidor",
                ).clicked() {
                    clicked = Some(TitleButton::Multijogador);
                }
                ui.add_space(16.0);

                // Botão Opções
                if self.render_button(
                    ui,
                    TitleButton::Opcoes,
                    "⚙ Opções",
                    "Configurações de vídeo, controles, etc.",
                ).clicked() {
                    clicked = Some(TitleButton::Opcoes);
                }
                ui.add_space(16.0);

                // Botão Sair
                if self.render_button(
                    ui,
                    TitleButton::Sair,
                    "✕ Sair",
                    "Fechar o launcher",
                ).clicked() {
                    clicked = Some(TitleButton::Sair);
                }
                ui.add_space(40.0);

                // Links rodapé
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Créditos: horizon-rust/horizon (MIT)")
                                .size(10.0)
                                .color(egui::Color32::from_gray(100)),
                        );
                        ui.label("  |  ");
                        ui.label(
                            egui::RichText::new("𝕷&𝕷-𝕸𝖎𝖓𝖊 — Recriação do zero")
                                .size(10.0)
                                .color(egui::Color32::from_gray(100)),
                        );
                    });
                });
            });
        });

        clicked
    }

    /// Renderiza um botão estilizado do menu principal.
    fn render_button(
        &mut self,
        ui: &mut egui::Ui,
        button: TitleButton,
        label: &str,
        subtitle: &str,
    ) -> egui::Response {
        let hovered = self.hovered_button == Some(button);
        let bg_color = if hovered {
            egui::Color32::from_rgb(60, 40, 100)
        } else {
            egui::Color32::from_rgb(30, 30, 40)
        };

        let response = ui
            .add(
                egui::Button::new(
                    egui::RichText::new(label)
                        .strong()
                        .size(20.0)
                        .color(egui::Color32::from_rgb(255, 255, 255)),
                )
                .min_size(egui::Vec2::new(260.0, 52.0))
                .fill(bg_color)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(130, 80, 255))),
            )
            .on_hover_text(subtitle);

        if response.hovered() {
            self.hovered_button = Some(button);
        } else if self.hovered_button == Some(button) {
            self.hovered_button = None;
        }

        // Subtítulo abaixo do botão
        if response.hovered() {
            ui.label(
                egui::RichText::new(subtitle)
                    .size(11.0)
                    .color(egui::Color32::from_gray(140)),
            );
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let screen = TitleScreen::new();
        assert!(screen.hovered_button.is_none());
    }
}
