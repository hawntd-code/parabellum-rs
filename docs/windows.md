# Parabellum en Windows

La versión que ya funciona (Windows 10 y 11). Lo compartido está en `ui/` y `src-tauri/src/`; lo que
solo sirve en Windows está en `src-tauri/src/plataforma/windows/`:

- `mod.rs` — Java (`javaw.exe`/`java.exe`), procesos (job de Windows, sin ventana de consola, taskkill,
  tasklist), argumentos de Java en ANSI (con nombres cortos 8.3), cifrado DPAPI, idioma, Explorador,
  memoria, tarjetas de video (registro), registro SRV (DnsQuery), sonido (PlaySound), imágenes (WIC) y
  el agente de playit.gg para Windows.
- `avisos.rs` — notificaciones de Windows (toasts) registradas por usuario, sin instalador.
- `accesos.rs` — accesos directos en el Escritorio y el menú Inicio.

Las dependencias de Windows (`windows`, `windows-sys`) están en `src-tauri/Cargo.toml`, en la sección
`[target.'cfg(windows)'.dependencies]`.

## Compilar

Requiere Rust (rustup, toolchain MSVC) y las Build Tools de Visual Studio con C++.

```
cd src-tauri
cargo build --release
```

Genera `src-tauri/target/release/Parabellum.exe` (la interfaz va dentro). Usa WebView2, que ya viene en
Windows 10 y 11.

Si cambias algo en `plataforma/mod.rs` (el contrato), agrégalo también en `plataforma/macos/` (aunque
sea con un `TODO(mac)`): así la versión de Mac sigue compilando. Ver `docs/mac.md`.

Si cambias algo compartido (`ui/`, `mod/`, `src-tauri/src/` fuera de `plataforma/windows/`), anótalo
en `docs/mac-cambios.md` en el mismo commit, con qué tiene que hacer Mac (o «nada»): Diego trabaja
la versión de Mac en paralelo y así se entera.
