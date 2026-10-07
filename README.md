# Parabellum (Rust + Tauri)

Launcher de Minecraft no premium. Motor en Rust (`src-tauri/`) e interfaz en HTML/CSS/JS sin dependencias (`ui/`).

## Compilar

Requiere Rust (rustup, toolchain MSVC) y las Build Tools de Visual Studio con C++.

```
cd src-tauri
cargo build --release
```

Genera `src-tauri/target/release/Parabellum.exe`, un único ejecutable (la interfaz va dentro).
Usa WebView2, que ya viene en Windows 10 y 11.

Para revisar el diseño sin compilar: sirve la carpeta `ui/` con cualquier servidor estático; fuera de
Tauri la interfaz usa datos de ejemplo.

## Carpeta portable

Todo cuelga de la carpeta del .exe (o de `PARABELLUM_ROOT`), con la misma organización que la versión en C#:

```
Parabellum.exe
instancias/   cada una con su instancia.json
base/         versions, libraries, assets, java compartidos por todas las instancias
datos/        ajustes y registros
```

## Diagnóstico

`Parabellum.exe --comprobar <instancia>`, `--crear <nombre> <versión>` y `--jugar <instancia> [servidor]`
escriben el resultado en `datos/logs/comando.txt`.

## Hoja de ruta

0. Proyecto y herramientas ✔
1. Launcher mínimo: nombre y UUID offline, versiones de Mojang, descarga con Java, jugar ✔ (en pruebas)
2. Servidores: servidor de sesión local (mundos LAN/e4mc en modo online), pruebas con servidores públicos
3. Interfaz completa
4. Fabric y mods
5. Paquetes y Google Drive (versiones para la escuela), auto-actualización
6. Forge/NeoForge e instancias de CurseForge
7. Social: cuentas de Google, amigos, presencia, unirse
8. Chat
