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
- **Versiones publicadas.** Cada vez que una versión de Windows se publica en Drive (la que se
  actualiza sola en los PC de los compañeros), se anota en «Versiones publicadas en Drive» en ese
  momento: número, fecha y qué cambió para quien juega. Lo pidió Diego para saber qué tienen ya los
  demás.
- Lo más nuevo, arriba. Cada entrada con su fecha y el título del commit. Para ver lo nuevo desde tu
  última sesión: `git log --oneline <último commit que viste>..origin/main`.

## Estado

- Última revisión de los PR de Mac en Windows: 2026-10-13, PR #1 a #12 (todos mezclados).
- **Windows está en pausa en la 0.28.0 hasta que Mac llegue a la par** (decisión de Juan, 2026-10-13).
  Mientras tanto no hay cambios nuevos de Windows en `main`. Ver «Para llegar a la 0.28.0», abajo.
- Windows: la 0.28.0 está publicada en Drive desde el 2026-10-13 (ver «Versiones publicadas en Drive»):
  los compañeros con Windows ya se actualizan a ella. Las reglas de Firebase de la 0.27 ya están
  publicadas.
- Mac: hechas las tareas 1 a 11 de `mac.md`. Queda la 12 (firma de Apple), que es decisión de Juan.

## Para llegar a la 0.28.0 (Mac)

Lo que falta para que Mac esté a la par de Windows:

1. Compilar `main` (0.28.0) en Mac y probar lo de «Pendientes para Mac», abajo, y lo de cada entrada de
   «Cambios de Windows» desde la 0.26.0:
   - 0.26: panel del juego en 26.3 (pestañas con el ratón, escribir) y abrir Parabellum dos veces (esto
     último probado el 2026-10-13: con la app abierta, abrirla otra vez muestra la misma, sin otra copia).
   - 0.27: página de Amigos, responder, reaccionar, grupos con foto y dueño, «Reportar problema».
   - 0.27.1: la página de Servidores (Tus servidores, De tus amigos, Guardados).
   - 0.28.0: vista previa al mandar una imagen (Cmd+V), punto de en línea en Mensajes y el panel del juego
     nuevo (`python mod/vista/vista.py` lo dibuja sin Minecraft; dentro del juego, revisa los emojis).
2. ~~Publicar la primera versión de Mac (0.28.0) en `_parabellum-mac`~~ — hecho el 2026-10-13 (ver
   «Versiones publicadas en Drive»). Compilada de `main` en `ee30e86`, publicada con
   `--publicar-launcher` y `--subir-launcher-drive` desde la cuenta de Diego. El catálogo la ve como
   `parabellum-mac 0.28.0 (Drive)` y el zip se baja sin iniciar sesión con el SHA-256 de su `.json`.
3. Contar en un PR o aquí qué no funcionó, para que Windows lo arregle antes de seguir.
   - 2026-10-14, Diego con la 0.28.0 de Mac: lo de la ventana va bien. «Reportar problema» dice «Mac
     (aarch64)» y se manda a Hawntd.
   - **No abrió «Fabric 26.3»**: «missing `server' JVM … libjvm.dylib». Su Java 25 estaba a medias
     (`libjvm.dylib.parte`): Parabellum se cerró mientras lo bajaba y después lo daba por instalado.
     Arreglado en el PR #13: un Java a medias ya no cuenta y «Jugar» lo completa. Puede pasar igual en
     Windows.
   - Falta lo de dentro del juego (panel en 26.3, emojis, capturas) y lo de con un amigo.

## Versiones publicadas en Drive

Lo que ya tienen los compañeros: Windows en `_parabellum` y Mac en `_parabellum-mac`. La más nueva, arriba.

- **Mac 0.28.0** · 2026-10-13 · la primera para Mac con chip Apple (macOS 13 o más nuevo), en
  `_parabellum-mac`: todo lo de la 0.28.0 de Windows. Desde aquí se actualiza sola.
- **0.28.0** · 2026-10-13 · trae todo lo de la 0.26.0 a la 0.28.0 (ninguna se publicó aparte):
  - Amigos: página propia en la barra (filtros, búsqueda, favoritos) y agregar también desde Mensajes.
  - Mensajes: responder, reaccionar, punto de en línea y vista previa antes de mandar una imagen.
  - Grupos con foto y dueño, y «Reportar problema».
  - Servidores y Hostear en una sola página, en filas.
  - El panel del juego (tecla O) igual que la ventana.
  - P2P más estable (un solo Parabellum abierto, se relee la presencia) y la telemetría de lag.
  - En 26.3, el panel ya acepta clics y teclado.
- **0.25.0** · 2026-10-12 · animaciones en toda la ventana; dentro del juego, panel animado, botón
  «Parabellum» en el menú de pausa, aviso «Pulsa O» al entrar a un mundo y avisos abajo a la derecha.
- **0.24.1** · 2026-10-12 · el mod del juego gasta lo mínimo y se aparta solo si hace caer el juego.

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
   instalación dice cómo abrirla la primera vez (desde macOS 15: Ajustes del Sistema › Privacidad y
   seguridad › «Abrir de todas formas»).

Siguiente para Diego: la **tarea 11**. Con la `.app` lista, es lo que falta para que un compañero con Mac
reciba las versiones nuevas sin compilar. Antes de hacerla, comenta la propuesta de arriba en un PR o
issue para acordar con Juan cómo se publica la de Mac en Drive.

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

- [ ] **PR #13: compilar y correr las pruebas en Windows.** Cambios compartidos:
  - `java::discover` ya no cuenta un Java a medias: con algún `.parte` o con la marca `.descargando`
    (`mojang::DOWNLOADING`), que ahora `mojang::java_files` pone en su carpeta y quita quien lo baja al
    terminar.
  - Antes, si Parabellum se cerraba mientras bajaba un Java, ese Java se daba por instalado sin su
    `libjvm` (en Windows, `jvm.dll`) y el juego no abría. Ahora «Jugar» lo completa (desde el PR #10,
    completa lo que falte de la base).
  - Prueba ampliada: `encuentra_el_java_de_mojang`.
  - Además, `reemplaza_la_app_entera` (solo Mac) ya no ejecuta la app de juguete: la Mac de GitHub
    (macOS 14) no la deja correr, y por eso falló el workflow de Mac del PR #12.

- [x] **PR #12 (actualizarse solo en Mac)** (2026-10-13, Juan lo mezcló): en Windows compila sin
  advertencias, pasan las pruebas (con `actualizacion_del_launcher_segun_el_sistema`) y «Publicar esta
  versión» deja `_parabellum` igual que antes: el mismo `LEEME.txt`, el zip con `Parabellum.exe` y el id
  `parabellum` (lo comparé con lo que publicaba la 0.27.1). Lo que
  Windows hacía con su `.exe` se movió tal cual de `catalog.rs` a `plataforma/windows/lanzador.rs`
  (módulo nuevo del contrato: `ID`, `published_files`, `install`, `restart`, `cleanup`, `readme`; el
  `LEEME.txt` es idéntico). `catalog::LAUNCHER_ID` ya no existe: es `plataforma::lanzador::ID`, que en
  Windows sigue siendo `"parabellum"`. También:
  - El título del diálogo de «Publicar esta versión» ya no nombra `_parabellum`, y el aviso de la
    interfaz usa el nombre real de la carpeta.
  - Comando nuevo `--subir-launcher-drive <carpeta>`: lo mismo que «Subir a Drive», sin la ventana.
  - `ui/`: abajo a la izquierda, junto a la versión, decía «Windows» fijo también en Mac; ahora pone el
    sistema que manda Parabellum (`platform`: «Windows» o «macOS»). Y en Ajustes › Comportamiento,
    «Un aviso de Windows cuando te escriben…» pasa a «Un aviso del sistema…» (también en `i18n.js`).
  - Para probar: «Publicar esta versión» en una carpeta de prueba deja `_parabellum` igual que antes, y
    `cargo test` pasa `actualizacion_del_launcher_segun_el_sistema` (la de Mac no se ofrece en Windows).

- [x] **PR #11 (`.app` y `.dmg` de Mac)** (2026-10-12): compila en Windows sin advertencias. `main.rs`
  atiende los eventos con un `match` y la rama de `RunEvent::Reopen` lleva `#[cfg(target_os = "macos")]`
  (en Windows no existe); `RunEvent::Exit` hace lo mismo que antes. Lo demás es de Mac: `src-tauri/mac/`
  y el workflow de Mac.

- [x] **PR #10 (versiones viejas en Mac)** (2026-10-12): en Windows pasan las 87 pruebas (incluida
  `version_mayor`), compila sin advertencias y, con `--probar-juego`, Fabric 1.16.5 llega al menú en 35 s
  y Fabric 1.18.2 en 58 s, con el mod de Parabellum. Revisé que exportar no meta cada biblioteca dos
  veces (`version_files` quita repetidos al final). Buen hallazgo lo de `major_of` con el Java 8 de
  Mojang: también fallaba en Windows. Lo compartido:
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

- En pausa hasta que Mac llegue a la 0.28.0 (ver «Estado»). Después: la prueba con compañeros en sus
  casas y lo que salga de ella.

## Cambios de Windows

### 2026-10-13 · 0.28.0: el panel del juego con lo mismo que la ventana

- Qué cambió:
  - Mod, núcleo (`mod/nucleo`): `Panel` rehecho (Amigos, Mensajes y Capturas como en la ventana), `Iconos`
    (los trazos de `ui/index.html`), `Emojis` (las seis reacciones dibujadas), `Lienzo` escribe con una
    fuente del sistema lo que Inter no tiene, y `Plataforma.teclaCaptura()` (método por defecto: «F2»).
    Los 18 .jar se recompilaron.
  - `puente.rs`: el estado trae favoritos, miembros, dueño y huella de la foto de cada grupo; operaciones
    nuevas (`reaccionar`, `favorito_chat`, `favorito_amigo`, `crear_grupo`, `agregar_al_grupo`,
    `editar_grupo`, `sacar_del_grupo`, `pasar_grupo`, `salir_chat`, `foto_grupo`, `abrir_carpeta`) y
    `enviar` acepta `respuesta`.
  - `chat.rs`: `watch` (el chat del juego) escucha también las reacciones y devuelve `chat::Watch`.
  - `main.rs`: `set_favorite_chat` (lo usan la ventana y el juego; avisa a los dos).
- Mac: nada de código. En Mac Java2D no tiene emojis de color: `Lienzo.respaldo` busca «Apple Symbols»
  (si no, «Dialog»); revisa cómo se ve un emoji en un mensaje dentro del juego y dime si sale un
  cuadrito. Prueba también «Abrir carpeta» en Capturas (abre Finder).

### 2026-10-13 · Vista previa de imágenes y quién está en línea en Mensajes

- Qué cambió: solo `ui/`. Al adjuntar, pegar o soltar una imagen en el chat ya no se manda directo: sale
  `#image-dialog` con la imagen tal como se va a mandar (Enter envía, Esc cancela). En Mensajes, las
  conversaciones directas y la cabecera llevan el punto de en línea (`presenceHead`).
- Mac: nada de código. Revisa que pegar una captura (Cmd+V) abra la vista previa.

### 2026-10-12 · 0.27.1: Servidores y Hostear en una sola página

- Qué cambió: solo `ui/`. Ya no hay página `host` (`navigate("host")` lleva a `servers`): «Tus servidores»
  (lo que era Hostear partida), «De tus amigos» y «Guardados» van en Servidores, en filas compactas
  (`.srv-row`, `serverCardHtml`). El punto de servidor encendido (`#host-dot`) está en el botón de
  Servidores.
- Mac: nada de código. Revisa que la página se vea bien en la ventana de Mac.

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
