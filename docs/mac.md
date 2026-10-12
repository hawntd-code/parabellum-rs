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
- Conexión directa entre PCs (P2P, `p2p.rs` con iroh): es código compartido y iroh funciona en Mac,
  así que no hay nada que hacer en `macos/` (falta probarla).
- El panel dentro del juego (`mod/`): es Java y se dibuja con Java2D en modo «headless» (Minecraft lo
  pone así en Mac), así que debería funcionar igual; falta probarlo. Los .jar se compilan en Windows o
  Mac con `mod/compilar.py` (JDK en `~/.jdks`).
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
  `--probar-aviso` dice por qué. Para probarlos antes de la tarea 10, arma una `Parabellum.app` en
  `src-tauri/target/`: dentro van `Contents/Info.plist` (con `CFBundleIdentifier` igual a
  `mx.parabellum.launcher`) y `Contents/MacOS/Parabellum`. Fírmala con `codesign --force --sign -` y ábrela con
  `open -n -W --env PARABELLUM_ROOT=… Parabellum.app --args --probar-aviso`.
- Lo compartido ya pregunta por Mac donde Mojang distingue sistemas: reglas `"os": "osx"` (así se
  agrega `-XstartOnFirstThread`), nativos `natives-macos`, Java de Mojang `mac-os-arm64` y separador
  del classpath `:`.
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
8. **Hostear servidores** — `PLAYIT_AGENT` es `None` porque playit.gg no publica su agente para Mac.
   Es de código abierto (github.com/playit-cloud/playit-agent): compilarlo para
   `aarch64-apple-darwin`, publicarlo en un release propio y poner aquí su enlace, SHA-256 y tamaño.
9. **Versiones viejas (1.18 y anteriores)**: no traen nativos para chip Apple. Hay que sustituir LWJGL
   por versiones arm64 (como hacen Prism y otros launchers) o correrlas con Rosetta y un Java x86_64.
10. **Empaquetar como `.app`** (y `.dmg`): `tauri.conf.json` tiene `bundle.active = false` y solo un
    icono `.ico`. Hacen falta un `.icns` y activar el empaquetado para Mac.
11. **Actualización automática** — `EXECUTABLE` y `catalog::apply_launcher_update` reemplazan el
    `.exe`; en Mac hay que reemplazar la `.app` entera y publicar en Drive una entrada aparte para Mac.
    Mientras tanto, `SELF_UPDATE` es `false` en Mac y Parabellum nunca pide actualizarse (si lo hiciera,
    bajaría el `.exe` de Windows y se quedaría bloqueado). Para tener la versión nueva: actualiza `main`
    y recompila. Al terminar esta tarea, ponlo en `true`.
12. **Firma de Apple**: sin cuenta de Apple Developer, la primera vez hay que abrir la app con clic
    derecho › Abrir (o Ajustes del Sistema › Privacidad › «Abrir de todas formas»).

## Compilar en GitHub (sin Mac a la mano)

`.github/workflows/mac.yml` compila en una Mac con chip Apple de GitHub:

- **Solo:** al subir a una rama `mac/…`.
- **A mano:** en GitHub › Actions › «Mac» › Run workflow (o `gh workflow run mac.yml --ref <rama>`).

Hace `cargo check` (rápido; avisa si algo no compila). Con «build» en la entrada del workflow, además
compila el programa y lo deja para descargar en la página de esa ejecución.

Ojo: en repos privados, los minutos de Mac en GitHub cuentan 10 veces. Úsalo cuando haga falta, no en
cada cambio pequeño.

Ya no quedan pruebas ignoradas en Mac por tareas pendientes. Si agregas una que todavía no puede pasar
en Mac, márcala con `#[cfg_attr(target_os = "macos", ignore = "TODO(mac): …")]` y quítalo al terminar
su tarea.

## Cómo probar sin romper nada

- Prueba siempre con `PARABELLUM_ROOT` apuntando a una carpeta de pruebas.
- Antes de abrir el Pull Request: `cargo build` y `cargo test` en tu Mac, y que la acción de GitHub
  pase. Juan revisa que Windows siga compilando.
- Si cambias algo compartido (fuera de `plataforma/macos/`), dilo en el Pull Request.
