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
/// Un cliente anterior no debe interpretar CurseForge como una lista vacía de Modrinth.
pub const PREFIX_V2: &str = "parabellum:instancia2:";
const UPDATE_HINT: &str = "\nParabellum 0.39.1+ · actualiza para instalar / update to install";
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
    recipe.mod_count = std::fs::read_dir(instance.game_dir.join("mods")).into_iter().flatten().flatten()
        .filter(|f| f.path().extension().is_some_and(|e| e.eq_ignore_ascii_case("jar"))).count() as u32;
    let source = instances_source(instance);
    if source.is_none() && instance.modpack_project.as_deref().is_some_and(|p| p.starts_with("cf:")) {
        return Err(t!("Falta el archivo original de CurseForge en esta instancia. Expórtala como paquete completo.", "The original CurseForge file is missing from this instance. Export it as a complete package."));
    }
    if let Some((provider, project, version)) = source {
        recipe.source = provider;
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
    let files = modrinth::identify(instance).await;
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
    if recipe.left_out > 0 {
        return Err(t!("Esta instancia contiene archivos que la receta del chat no puede incluir. Expórtala como paquete completo para conservarlos.", "This instance contains files the chat recipe cannot include. Export it as a complete package to keep them."));
    }
    Ok(recipe)
}

/// De dónde vino la instancia según su instancia.json.
fn instances_source(instance: &Instance) -> Option<(String, String, String)> {
    let json: serde_json::Value = std::fs::read_to_string(instance.folder.join(crate::instances::INSTANCE_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let modpack = &json["modpack"];
    let provider = modpack["source"].as_str()?;
    let project = modpack["projectId"].as_str()?;
    let version = modpack["versionId"].as_str()?;
    match provider {
        "curseforge" if cf_id(project) && cf_id(version) => Some((provider.into(), project.into(), version.into())),
        "modrinth" if valid_id(project) && valid_id(version) => Some((provider.into(), project.into(), version.into())),
        _ => None,
    }
}

fn cf_id(id: &str) -> bool {
    id.strip_prefix("cf:").is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()) && n.parse::<u64>().is_ok_and(|n| n > 0))
}

fn valid_recipe(recipe: &Recipe, v2: bool) -> bool {
    let ids_ok = [&recipe.mods, &recipe.resourcepacks, &recipe.shaders].iter().all(|joined|
        joined.is_ascii() && joined.len() % ID_LEN == 0 && split(joined).iter().all(|id| valid_id(id)));
    let no_list = recipe.mods.is_empty() && recipe.resourcepacks.is_empty() && recipe.shaders.is_empty() && recipe.left_out == 0;
    let origin_ok = match recipe.source.as_str() {
        "curseforge" => v2 && no_list && recipe.project.as_deref().is_some_and(cf_id) && recipe.version.as_deref().is_some_and(cf_id),
        "modrinth" => no_list && recipe.project.as_deref().is_some_and(valid_id) && recipe.version.as_deref().is_some_and(valid_id),
        "catalogo" => no_list && recipe.catalog.as_ref().is_some_and(|s| !s.trim().is_empty()),
        // Las listas antiguas no permiten saber si perdieron el origen de un modpack y sus overrides.
        "mods" => v2 && recipe.left_out == 0 && recipe.project.is_none() && recipe.version.is_none(),
        _ => false,
    };
    ids_ok && origin_ok && !recipe.name.trim().is_empty() && !recipe.minecraft.trim().is_empty()
        && ["vanilla", "none", "fabric", "quilt", "forge", "neoforge"].contains(&recipe.loader.to_ascii_lowercase().as_str())
}

/// El texto del mensaje.
pub fn encode(recipe: &Recipe) -> Result<String, String> {
    let v2 = matches!(recipe.source.as_str(), "curseforge" | "mods");
    if !valid_recipe(recipe, v2) { return Err(t!("La receta está incompleta. Exporta el paquete completo o vuelve a compartir el modpack original.", "The recipe is incomplete. Export the complete package or share the original modpack again.")); }
    let prefix = if v2 { PREFIX_V2 } else { PREFIX };
    let hint = if v2 { UPDATE_HINT } else { "" };
    let text = format!("{prefix}{}{hint}", serde_json::to_string(recipe).map_err(|e| e.to_string())?);
    if text.chars().count() > MAX_TEXT {
        return Err(t!(
            "Tiene demasiados mods para compartirla por el chat (unos 200 de Modrinth como mucho). Expórtala como paquete y compártela por Drive o USB.",
            "It has too many mods to share it through the chat (about 200 from Modrinth at most). Export it as a package and share it through Drive or USB."
        ));
    }
    Ok(text)
}

pub fn decode(text: &str) -> Option<Recipe> {
    if text.chars().count() > MAX_TEXT { return None; }
    let (body, v2) = match text.strip_prefix(PREFIX_V2) {
        Some(body) => (body.strip_suffix(UPDATE_HINT)?, true),
        None => (text.strip_prefix(PREFIX)?, false),
    };
    let recipe: Recipe = serde_json::from_str(body).ok()?;
    valid_recipe(&recipe, v2).then_some(recipe)
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
            left_out: 0,
            ..Default::default()
        };
        let text = encode(&recipe).unwrap();
        assert!(text.starts_with(PREFIX_V2) && text.len() < 300, "{text}");
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

    #[test]
    fn curseforge_exige_origen_exacto_y_protocolo_nuevo() {
        let recipe = Recipe { name: "ATM10SKY".into(), minecraft: "1.21.1".into(), loader: "NeoForge".into(),
            source: "curseforge".into(), project: Some("cf:1298402".into()), version: Some("cf:9261952".into()), ..Default::default() };
        let text = encode(&recipe).unwrap();
        assert!(text.starts_with(PREFIX_V2));
        assert!(!text.starts_with(PREFIX));
        assert_eq!(decode(&text), Some(recipe.clone()));
        assert!(decode(&format!("{PREFIX}{}", serde_json::to_string(&recipe).unwrap())).is_none());
        for broken in [Recipe { version: None, ..recipe.clone() }, Recipe { source: "otra".into(), ..recipe.clone() },
            Recipe { mods: "abcdefgh".into(), ..recipe.clone() }, Recipe { project: Some("cf:0".into()), ..recipe.clone() }] {
            assert!(encode(&broken).is_err());
        }
    }

    #[test]
    fn rechaza_recetas_anteriores_con_archivos_omitidos() {
        let old = format!("{PREFIX}{{\"n\":\"Pack\",\"mc\":\"1.21.1\",\"l\":\"NeoForge\",\"s\":\"mods\",\"x\":2}}");
        assert!(decode(&old).is_none());
        assert!(decode(&old.replace("\"x\":2", "\"x\":0")).is_none());
    }

    #[tokio::test]
    async fn conserva_curseforge_sin_identificar_en_modrinth() {
        let root = std::env::temp_dir().join(format!("receta-cf-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let instance = Instance { name: "ATM10SKY".into(), minecraft_version: "1.21.1".into(), loader: "NeoForge".into(),
            folder: root.clone(), game_dir: root.clone(), ..Default::default() };
        std::fs::write(root.join(crate::instances::INSTANCE_FILE), r#"{"modpack":{"source":"curseforge","projectId":"cf:1298402","versionId":"cf:9261952"}}"#).unwrap();
        let recipe = tokio::time::timeout(std::time::Duration::from_secs(2), recipe(&instance)).await.unwrap().unwrap();
        assert_eq!(recipe.source, "curseforge");
        assert_eq!(recipe.version.as_deref(), Some("cf:9261952"));
        assert!(recipe.mods.is_empty() && recipe.left_out == 0);
        std::fs::remove_dir_all(root).unwrap();
    }
}
