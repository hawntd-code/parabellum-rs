//! Compartir una instancia por el chat. No viajan los archivos (la base de datos solo guarda texto
//! corto): viaja una receta para que el amigo la arme igual en su PC.
//!   - "modrinth": viene de un modpack de Modrinth → se instala esa misma versión del modpack.
//!   - "catalogo": viene de un paquete del catálogo de Drive → se instala ese paquete.
//!   - "mods": hecha a mano → misma versión de Minecraft y loader, y sus mods, resource packs y
//!     shaders de Modrinth (las versiones exactas). Lo que no es de Modrinth no viaja.
//! Va en un mensaje de texto que empieza con PREFIX (cabe en las reglas actuales de Firebase).

use serde::{Deserialize, Serialize};

use crate::instances::Instance;
use crate::{modrinth, packs};

pub const PREFIX: &str = "parabellum:instancia:";
/// Largo máximo de un mensaje (las reglas de Firebase permiten 2000).
const MAX_TEXT: usize = crate::chat::MAX_TEXT;
/// Los ids de versión de Modrinth tienen 8 letras: van pegados para que quepan más.
const ID_LEN: usize = 8;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Recipe {
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "mc")]
    pub minecraft: String,
    /// "Vanilla", "Fabric", "Quilt", "Forge" o "NeoForge".
    #[serde(rename = "l")]
    pub loader: String,
    #[serde(rename = "lv", default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    /// "modrinth", "catalogo" o "mods".
    #[serde(rename = "s")]
    pub source: String,
    /// Modpack de Modrinth: proyecto y versión.
    #[serde(rename = "p", default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(rename = "v", default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Paquete del catálogo de Drive.
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    /// Versiones de Modrinth pegadas (8 letras cada una).
    #[serde(rename = "m", default, skip_serializing_if = "String::is_empty")]
    pub mods: String,
    #[serde(rename = "r", default, skip_serializing_if = "String::is_empty")]
    pub resourcepacks: String,
    #[serde(rename = "h", default, skip_serializing_if = "String::is_empty")]
    pub shaders: String,
    /// Cuántos mods tiene (para mostrarlo) y cuántos archivos no son de Modrinth (no viajan).
    #[serde(rename = "k", default)]
    pub mod_count: u32,
    #[serde(rename = "x", default)]
    pub left_out: u32,
}

impl Recipe {
    /// (categoría, ids de versión) de lo que se baja de Modrinth.
    pub fn versions(&self) -> Vec<(modrinth::ContentType, Vec<String>)> {
        [(modrinth::ContentType::Mod, &self.mods), (modrinth::ContentType::ResourcePack, &self.resourcepacks), (modrinth::ContentType::Shader, &self.shaders)]
            .into_iter()
            .map(|(kind, joined)| (kind, split(joined)))
            .filter(|(_, ids)| !ids.is_empty())
            .collect()
    }
}

fn split(joined: &str) -> Vec<String> {
    joined.as_bytes().chunks(ID_LEN).filter(|c| c.len() == ID_LEN).filter_map(|c| std::str::from_utf8(c).ok()).map(String::from).collect()
}

fn valid_id(id: &str) -> bool {
    id.len() == ID_LEN && id.chars().all(|c| c.is_ascii_alphanumeric())
}

/// La receta de una instancia.
pub async fn recipe(instance: &Instance) -> Result<Recipe, String> {
    let mut recipe = Recipe {
        name: instance.name.clone(),
        minecraft: instance.minecraft_version.clone(),
        loader: instance.loader.clone(),
        loader_version: instance.loader_version.clone(),
        ..Default::default()
    };
    let files = modrinth::identify(instance).await;
    recipe.mod_count = files.iter().filter(|f| f.kind == "mods" && !f.file.ends_with(".disabled")).count() as u32;
    let source = instances_source(instance);
    if let Some((project, version)) = source.modrinth {
        recipe.source = "modrinth".into();
        recipe.project = Some(project);
        recipe.version = Some(version);
        return Ok(recipe);
    }
    if let Some(manifest) = packs::installed_manifest(instance) {
        recipe.source = "catalogo".into();
        recipe.catalog = Some(manifest.id);
        return Ok(recipe);
    }
    recipe.source = "mods".into();
    for file in files.iter().filter(|f| !f.file.ends_with(".disabled")) {
        let Some(id) = file.version_id.as_deref().filter(|id| valid_id(id)) else {
            recipe.left_out += 1;
            continue;
        };
        match file.kind {
            "mods" => recipe.mods.push_str(id),
            "resourcepacks" => recipe.resourcepacks.push_str(id),
            _ => recipe.shaders.push_str(id),
        }
    }
    Ok(recipe)
}

struct Source {
    modrinth: Option<(String, String)>,
}

/// De dónde vino la instancia según su instancia.json.
fn instances_source(instance: &Instance) -> Source {
    let json: serde_json::Value = std::fs::read_to_string(instance.folder.join(crate::instances::INSTANCE_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let modpack = &json["modpack"];
    let modrinth = (modpack["source"] == "modrinth")
        .then(|| Some((modpack["projectId"].as_str()?.to_string(), modpack["versionId"].as_str()?.to_string())))
        .flatten()
        .filter(|(project, version)| !project.is_empty() && !version.is_empty());
    Source { modrinth }
}

/// El texto del mensaje.
pub fn encode(recipe: &Recipe) -> Result<String, String> {
    let text = format!("{PREFIX}{}", serde_json::to_string(recipe).map_err(|e| e.to_string())?);
    if text.chars().count() > MAX_TEXT {
        return Err(t!(
            "Tiene demasiados mods para compartirla por el chat (unos 200 de Modrinth como mucho). Expórtala como paquete y compártela por Drive o USB.",
            "It has too many mods to share it through the chat (about 200 from Modrinth at most). Export it as a package and share it through Drive or USB."
        ));
    }
    Ok(text)
}

pub fn decode(text: &str) -> Option<Recipe> {
    let recipe: Recipe = serde_json::from_str(text.strip_prefix(PREFIX)?).ok()?;
    let ids_ok = [&recipe.mods, &recipe.resourcepacks, &recipe.shaders].iter().all(|joined| joined.len() % ID_LEN == 0 && split(joined).iter().all(|id| valid_id(id)));
    (ids_ok && !recipe.name.trim().is_empty() && !recipe.minecraft.is_empty()).then_some(recipe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receta_de_ida_y_vuelta() {
        let recipe = Recipe {
            name: "Mi pack".into(),
            minecraft: "1.21.1".into(),
            loader: "Fabric".into(),
            loader_version: Some("0.16.5".into()),
            source: "mods".into(),
            mods: "AANobbMIZZZZZZZZ".into(),
            shaders: "q1w2e3r4".into(),
            mod_count: 3,
            left_out: 1,
            ..Default::default()
        };
        let text = encode(&recipe).unwrap();
        assert!(text.starts_with(PREFIX) && text.len() < 200, "{text}");
        let back = decode(&text).unwrap();
        assert_eq!(back, recipe);
        assert_eq!(back.versions().len(), 2);
        assert_eq!(back.versions()[0].1, vec!["AANobbMI", "ZZZZZZZZ"]);
        // Ids rotos o texto cualquiera: no es una receta.
        assert!(decode(&text.replace("ZZZZZZZZ", "ZZ/ZZ")).is_none());
        assert!(decode("hola").is_none());
        assert!(decode(&format!("{PREFIX}{{}}")).is_none());
        // Demasiados mods: no cabe en un mensaje.
        let big = Recipe { mods: "abcdefgh".repeat(260), ..recipe };
        assert!(encode(&big).is_err());
    }
}
