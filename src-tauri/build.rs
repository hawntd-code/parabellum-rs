fn main() {
    // Configuración integrada (carpeta de Drive y claves de Google/Firebase): se toma de
    // config-integrada.json en la raíz del proyecto, que no se sube al repositorio. Sin ese archivo
    // el exe se compila igual, solo que sin catálogo de Drive.
    println!("cargo:rerun-if-changed=../config-integrada.json");
    let content = std::fs::read_to_string("../config-integrada.json").unwrap_or_else(|_| "{}".into());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("config-integrada.json");
    std::fs::write(out, content).unwrap();
    tauri_build::build()
}
