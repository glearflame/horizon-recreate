//! Tema visual Glear-Mine para o HUD.

use egui::{Color32, Rounding};

/// Tema Glear-Mine: cores, arredondamento e nome.
#[derive(Debug, Clone)]
pub struct GlearTheme {
    pub name: String,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub background: Color32,
    pub panel_bg: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub danger: Color32,
    pub success: Color32,
    pub rounding: Rounding,
}

impl Default for GlearTheme {
    fn default() -> Self {
        Self::glear_mine()
    }
}

impl GlearTheme {
    /// Tema padrão Glear-Mine (escuro com acento verde/azul).
    pub fn glear_mine() -> Self {
        Self {
            name: "Glear-Mine".to_string(),
            primary: Color32::from_rgb(50, 150, 50),
            primary_hover: Color32::from_rgb(70, 180, 70),
            background: Color32::from_rgb(18, 18, 22),
            panel_bg: Color32::from_rgb(28, 28, 34),
            text: Color32::from_rgb(230, 230, 235),
            text_dim: Color32::from_gray(150),
            accent: Color32::from_rgb(80, 160, 255),
            danger: Color32::from_rgb(255, 80, 80),
            success: Color32::from_rgb(100, 255, 100),
            rounding: Rounding::same(8.0),
        }
    }

    /// Aplica o tema em um contexto egui.
    pub fn apply(&self, ctx: &egui::Context) {
        let mut style = ctx.style().as_ref().clone();
        style.visuals.widgets.inactive.fg_stroke.color = self.text;
        style.visuals.widgets.hovered.fg_stroke.color = self.primary_hover;
        style.visuals.widgets.active.fg_stroke.color = self.primary;
        style.visuals.panel_fill = self.panel_bg;
        style.visuals.window_fill = self.panel_bg;
        style.visuals.override_text_color = Some(self.text);
        ctx.set_style(style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_name() {
        let t = GlearTheme::glear_mine();
        assert_eq!(t.name, "Glear-Mine");
    }
}
