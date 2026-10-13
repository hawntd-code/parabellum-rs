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

- Última revisión de los PR de Mac en Windows: 2026-10-12, PR #1 a #6 (todos mezclados).
- Windows: la 0.25.0 (animaciones) está publicada en Drive desde el 2026-10-12. Sigue la 0.26.0.

## Pendientes para Mac

- [ ] **Con un amigo de verdad** (Diego todavía no tiene amigos en Parabellum): lo único que queda de
  las pruebas de abajo.
  - Dentro del juego (Fabric 1.21.1, tecla O): clic en un amigo y escribirle un mensaje.
  - El clic en un aviso (PR #4): un mensaje con la ventana minimizada; al pulsarlo debe abrirse ese
    chat. Los avisos ya salen en Mac desde una `Parabellum.app` (ver «Avisos» en `mac.md`).
  - Animaciones del chat: mensaje nuevo, borrar y «Mensajes nuevos».
- [x] **El Java que se bajó antes del PR #3 no tiene permiso de ejecución**: resuelto en el PR #6, con
  lo robusto. `java::discover` pasa `make_executable` por `bin/java` y `lib/jspawnhelper` de cada
  Java que encuentra. Probado: un Java con `-rw-r--r--` queda en `-rwxr-xr-x` y Fabric 1.21.1 llega al
  menú.
- [x] **Probar el panel dentro del juego en Mac** (2026-10-12, PR #6): Fabric 1.21.1, la tecla O abre el
  panel y se ve bien, aunque Java2D corre en modo headless. Falta lo del amigo (arriba).
- [x] **Revisar las animaciones de la 0.25 en la ventana de Mac** (2026-10-12, PR #6): cambiar de página,
  la portada que viaja al abrir una instancia y buscar en la Biblioteca se ven bien. Las APIs que usan
  están desde Safari 15.5, y macOS 13 (el mínimo) ya trae Safari 16. El chat queda con lo del amigo.
- [x] **Dentro del juego en Mac** (2026-10-12, PR #6): el aviso «Pulsa O» al entrar a un mundo y el botón
  «Parabellum» del menú de pausa funcionan.
- [x] Textos que quedaron viejos al mezclar los PR #1 a #4 (el comentario de `Cargo.toml`, el
  encabezado de `macos/mod.rs` y la tarea 1 de `mac.md`): resuelto en el PR #5.

## Pendientes para Windows

Nada por ahora.

- [x] **PR #6: compilar y correr las pruebas en Windows** (2026-10-12): pasan las 83 pruebas (incluida
  `encuentra_el_java_de_mojang`), compila, y Vanilla 1.21.1 llega al menú en 66 s con el Java que encuentra
  `java::discover`. Juan lo mezcló.

- [x] PR #5 (imágenes): verificado en Windows el 2026-10-12 (pasan las 83 pruebas, compila, y el
  error de una imagen que no se puede abrir dice lo mismo que antes). Juan lo mezcló.

## Próximo en Windows

- **0.26.0:** «Reportar problema» (manda un resumen por chat a Juan) y la prueba con compañeros.

## Cambios de Windows

### 2026-10-12 · 0.26.0: lo que encontró Juan probando con sus amigos

- Qué cambió:
  - Mod, Fabric 26.3: los botones del ratón llegan con números de SDL3 (izquierdo 1, derecho 3) y
    `PantallaPanel` los traduce a los de GLFW para el núcleo. Antes no se podía cambiar de pestaña.
  - Mod, núcleo: sección «Grupos» en Amigos y pestaña Capturas pulida. Los 18 .jar se recompilaron.
  - `servidor.rs`: lee «System chat: X joined the game» y «The game time is N tick(s)» de 26.x.
  - `p2p.rs` + `hosting.rs` + `ui/hostear.js`: el anfitrión ve quién entra por P2P, si va directo o por
    relevo y su ping (`P2p::guests`, `p2pGuests` en la vista del servidor y del mundo abierto).
- Mac: nada de código. Si juegas 26.3 en Mac, revisa que en el panel se pueda cambiar de pestaña (en Mac
  también es SDL3).

### 2026-10-12 · Reglas de Firebase: nadie puede inventarse una solicitud de amistad

- Qué cambió: en `firebase-reglas.json`, quien recibe una solicitud ya solo puede borrarla (aceptar o
  rechazar), no crearla. Antes, alguien podía escribir una solicitud falsa «de» otra persona hacia sí
  mismo y con eso hacerse su amigo sin que lo aceptara. Parabellum ya solo borraba como receptor, así
  que no cambia nada en el código. Juan publica las reglas en la consola de Firebase.
- Mac: nada.

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
