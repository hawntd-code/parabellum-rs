//! Carpetas del launcher. Todo cuelga de la carpeta del .exe para que se pueda copiar a un USB,
//! con la misma organización que la versión anterior (instancias, base, datos).

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
    pub instances: PathBuf,
    pub base: PathBuf,
    pub versions: PathBuf,
    pub libraries: PathBuf,
    pub assets: PathBuf,
    pub java: PathBuf,
    pub natives: PathBuf,
    pub data: PathBuf,
    pub logs: PathBuf,
}

impl Paths {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let base = root.join("base");
        let data = root.join("datos");
        Self {
            instances: root.join("instancias"),
            versions: base.join("versions"),
            libraries: base.join("libraries"),
            assets: base.join("assets"),
            java: base.join("java"),
            natives: base.join("natives"),
            logs: data.join("logs"),
            base,
            data,
            root,
        }
    }

    /// PARABELLUM_ROOT permite apuntar a otra carpeta durante el desarrollo.
    pub fn detect() -> Self {
        if let Ok(root) = std::env::var("PARABELLUM_ROOT") {
            if !root.trim().is_empty() {
                return Self::new(root);
            }
        }
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        Self::new(exe.parent().unwrap_or(Path::new(".")))
    }

    pub fn ensure_created(&self) -> std::io::Result<()> {
        for dir in [&self.instances, &self.versions, &self.libraries, &self.assets, &self.java, &self.natives, &self.logs] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn version_json(&self, id: &str) -> PathBuf {
        self.versions.join(id).join(format!("{id}.json"))
    }

    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.versions.join(id).join(format!("{id}.jar"))
    }

    pub fn settings_file(&self) -> PathBuf {
        self.data.join("ajustes.json")
    }
}
