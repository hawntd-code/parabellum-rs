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
- Argumentos de Java en UTF-8 (en Windows hacía falta ANSI; en Mac no).
- Ejecutable de Java de los runtimes de Mojang (`jre.bundle/Contents/Home/bin/java`).
- Carpeta de datos en `~/Library/Application Support/Parabellum` cuando corre desde una `.app`.
- El juego se abre en su propio grupo de procesos (sigue abierto si se cierra Parabellum).
- Lo compartido ya pregunta por Mac donde Mojang distingue sistemas: reglas `"os": "osx"` (así se
  agrega `-XstartOnFirstThread`), nativos `natives-macos`, Java de Mojang `mac-os-arm64` y separador
  del classpath `:`.

## Lista de tareas (busca `TODO(mac)` en el código)

Ordenadas por prioridad:

1. **Que compile y abra la ventana.** Primero que todo: `cargo build` en tu Mac. Si algo de lo
   compartido no compila en Mac, muévelo a la plataforma (regla 1).
2. **Guardar la sesión** — `protect` / `unprotect` en `macos/mod.rs`. Hoy devuelven `None`, así que no
   se guarda la sesión de Google (hay que volver a entrar cada vez) ni las claves de playit.gg y Drive.
   Idea: una clave aleatoria guardada en el Llavero (crate `security-framework` o `keyring`) y cifrar
   con ella (por ejemplo ChaCha20-Poly1305).
3. **Jugar 1.19 en adelante** (vanilla y Fabric): Mojang trae nativos para chip Apple. Probar con
   `--probar-juego`. Revisar que se baje el Java de Mojang (`mojang.rs`, `ensure_java`) y que
   `java_binary` lo encuentre.
4. **Avisos** — `macos/avisos.rs`. Lo más directo: el plugin oficial `tauri-plugin-notification`.
5. **Imágenes raras en el chat** (HEIC del iPhone, TIFF…) — `decode_image` en `macos/mod.rs`. `sips`
   (viene con macOS) las pasa a PNG; falta leer ese PNG a RGBA (crate `png`), o cambiar el contrato
   para que en Mac devuelva el PNG directo.
6. **Forge y NeoForge**: sus instaladores son Java (funcionan igual); probar un modpack 1.20.1.
7. **Procesos atados a Parabellum** — `tie_to_launcher`: en Windows, el servidor y el agente de
   playit.gg se cierran solos si Parabellum se cae. En Mac: guardar los PID y cerrarlos al arrancar si
   quedaron vivos, o un vigilante con kqueue.
8. **Hostear servidores** — `PLAYIT_AGENT` es `None` porque playit.gg no publica su agente para Mac.
   Es de código abierto (github.com/playit-cloud/playit-agent): compilarlo para
   `aarch64-apple-darwin`, publicarlo en un release propio y poner aquí su enlace, SHA-256 y tamaño.
9. **Versiones viejas (1.18 y anteriores)**: no traen nativos para chip Apple. Hay que sustituir LWJGL
   por versiones arm64 (como hacen Prism y otros launchers) o correrlas con Rosetta y un Java x86_64.
10. **Empaquetar como `.app`** (y `.dmg`): `tauri.conf.json` tiene `bundle.active = false` y solo un
    icono `.ico`. Hacen falta un `.icns` y activar el empaquetado para Mac.
11. **Actualización automática** — `EXECUTABLE` y `catalog::apply_launcher_update` reemplazan el
    `.exe`; en Mac hay que reemplazar la `.app` entera y publicar en Drive una entrada aparte para Mac.
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

Dos pruebas están marcadas como «ignoradas en Mac» hasta que se hagan sus tareas (cifrar la sesión e
imágenes): al terminarlas, quita su `#[cfg_attr(target_os = "macos", ignore = …)]` para que vuelvan a
correr.

## Cómo probar sin romper nada

- Prueba siempre con `PARABELLUM_ROOT` apuntando a una carpeta de pruebas.
- Antes de abrir el Pull Request: `cargo build` y `cargo test` en tu Mac, y que la acción de GitHub
  pase. Juan revisa que Windows siga compilando.
- Si cambias algo compartido (fuera de `plataforma/macos/`), dilo en el Pull Request.
