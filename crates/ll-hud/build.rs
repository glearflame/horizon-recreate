fn main() {
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(
        out_dir.join("native_render_generated.rs"),
        "// Gerado automaticamente pelo build.rs do horizon-hud\npub fn native_render_api() {}\n",
    ).unwrap();
    println!("cargo:rerun-if-changed=src/gfx/native_render.rs");
}
