# Windows ↔ Mac: cambios y pendientes

Este archivo es el canal entre los dos lados del proyecto. Juan trabaja en Windows y en lo compartido;
Diego (@diegovc20) en la versión de Mac. Cada uno trabaja con su agente. **Léelo al empezar cada
sesión, después de actualizar `main`.** La guía de Mac sigue siendo [mac.md](mac.md).

## Cómo se usa

- **Windows → Mac.** Cada commit a `main` que toque código compartido (`ui/`, `mod/`, `src-tauri/src/`
  fuera de `plataforma/windows/`, `Cargo.toml` o el contrato `plataforma/mod.rs`) agrega, en ese mismo
  commit, una entrada en «Cambios de Windows»: qué cambió y **qué tiene que hacer Mac** (o «nada»).
- **Revisión de los PR de Mac.** Juan compila y prueba en Windows cada PR. Lo que encuentre va a
  «Pendientes para Mac». Cuando lo resuelvas, márcalo `[x]` en tu PR y di en cuál.
- **Mac → Windows.** Lo que necesites de Windows (compilar, probar, cambiar el contrato) sigue en la
  descripción del PR («para que Juan revise»). Si queda pendiente después de mezclarlo, anótalo en
  «Pendientes para Windows».
- Lo más nuevo, arriba. Cada entrada con su fecha y el título del commit. Para ver lo nuevo desde tu
  última sesión: `git log --oneline <último commit que viste>..origin/main`.

## Estado

- Última revisión de los PR de Mac en Windows: 2026-10-12, PR #1 a #5 (todos mezclados).
- Windows: la 0.24.1 está publicada en Drive. Sigue la 0.25.0 (ver «Próximo en Windows»).

## Pendientes para Mac

- [ ] **El Java que se bajó antes del PR #3 no tiene permiso de ejecución.** Ahora `java::discover`
  lo encuentra, así que ya no se vuelve a bajar, y el juego no abre («Permission denied»). Solo le
  pasa a una Mac que ya usaba Parabellum antes del PR #3. Lo rápido: borrar `base/java/` de tu carpeta
  de pruebas. Lo robusto, si quieres: al encontrar un Java, pasar `make_executable` por `bin/java` y
  `lib/jspawnhelper`.
- [ ] **Probar el panel dentro del juego en Mac.** Fabric 1.21.1, tecla O: clic en un amigo y escribir
  un mensaje. En las pruebas del PR #3 el juego llegó al menú con el mod, pero falta abrir el panel
  (en Mac, Java2D corre en modo headless).
- [ ] **Probar el clic en un aviso** (PR #4): un mensaje real de un amigo con la ventana minimizada; al
  pulsarlo debe abrirse ese chat.
- [x] Textos que quedaron viejos al mezclar los PR #1 a #4 (el comentario de `Cargo.toml`, el
  encabezado de `macos/mod.rs` y la tarea 1 de `mac.md`): resuelto en el PR #5.

## Pendientes para Windows

Nada por ahora.

- [x] PR #5 (imágenes): verificado en Windows el 2026-10-12 (pasan las 83 pruebas, compila, y el
  error de una imagen que no se puede abrir dice lo mismo que antes). Juan lo mezcló.

## Próximo en Windows

Para que lo tengas en cuenta antes de que llegue:

- **0.25.0: animaciones.**
  - En la interfaz (`ui/`): transiciones entre páginas (View Transitions), listas que se deslizan,
    chat y barras de progreso.
  - En el mod: el panel y los avisos se animan, y hay un botón de Parabellum en el menú de pausa.

  Es código compartido y en Mac llega solo. Pero la ventana de Mac usa WKWebView (Safari), no Chromium
  como en Windows, así que habrá que revisar en Mac que se vea bien. Si WKWebView no tiene View
  Transitions, cambia de página sin animación, sin errores.
- **0.26.0:** «Reportar problema» (manda un resumen por chat a Juan) y la prueba con compañeros.

## Cambios de Windows

### 2026-10-12 · Canal Windows ↔ Mac (este archivo y CLAUDE.md)

- Qué cambió:
  - Este archivo y `CLAUDE.md` en la raíz del repo, que Claude Code lee solo al abrir el repo.
  - Se quitó el módulo de pruebas vacío de `launch.rs`: era la advertencia `unused import: super::*`
    que salía en los dos sistemas.
- Mac: nada. Desde ahora, lee este archivo al empezar.

### Punto de partida: `main` en 1984f34 (0.24.1 y los PR #1 a #5)

Mac ya tiene todo lo anterior.
