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
- Windows: la 0.24.1 está publicada en Drive; la 0.25.0 (animaciones) ya está en `main`. Sigue la 0.26.0.

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
- [ ] **Revisar las animaciones de la 0.25 en la ventana de Mac** (WKWebView): cambiar de página, abrir una
  instancia desde la Biblioteca (la portada viaja), buscar en la Biblioteca, el chat (mensaje nuevo, borrar,
  «Mensajes nuevos») y un aviso que se va. Usan Web Animations (`element.animate`), `DOMMatrixReadOnly`,
  `inert` y `scrollTo({ behavior: "smooth" })`: Safari 15.4 o más nuevo.
- [ ] **Dentro del juego en Mac**: el botón «Parabellum» del menú de pausa y el aviso «Pulsa O» al entrar a
  un mundo (la marca es `panel-visto.txt` en la carpeta de datos; bórrala para que vuelva a salir).
- [x] Textos que quedaron viejos al mezclar los PR #1 a #4 (el comentario de `Cargo.toml`, el
  encabezado de `macos/mod.rs` y la tarea 1 de `mac.md`): resuelto en el PR #5.

## Pendientes para Windows

Nada por ahora.

- [x] PR #5 (imágenes): verificado en Windows el 2026-10-12 (pasan las 83 pruebas, compila, y el
  error de una imagen que no se puede abrir dice lo mismo que antes). Juan lo mezcló.

## Próximo en Windows

- **0.26.0:** «Reportar problema» (manda un resumen por chat a Juan) y la prueba con compañeros.

## Cambios de Windows

### 2026-10-12 · 0.25.0: animaciones y el panel se encuentra solo

- Qué cambió:
  - `ui/styles.css`: duraciones y curvas como variables (`--dur-*`, `--ease-*`); estilos nuevos al final
    («0.25: movimiento»).
  - `ui/app.js`:
    - `setHtml(elemento, html, { scope, after })` anima las filas con `data-key`.
    - `navigate` desvanece la página anterior; `morphArt` lleva la portada a la cabecera.
    - Chat: `showNewBelow`. Progreso: `timeLeft`. Jugar: `showLaunching`. Con «reducir movimiento» o
      la ventana minimizada no se anima nada (`motionOk`).
  - Mod: transparencia en `Texturas.dibujar(..., alfa)` de todas las versiones, botón en el menú de
    pausa (`PauseScreenMixin` en Fabric, evento de pantalla en Forge/NeoForge), avisos abajo a la derecha
    (`Juego.avisosX/avisosY`) y pista «Pulsa O» (`datos/panel-visto.txt`). Los 18 .jar se recompilaron.
  - `--abrir-juego` acepta un servidor al final para entrar directo.
- Mac: nada de código. Revisar en Mac lo de «Pendientes para Mac» (animaciones en WKWebView y el botón
  de pausa dentro del juego).

### 2026-10-12 · Canal Windows ↔ Mac (este archivo y CLAUDE.md)

- Qué cambió:
  - Este archivo y `CLAUDE.md` en la raíz del repo, que Claude Code lee solo al abrir el repo.
  - Se quitó el módulo de pruebas vacío de `launch.rs`: era la advertencia `unused import: super::*`
    que salía en los dos sistemas.
- Mac: nada. Desde ahora, lee este archivo al empezar.

### Punto de partida: `main` en 1984f34 (0.24.1 y los PR #1 a #5)

Mac ya tiene todo lo anterior.
