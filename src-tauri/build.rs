fn main() {
    // Configuración integrada (carpeta de Drive y claves de Google/Firebase): se toma de
    // config-integrada.json en la raíz del proyecto, que no se sube al repositorio. Sin ese archivo
    // el exe se compila igual, solo que sin catálogo de Drive.
    println!("cargo:rerun-if-changed=../config-integrada.json");
    // El icono va dentro del exe: si cambia (un logo nuevo), hay que volver a meterlo. Sin esto Cargo
    // reutilizaba el recurso viejo y el exe y sus accesos directos seguían con el logo anterior.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");
    // Solo entran los campos que usa el exe (config.rs). Lo demás del archivo no: la clave de CurseForge,
    // por ejemplo, vive en el puente (puente-curseforge), y del exe, que se reparte, cualquiera la sacaría.
    const EMBEDDED: [&str; 7] = ["driveFolderId", "driveApiKey", "googleClientId", "googleClientSecret", "firebaseApiKey", "firebaseDatabaseUrl", "supportName"];
    let content = std::fs::read_to_string("../config-integrada.json").unwrap_or_else(|_| "{}".into());
    let all: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&content).unwrap_or_default();
    let kept: serde_json::Map<String, serde_json::Value> = all.into_iter().filter(|(key, _)| EMBEDDED.contains(&key.as_str())).collect();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("config-integrada.json");
    std::fs::write(out, serde_json::to_string(&kept).unwrap()).unwrap();
    tauri_build::build()
}
