//! Accesos directos: en Mac no aplican (la app se fija en el Dock o se abre desde Aplicaciones).
//! Mismas funciones que plataforma/windows/accesos.rs; la interfaz oculta esas opciones porque
//! SUPPORTED es false.

use serde::Serialize;
use std::path::Path;

pub const SUPPORTED: bool = false;

#[derive(Clone, Copy, Debug)]
pub enum Place {
    Desktop,
    StartMenu,
}

impl Place {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "escritorio" => Some(Self::Desktop),
            "inicio" => Some(Self::StartMenu),
            _ => None,
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// "no" (no hay), "si" (apunta a este Parabellum) u "otro" (apunta a otra carpeta).
    pub desktop: &'static str,
    pub start_menu: &'static str,
}

pub fn status(_exe: &Path) -> Status {
    Status { desktop: "no", start_menu: "no" }
}

pub fn create(_place: Place, _exe: &Path) -> Result<(), String> {
    Err(t!("En Mac no hay accesos directos: fija Parabellum en el Dock.", "There are no shortcuts on Mac: keep Parabellum in the Dock."))
}

pub fn remove(_place: Place) -> Result<(), String> {
    Ok(())
}
