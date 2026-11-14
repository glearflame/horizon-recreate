//! Overlays do HUD in-game do Glear-Mine (F3 debug, FPS, coords).

use ll_core::BlockPos;

/// Estado do overlay de debug (tecla F3).
#[derive(Debug, Clone, Default)]
pub struct DebugOverlay {
    pub show: bool,
    pub fps: u32,
    pub pos: BlockPos,
    pub facing: String,
    pub biome: String,
    pub seed: u64,
}

impl DebugOverlay {
    pub fn new() -> Self {
        Self {
            show: false,
            fps: 60,
            pos: BlockPos::new(0, 64, 0),
            facing: "south".into(),
            biome: "plains".into(),
            seed: 0xC0FFEE,
        }
    }

    /// Renderiza o overlay F3 no canto superior esquerdo.
    pub fn render(&self, ctx: &egui::Context) {
        if !self.show {
            return;
        }
        egui::Window::new("Glear-Mine Debug")
            .anchor(egui::Align2::LEFT_TOP, [10.0, 10.0])
            .title_bar(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.monospace(format!("Glear-Mine 26.3 ({} fps)", self.fps));
                ui.monospace(format!("XYZ: {:.1} / {:.1} / {:.1}", self.pos.x, self.pos.y, self.pos.z));
                ui.monospace(format!("Facing: {}", self.facing));
                ui.monospace(format!("Biome: {}", self.biome));
                ui.monospace(format!("Seed: {:#x}", self.seed));
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_default_hidden() {
        let o = DebugOverlay::new();
        assert!(!o.show);
        assert_eq!(o.fps, 60);
    }
}
