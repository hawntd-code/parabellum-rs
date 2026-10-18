//! Canal de actualizaciones (mejora 7 de docs/plan-diciembre.md): estable o de pruebas. Abierto a todos
//! (pedido de Juan): cada quien lo elige en Ajustes › Acerca de.
//!  - Estable: las versiones de `_parabellum` (en Mac, `_parabellum-mac`).
//!  - Pruebas: además las de `_parabellum-pruebas` (en Mac, `_parabellum-mac-pruebas`); se queda con la
//!    más nueva de las dos. Quien publica elige el canal al publicar.
//!  - Volver a estable con una versión de pruebas más nueva baja a la estable sin tocar los datos
//!    (`volver_a_estable`); por eso los formatos compartidos tienen que seguir leyéndose en la anterior
//!    (lista para publicar en el plan).

use std::sync::atomic::{AtomicBool, Ordering};

pub const STABLE: &str = "estable";
pub const TEST: &str = "pruebas";

static ON_TEST: AtomicBool = AtomicBool::new(false);

pub fn set(channel: &str) {
    ON_TEST.store(channel == TEST, Ordering::Relaxed);
}

/// Este Parabellum recibe las versiones de pruebas.
pub fn on_test() -> bool {
    ON_TEST.load(Ordering::Relaxed)
}

/// La carpeta de Drive de las versiones de este sistema en ese canal: `_parabellum` o
/// `_parabellum-pruebas` (en Mac, `_parabellum-mac` o `_parabellum-mac-pruebas`).
pub fn folder(test: bool) -> String {
    let id = crate::plataforma::lanzador::ID;
    if test {
        format!("_{id}-{TEST}")
    } else {
        format!("_{id}")
    }
}

/// De qué canal es una versión, por la carpeta donde está («1.20.1/…» u otra cosa: estable, como antes).
pub fn is_test_location(location: &str) -> bool {
    location.split('/').next().is_some_and(|first| first.eq_ignore_ascii_case(&folder(true)))
}

/// Desde esta versión Parabellum confirma su arranque (arranque.rs). Si se baja a una anterior, no hay
/// que esperar esa confirmación.
pub const FIRST_CONFIRMING: &str = "0.30.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carpetas_del_canal() {
        let id = crate::plataforma::lanzador::ID;
        assert_eq!(folder(false), format!("_{id}"));
        assert_eq!(folder(true), format!("_{id}-pruebas"));
        assert!(is_test_location(&format!("_{id}-pruebas")));
        assert!(!is_test_location(&format!("_{id}")));
        assert!(!is_test_location("1.20.1/Better MC"));
    }
}
