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

- Última revisión de los PR de Mac en Windows: 2026-10-18, PR #1 a #15 (todos mezclados).
- **Publicado en Drive:** Windows **0.32.0** desde el 2026-10-20 (Juan; el exe publicado es idéntico
  al compilado de `main` en `52fe1d5`). Las reglas de Firebase de Comunidades ya están publicadas.
  **Mac sigue en 0.28.0**, cuatro versiones atrás: puede publicar directo la 0.32.0 (lo de «Para
  llegar a la 0.30.0», abajo, sigue valiendo).
- **En `main`: 0.32.0**, lo mismo que está publicado.
- Mac: hechas las tareas 1 a 11 de `mac.md`. Queda la 12 (firma de Apple), que es decisión de Juan.
  Las reglas de Firebase de la 0.27 ya están publicadas.

## Para llegar a la 0.30.0 (Mac)

Lo que falta para publicar la 0.30.0 de Mac (trae lo de la 0.29.0), en este orden:

1. **Lo que quedó de la 0.28.0** (abajo, paso 3): dentro del juego (pestañas, escribir, emojis,
   capturas) y lo de con un amigo. Juan puede ser el amigo. El panel ya abre con la O en «Fabric 26.3»
   (2026-10-17).
2. **Probar lo de la 0.29.0** (ver la entrada de la 0.29.0 en «Cambios de Windows»):
   - Descargas: dos modpacks seguidos (el segundo en cola), cancelar uno, reintentarlo y el límite de
     velocidad.
   - Cmd+V en cualquier parte de Mensajes y arrastrar una imagen desde Finder. Arrastrar desde el
     Finder (Descargas), Safari y Chrome probado el 2026-10-18 (PR #16); desde «Recientes» no se puede
     (ver el PR #16).
   - Pregenerar un servidor Fabric y «Buscar el mod culpable» en una instancia con un mod roto.
   - 0.30.0: la pantalla de carga, el botón «Modo escuela» y el chat (mandar, borrador, emojis,
     buscar, «está escribiendo…» con Juan).
3. ~~**Código:** el `TODO(mac)` de `plataforma/macos/arrastre.rs`: `clipboard_image` con NSPasteboard
   (lo que la ventana no sabe pegar, como un archivo copiado en Finder).~~ — hecho en el PR #15, con
   AppleScript (`osascript`). Arrastrar una imagen sin archivo desde Safari, Chrome o Discord: hecho en
   el PR #16.
4. **Publicar la Mac 0.30.0** cuando Juan publique la 0.30.0 de Windows (compilada de `main` en ese
   commit, como la 0.28.0).
5. **Opcional: los paquetes de Mac en el espejo de Drive** (así una Mac también juega en la escuela).
   El espejo ya tiene lo que sirve en los dos sistemas (sonidos, bibliotecas, loaders, servidores);
   a Mac le faltan solo su Minecraft (los nativos) y su Java, que van con el sistema en el nombre
   (`minecraft/1.20.1@macos-aarch64`, `java/…@macos-aarch64`). Desde tu Mac, con «Conectar Drive»
   (tu cuenta ya puede escribir en la carpeta): `--espejo-subir todo sin-servidores`. Lo que ya está
   se salta: solo sube lo de Mac. No lo corran los dos a la vez (las capas se numeran solas).

## Para llegar a la 0.28.0 (Mac)

Lo que faltaba para que Mac estuviera a la par de Windows en la 0.28.0 (publicada; queda el paso 3):

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
   - **El modpack «Fabric 26.3» de Drive trae la tecla de Parabellum en `[`**: su `options.txt` dice
     `key_key.parabellum.abrir:key.keyboard.left.bracket`, así que la O no abre el panel a quien lo
     instala (en Windows o en Mac). Diego la cambió a mano (2026-10-15). Sugerencia: ponerla en O en tu
     instancia y volver a exportar, o que al armar paquetes se quite esa línea para que quede la O por
     defecto.
   - 2026-10-17: con la O ya en `options.txt`, **el panel abre en «Fabric 26.3»**. Ojo: en una instancia
     **vanilla** 26.3 la O abre la «Lista de amigos» de Mojang (`key_key.friends:key.keyboard.o`, que es
     lo que trae 26.x por defecto). Ahí no hay panel (el mod solo va con un loader), pero en una
     instancia Fabric 26.x nueva **la de Mojang y la de Parabellum quedan las dos en la O**. En el
     modpack de Drive no pasa porque trae la de Mojang en `]`. Sugerencia: otra tecla por defecto en
     `mod/versiones/fabric-26.*`, o mover la de Mojang al poner el mod.
   - Falta lo de dentro del juego (pestañas, escribir, emojis, capturas) y lo de con un amigo.

## Versiones publicadas en Drive

Lo que ya tienen los compañeros: Windows en `_parabellum` y Mac en `_parabellum-mac`. La más nueva, arriba.
Desde la 0.30.1 hay también un canal de pruebas: `_parabellum-pruebas` y `_parabellum-mac-pruebas`.

- **0.32.0** · 2026-10-20 · trae lo de la 0.31.3 y la 0.32.0. Publicada por Juan, compilada de `main`
  en `52fe1d5`:
  - **Modrinth en la escuela sin modo escuela**: si la red lo bloquea, Parabellum pregunta por el
    puente (`puente-modrinth/`) y los archivos siguen bajando del CDN de Modrinth.
  - **Interfaz pulida** de punta a punta (`docs/auditoria-ui.md`):
    - campos visibles en tema oscuro;
    - Amigos en dos columnas;
    - gráficas del servidor;
    - esqueletos mientras carga;
    - cambio de tema con un círculo;
    - y los errores de la auditoría.
  - Arreglado: a veces no se cargaban amigos ni mensajes al abrir.
- **0.31.2** · 2026-10-18 · Publicada por Juan, compilada de `main` en `6cd5a51`: el catálogo
  reintenta los `.json` que Drive no entrega a la primera. Antes, a veces no aparecía un paquete o la
  versión nueva.
- **0.31.1** · 2026-10-18 · trae lo de la 0.30.1, la 0.31.0 y la 0.31.1. Publicada por Juan, compilada
  de `main` en `949be92`, con las reglas de Firebase de Comunidades publicadas antes:
  - **Comunidades** (botón del escudo):
    - unirse con un código o con «Unirse» en una invitación por mensaje;
    - anuncios, reglas, servidores con su estado y miembros con sus permisos;
    - «Jugar» instala o actualiza el paquete oficial y entra directo al servidor.
  - El paquete oficial sale de una instancia de quien lo publica: se sube solo a Drive.
  - **Canal de pruebas** en Ajustes › Acerca de (abierto a todos).
  - El logo nuevo también en el exe y en los accesos directos.
- **0.30.0** · 2026-10-18 · trae lo de la 0.29.0 y la 0.30.0 (la 0.29.0 no salió sola). Publicada por
  Juan, compilada de `main` en `853716e`:
  - **Todo Minecraft en Google Drive** (el espejo: versiones, loaders y servidores) y el botón **Modo
    escuela**.
  - Gestor de **Descargas**: cola, cancelar, reintentar y límite de velocidad.
  - Descargas más rápidas.
  - Pegar y arrastrar imágenes al chat.
  - **Pantalla de carga**: busca la versión nueva antes de mostrar lo demás y espera a tus amigos y
    mensajes.
  - Chat:
    - el mensaje sale al momento, con «Reintentar» si falla;
    - borradores, enlaces, buscar, emojis;
    - «está escribiendo…».
  - Servidores: menos trabones (guardado sin forzar el disco) y «Pregenerar el mundo».
  - Doctor: «Buscar el mod culpable».
  - «Verificar» revisa por huella también Minecraft y su Java.
  - **Una actualización que no arranca se deshace sola**, desde esta versión.
  - Logo nuevo en la ventana. El exe todavía lleva el icono viejo; se corrige en la 0.30.1.

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

- [ ] **PR #16 (arrastrar imágenes en Mac): nada que compilar en Windows** (solo cambia
  `plataforma/macos/arrastre.rs` y los docs). Para saber:
  - **Arreglo de un cierre en la Mac 0.28.0 publicada:** arrastrar a la ventana una imagen desde
    «Recientes» del Finder cerraba Parabellum. El Finder da ahí una referencia
    (`file:///.file/id=…`) que macOS no convierte en ruta, y wry (Tauri) hace `unwrap()` sobre esa
    ruta en `draggingEntered:`. Ahora ese arrastre se rechaza (el cursor dice que no) y la app sigue
    abierta. Desde la carpeta de verdad (Descargas, Escritorio…) sí se puede. Es solo de Mac: en
    Windows, wry usa otro código.
  - Las imágenes sin archivo (Safari, Chrome, Discord) llegan a la interfaz por el mismo evento
    "arrastre" que en Windows; `ui/` no cambia.

- [x] **PR #15 (pegar imágenes en Mac): nada que compilar en Windows** (solo cambia
  `plataforma/macos/arrastre.rs` y los docs). Hechas las dos sugerencias el 2026-10-18 (en la 0.30.0):
  - `juego::free_panel_key`: en 26.x, mientras la tecla del panel siga en la O, la «Lista de amigos»
    de Mojang pasa a «]» al poner el mod (si ya hay `options.txt`). Prueba `tecla_del_panel_en_26`.
  - Al exportar un paquete, `options.txt` va sin `key_key.parabellum.abrir` (`packs::without_panel_key`):
    a cada quien le queda la O. El modpack «Fabric 26.3» de Drive se arregla al volver a exportarlo.
  - `cuenta::leer_imagen` (los dos sistemas) rechaza enlaces y archivos que no son imagen por dentro
    (`imagen::is_image`, prueba `imagen_por_dentro`).

  Lo que sugería Diego:
  - **La tecla O en 26.x** choca con la «Lista de amigos» de Mojang (ver «Para llegar a la 0.28.0»,
    paso 3).
  - **Revisar que una imagen pegada sea imagen de verdad.** En Mac, `clipboard_image` ya no acepta un
    archivo del Finder solo por su nombre: lo rechaza si es un enlace o si sus primeros bytes no son de
    PNG, JPEG, GIF, WebP, BMP, TIFF, ICO o HEIC/AVIF (`is_image_file`). Antes, `cuenta::leer_imagen`
    habría pasado a la ventana cualquier archivo con nombre de imagen. En Windows (`windows/arrastre.rs`
    y `leer_imagen`) sigue bastando el nombre.

- [x] **PR #14 (base de Drive en Mac): compilar, correr las pruebas e instalar una versión desde Drive.**
  Hecho el 2026-10-17 (Juan lo mezcló): compila sin advertencias y pasan las 93 pruebas de Windows. Con
  Mojang bloqueado (un proxy que solo deja pasar Google, como la red de la escuela), «Escuela 1.20.1»
  se creó desde Drive en 38 s sin pasar por Mojang (`base_complete` da que sí) y `--probar-juego` llegó
  al menú en 14 s; una versión que no está en Drive (1.19.4) da el aviso de Mojang bloqueado. Las 5
  bases de Drive (1.20.1, 1.21.1, 26.1.2, 26.2 y 26.3) no traen ningún `.parte` ni `.descargando`.
  Lo que pedía el PR:
  `install_vanilla` (`main.rs`):
  - **El problema:** la base de Drive se arma en Windows y trae su Java y sus nativos. En Mac, una 26.3
    desde Drive quedaba sin Java de Mac y sin `natives-macos`, y cada «Jugar» volvía a tomar la base sin
    pasar por Mojang.
  - **El arreglo:** tras instalar la base, `base_complete` revisa si sirve en este sistema (los archivos
    de `launch::version_files` y un Java de su versión). Si sirve (Windows), termina ahí sin Mojang,
    como antes. Si no (Mac), completa con `mojang::install`.
  - **Para probar en Windows:** una versión desde Drive no debe pasar por Mojang (sin red a Mojang
    tiene que seguir sirviendo).
  - **Pendiente para después:** sin Mojang (en la escuela), una Mac no puede jugar esas versiones;
    harían falta bases para Mac en Drive.

- [x] **PR #13: compilar y correr las pruebas en Windows.** Hecho el 2026-10-17 (Juan lo mezcló): pasa
  `encuentra_el_java_de_mojang`. Encontré un caso que quedaba abierto, y lo arreglé en `main` (ver
  «Cambios de Windows», 2026-10-17): un Java cortado en casa y completado desde Drive en la escuela
  seguía «a medias» para siempre. También moví el comentario de `major_of`, que había quedado encima
  de `interrupted`. Cambios compartidos del PR:
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

- La 0.29.0 está lista para que Juan la publique. Después: la lista de cambios de Juan para la siguiente
  iteración (2026-10-17).

## Cambios de Windows

### 2026-10-20 · Pulido de la interfaz (0.32.0)

- Qué cambió: la auditoría de `docs/auditoria-ui.md`, por tandas. Hasta ahora:
  - **arranque**: `init()` espera a que estén cargados `hostear.js` y `comunidad.js`. Antes, si los
    datos llegaban primero, fallaba la lista de Servidores y no se cargaban amigos ni mensajes;
  - **campos, listas desplegables, casillas y opciones**: una sola forma en `styles.css`. El campo lleva
    su borde (`--field-border`), las listas desplegables una flecha propia (`--chevron`) y las casillas
    y opciones son propias;
  - en las ventanas largas, la fila de botones se queda abajo a la vista (`fitModal`, clase `scrolls`);
  - el menú del clic derecho sale donde está el ratón (`contextMenu(..., { at })`);
  - las columnas de Contenido alinean aunque un mod tenga actualización;
  - **Amigos** en dos columnas (solicitudes y amigos; grupos) con agregar arriba; tu tarjeta en el
    panel con tu cabeza y la dirección de tu mundo;
  - **Detalles** de la instancia en tarjetas; **Acerca de** con logo y versión;
  - **servidor**: minigráficas de jugadores, TPS y procesador, y números que cuentan;
  - **esqueletos** mientras carga (Explorar, Contenido, Proyecto, Comunidades);
  - **cambio de tema** con un círculo (`document.startViewTransition`: en WebKit de Mac quizá no
    exista; entonces cambia sin animación, que es lo esperado);
  - los errores internos de la ventana ya no salen tal cual en los avisos (`friendlyError`).
- Mac: nada distinto. Si algo se ve raro en WebKit (las casillas propias, la flecha de las listas
  desplegables), avisa en un PR.

### 2026-10-19 · Modrinth en redes que lo bloquean, sin modo escuela (0.31.3)

- Medido en la red de la escuela (UABC):
  - un FortiGate bloquea por nombre `api.modrinth.com`, Mojang, `meta.fabricmc.net`, Forge, Quilt,
    CurseForge, playit y los relevos P2P;
  - deja pasar Google, Drive, Firebase, `*.run.app`, `cdn.modrinth.com`, `maven.fabricmc.net`,
    NeoForge y GitHub.
- Qué cambió:
  - **`puente-modrinth/`** (nuevo): una función de Cloud Run en el proyecto `parabellum52`,
    `https://modrinth-86460886589.us-central1.run.app`. Hace las mismas peticiones a la API de
    Modrinth, solo en las 10 rutas que usa Parabellum, con caché y gzip. Lo publicó Juan el
    2026-10-19; cómo actualizarlo está en su README.
  - **`modrinth.rs`**: va directo y, si la red no deja (no conecta o contesta un filtro), sigue por
    el puente toda la sesión. En modo escuela, el puente primero. Los archivos siguen bajando de
    `cdn.modrinth.com`. `PARABELLUM_MODRINTH_PUENTE` cambia el puente, para probar.
  - **Explorar** ya no abre en los paquetes de Drive con el modo escuela: Modrinth funciona.
  - **`espejo::install_loader`**: si el espejo no tiene la versión exacta del loader que pide un
    modpack de Modrinth, usa la más nueva que tenga para esa versión de Minecraft, como ya hacían los
    servidores.
- Probado con el bloqueo de la escuela simulado (proxy muerto salvo Google y el CDN):
  - buscar (2.4 s, por el puente);
  - instalar Sodium (3 s);
  - revisar actualizaciones;
  - instalar Fabulously Optimized desde Modrinth (49 mods, 52 s), que llegó al menú con 150 mods.
  - En la escuela de verdad, con un puente local: el cambio al puente tardó 0.1 s.
- Mac: nada distinto. El puente sirve igual en Mac.

### 2026-10-18 · El catálogo reintenta los .json que Drive no entrega (0.31.2)

- Qué cambió: `catalog::download_text` (los `.json` del catálogo y el índice del espejo) reintenta dos
  veces si Drive contesta con una página o se corta. No reintenta si el archivo no existe o no está
  compartido. Al leer el catálogo con la 0.31.1 recién publicada, 2 de 5 lecturas perdían un `.json`;
  una de ellas fue el de la versión nueva, que esa vez no aparecía como actualización.
- Mac: nada distinto (también lee `_parabellum-mac` con esto).

### 2026-10-18 · Comunidades: formularios nuevos y el paquete oficial desde una instancia (0.31.1)

- Qué cambió:
  - **Arreglo:** «Pedir la versión que está ahora en Drive» (y cada vez que se guardaba un paquete o un
    servidor) creaba otro igual. El `id` llegaba vacío porque `Package.id` y `Server.id` tenían
    `#[serde(skip_deserializing)]`. Ahora se respeta.
  - **Paquete oficial desde una de tus instancias:** `publicar_paquete_comunidad(id, instancia,
    paquete?, tarea)`. Exporta la instancia sin Minecraft (`ExportOptions.skip_base`, sale del espejo)
    y la sube al catálogo de Drive con «Conectar Drive». Después queda como paquete de la comunidad.
    Es una tarea de Descargas.
  - `borrar_paquete_comunidad`.
  - `comunidad` ahora trae, por paquete:
    - `local`: el estado en este PC (`lista`, `falta` o `desactualizada`). `source` indica que este
      PC tiene la instancia original de quien la publicó, y entonces se juega con ella.
    - `catalog`: el icono, los mods, el tamaño y la versión que hay en Drive.
  - **Ventanas nuevas** en `ui/comunidad.js` (`cmDialog`, en `#cm-dialog`, que reemplaza a
    `askForm`/`#cm-form-dialog`):
    - comunidad: la imagen se elige con un clic, y también desde la imagen de arriba;
    - anuncio y reglas: con vista previa en Markdown (limpio, sin imágenes);
    - servidor: con «Probar», y el paquete elegido en tarjetas;
    - invitar: el código grande, cuándo vence y varios **amigos y grupos** a la vez;
    - miembro: rol y permisos explicados;
    - paquete: «Mis instancias» o «Ya en Drive», en tarjetas.
  - La tarjeta del paquete muestra si está instalado, con «Instalar» o «Actualizar». En sus opciones:
    publicar otra versión, pedir la de Drive, cambiarlo o quitarlo.
  - La columna derecha es más angosta (~30 %) y compacta; servidores y anuncios ocupan el resto.
- Mac: nada distinto, es código compartido. Para probar «Mis instancias» hace falta «Conectar Drive».

### 2026-10-18 · Comunidades: la página

- Qué cambió: `ui/comunidad.js` (nuevo, se carga después de `hostear.js`) y la página `#page-communities`
  con su botón en la barra lateral (`i-community`).
  - Unirse con un código, o con «Unirse» en una invitación por mensaje
    (`parabellum:invitacion:{"t":"comunidad","c":código}`).
  - Anuncios, reglas, servidores con su estado y las direcciones Java y Bedrock para copiar.
  - «Jugar» instala o actualiza el paquete oficial con `instalar_de_catalogo` y entra directo con
    `play(id, dirección)`.
  - Paquete oficial (un modpack del catálogo), códigos de invitación y miembros con su rol y sus permisos.
  - Formularios: `#cm-form-dialog` (`askForm`).
- `PARABELLUM_FIREBASE_EMULADOR` (solo para pruebas): la base de datos del emulador, para probar con
  reglas sin publicar (`firebase::Database::url` acepta `?ns=`).
- Mac: nada distinto. Prueba unirte con un código que te mande Juan cuando publique las reglas.

### 2026-10-18 · Comunidades (en curso): reglas de Firebase y motor

- Qué cambió:
  - `firebase-reglas.json`: `communities`, `communityMembers`, `userCommunities`, `inviteCodes`,
    `communityInvites`, `communityServers`, `communityPosts`, `communityRules` y `communityPackages`.
  - `docs/probar-reglas.py`: 45 casos nuevos (unirse con código, darse permisos, roles, leer sin ser
    miembro…). Todos pasan en el emulador. **Todavía no están publicadas: las publica Juan antes que
    la versión.**
  - `comunidad.rs` (nuevo): comandos para crear, unirse con código, invitar, anuncios, reglas,
    servidores, staff y el paquete oficial (`paquete_comunidad`). Sin escuchas abiertas: se lee al
    abrir la página.
  - **Falta la interfaz** (`ui/comunidad.js`), que viene en el siguiente paso.
- Mac: nada distinto, es código compartido.

### 2026-10-18 · 0.30.1: canal de pruebas, abierto a todos

- Qué cambió: `canal.rs` (nuevo). Cada quien elige en Ajustes › Acerca de «Estable» o «Pruebas»
  (`settings.updateChannel`, comando `canal_actualizaciones`).
  - En pruebas también recibe las versiones de `_parabellum-pruebas` (en Mac, `_parabellum-mac-pruebas`)
    y se queda con la más nueva de las dos carpetas.
  - Si vuelves a estable con una versión de pruebas más nueva, aparece «Volver a la versión estable»
    (`volver_a_estable`, `estado_canal`); los datos se conservan.
  - Al publicar se elige el canal: «Publicar en» en Acerca de, o `--publicar-launcher <carpeta> [notas]
    pruebas`.
  - `catalog::publish_launcher(out, notes, test)`, `catalog::newest_stable`.
  - `arranque::hand_over`: al bajar a una versión anterior a la 0.30.0 no se usa el vigía, porque esa
    versión no confirma su arranque.
- Mac: nada de código. Para probar versiones de Mac antes que todos, publícalas en
  `_parabellum-mac-pruebas` con «Publicar en: Pruebas».

### 2026-10-18 · El logo nuevo también en el exe y en los accesos directos

- Qué cambió: el exe seguía con el logo viejo, porque Cargo no volvía a meter el icono al cambiar
  `icons/icon.ico` y reutilizaba el recurso anterior. `build.rs` ahora vigila `icons/icon.ico` e
  `icon.png`. Contrato de `plataforma::accesos`: `refresh_icons()` (Windows: `SHChangeNotify` para
  que el Escritorio y el menú Inicio vuelvan a leer el icono; se llama al crear un acceso y la primera
  vez que abre cada versión).
- Mac: `refresh_icons` es una función vacía en `macos/accesos.rs`. La `.app` toma el logo de
  `ui/logo.png` al empaquetarla (`empaquetar.sh`). Revisa que el `.icns` salga con el logo nuevo.

### 2026-10-18 · Firebase medido, pruebas de Windows en los PR y sugerir el modo escuela

- Qué cambió:
  - `firebase::metrics`: contadores de escuchas, conexiones, eventos y bytes; `cuenta::startup` los anota
    cada minuto en `datos/logs/firebase.csv` con el tiempo hasta sincronizar. `firebase::Listener` ahora
    es un struct (`Listener::task` para las tareas que no son escuchas).
  - `.github/workflows/windows.yml`: **tus PR también se compilan y prueban en Windows**, sin
    advertencias (`-D warnings`).
  - `ui/`: el aviso de Mojang o de un loader bloqueado trae «Activar modo escuela»; en «Crear
    instancia», la franja `#new-school-hint`.
- Mac: nada distinto. Si quieres comparar, manda tu `firebase.csv`.

### 2026-10-18 · «Verificar» revisa por huella también Minecraft y su Java

- Qué cambió: `launch::known_hashes` (las huellas que publica Mojang de cada archivo de una versión),
  `espejo::Remote::install` con `verify` y `espejo::verify_java`, `mojang::verify_java`, y en `main.rs`
  `verify_base` dentro de `ensure_complete` (solo a fondo). Lo dañado se borra y se baja de nuevo.
- Mac: nada distinto. Para probar: dañar un archivo de una biblioteca sin cambiar su tamaño y usar
  «Verificar» en la instancia.

### 2026-10-18 · Una actualización que no arranca se deshace sola

- Qué cambió: `arranque.rs` (nuevo). Al actualizar, `arranque::hand_over` anota
  `datos/actualizacion/pendiente.json` y abre un **vigía**: el programa anterior con
  `--vigilar-actualizacion`, que abre la versión nueva y espera su confirmación (`confirmar_arranque`,
  lo primero que hace la ventana, sin internet). Si en 90 s no llega, la cierra, vuelve a la anterior,
  la abre y anota `fallida.json` (esa versión no se vuelve a ofrecer). `lanzador::cleanup` ya no corre
  al abrir una versión recién puesta: corre al confirmar.
- **Contrato de `plataforma::lanzador`:** `previous_executable(installed)` y `rollback(installed)`. En
  Mac ya están escritas (`.app.old/Contents/MacOS/Parabellum` como vigía; `.app` → `.app.fallida` y
  `.app.old` → `.app`), pero **sin probar** (`TODO(mac)`).
- Mac: probarlo con una `.app` de versión falsa que no arranque. Se puede acortar la espera con
  `PARABELLUM_ESPERA_ARRANQUE=10`. En Windows lo probé así con `whoami.exe` en lugar de la versión
  nueva (`scratchpad/f57_vigia.py`).

### 2026-10-18 · Logo nuevo y plan hacia diciembre

- Qué cambió: el logo nuevo de Parabellum en `ui/logo.png` (ventana, pantalla de carga) y en
  `src-tauri/icons/icon.png` e `icon.ico` (exe, barra de tareas, bandeja, avisos de Windows). Plan nuevo:
  [plan-diciembre.md](plan-diciembre.md) (comunidades, soporte con tarjetas de ajustes, mundos en el
  Drive de cada jugador, canal de pruebas). Todavía es propuesta: Juan tiene que contestar sus
  decisiones.
- Mac: `empaquetar.sh` arma el `.icns` desde `ui/logo.png`, así que la `.app` toma el logo sola al
  empaquetarla. Lee el plan: cada etapa dice qué toca a Mac (sección 13).

### 2026-10-18 · 0.30.0: espejo de Minecraft en Drive, modo escuela, pantalla de carga y chat

- Qué cambió:
  - `espejo.rs` (nuevo): el espejo de Minecraft en `_espejo/` (capas `capa-NNN.zip` con cada archivo por
    su SHA-1, `o/<sha1>`, y el índice `espejo.indice`). `Builder` lo arma (quien publica), `remote`,
    `install_minecraft`, `install_loader` e `install_server` lo usan. Los paquetes que dependen del
    sistema llevan `@<os>-<arch>` (`espejo::platform()`). `catalog::Catalog` tiene `mirror` (lo
    encuentra al listar) y `RemoteEntry` se guarda en `datos/espejo/`.
  - `main.rs`: `install_vanilla` (modo escuela: espejo y base del catálogo, sin Mojang; si no, Mojang y
    Drive si no responde), `install_loader`, `listar_versiones` (`school`), `versiones_compatibles`,
    `versiones_loader`, `subir_versiones_drive` (ahora al espejo), comandos `modo_escuela` y
    `cuenta::escribiendo`, `--espejo` y `--espejo-subir`. `packs::export_base` ya no existe.
  - `servidor.rs`: `software` (espejo en modo escuela o si falla lo de siempre), `install_software_only`,
    `warm_up`; el instalador de Quilt corre con Java 17 o más nuevo.
  - `settings.rs`: `schoolMode`. `catalog.rs`: carpetas y .json de cada nivel a la vez,
    `quick_launcher_update`; un modpack sin su base la toma del espejo.
  - Pantalla de carga: `SocialView` trae `restoring` y `synced` (`Social::synced`, `Chat::synced`,
    `google::has_session`); la sesión de Google se recupera aparte de la conexión directa.
  - Chat: `Presence.typing` (la conversación donde escribes; las versiones anteriores lo ignoran).
  - `ui/`: `#splash` y `bootSplash`, `#school-toggle`, y en Mensajes buscar, emojis, mensajes al momento,
    borradores y «está escribiendo…».
  - Lo del PR #15: `juego::free_panel_key` (la O en 26.x), `packs::without_panel_key` e
    `imagen::is_image` en `cuenta::leer_imagen` (ver «Pendientes para Windows»).
- Mac: compila y prueba lo de arriba. Nada de código obligatorio. Opcional: tus paquetes en el espejo
  («Para llegar a la 0.30.0», paso 5).

### 2026-10-17 · Un Java completado desde Drive vuelve a contar, y la base pide su propio Java

- Qué cambió (revisando el PR #13):
  - **El caso:** un compañero empieza a bajar Minecraft en casa y lo cierra a medias. Su Java queda con la
    marca `.descargando` y algunos `.parte`. En la escuela, la base de Drive completa ese Java, pero la
    marca y los `.parte` seguían ahí: `java::discover` no lo contaba, `base_complete` decía que no y
    se iba a Mojang, que ahí está bloqueado. No había forma de jugar esa versión en la escuela.
  - `java::package_installed` (nuevo): al terminar de instalar un paquete (`packs::install`,
    `catalog::base_delta` y `catalog::delta_update`), los Java que trae quedan completos: se les quita
    la marca y los `.parte`. Un `.parte` nunca sirve, porque las descargas empiezan de cero.
  - `java::interrupted` ya no cuenta un `.parte` que está junto a su archivo (el archivo llegó de otro
    lado). Prueba ampliada: `encuentra_el_java_de_mojang`.
  - **El Java exacto** (`launch::VersionFiles::own_java`, campo nuevo `java_major`): con el Java 17 a
    medias, `java::select` da uno más nuevo (el 25), y `base_complete` lo daba por bueno. Así, una 1.20.1
    se quedaba con el Java 25 sin bajar el suyo, y Forge 1.20.1 no abre con él. Ahora `base_complete`
    pide la misma versión mayor, como `mojang::install`.
  - Por lo mismo, exportar un modpack o una base (`packs::export` y `packs::export_base`) ya no mete un
    Java de reemplazo en el paquete, porque le llegaría a todos. Si falta el suyo, lo dice. Tampoco mete
    un `.parte` suelto.
  - Probado en Windows con Mojang bloqueado: con el Java 17 de una carpeta de prueba cortado (la marca
    y `jvm.dll.parte`), «--crear» 1.20.1 lo completó desde Drive en 21 s. Ahora usa el Java 17 (antes,
    el 25) y llega al menú en 12 s.
- Mac: nada. Sirve igual en Mac cuando haya bases de Mac en Drive.

### 2026-10-14 · 0.29.0: gestor de descargas, imágenes al chat, descargas más rápidas y Minecraft desde Drive

- Qué cambió:
  - `descargas.rs` (nuevo): el gestor. Cada comando que baja algo corre como una tarea
    (`descargas::start(...).queued().retry(...)` y `job.run(...)`): cola, cancelar, velocidad y límite.
    `download.rs`, `catalog.rs` y `remote_zip.rs` llaman a `descargas::check`/`received` en cada pedazo.
  - `main.rs`: comandos nuevos `lista_descargas`, `cancelar_descarga`, `quitar_descarga`,
    `ajustes_descargas`, `activar_arrastre`, `arrastre_imagenes`, `imagen_portapapeles`. Los de instalar
    aceptan `tarea` (un id que elige la interfaz) y `instalar_modpack` también `titulo` e `icono`.
  - `settings.rs`: `downloadParallel` y `downloadLimitMb`.
  - **Contrato de `plataforma`**: módulo nuevo `arrastre` (`install`, `accept_images`,
    `clipboard_image`). En Windows es un IDropTarget propio que reemplaza al de Tauri (acepta imágenes
    sin archivo del navegador) y lee el portapapeles con OLE. En Mac ya está `plataforma/macos/arrastre.rs`
    sin hacer nada: el arrastre de Tauri sigue igual y la interfaz escucha sus eventos.
  - `ui/`: el panel de Descargas (la flecha de arriba), pegar en cualquier parte de Mensajes.
  - Velocidad: `download.rs` baja 32 a la vez (antes 8), los grandes primero, y corta solo si no llega
    nada en 60 s (antes cortaba toda descarga de más de 300 s). `mojang.rs` pide la lista de sonidos y la
    de Java a la vez. `descargas.rs` manda a la ventana como mucho ~10 avisos de progreso por segundo.
  - `remote_zip::read_many` pide los bloques de 6 en 6 (hilos, cada uno dentro de la tarea de
    Descargas con `descargas::enter`; `descargas::Scope` ahora es Clone). Sirve también en Mac.
  - Minecraft desde Drive: `install_vanilla` (main.rs) usa primero la base del catálogo
    (`catalog::install_base`, por partes) y si no, Mojang. `listar_versiones` trae `drive`.
  - Servidores: `servidor::apply_tuning` (`sync-chunk-writes=false`, campo `tuning`) y pregenerar con
    Chunky (`hosting::pregenerate`, `servidor::chunky_line`, comandos `pregenerar_mundo` y
    `pausar_pregeneracion`, campo `pregenerated`). Mac: nada distinto; pruébalo con un servidor Fabric.
  - Doctor: `culpable.rs` (buscar el mod culpable por mitades), comandos `buscar_culpable` y
    `detener_busqueda`; `probe_launch` (main.rs) es la prueba de abrir que usan los dos;
    `diagnostico::wait_for_menu` reconoce el error de Fabric/Quilt de mods incompatibles y
    `diagnostico::set_aside_log` aparta el latest.log anterior antes de cada prueba. Mac: nada
    distinto (renombra `.jar` a `.jar.disabled`, igual que Contenido).
  - Subir versiones a Drive (quien publica): `packs::export_base`, `drive::Publisher::storage`, comandos
    `versiones_en_drive` y `subir_versiones_drive`.
- Mac:
  - Compila y prueba el panel de Descargas (instalar dos modpacks seguidos, cancelar, reintentar).
  - TODO(mac) en `plataforma/macos/arrastre.rs`: `clipboard_image` con NSPasteboard (public.png,
    public.tiff o un archivo copiado en el Finder) y, si se puede, imágenes arrastradas desde Safari.
    — `clipboard_image` hecho en el PR #15, con AppleScript (`osascript`, sin dependencias nuevas): un
    archivo de imagen copiado en el Finder o la imagen copiada (PNG o TIFF). Arrastrar desde Safari,
    Chrome o Discord: hecho en el PR #16 (delante del soltar de wry, sin cambiar `ui/`).

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
