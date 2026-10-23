//! Ajustes compartibles de Minecraft. Conserva las otras líneas y comprueba el valor esperado.
use crate::{ajustes::Outcome, instances::Instance};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Target {
    pub package: String,
    pub minecraft: String,
    pub content_hash: String,
}

#[derive(Clone, Copy)]
enum Kind {
    Bool,
    Number(f64, f64, bool),
}
pub struct Definition {
    pub id: &'static str,
    key: &'static str,
    since: &'static str,
    es: &'static str,
    en: &'static str,
    kind: Kind,
    default: f64,
}
const fn number(
    id: &'static str,
    key: &'static str,
    since: &'static str,
    es: &'static str,
    en: &'static str,
    min: f64,
    max: f64,
    integer: bool,
    default: f64,
) -> Definition {
    Definition {
        id,
        key,
        since,
        es,
        en,
        kind: Kind::Number(min, max, integer),
        default,
    }
}
pub const REGISTRY: &[Definition] = &[
    number(
        "minecraft.renderDistance",
        "renderDistance",
        "1.8",
        "Distancia de renderizado",
        "Render distance",
        2.,
        32.,
        true,
        12.,
    ),
    number(
        "minecraft.simulationDistance",
        "simulationDistance",
        "1.18",
        "Distancia de simulación",
        "Simulation distance",
        2.,
        32.,
        true,
        12.,
    ),
    number(
        "minecraft.graphics",
        "graphicsMode",
        "1.8",
        "Calidad gráfica",
        "Graphics quality",
        0.,
        2.,
        true,
        1.,
    ),
    number(
        "minecraft.maxFps",
        "maxFps",
        "1.8",
        "FPS máximos",
        "Maximum FPS",
        10.,
        260.,
        true,
        120.,
    ),
    Definition {
        id: "minecraft.vsync",
        key: "enableVsync",
        since: "1.8",
        es: "Sincronización vertical",
        en: "VSync",
        kind: Kind::Bool,
        default: 1.,
    },
    number(
        "minecraft.fov",
        "fov",
        "1.8",
        "Campo de visión (grados)",
        "Field of view (degrees)",
        30.,
        110.,
        true,
        70.,
    ),
    number(
        "minecraft.guiScale",
        "guiScale",
        "1.8",
        "Escala de interfaz (0: automática)",
        "UI scale (0: automatic)",
        0.,
        8.,
        true,
        0.,
    ),
    number(
        "minecraft.particles",
        "particles",
        "1.8",
        "Partículas",
        "Particles",
        0.,
        2.,
        true,
        0.,
    ),
    Definition {
        id: "minecraft.entityShadows",
        key: "entityShadows",
        since: "1.8",
        es: "Sombras de entidades",
        en: "Entity shadows",
        kind: Kind::Bool,
        default: 1.,
    },
    number(
        "minecraft.gamma",
        "gamma",
        "1.8",
        "Brillo",
        "Brightness",
        0.,
        1.,
        false,
        0.5,
    ),
];
pub fn find(id: &str) -> Option<&'static Definition> {
    REGISTRY.iter().find(|d| d.id == id)
}
pub fn label(id: &str) -> String {
    find(id)
        .map(|d| if crate::idioma::english() { d.en } else { d.es })
        .unwrap_or(id)
        .into()
}
fn at_least(version: &str, since: &str) -> bool {
    crate::loaders::compare_versions(version, since) != std::cmp::Ordering::Less
}
pub fn validate(id: &str, value: &Value, version: Option<&str>) -> Result<Value, String> {
    let def = find(id).ok_or_else(|| {
        t!(
            "Actualiza Parabellum para usar este ajuste.",
            "Update Parabellum to use this setting."
        )
    })?;
    if version.is_some_and(|v| !at_least(v, def.since)) {
        return Err(t!(
            "Este ajuste no existe en esta versión de Minecraft.",
            "This setting is unavailable in this Minecraft version."
        ));
    }
    match def.kind {
        Kind::Bool => value.as_bool().map(Value::Bool),
        Kind::Number(min, max, integer) => value
            .as_f64()
            .filter(|v| {
                v.is_finite()
                    && *v >= min
                    && *v <= max
                    && (!integer || v.fract() == 0.)
                    && !(id == "minecraft.graphics"
                        && *v == 2.
                        && version.is_some_and(|v| !at_least(v, "1.16")))
            })
            .map(|v| if integer { json!(v as i64) } else { json!(v) }),
    }
    .ok_or_else(|| {
        t!(
            "El valor del ajuste está fuera de su intervalo permitido.",
            "The setting value is outside its allowed range."
        )
    })
}
fn key(def: &Definition, version: &str) -> &'static str {
    if def.id == "minecraft.graphics" && !at_least(version, "1.16") {
        "fancyGraphics"
    } else {
        def.key
    }
}
fn decode(def: &Definition, version: &str, text: Option<&str>) -> Result<Value, String> {
    let Some(text) = text else {
        return Ok(match def.kind {
            Kind::Bool => json!(def.default != 0.),
            _ => json!(def.default),
        });
    };
    let value = if key(def, version) == "fancyGraphics" {
        json!(match text {
            "true" => 1,
            "false" => 0,
            _ =>
                return Err(t!(
                    "La opción gráfica no es válida.",
                    "The graphics option is invalid."
                )),
        })
    } else if matches!(def.kind, Kind::Bool) {
        json!(text
            .parse::<bool>()
            .map_err(|_| t!("La opción no es válida.", "The option is invalid."))?)
    } else {
        let number = text.parse::<f64>().map_err(|_| {
            t!(
                "La opción no es un número válido.",
                "The option is not a valid number."
            )
        })?;
        json!(if def.id == "minecraft.fov" {
            (number * 40. + 70.).round()
        } else {
            number
        })
    };
    validate(def.id, &value, Some(version))
}
fn encode(def: &Definition, version: &str, value: &Value) -> String {
    if key(def, version) == "fancyGraphics" {
        (value.as_i64() == Some(1)).to_string()
    } else if def.id == "minecraft.fov" {
        ((value.as_f64().unwrap() - 70.) / 40.).to_string()
    } else {
        value.to_string()
    }
}
pub fn equal(a: &Value, b: &Value) -> bool {
    a == b
        || a.as_f64()
            .zip(b.as_f64())
            .is_some_and(|(a, b)| (a - b).abs() < 1e-9)
}
fn read_text(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}
pub fn read(instance: &Instance, id: &str) -> Result<Value, String> {
    let def = find(id).ok_or_else(|| t!("Ajuste desconocido.", "Unknown setting."))?;
    if !at_least(&instance.minecraft_version, def.since) {
        return Err(t!(
            "Ajuste incompatible con esta versión de Minecraft.",
            "Setting incompatible with this Minecraft version."
        ));
    }
    let text = read_text(&instance.game_dir.join("options.txt"))?;
    let current = text
        .lines()
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| *k == key(def, &instance.minecraft_version))
        .last()
        .map(|(_, v)| v);
    decode(def, &instance.minecraft_version, current)
}
pub fn apply(
    instance: &Instance,
    id: &str,
    value: &Value,
    expected: &Value,
) -> Result<Outcome, String> {
    let value = validate(id, value, Some(&instance.minecraft_version))?;
    let path = instance.game_dir.join("options.txt");
    let before = read_text(&path)?;
    let current = read(instance, id)?;
    if !equal(&current, expected) {
        return Ok(Outcome::Conflict { current });
    }
    if equal(&current, &value) {
        return Ok(Outcome::Unchanged { value });
    }
    let def = find(id).unwrap();
    let wanted = key(def, &instance.minecraft_version);
    let mut written = false;
    let mut lines: Vec<String> = before
        .lines()
        .filter_map(|line| {
            if line.split_once(':').is_some_and(|(k, _)| k == wanted) {
                if written {
                    return None;
                }
                written = true;
                Some(format!(
                    "{wanted}:{}",
                    encode(def, &instance.minecraft_version, &value)
                ))
            } else {
                Some(line.into())
            }
        })
        .collect();
    if !written {
        lines.push(format!(
            "{wanted}:{}",
            encode(def, &instance.minecraft_version, &value)
        ));
    }
    if read_text(&path)? != before {
        return Err(t!(
            "Las opciones cambiaron mientras tanto. Vuelve a intentarlo.",
            "The options changed meanwhile. Try again."
        ));
    }
    let staged = instance.game_dir.join(".parabellum-options.tmp");
    let newline = if before.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    std::fs::write(&staged, format!("{}{newline}", lines.join(newline)))
        .map_err(|e| e.to_string())?;
    // La copia previa también permite recuperar una escritura interrumpida.
    if path.exists() {
        std::fs::copy(&path, instance.state_dir().join("options-anterior.txt"))
            .or_else(|_| {
                std::fs::create_dir_all(instance.state_dir())?;
                std::fs::copy(&path, instance.state_dir().join("options-anterior.txt"))
            })
            .map_err(|e| e.to_string())?;
    }
    std::fs::rename(staged, path).map_err(|e| e.to_string())?;
    Ok(Outcome::Applied { value })
}
pub fn target(instance: &Instance) -> Option<Target> {
    let reference = crate::identidad::reference(instance)?;
    Some(Target {
        package: reference.package,
        minecraft: instance.minecraft_version.clone(),
        content_hash: reference.content.content_hash,
    })
}
pub fn compatible(instance: &Instance, target: &Target) -> bool {
    instance.minecraft_version == target.minecraft
        && crate::identidad::reference(instance).is_some_and(|r| r.package == target.package)
        && crate::identidad::content(instance).content_hash == target.content_hash
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Described {
    pub id: &'static str,
    pub label: String,
    pub kind: &'static str,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub options: Vec<f64>,
    pub since: &'static str,
    pub value: Value,
    pub instance: String,
    pub instance_name: String,
    pub target: Target,
}
pub fn describe(instance: &Instance) -> Vec<Described> {
    let Some(target) = target(instance) else {
        return Vec::new();
    };
    REGISTRY
        .iter()
        .filter_map(|d| {
            let value = read(instance, d.id).ok()?;
            let (kind, min, max, options) = match d.kind {
                Kind::Bool => ("bool", None, None, vec![]),
                Kind::Number(_, _, _)
                    if matches!(d.id, "minecraft.graphics" | "minecraft.particles") =>
                {
                    (
                        "choice",
                        None,
                        None,
                        if d.id == "minecraft.graphics"
                            && !at_least(&instance.minecraft_version, "1.16")
                        {
                            vec![0., 1.]
                        } else {
                            vec![0., 1., 2.]
                        },
                    )
                }
                Kind::Number(a, b, _) => ("range", Some(a), Some(b), vec![]),
            };
            Some(Described {
                id: d.id,
                label: label(d.id),
                kind,
                min,
                max,
                options,
                since: "0.39.0",
                value,
                instance: instance.id.clone(),
                instance_name: instance.name.clone(),
                target: target.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn instance(version: &str) -> Instance {
        let folder = std::env::temp_dir().join(format!(
            "parabellum-opciones39-{}-{}",
            crate::mundos_archivo::id(),
            version
        ));
        std::fs::create_dir_all(folder.join(".lanzador")).unwrap();
        Instance {
            game_dir: folder.clone(),
            folder,
            minecraft_version: version.into(),
            ..Default::default()
        }
    }
    #[test]
    fn conserva_opciones_y_respeta_cambios_posteriores() {
        let i = instance("1.21.1");
        std::fs::write(
            i.game_dir.join("options.txt"),
            "key_key.attack:key.mouse.left\r\nfov:0.0\r\nrenderDistance:12\r\nidioma:es_mx\r\n",
        )
        .unwrap();
        assert!(matches!(
            apply(&i, "minecraft.renderDistance", &json!(8), &json!(12)).unwrap(),
            Outcome::Applied { .. }
        ));
        let text = read_text(&i.game_dir.join("options.txt")).unwrap();
        assert!(text.contains("key_key.attack:key.mouse.left\r\n"));
        assert!(text.contains("renderDistance:8\r\n"));
        assert!(matches!(
            apply(&i, "minecraft.renderDistance", &json!(16), &json!(12)).unwrap(),
            Outcome::Conflict { .. }
        ));
        assert_eq!(read(&i, "minecraft.fov").unwrap(), json!(70));
        apply(&i, "minecraft.fov", &json!(90), &json!(70)).unwrap();
        assert!(read_text(&i.game_dir.join("options.txt"))
            .unwrap()
            .contains("fov:0.5"));
        std::fs::remove_dir_all(i.folder).unwrap();
    }
    #[test]
    fn adapta_versiones_y_rechaza_valores_no_compatibles() {
        let i = instance("1.8.9");
        std::fs::write(i.game_dir.join("options.txt"), "fancyGraphics:true\n").unwrap();
        assert_eq!(read(&i, "minecraft.graphics").unwrap(), json!(1));
        apply(&i, "minecraft.graphics", &json!(0), &json!(1)).unwrap();
        assert!(read_text(&i.game_dir.join("options.txt"))
            .unwrap()
            .contains("fancyGraphics:false"));
        assert!(validate("minecraft.graphics", &json!(2), Some("1.8.9")).is_err());
        assert!(validate("minecraft.simulationDistance", &json!(8), Some("1.16.5")).is_err());
        assert!(validate("minecraft.gamma", &json!(1.1), None).is_err());
        assert!(validate("minecraft.vsync", &json!("true"), None).is_err());
        std::fs::remove_dir_all(i.folder).unwrap();
    }
    #[test]
    fn aplica_al_cerrar_y_deshace_solo_si_el_valor_se_conserva() {
        let i = instance("26.3");
        std::fs::write(
            i.game_dir.join("options.txt"),
            "renderDistance:12\ngamma:0.5\n",
        )
        .unwrap();
        let previous = read(&i, "minecraft.renderDistance").unwrap();
        // El registro pendiente conserva este valor mientras el juego sigue escribiendo sus opciones.
        std::fs::write(
            i.game_dir.join("options.txt"),
            "renderDistance:10\ngamma:0.5\n",
        )
        .unwrap();
        assert!(matches!(
            apply(&i, "minecraft.renderDistance", &json!(8), &previous).unwrap(),
            Outcome::Conflict { .. }
        ));
        assert_eq!(read(&i, "minecraft.renderDistance").unwrap(), json!(10));
        apply(&i, "minecraft.renderDistance", &json!(8), &json!(10)).unwrap();
        assert!(matches!(
            apply(&i, "minecraft.renderDistance", &json!(10), &json!(8)).unwrap(),
            Outcome::Applied { .. }
        ));
        assert!(matches!(
            apply(&i, "minecraft.renderDistance", &json!(12), &json!(8)).unwrap(),
            Outcome::Conflict { .. }
        ));
        apply(&i, "minecraft.gamma", &json!(0.7), &json!(0.5)).unwrap();
        assert!(equal(&read(&i, "minecraft.gamma").unwrap(), &json!(0.7)));
        std::fs::remove_dir_all(i.folder).unwrap();
    }
    #[test]
    fn formatos_antiguos_y_modernos_conservan_teclas_y_opciones_del_usuario() {
        for version in ["1.8.9", "1.16.5", "1.20.1", "1.21.1", "26.3"] {
            let i = instance(version);
            let graphics = if at_least(version, "1.16") {
                "graphicsMode:1"
            } else {
                "fancyGraphics:true"
            };
            std::fs::write(i.game_dir.join("options.txt"),format!("version:1\n{graphics}\nfov:-1.0\nkey_key.attack:key.mouse.left\nmodOption:keep:this\n")).unwrap();
            assert_eq!(read(&i, "minecraft.fov").unwrap(), json!(30));
            apply(&i, "minecraft.graphics", &json!(0), &json!(1)).unwrap();
            apply(&i, "minecraft.fov", &json!(110), &json!(30)).unwrap();
            let text = read_text(&i.game_dir.join("options.txt")).unwrap();
            assert!(text.contains("modOption:keep:this"));
            assert!(text.contains("key_key.attack:key.mouse.left"));
            assert!(text.contains("fov:1"));
            std::fs::remove_dir_all(i.folder).unwrap();
        }
    }
    #[test]
    fn paquete_modificado_o_minecraft_distinto_no_son_compatibles() {
        let mut i = instance("1.20.1");
        i.loader = "Vanilla".into();
        let reference = crate::identidad::Reference {
            package: "oficial".into(),
            version: "1".into(),
            content: crate::identidad::content(&i),
            ..Default::default()
        };
        crate::mundos_archivo::atomic(
            &i.state_dir().join(crate::identidad::REFERENCE_FILE),
            &reference,
        )
        .unwrap();
        let target = target(&i).unwrap();
        assert!(compatible(&i, &target));
        std::fs::create_dir_all(i.game_dir.join("mods")).unwrap();
        std::fs::write(i.game_dir.join("mods/extra.jar"), b"mod nuevo").unwrap();
        assert!(!compatible(&i, &target));
        std::fs::remove_dir_all(i.folder).unwrap();
    }
}
