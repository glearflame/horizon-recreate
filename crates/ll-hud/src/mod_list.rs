//! Lista de mods instalados do Glear-Mine.

use serde::{Deserialize, Serialize};

/// Um mod instalado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub description: String,
}

impl ModEntry {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            enabled: true,
            description: String::new(),
        }
    }
}

/// Lista de mods do Glear-Mine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModList {
    pub mods: Vec<ModEntry>,
}

impl Default for ModList {
    fn default() -> Self {
        Self {
            mods: vec![
                ModEntry::new("sodium", "Sodium", "0.6.0"),
                ModEntry::new("iris", "Iris Shaders", "1.8.1"),
                ModEntry::new("lithium", "Lithium", "1.3.0"),
                ModEntry::new("ferrite_core", "Ferrite Core", "6.0.0"),
                ModEntry::new("entity_culling", "Entity Culling", "1.5.0"),
            ]
            .into_iter()
            .map(|m| {
                let mut entry = m;
                entry.description = match entry.id.as_str() {
                    "sodium" => "Otimização massiva de renderização. Substitui o motor gráfico do Minecraft.".to_string(),
                    "iris" => "Suporte a shaders compatível com Sodium.".to_string(),
                    "lithium" => "Otimizações gerais de lógica interna e tick do servidor.".to_string(),
                    "ferrite_core" => "Reduz o uso de memória através de otimizações de estruturas de dados.".to_string(),
                    "entity_culling" => "Não renderiza entidades que não estão visíveis na tela.".to_string(),
                    _ => String::new(),
                };
                entry
            })
            .collect(),
        }
    }
}

impl ModList {
    pub fn add(&mut self, m: ModEntry) {
        self.mods.push(m);
    }

    pub fn enabled_count(&self) -> usize {
        self.mods.iter().filter(|m| m.enabled).count()
    }

    /// Renderiza a lista de mods com toggles.
    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.heading("📦 Mods — Glear-Mine");
        ui.label(format!("{} mods ({} ativos)", self.mods.len(), self.enabled_count()));
        ui.separator();

        if self.mods.is_empty() {
            ui.label("Nenhum mod instalado. Use o navegador de mods.");
            return;
        }

        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            for m in &mut self.mods {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut m.enabled, "");
                    ui.label(egui::RichText::new(&m.name).strong());
                    ui.label(format!("v{}", m.version));
                });
                if !m.description.is_empty() {
                    ui.label(egui::RichText::new(&m.description).small().color(egui::Color32::from_gray(150)));
                }
                ui.separator();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_list_counts() {
        let mut list = ModList::default();
        list.add(ModEntry::new("xray", "X-Ray", "1.0"));
        list.add(ModEntry::new("zoom", "Zoom", "2.0"));
        list.mods[1].enabled = false;
        assert_eq!(list.mods.len(), 2);
        assert_eq!(list.enabled_count(), 1);
    }
}
