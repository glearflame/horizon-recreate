//! Botão de lançamento do Minecraft integrado à UI (eframe/egui).
//!
//! Versão simples e direta: roda o Java em uma thread separada com
//! std::process::Command e compartilha o estado via Arc<Mutex>.

use eframe::egui;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

const VERSION_ZIP_URL: &str = "https://pixeldrain.com/api/file/3h4sNhZv?download";

/// Estado do lançamento do jogo (compartilhado entre UI e thread).
#[derive(Debug, Clone, Default)]
pub struct LaunchState {
    pub is_launching: bool,
    pub error: Option<String>,
    pub logs: Vec<String>,
    pub last_command: Option<String>,
    pub mem_mb: u32,
    pub username: String,
    pub version_info: Option<String>,
}

impl LaunchState {
    pub fn new() -> Self {
        Self {
            is_launching: false,
            error: None,
            logs: Vec::new(),
            last_command: None,
            mem_mb: 4096,
            username: "Player".to_string(),
            version_info: None,
        }
    }

    pub fn add_log(&mut self, msg: String) {
        self.logs.push(msg);
        if self.logs.len() > 200 {
            self.logs.drain(0..self.logs.len() - 200);
        }
    }
}

/// Renderiza o botão "Jogar" e a área de log na aba Início.
pub fn render_launch_section(
    ui: &mut egui::Ui,
    state: &Arc<Mutex<LaunchState>>,
    version_dir: &Path,
    java_path: &Path,
) {
    ui.heading("Jogar");
    ui.separator();

    // Campos persistentes (lidos/escritos no state compartilhado).
    let mut mem_mb: u32;
    let mut username: String;
    {
        let s = state.lock().unwrap();
        mem_mb = s.mem_mb;
        username = s.username.clone();
    }

    ui.horizontal(|ui| {
        ui.label("Memória (MB):");
        ui.add(egui::DragValue::new(&mut mem_mb).range(512..=16384));
        ui.separator();
        ui.label("Usuário:");
        ui.text_edit_singleline(&mut username);
    });

    // Salva de volta no state.
    {
        let mut s = state.lock().unwrap();
        s.mem_mb = mem_mb;
        s.username = username.clone();
    }

    ui.separator();

    // Info da versão (se disponível).
    let version_info = {
        let s = state.lock().unwrap();
        s.version_info.clone()
    };
    if let Some(info) = &version_info {
        ui.label(egui::RichText::new(info).small().color(egui::Color32::from_gray(150)));
    }

    ui.horizontal(|ui| {
        // Lê estado atual.
        let (is_launching, _error, last_cmd) = {
            let s = state.lock().unwrap();
            (s.is_launching, s.error.clone(), s.last_command.clone())
        };

        let button_text = if is_launching {
            "🔄 Rodando Minecraft..."
        } else {
            "▶ Jogar Glear-Mine"
        };

        let button = egui::Button::new(button_text)
            .min_size(egui::Vec2::new(180.0, 40.0))
            .fill(if is_launching {
                egui::Color32::from_rgb(100, 100, 100)
            } else {
                egui::Color32::from_rgb(50, 150, 50)
            });

        if ui.add_enabled(!is_launching, button).clicked() {
            launch_simple(state.clone(), version_dir, java_path, mem_mb, username.clone());
        }

        // Botão Abrir Pasta.
        if ui.button("📁 Abrir pasta").clicked() {
            let game_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("Glear-Mine/game");
            let _ = std::fs::create_dir_all(&game_dir);
            let _ = Command::new("xdg-open").arg(&game_dir).spawn();
        }

        // Botão Copiar Comando.
        if let Some(cmd) = &last_cmd {
            if ui.button("📋 Copiar comando").clicked() {
                ui.output_mut(|o| o.copied_text = cmd.clone());
            }
        }
    });

    // Erro.
    let error = {
        let s = state.lock().unwrap();
        s.error.clone()
    };
    if let Some(err) = &error {
        ui.colored_label(egui::Color32::from_rgb(255, 80, 80), format!("Erro: {err}"));
        ui.separator();
    }

    // Logs.
    let logs = {
        let s = state.lock().unwrap();
        s.logs.clone()
    };
    ui.collapsing("Log de lançamento", |ui| {
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if logs.is_empty() {
                    ui.label("Nenhum log ainda. Clique em Jogar.");
                } else {
                    for log in &logs {
                        if log.starts_with("✗") || log.starts_with("Erro") || log.starts_with("[ERR]") {
                            ui.colored_label(egui::Color32::from_rgb(255, 80, 80), log);
                        } else {
                            ui.monospace(log);
                        }
                    }
                }
            });
    });
}

/// Detecta e preenche informações da versão instalada.
pub fn detect_version_info(state: &mut LaunchState, version_dir: &Path) {
    let json = match find_file_recursive(version_dir, "26.3.json") {
        Some(p) => p,
        None => return,
    };
    let content = match std::fs::read_to_string(&json) {
        Ok(c) => c,
        Err(_) => return,
    };
    let manifest: serde_json::Value = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(_) => return,
    };
    let main_class = manifest["mainClass"].as_str().unwrap_or("?");
    let libs = manifest["libraries"].as_array().map(|a| a.len()).unwrap_or(0);
    state.version_info = Some(format!("Versão 26.3 • mainClass: {main_class} • {libs} bibliotecas"));
}

/// Verifica se existe algum .jar na pasta (busca recursiva).
pub fn find_any_jar(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    for entry in std::fs::read_dir(dir).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_dir() {
                if find_any_jar(&path) {
                    return true;
                }
            } else if path.extension().map(|e| e == "jar").unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

/// Instala a versão 26.3 (baixa zip do PixelDrain e extrai).
pub fn do_install(state: Arc<Mutex<LaunchState>>, install_path: &Path) {
    {
        let mut s = state.lock().unwrap();
        s.is_launching = true;
        s.error = None;
        s.logs.clear();
        s.add_log("🔍 Baixando versão 26.3 do PixelDrain…".to_string());
    }

    let install_path = install_path.to_path_buf();
    std::thread::spawn(move || {
        let result = (|| -> Result<PathBuf, String> {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .map_err(|e| format!("Falha ao criar cliente HTTP: {e}"))?;
            let mut resp = client
                .get(VERSION_ZIP_URL)
                .send()
                .map_err(|e| format!("Falha ao baixar: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("Servidor respondeu {}", resp.status()));
            }

            let zip_path = std::env::temp_dir().join("glear-mine-263.zip");
            let mut file = std::fs::File::create(&zip_path)
                .map_err(|e| format!("Falha ao criar zip temp: {e}"))?;
            std::io::copy(&mut resp, &mut file).map_err(|e| format!("Erro ao gravar: {e}"))?;
            drop(file);

            if install_path.exists() {
                if install_path.is_dir() {
                    std::fs::remove_dir_all(&install_path).map_err(|e| e.to_string())?;
                } else {
                    std::fs::remove_file(&install_path).map_err(|e| e.to_string())?;
                }
            }
            std::fs::create_dir_all(&install_path).map_err(|e| e.to_string())?;

            let zip_file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
            let mut archive = zip::ZipArchive::new(zip_file).map_err(|e| format!("Zip inválido: {e}"))?;
            for i in 0..archive.len() {
                let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
                let out_path = match f.enclosed_name() {
                    Some(name) => install_path.join(name),
                    None => continue,
                };
                if f.is_dir() {
                    std::fs::create_dir_all(&out_path).ok();
                } else {
                    if let Some(p) = out_path.parent() {
                        std::fs::create_dir_all(p).ok();
                    }
                    let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
                    std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
                }
            }
            let _ = std::fs::remove_file(&zip_path);
            Ok(install_path)
        })();

        let mut s = state.lock().unwrap();
        s.is_launching = false;
        match result {
            Ok(path) => {
                s.add_log(format!("✅ Instalado em: {}", path.display()));
                s.error = None;
            }
            Err(e) => {
                s.error = Some(e.clone());
                s.add_log(format!("✗ Erro na instalação: {e}"));
            }
        }
    });
}

/// Lança o Minecraft em thread separada.
pub fn launch_simple(
    state: Arc<Mutex<LaunchState>>,
    version_dir: &Path,
    java_path: &Path,
    mem_mb: u32,
    username: String,
) {
    {
        let mut s = state.lock().unwrap();
        s.is_launching = true;
        s.error = None;
        s.logs.clear();
        s.add_log("🔍 Procurando arquivos da versão 26.3…".to_string());
    }

    let version_dir = version_dir.to_path_buf();
    let java_path = java_path.to_path_buf();

    std::thread::spawn(move || {
        let result = do_launch(&state, &version_dir, &java_path, mem_mb, &username);
        let mut s = state.lock().unwrap();
        s.is_launching = false;
        if let Err(e) = result {
            s.error = Some(e.clone());
            s.add_log(format!("✗ Erro: {e}"));
        }
    });
}

/// Monta e executa o comando Java.
fn do_launch(
    state: &Arc<Mutex<LaunchState>>,
    version_dir: &Path,
    java_path: &Path,
    mem_mb: u32,
    username: &str,
) -> Result<(), String> {
    let json_path = find_file_recursive(version_dir, "26.3.json")
        .ok_or("❌ 26.3.json não encontrado")?;
    {
        let mut s = state.lock().unwrap();
        s.add_log(format!("✅ version.json: {}", json_path.display()));
    }

    let version_jar = find_file_recursive(version_dir, "26.3.jar")
        .ok_or("❌ 26.3.jar não encontrado")?;
    {
        let mut s = state.lock().unwrap();
        s.add_log(format!("✅ version.jar: {}", version_jar.display()));
    }

    let json_content = std::fs::read_to_string(&json_path)
        .map_err(|e| format!("Erro ao ler version.json: {e}"))?;
    let manifest: serde_json::Value = serde_json::from_str(&json_content)
        .map_err(|e| format!("version.json inválido: {e}"))?;
    let main_class = manifest["mainClass"].as_str()
        .ok_or("❌ mainClass não encontrado")?
        .to_string();
    {
        let mut s = state.lock().unwrap();
        s.add_log(format!("✅ mainClass: {main_class}"));
    }

    let libs_dir = version_dir.join("libraries");
    let mut cp_entries = vec![version_jar.to_string_lossy().to_string()];
    if libs_dir.exists() {
        collect_jars(&libs_dir, &mut cp_entries);
    }
    let classpath = cp_entries.join(":");
    {
        let mut s = state.lock().unwrap();
        s.add_log(format!("✅ classpath: {} entradas", cp_entries.len()));
    }

    let game_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Glear-Mine/game");
    std::fs::create_dir_all(&game_dir).map_err(|e| format!("Erro ao criar game dir: {e}"))?;

    let natives_dir = version_dir.join("natives");
    std::fs::create_dir_all(&natives_dir).ok();

    let cmd_str = format!(
        "java -Xmx{mem_mb}m -Xms512m -Djava.library.path={natives} -cp {classpath} {main_class} --username {username} --version 26.3 --gameDir {game_dir} --assetsDir {assets_dir} --assetIndex 34 --uuid 00000000-0000-0000-0000-000000000000 --accessToken offline --versionType release",
        mem_mb = mem_mb,
        natives = natives_dir.display(),
        classpath = &classpath[..classpath.len().min(200)],
        main_class = main_class,
        username = username,
        game_dir = game_dir.display(),
        assets_dir = version_dir.join("assets").display(),
    );

    {
        let mut s = state.lock().unwrap();
        s.last_command = Some(cmd_str.clone());
        s.add_log("🚀 Executando Java…".to_string());
    }

    let output = Command::new(java_path)
        .arg(format!("-Xmx{mem_mb}m"))
        .arg("-Xms512m")
        .arg(format!("-Djava.library.path={}", natives_dir.display()))
        .arg("-cp")
        .arg(&classpath)
        .arg(&main_class)
        .arg("--username").arg(username)
        .arg("--version").arg("26.3")
        .arg("--gameDir").arg(&game_dir)
        .arg("--assetsDir").arg(version_dir.join("assets"))
        .arg("--assetIndex").arg("34")
        .arg("--uuid").arg("00000000-0000-0000-0000-000000000000")
        .arg("--accessToken").arg("offline")
        .arg("--versionType").arg("release")
        .current_dir(&game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Falha ao executar Java: {e}"))?;

    {
        let mut s = state.lock().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stdout.lines().take(100) {
            s.add_log(format!("[MC] {line}"));
        }
        for line in stderr.lines().take(100) {
            s.add_log(format!("[ERR] {line}"));
        }
        s.add_log(format!("📋 Exit code: {:?}", output.status.code()));
    }

    if !output.status.success() {
        return Err(format!("Processo saiu com código {:?}", output.status.code()));
    }

    Ok(())
}

fn find_file_recursive(dir: &Path, filename: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, filename) {
                return Some(found);
            }
        } else if path.file_name().map(|n| n == filename).unwrap_or(false) {
            return Some(path);
        }
    }
    None
}

fn collect_jars(dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_jars(&path, out);
            } else if path.extension().map(|e| e == "jar").unwrap_or(false) {
                out.push(path.to_string_lossy().to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_state_default() {
        let state = LaunchState::new();
        assert!(!state.is_launching);
        assert!(state.error.is_none());
        assert!(state.logs.is_empty());
        assert_eq!(state.mem_mb, 4096);
        assert_eq!(state.username, "Player");
    }

    #[test]
    fn add_log_keeps_limit() {
        let mut state = LaunchState::new();
        for i in 0..250 {
            state.add_log(format!("log {}", i));
        }
        assert!(state.logs.len() <= 200);
        assert_eq!(state.logs.last(), Some(&"log 249".to_string()));
    }
}
