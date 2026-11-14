//! HUD e painéis in-game do **Glear-Mine**.
//!
//! Recriação do zero inspirada na estrutura pública do horizon-rust/horizon
//! (sem copiar código). Todos os painéis são marca Glear-Mine.

pub mod theme;
pub mod graphics_ui;
pub mod mod_browser;
pub mod mod_list;
pub mod hud_overlay;
pub mod settings;
pub mod graphics_panel;
pub mod video_settings;
pub mod title_screen;

// TODO: client_render (horizon-rust/horizon) copiado para src/client_render/,
//       não compilado devido a dependências externas (wgpu, horizon-assets, etc.)
// pub mod client_render;

pub use theme::GlearTheme;
pub use title_screen::{TitleButton, TitleScreen};

/// Versão do tema HUD do Glear-Mine.
pub const HUD_THEME_VERSION: &str = "glear-mine-1.0";
