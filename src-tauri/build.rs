fn main() {
    // Configuración integrada (carpeta de Drive y claves de Google/Firebase): se toma de
    // config-integrada.json en la raíz del proyecto, que no se sube al repositorio. Sin ese archivo
    // el exe se compila igual, solo que sin catálogo de Drive.
    println!("cargo:rerun-if-changed=../config-integrada.json");
    // El icono va dentro del exe: si cambia (un logo nuevo), hay que volver a meterlo. Sin esto Cargo
    // reutilizaba el recurso viejo y el exe y sus accesos directos seguían con el logo anterior.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");
    let content = std::fs::read_to_string("../config-integrada.json").unwrap_or_else(|_| "{}".into());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("config-integrada.json");
    std::fs::write(out, content).unwrap();
    tauri_build::build()
}
