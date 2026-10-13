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

- Última revisión de los PR de Mac en Windows: 2026-10-12, PR #1 a #9 (todos mezclados).
- Windows: la 0.25.0 está publicada en Drive; la 0.27.0 está en `main` (Juan publica primero las reglas
  de Firebase y luego la versión).
- Mac: hechas las tareas 1 a 8 de `mac.md`. Quedan la 9 a la 12; el orden que sugiere Juan está en
  «Lo que sigue en Mac», abajo.

## Lo que sigue en Mac

En este orden (las dos primeras hacen que un compañero con Mac pueda usar Parabellum sin compilar):

1. ~~**Tarea 10, empaquetar como `.app` y `.dmg`.**~~ — hecha en el PR #11, con un script propio
   (`src-tauri/mac/empaquetar.sh`; `bundle.active` sigue en `false`) y `RunEvent::Reopen`. Lo que pedía:
   - **Clic en el Dock con la ventana escondida.** Al cerrar la ventana, Parabellum se queda junto al
     reloj (en Mac, en la barra de menús). Abrir la `.app` otra vez no abre otra copia: macOS le manda
     al que ya está abierto `RunEvent::Reopen`, y hoy nadie lo atiende (en `main.rs` solo se atiende
     `RunEvent::Exit`). Que llame a `cuenta::show_window`. Va en código compartido con
     `#[cfg(target_os = "macos")]`: dilo en el PR.
2. ~~**Tarea 11, actualizarse solo.**~~ — hecha en el PR #12 (ver abajo lo que contestó Juan). Propuesta,
   para que la discutamos en el PR antes de hacerla:
   - En Drive, un zip aparte para Mac con su propio id en el catálogo (p. ej. `parabellum-mac`, junto a
     `LAUNCHER_ID`), así un Windows nunca baja la de Mac ni al revés. `launcher_update` filtra por el id
     del sistema (constante nueva en el contrato).
   - `apply_launcher_update` en Mac reemplaza la `.app` entera (mover la vieja a `.app.old`, poner la
     nueva, abrirla con `open -n` y `--reiniciar`). Lo que hoy hace con el `.exe` va a
     `plataforma::replace_launcher` (contrato nuevo; Windows mueve ahí lo que ya hace).
   - `--publicar-launcher` lo corre Juan en Windows: hace falta poder pasarle el zip de Mac que le
     mandes (o un `--publicar-launcher-mac <zip>`). Juan lo publica en Drive.
   - Después, `SELF_UPDATE = true`.

   **Lo que contestó Juan (2026-10-13):** la de Mac va en la raíz del catálogo (`_parabellum-mac`, junto a
   `_parabellum`); la sube Diego desde su cuenta (editor de la carpeta; puede delegárselo a Claude); los
   nombres están bien, y Juan avisa aquí cuando publique una versión de Windows. Quedó como en la
   respuesta de abajo; el id y lo demás del launcher están en un módulo del contrato,
   `plataforma::lanzador` (`ID`, `published_files`, `install`, `restart`, `cleanup`, `readme`).

   **Respuesta de Mac (PR #12).** De acuerdo con todo, con estos detalles:
   - **Id y carpeta.** Contrato nuevo `LAUNCHER_ID`: `"parabellum"` en Windows (como hoy) y
     `"parabellum-mac"` en Mac. Mac publica en su propia carpeta de Drive, `_parabellum-mac` (el nombre
     sale del id), y no en `_parabellum`: `publish_launcher` y `Publisher::remove_old_launchers` borran
     todo lo que empieza con `Parabellum-` en la carpeta que se publica, así que en la misma carpeta se
     borrarían uno al otro. El catálogo ya la encuentra sola (lee hasta 3 niveles de carpetas), y
     `merge` separa los launchers por id y versión.
   - **Publicar.** En vez de un `--publicar-launcher-mac <zip>` en Windows, «Publicar esta versión» (y
     `--publicar-launcher`) haría lo mismo en Mac: comprime la `.app` con la que está abierto Parabellum
     y deja `_parabellum-mac/` con el zip, el `.json` y un `LEEME.txt` para Mac. Así cada versión se
     publica desde el sistema donde se compiló. Para subirla, dos opciones (decides tú, el Drive es tuyo):
     - **(a)** Me das permiso de edición en la carpeta del catálogo y la subo desde mi Mac con «Subir a
       Drive» (el permiso de Drive queda cifrado con el Llavero, como en Windows con DPAPI).
     - **(b)** Te mando la carpeta `_parabellum-mac` y la subes tú (a mano en Drive o con «Subir a
       Drive»).
   - **Reemplazar** (`plataforma::replace_launcher`, contrato nuevo; Windows mueve ahí lo de hoy). En
     Mac:
     1. Descomprime en `datos/actualizacion/Parabellum.app` y da permiso de ejecución al programa.
     2. Comprueba la firma (`codesign --verify --strict`) y que el identificador sea
        `mx.parabellum.launcher`.
     3. Aparta la actual como `Parabellum.app.old` y pone la nueva en su lugar; si algo falla, deja la
        anterior.
     4. Reabre con `open -n <app> --args --reiniciar`, que también sería del contrato, porque hoy es un
        `Command::new(exe)` en `main.rs`.
     5. `cleanup_previous_version` borra la `.app.old` al arrancar.

     Si la app corre desde el `.dmg` o desde la copia temporal que hace macOS a las apps recién bajadas
     («App Translocation»), no se puede reemplazar: dice que la arrastren a Aplicaciones.
   - **Versiones.** El mismo número que Windows: cuando publicas una, compilo esa misma de `main` en Mac
     y la publico. Si Mac se atrasa, los de Mac siguen en la anterior hasta que salga la suya.
   - **Cómo lo pruebo antes del PR final.** Con una versión falsa (0.26.4) en la carpeta `paquetes` local:
     el 0.26.3 de una `.app` se actualiza solo y reabre en 0.26.4.
3. ~~**Tarea 9, versiones 1.18 y anteriores**~~ — hecha en el PR #10: 1.18 nativa (LWJGL 3.3.1) y 1.17.1
   y anteriores con Rosetta y el Java de Intel de Mojang.
4. **Tarea 12, firma de Apple**: cuesta 99 USD al año, decisión de Juan. Mientras, la guía de
   instalación dice lo del clic derecho › Abrir.

Opcional y pequeño: los servidores de Forge 1.17+ creados en Mac **antes** del PR #7 guardaron
`win_args.txt` en `parabellum-servidor.json` (`launch`) y siguen sin arrancar. Si tienes alguno,
bórralo y créalo otra vez (o cambia `win_args.txt` por `unix_args.txt` en ese archivo).

## Pendientes para Mac

- [ ] **0.26.0 a 0.26.3 en Mac** (ver «Cambios de Windows»): en 26.3, cambiar de pestaña con el ratón y
  escribir en el panel del juego (SDL3 también en Mac); abrir Parabellum dos veces y que se muestre el
  primero.
- [ ] **Hostear con playit.gg de verdad** (PR #9): vincular una cuenta de playit.gg desde «Hostear
  partida», encender un servidor y entrar desde otro PC por la dirección pública.
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

- [ ] **PR #12 (actualizarse solo en Mac): compilar, correr las pruebas y publicar una de prueba.** Lo que
  Windows hacía con su `.exe` se movió tal cual de `catalog.rs` a `plataforma/windows/lanzador.rs`
  (módulo nuevo del contrato: `ID`, `published_files`, `install`, `restart`, `cleanup`, `readme`; el
  `LEEME.txt` es idéntico). `catalog::LAUNCHER_ID` ya no existe: es `plataforma::lanzador::ID`, que en
  Windows sigue siendo `"parabellum"`. También:
  - El título del diálogo de «Publicar esta versión» ya no nombra `_parabellum`, y el aviso de la
    interfaz usa el nombre real de la carpeta.
  - Comando nuevo `--subir-launcher-drive <carpeta>`: lo mismo que «Subir a Drive», sin la ventana.
  - Para probar: «Publicar esta versión» en una carpeta de prueba deja `_parabellum` igual que antes, y
    `cargo test` pasa `actualizacion_del_launcher_segun_el_sistema` (la de Mac no se ofrece en Windows).

- [ ] **PR #11 (`.app` y `.dmg` de Mac): compilar en Windows.** `main.rs` atiende los eventos con un
  `match` y la rama de `RunEvent::Reopen` lleva `#[cfg(target_os = "macos")]` (en Windows no existe);
  `RunEvent::Exit` hace lo mismo que antes. Lo demás es de Mac: `src-tauri/mac/` y el workflow de Mac.

- [ ] **PR #10 (versiones viejas en Mac): compilar, correr las pruebas y abrir una 1.16.5 y una 1.18.2
  en Windows.** En Windows todo debe seguir igual. Lo compartido:
  - Contrato: `java_runtime_platform(major)` (en Windows, `JAVA_RUNTIME_PLATFORM`), `adapt_libraries`
    (devuelve las mismas), `foreign_java_ready()` (`true`) e `install_foreign_java()` (no hace nada).
  - `mojang.rs`: el Java se baja de `java_runtime_platform(major)` y, si Mojang no tiene ese componente,
    sirve otro de la misma versión mayor. También se bajan las bibliotecas de `adapt_libraries`.
  - `java.rs`: `major_of` entiende «8u51-cacert462b08», el nombre que Mojang da a su Java 8. Antes daba 0
    y `ensure_java(8)` (servidores, «Usar Java 8») decía «Mojang no ofrece Java 8», también en Windows.
  - `launch.rs`: usa `adapt_libraries`; con un Java ajeno no acepta otro de reemplazo y avisa si falta
    Rosetta.
  - `main.rs` (`jugar`): si al armar el lanzamiento falta algo de la base, primero lo completa con
    Mojang (`install_vanilla`). Antes salía «Faltan archivos».
  - Arreglo nuevo `Fix::InstallForeignJava` (doctor de instancias y servidores) y `servidor::command`
    avisa si un servidor viejo necesita Rosetta.
  - `ui/app.js`: la revisión de antes de jugar también corre en Vanilla (en Windows no encuentra nada).

- [x] **PR #9 (playit en Mac)** (2026-10-12): en Windows pasan las 85 pruebas (incluidas
  `otro_archivo_del_mismo_tamano_no_es_el_agente` y `ultima_linea_sin_trabarse`) y compila sin
  advertencias. El agente que ya tiene Juan coincide con el SHA-256 nuevo, así que no se vuelve a bajar.
  Buen hallazgo lo de `last_line`: también trababa Windows. Tres cambios en `playit.rs` (compartido):
  - `Agent::start` llama a `plataforma::make_executable` (en Windows no hace nada).
  - `agent_installed` compara el SHA-256 y no solo el tamaño (revisión de seguridad): antes, cualquier
    archivo del mismo tamaño en `base/playit` se ejecutaba. Si no coincide, se vuelve a bajar. Prueba
    nueva: `otro_archivo_del_mismo_tamano_no_es_el_agente`.
  - `Agent::last_line` ya no se traba. Tomaba dos veces el mismo candado y esperaba para siempre: si el
    agente no se conectaba, «Hostear partida» se quedaba en «Conectando» sin mostrar el error, y si se
    cerraba solo, se trababa todo lo de playit.gg, incluso salir de Parabellum. Prueba nueva:
    `ultima_linea_sin_trabarse`.
- [x] **PR #7 (Forge y NeoForge)** (2026-10-12): un servidor de Forge 1.18.2 creado desde cero en
  Windows guarda `@libraries/net/minecraftforge/forge/1.18.2-40.3.12/win_args.txt` y llega a «Done» en
  63 s. El contrato tiene una constante nueva, `plataforma::FORGE_SERVER_ARGS`, que en Windows vale
  `"win_args.txt"` (lo mismo que antes), y `servidor::forge_launch` (compartido) la usa.
- [x] **PR #8 (procesos atados)**: solo toca `macos/`; en Windows compila igual.

- [x] **PR #6: compilar y correr las pruebas en Windows** (2026-10-12): pasan las 83 pruebas (incluida
  `encuentra_el_java_de_mojang`), compila, y Vanilla 1.21.1 llega al menú en 66 s con el Java que encuentra
  `java::discover`. Juan lo mezcló.

- [x] PR #5 (imágenes): verificado en Windows el 2026-10-12 (pasan las 83 pruebas, compila, y el
  error de una imagen que no se puede abrir dice lo mismo que antes). Juan lo mezcló.

## Próximo en Windows

- La 0.27.0 ya está en `main` (ver «Cambios de Windows»). Sigue: responder y reaccionar desde el panel
  del juego, y la prueba con compañeros en sus casas.

## Cambios de Windows

### 2026-10-12 · 0.27.0: amigos, mensajes y grupos

- Qué cambió:
  - `ui/`: página «Amigos» (`renderFriendsPage`), Inicio sin las tarjetas, responder y reaccionar en el
    chat (`startReply`, `react`, `contextMenu(..., { emojis })`), grupos con foto y dueño (diálogo del
    grupo con modos `create`, `add`, `edit` y `members`), tarjeta de invitación y «Reportar problema».
  - `chat.rs`: `Message` trae `reply` y `reactions`; `Summary` trae `owner`, `photo` y `canEdit`; `Member`
    trae `you`. Nuevos: `react`, `edit_group`, `remove_member`, `transfer_group`; `send_text` recibe la
    respuesta. Comandos nuevos en `cuenta.rs`: `reaccionar`, `editar_grupo`, `sacar_del_grupo`,
    `pasar_grupo`, `invitar_amigo`.
  - `reporte.rs` (nuevo) y el comando `reporte_problema`; `Config` tiene `support_name`
    (`supportName` en config-integrada.json).
  - `firebase-reglas.json`: reglas nuevas. Juan las publica antes de la 0.27.
- Mac: nada de código. Revisa en la ventana de Mac: la página de Amigos, responder (Esc cancela),
  reaccionar, la foto de un grupo (elegir una imagen; HEIC no se puede elegir desde el navegador) y
  «Reportar problema» (el resumen dice «Mac (aarch64)»). Si tu `config-integrada.json` no trae
  `supportName`, el reporte sale sin nadie elegido: pídeselo a Juan.

### 2026-10-12 · 0.26.3: un solo Parabellum abierto

- Qué cambió: `main.rs` toma un candado por carpeta (`datos/parabellum.lock` con `File::try_lock`, de la
  biblioteca estándar de Rust). Si ya hay otro abierto, deja `datos/mostrar-ventana` y se cierra; el que
  está abierto (bucle de 2 s en `cuenta.rs`) lo ve y se muestra. Con `--reiniciar` espera hasta 20 s.
- Mac: nada de código (el bloqueo de archivos es el mismo en Mac). Revisa que al abrir Parabellum dos
  veces se muestre el primero y no queden dos.

### 2026-10-12 · 0.26.2: P2P rechazado, actualizar y escribir en 26.3

- Qué cambió:
  - `p2p.rs`: `Gate` devuelve `Result<u16, String>` (el motivo del rechazo va a `p2p.log`); `note` es
    `pub(crate)`.
  - `hosting.rs` + `amigos.rs`: si quien entra por P2P no coincide con ningún amigo, `refresh_presence`
    vuelve a leer la presencia de todos (como mucho cada 10 s).
  - `catalog.rs`: si el `.old` anterior no se deja borrar, se usa `.old-<hora>`; al abrir se limpian.
  - Mod, Fabric 26.3: `PantallaPanel` pide entrada de texto (`textInputManager().startTextInput`) mientras
    está abierto; con SDL3 si no, no llegan las letras.
- Mac: nada de código. En Mac `SELF_UPDATE` es `false`, así que lo de actualizar no aplica todavía; si
  juegas 26.3 en Mac, revisa que se pueda escribir en el panel.

### 2026-10-12 · 0.26.1: de dónde viene el lag

- Qué cambió:
  - `p2p.rs`: `monitor` mide cada segundo (anfitrión e invitado) ping, camino y pérdidas, en
    `datos/logs/p2p-latencia.csv`, y avisa de picos en `p2p.log`.
  - `servidor.rs` + `hosting.rs`: «tick query» cada 4 s (1.20.3+), `rendimiento.csv` en `.lanzador` del
    servidor, y `mspt` / `hitchMs` en la vista.
  - `firebase.rs`: los errores ya no llevan la dirección con `auth=` (la clave de la sesión) y el 402 se
    explica.
  - El proyecto de Firebase está en el plan Blaze: el límite de 100 conexiones a la vez se llenaba.
- Mac: nada de código.

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
