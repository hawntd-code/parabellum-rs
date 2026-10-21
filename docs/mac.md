# Parabellum en Mac (chip Apple) — guía para desarrollar

Parabellum es **un solo proyecto para Windows y Mac**. Casi todo el código es compartido. Lo único
que cambia entre sistemas vive en `src-tauri/src/plataforma/`.

```
ui/                          interfaz (HTML/CSS/JS): la misma en los dos sistemas
src-tauri/src/               motor en Rust: compartido (instancias, Modrinth, amigos, chat,
                             servidores, armar el comando de Java, catálogo de Drive…)
src-tauri/src/plataforma/
  mod.rs                     EL CONTRATO: lista de lo que cada sistema tiene que dar
  windows/                   Windows (ya funciona; no tocar sin avisar a Juan)
  macos/                     ← AQUÍ VA TU TRABAJO
    mod.rs                   sistema: Java, procesos, cifrado, memoria, sonido, imágenes…
    avisos.rs                notificaciones
    accesos.rs               accesos directos (en Mac no aplican)
docs/mac.md                  esta guía (actualízala conforme avances)
docs/mac-cambios.md          cambios de Windows que te tocan y pendientes (léelo al empezar)
.github/workflows/mac.yml    compila en una Mac de GitHub (manual o al subir una rama mac/…)
```

## Reglas para no romper Windows

1. **Lo de Mac va en `plataforma/macos/`.** Si algo compartido necesita comportarse distinto en Mac,
   no pongas `if mac` en medio del código: agrega una función al contrato (`plataforma/mod.rs`),
   impleméntala en `windows/` (con lo que ya hacía) y en `macos/`.
2. **Las dos carpetas dan exactamente lo mismo** (mismas funciones, constantes y tipos). Si agregas algo
   en una y no en la otra, el otro sistema deja de compilar.
3. **Dependencias solo de Mac** en `src-tauri/Cargo.toml`, en la sección
   `[target.'cfg(target_os = "macos")'.dependencies]`.
4. **Textos visibles** siempre en español e inglés: en Rust con `t!("español", "English")`, en la
   interfaz con `t("español", "English")`. Los comentarios del código van en español.
5. **Nada de secretos en el repositorio.** `config-integrada.json` (claves de Google, Firebase y Drive)
   no se sube nunca: pídeselo a Juan por privado y ponlo en la raíz del proyecto. Sin él, Parabellum
   compila igual, pero sin iniciar sesión con Google ni catálogo de Drive.
6. **Ramas:** trabaja en ramas `mac/<tema>` (por ejemplo `mac/llavero`) y abre un Pull Request a `main`.
   Al subir una rama `mac/…`, GitHub la compila en una Mac (ver «Compilar»).

## Compilar en tu Mac

Requisitos: macOS 13 o más nuevo, **Xcode Command Line Tools** (`xcode-select --install`) y **Rust**
(https://rustup.rs). No hace falta Node ni nada más.

```bash
cd src-tauri
cargo build --release
./target/release/Parabellum
```

Para no mezclar tus pruebas con tus datos, usa una carpeta aparte:

```bash
PARABELLUM_ROOT=~/parabellum-pruebas ./target/release/Parabellum
```

Para armar `Parabellum.app` y el `.dmg` que se reparte (compila y deja los dos en `src-tauri/target/mac/`):

```bash
src-tauri/mac/empaquetar.sh
```

Usa herramientas que trae macOS: `sips` e `iconutil` para el ícono (sale de `ui/logo.png`), `codesign`
para la firma ad hoc y `hdiutil` para el `.dmg`. El `Info.plist` está en `src-tauri/mac/`. Para abrir la
app con una carpeta de pruebas: `open -n --env PARABELLUM_ROOT=~/parabellum-pruebas
src-tauri/target/mac/Parabellum.app`. Sin `PARABELLUM_ROOT`, usa `~/Library/Application Support/Parabellum`.

Parabellum también tiene comandos sin ventana para probar (el resultado queda en
`datos/logs/comando.txt`). Ver la tabla del README; los más útiles para Mac:

```bash
./target/release/Parabellum --crear "Prueba" 1.21.1 fabric      # crea una instancia
./target/release/Parabellum --probar-juego "Prueba" 240           # la abre y espera al menú
./target/release/Parabellum --ping mc.universocraft.com            # SRV + estado de un servidor
./target/release/Parabellum --probar-sonido                        # sonido de mensaje nuevo
./target/release/Parabellum --convertir-imagen foto.heic salida.png
```

## Qué ya está hecho en Mac

En `plataforma/macos/mod.rs`, con herramientas que trae macOS:

- Abrir carpetas, mostrar archivos y abrir enlaces (`open`).
- Memoria del sistema (`sysctl`, `vm_stat`) y uso de memoria/CPU de un proceso (`ps`).
- Cerrar procesos (`kill`, `pkill`), saber si un PID es Java y listar los Java abiertos (`ps`).
- Idioma del sistema (`defaults read -g AppleLanguages`).
- Registro SRV de Minecraft para las direcciones de playit.gg (`dig`).
- Sonido de mensaje nuevo (`afplay`).
- Guardar la sesión de Google y las claves de playit.gg y Drive (`protect` / `unprotect`): una clave
  aleatoria en el Llavero (servicio «Parabellum», cuenta «clave», con el comando `security`) y cifrado
  ChaCha20-Poly1305 (crate `ring`). Como DPAPI en Windows: solo lo lee tu usuario, en esa Mac. Se usa
  `security` y no la API del Llavero para que macOS no pida la contraseña tras cada actualización
  (sin firma de Apple, cada versión sería otro programa). `cargo test` usa su propia entrada,
  «Parabellum / pruebas».
- Argumentos de Java en UTF-8 (en Windows hacía falta ANSI; en Mac no).
- Ejecutable de Java de los runtimes de Mojang (`jre.bundle/Contents/Home/bin/java`). Al bajarlo, los
  programas que el manifiesto marca como ejecutables reciben su permiso (`make_executable`), y
  `java::discover` lee el `release` junto a `bin/` (en Mac, dentro de `jre.bundle/Contents/Home`).
- Jugar 1.19 en adelante: probado con `--probar-juego` hasta el menú principal en 1.19.2 vanilla y Fabric (Java 17),
  1.21.1 vanilla y Fabric (Java 21, con el mod de Parabellum) y 26.3 (Java 25).
- Forge y NeoForge: sus instaladores son Java y corren igual que en Windows. Probado con
  `--probar-juego` hasta el menú principal: Forge 1.20.1 (47.4.26) y NeoForge 1.21.1 (21.1.259)
  desde cero, los dos con el mod de Parabellum, y los modpacks de Modrinth Cave Horror Project
  (Forge 47.4.23, 165 mods, menú en 26 s) y Better MC BMC4 (Forge 47.4.20, 439 mods, menú en 63 s).
- Servidores de Forge y NeoForge (1.17 en adelante): arrancan con `unix_args.txt`
  (`FORGE_SERVER_ARGS`). El de Windows, `win_args.txt`, separa las rutas con «;» y en Mac Java no
  encontraba la clase principal. Probado: un servidor de Forge 1.20.1 llega a «Done» y uno de NeoForge
  1.21.1 arranca hasta que pide el EULA.
- Carpeta de datos en `~/Library/Application Support/Parabellum` cuando corre desde una `.app`.
- Hostear con playit.gg: el agente 0.17.1 (el mismo que en Windows), compilado para chip Apple por
  GitHub Actions a partir de la etiqueta oficial, sin cambios, en el fork público
  [diegovc20/playit-agent](https://github.com/diegovc20/playit-agent) (rama `parabellum-macos`, release
  `v0.17.1-macos-arm64`). La compilación es reproducible: dos ejecuciones del workflow dieron el mismo
  binario. Parabellum lo baja de ahí, comprueba su SHA-256 (`PLAYIT_AGENT`) al bajarlo y antes de
  abrirlo, y le da permiso de ejecución. playit.gg dejó de publicar binarios para Mac después de la
  0.15.13, y esa no sirve: lee `PLAYIT_SECRET` al compilar, no al abrirse. Para otra versión, una etiqueta
  `v<versión>-macos-arm64` en el fork sobre la oficial compila y publica sola; luego se cambia aquí el
  enlace, el SHA-256 y el tamaño. Probado: se baja, abre y, con una clave falsa, explica «InvalidSecret».
  Falta probar un túnel con una cuenta de playit.gg.
- Conexión directa entre PCs (P2P, `p2p.rs` con iroh): es código compartido y iroh funciona en Mac,
  así que no hay nada que hacer en `macos/`. Probada con Juan (Mac ↔ Windows) el 2026-10-12: directa,
  62–75 ms.
- El panel dentro del juego (`mod/`): es Java y se dibuja con Java2D en modo «headless» (Minecraft lo
  pone así en Mac). Probado en el PR #6 (Fabric 1.21.1, tecla O); falta el panel nuevo de la 0.28.0
  (ver «Para llegar a la 0.28.0» en `mac-cambios.md`). Los .jar se compilan en Windows o Mac con
  `mod/compilar.py` (JDK en `~/.jdks`).
- El juego se abre en su propio grupo de procesos (sigue abierto si se cierra Parabellum).
- El servidor y el agente de playit.gg se cierran solos si Parabellum se cae (`tie_to_launcher`). Un
  vigilante (`sh`) espera leyendo una tubería que solo Parabellum tiene abierta: cuando Parabellum
  termina, aunque sea de golpe, macOS la cierra y el vigilante le manda SIGTERM al proceso (el servidor
  guarda el mundo; en Windows se cierra sin guardar) y, si a los 40 s sigue abierto, SIGKILL. Si el
  proceso termina antes, un hilo de Parabellum se entera con kqueue y cierra al vigilante. Y solo manda
  una señal si el proceso con ese número empezó a la hora que anotó Parabellum (`ps -o lstart=`): así
  no toca otro programa si macOS ya le dio ese número. Probado con `kill -9` a Parabellum: un servidor
  1.21.1 guarda el mundo y se cierra en 1 s. Ojo: Minecraft no anota ese apagado en `latest.log` (su
  registro se apaga antes); se ve en la hora de `level.dat`.
- Avisos (`macos/avisos.rs`) con la API de Apple, UserNotifications (crate `objc2-user-notifications`):
  título, texto, sonido y clic (abre Parabellum o el chat). macOS solo los deja salir desde una `.app`
  que no esté en una carpeta temporal: con el programa suelto de `cargo build` no salen, y
  `--probar-aviso` dice por qué. Para probarlos, arma la app con `src-tauri/mac/empaquetar.sh` y ábrela
  con `open -n -W --env PARABELLUM_ROOT=… src-tauri/target/mac/Parabellum.app --args --probar-aviso`.
- Se actualiza sola (`plataforma/macos/lanzador.rs`, `SELF_UPDATE = true`). Las versiones de Mac van en
  el catálogo con su propio id, `parabellum-mac`, en la carpeta `_parabellum-mac` de la raíz del Drive
  (junto a `_parabellum`, la de Windows): ninguno baja la del otro. Al actualizarse, Parabellum:
  1. Baja el zip de la app y comprueba su SHA-256.
  2. Lo descomprime en `datos/actualizacion` y comprueba la firma (`codesign --verify`) y el
     identificador.
  3. Aparta la app actual como `Parabellum.app.old` y pone la nueva en su lugar; si algo falla, deja la
     anterior.
  4. Se reabre con `open -n … --args --reiniciar` y, al arrancar, borra la `.old`.

  Desde el `.dmg` o desde la copia temporal de macOS («App Translocation») no se puede: pide moverla a
  Aplicaciones. Probado con una 0.27.1 de prueba en la carpeta `paquetes` local: una `.app` 0.27.0 la
  ofreció, se reemplazó y se reabrió en 0.27.1, con la firma válida y con su `PARABELLUM_ROOT`.
- `Parabellum.app` y su `.dmg` (`src-tauri/mac/empaquetar.sh`, ver «Compilar en tu Mac»): la app lleva
  todo dentro (la interfaz, el mod del juego y las fuentes van en el programa) y guarda los datos en
  `~/Library/Application Support/Parabellum`. Firma ad hoc. Al cerrar la ventana, Parabellum sigue en la
  barra de menús; un clic en su ícono del Dock (o abrir la app otra vez) la vuelve a mostrar
  (`RunEvent::Reopen` en `main.rs`). Probado: los avisos salen desde la app y `spctl` la rechaza, como
  se espera sin firma de Apple (tarea 12).
- Lo compartido ya pregunta por Mac donde Mojang distingue sistemas: reglas `"os": "osx"` (así se
  agrega `-XstartOnFirstThread`), nativos `natives-macos`, Java de Mojang `mac-os-arm64` y separador
  del classpath `:`.
- Versiones viejas (1.18 y anteriores), que no traen nativos para chip Apple:
  - **1.18.x** pide Java 17, que sí hay para chip Apple. Se le cambia LWJGL 3.2 por la 3.3.1 de 1.19,
    con sus nativos arm64, y java-objc-bridge por la 1.1 (`adapt_libraries`). Las de Mojang se bajan
    igual, para que un paquete hecho en Mac sirva en Windows.
  - **1.17.1 y anteriores** piden Java 16 u 8, que Mojang solo tiene para Mac con Intel: se baja ese
    (`java_runtime_platform`) y el juego corre con Rosetta, con sus nativos de Intel tal cual, también
    los de LWJGL 2 (1.12.2 y antes). Con LWJGL 3.3.1 no se puede: su GLFW da el error 65548 al ponerle
    ícono a la ventana y Minecraft (hasta 1.17) lo trata como fatal.
  - Si falta Rosetta, la revisión de antes de jugar (ahora también en Vanilla) lo dice y ofrece
    instalarlo: `softwareupdate --install-rosetta`, con la contraseña del administrador. Sin probar en
    una Mac sin Rosetta.
  - Si a una instancia le falta algo de la base (las bibliotecas nuevas, el Java de Intel), «Jugar» lo
    completa con Mojang antes de abrir.
  - Apple anunció que macOS 27 es el último con Rosetta completo. Cuando lo quite, 1.17.1 y anteriores
    necesitarán otra cosa: un GLFW parcheado y un Java 8 arm64, y LWJGL 2 arm64 para 1.12.2 y antes.
  - Probado con `--crear` y `--probar-juego` hasta el menú: 1.18.2 vanilla, Fabric y Forge (nativo,
    12–23 s); 1.17.1, 1.16.5 vanilla, Fabric y Forge, 1.12.2 y 1.8.9 (Rosetta, 20–56 s); el mod de
    Parabellum carga en Fabric 1.16.5, Forge 1.16.5 y Forge 1.18.2. Un servidor 1.16.5 arranca con el
    Java 8 de Intel.
- Pegar imágenes en el chat (`macos/arrastre.rs`, `clipboard_image`): lo que la ventana no pega sola
  (un `.heic` o cualquier imagen copiada en el Finder, o una imagen copiada de Vista Previa) lo lee
  Parabellum del portapapeles con AppleScript (`osascript`): la ruta del archivo (si no es un enlace y
  sus primeros bytes son de imagen) o la imagen guardada como PNG o TIFF en `datos/temporal`.
- Arrastrar al chat una imagen sin archivo (Safari, Chrome, Discord; `macos/arrastre.rs`, `install`):
  Tauri (wry) solo da rutas, así que `install` pone delante de su `performDragOperation:` una función
  que, con Mensajes abierto, guarda la imagen del arrastre (PNG, JPEG, GIF, WebP o HEIC tal cual; si
  no, el archivo prometido, y si tampoco, el TIFF) y la manda como el evento "arrastre". Safari y
  Chrome traen la imagen en JPEG (probado el 2026-10-18). También va delante de `draggingEntered:`:
  wry se caía (`unwrap()` en `collect_paths`) con un archivo que macOS no sabe convertir en ruta, como
  una imagen de «Recientes» del Finder (`file:///.file/id=…`); ese arrastre ahora se rechaza.
- Imágenes del chat en cualquier formato que abra el Mac (HEIC, TIFF, GIF, BMP…; `decode_image`):
  `sips -g` las mide, `sips` las pasa a PNG en un proceso aparte (`-Z` solo si son más grandes que
  el límite, porque también agranda) y el crate `png` las lee a RGBA. Como en Windows, no se giran
  según su EXIF. Probar con `--convertir-imagen foto.heic salida.png`.

## Lista de tareas (busca `TODO(mac)` en el código)

Ordenadas por prioridad:

1. ~~**Que compile y abra la ventana.**~~ — hecho: compila en Mac con chip Apple y la ventana abre.
2. ~~**Guardar la sesión**~~ — hecho (ver «Qué ya está hecho»).
3. ~~**Jugar 1.19 en adelante**~~ — hecho (ver «Qué ya está hecho»).
4. ~~**Avisos**~~ — hecho, con UserNotifications en vez del plugin (ver «Qué ya está hecho»).
5. ~~**Imágenes raras en el chat**~~ — hecho, con `sips` y el crate `png` (ver «Qué ya está hecho»).
6. ~~**Forge y NeoForge**~~ — hecho: el juego y los modpacks funcionan igual; los servidores
   necesitaban `unix_args.txt` (ver «Qué ya está hecho»).
7. ~~**Procesos atados a Parabellum**~~ — hecho, con un vigilante y kqueue (ver «Qué ya está hecho»).
8. ~~**Hostear servidores**~~ — hecho, con el agente de playit.gg compilado en el fork
   diegovc20/playit-agent (ver «Qué ya está hecho»). Falta probar un túnel con una cuenta de playit.gg.
9. ~~**Versiones viejas (1.18 y anteriores)**~~ — hecho: 1.18 nativa con LWJGL 3.3.1 y 1.17.1 y
   anteriores con Rosetta (ver «Qué ya está hecho»).
10. ~~**Empaquetar como `.app`** (y `.dmg`)~~ — hecho, con `src-tauri/mac/empaquetar.sh` (ver «Qué ya
    está hecho»). No usa el CLI de Tauri: `bundle.active` sigue en `false`.
11. ~~**Actualización automática**~~ — hecho: la app entera se reemplaza sola con la versión de Mac que
    se publica en `_parabellum-mac` (ver «Qué ya está hecho» y «Publicar una versión para Mac»).
12. **Firma de Apple**: sin cuenta de Apple Developer, macOS no abre la app bajada de internet la primera
    vez. Hay que ir a Ajustes del Sistema › Privacidad y seguridad › «Abrir de todas formas» (desde
    macOS 15 ya no sirve clic derecho › Abrir).

## Publicar una versión para Mac

Cuando Juan publica una versión de Windows, lo avisa en `mac-cambios.md`. Para publicar la de Mac del
mismo commit:

1. Actualiza `main` a ese commit y arma la app: `src-tauri/mac/empaquetar.sh`.
2. La primera vez, conecta Drive desde esa app (Acerca de › Conectar Drive) con la cuenta que es
   editora de la carpeta del catálogo. El permiso queda cifrado con el Llavero en
   `~/Library/Application Support/Parabellum/datos`.
3. Publica desde la ventana (Acerca de › Publicar esta versión: arma `_parabellum-mac` y la sube) o sin
   ella:

```bash
APP=src-tauri/target/mac/Parabellum.app/Contents/MacOS/Parabellum
$APP --publicar-launcher ~/parabellum-publicar "Notas de la versión"
$APP --subir-launcher-drive ~/parabellum-publicar/_parabellum-mac
```

El resultado queda en `~/Library/Application Support/Parabellum/datos/logs/comando.txt`. Al subir, las
versiones anteriores de `_parabellum-mac` se van a la papelera de Drive. Cada Mac con Parabellum se
actualiza la próxima vez que lo abra.

## Compilar en GitHub (sin Mac a la mano)

`.github/workflows/mac.yml` compila en una Mac con chip Apple de GitHub:

- **Solo:** al subir a una rama `mac/…`.
- **A mano:** en GitHub › Actions › «Mac» › Run workflow (o `gh workflow run mac.yml --ref <rama>`).

Hace `cargo check` (rápido; avisa si algo no compila). Con «build» en la entrada del workflow, además
arma `Parabellum.app` (`mac/empaquetar.sh`) y deja el `.dmg` para descargar en la página de esa ejecución.

El repo es público desde el 2026-10-20, así que estas ejecuciones (también las de Mac) ya no gastan minutos
de pago. Aun así, úsalo cuando haga falta, no en cada cambio pequeño.

Ya no quedan pruebas ignoradas en Mac por tareas pendientes. Si agregas una que todavía no puede pasar
en Mac, márcala con `#[cfg_attr(target_os = "macos", ignore = "TODO(mac): …")]` y quítalo al terminar
su tarea.

## Cómo probar sin romper nada

- Prueba siempre con `PARABELLUM_ROOT` apuntando a una carpeta de pruebas.
- Antes de abrir el Pull Request: `cargo build` y `cargo test` en tu Mac, y que la acción de GitHub
  pase. Juan revisa que Windows siga compilando.
- Si cambias algo compartido (fuera de `plataforma/macos/`), dilo en el Pull Request.
