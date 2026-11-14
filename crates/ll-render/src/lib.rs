//! Render do Horizon: trait de renderer e implementação stub.
//!
//! Recriação do zero — partes de 100% de autoria do horizon-rust/horizon.
//! Não contém código de terceiros (sem backend gráfico real, sem assets).

use ll_core::Aabb;

/// Resultado de uma operação de renderização.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderResult {
    Ok,
    /// A janela/dispositivo foi fechado ou perdido.
    Lost,
}

/// Trait abstrato de renderização. Implementações reais (Vulkan/Metal/DX12)
/// preenchem isso; o stub não desenha nada.
pub trait Renderer {
    /// Inicializa o renderer.
    fn init(&mut self) -> RenderResult;
    /// Renderiza um frame. Retorna `Lost` se o dispositivo foi perdido.
    fn render_frame(&mut self) -> RenderResult;
    /// Desenha uma caixa delimitadora (útil para colisão/debug).
    fn draw_aabb(&mut self, aabb: Aabb);
    /// Encerra o renderer.
    fn shutdown(&mut self);
}

/// Implementação stub: não desenha nada, apenas acumula frames.
pub struct StubRenderer {
    pub frames_rendered: u64,
    pub initialized: bool,
}

impl Default for StubRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl StubRenderer {
    pub fn new() -> Self {
        Self {
            frames_rendered: 0,
            initialized: false,
        }
    }
}

impl Renderer for StubRenderer {
    fn init(&mut self) -> RenderResult {
        self.initialized = true;
        RenderResult::Ok
    }

    fn render_frame(&mut self) -> RenderResult {
        if !self.initialized {
            return RenderResult::Lost;
        }
        self.frames_rendered += 1;
        RenderResult::Ok
    }

    fn draw_aabb(&mut self, _aabb: Aabb) {
        // stub: não desenha
    }

    fn shutdown(&mut self) {
        self.initialized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_renderer_counts_frames() {
        let mut r = StubRenderer::new();
        assert_eq!(r.init(), RenderResult::Ok);
        assert_eq!(r.render_frame(), RenderResult::Ok);
        assert_eq!(r.render_frame(), RenderResult::Ok);
        assert_eq!(r.frames_rendered, 2);
        r.shutdown();
        assert_eq!(r.render_frame(), RenderResult::Lost);
    }
}
