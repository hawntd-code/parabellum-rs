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

Ver [ROADMAP.md](ROADMAP.md).

## Créditos

Fuente [Inter](https://github.com/rsms/inter) bajo la SIL Open Font License (`ui/fonts/OFL.txt`).
