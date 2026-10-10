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

Todo cuelga de la carpeta del .exe (o de `PARABELLUM_ROOT`):

```
Parabellum.exe
instancias/   cada una con su instancia.json
base/         versions, libraries, assets, java compartidos por todas las instancias
datos/        ajustes y registros
paquetes/     (opcional) zips de modpacks copiados de una USB
```

### Desde la versión C# (retirada)

La primera versión de Parabellum, en C#, quedó retirada el 2026-10-08 (repositorio archivado
`parabellum-launcher`). Usaba la misma organización de carpetas, así que Parabellum.exe abre una
de sus carpetas tal cual: instancias (también las de CurseForge), bases, mundos y, si aún no hay
`datos/ajustes.json`, el nombre, el UUID fijo y la memoria de su `datos/config.json`.
Su actualizador acepta cualquier versión nueva del catálogo, así que al publicar Parabellum en
Drive (`_parabellum`) quienes sigan con la versión C# reciben esta en su lugar.

## Para quien lo usa por primera vez

Parabellum no se instala: se descomprime en una carpeta y se abre `Parabellum.exe`. El exe no lleva
firma de pago, así que la primera vez Windows puede mostrar «Windows protegió tu PC»: «Más
información» › «Ejecutar de todas formas». Las actualizaciones las baja Parabellum mismo y ya no
muestran ese aviso. Al publicar una versión (Acerca de › Publicar en la carpeta) se deja un
`LEEME.txt` con estos pasos en `_parabellum`. Los accesos directos del Escritorio y del menú Inicio
se crean desde Ajustes › Comportamiento.

## Idiomas y accesibilidad

- Español e inglés (Ajustes › Apariencia; la primera vez, el de Windows). El motor arma sus mensajes
  con `t!("español", "English")` (`src-tauri/src/idioma.rs`) y la interfaz con `t("español", "English")`
  (`ui/i18n.js`, donde también están los textos fijos de `index.html`). Las instancias nuevas abren
  Minecraft en el mismo idioma (`lang:` de `options.txt`).
- Todo se puede usar con el teclado: foco visible, diálogos que no dejan escapar el foco, menús con
  flechas y Escape, tarjetas con Enter. Avisos anunciados al lector de pantalla, contraste suficiente
  en los dos temas y tamaño de la interfaz de 90 a 150 % (Ctrl + / Ctrl − / Ctrl 0).

## Configuración

La carpeta de Drive y las claves de Google y Firebase se integran al compilar desde
`config-integrada.json` en la raíz del proyecto (no está en el repositorio): `driveFolderId`,
`driveApiKey`, `googleClientId`, `googleClientSecret`, `firebaseApiKey` y `firebaseDatabaseUrl`.
Sin ese archivo se compila igual, sin catálogo de Drive. Un `lanzador-config.json` junto al exe
sustituye los valores que traiga. Las reglas de la base de datos de Firebase están en
`firebase-reglas.json`.

## Diagnóstico

Sin ventana, con el resultado en `datos/logs/comando.txt`:

| Comando | Qué hace |
|---|---|
| `--comprobar <instancia>` | Qué falta para poder jugar |
| `--crear <nombre> <versión> [fabric\|quilt\|forge\|neoforge] [versión del loader]` | Descarga y crea la instancia (Forge y NeoForge con su instalador oficial) |
| `--cambiar-loader <instancia> <versión>` | Cambia la versión del loader |
| `--agregar <instancia> <archivo>...` | Agrega mods, resource packs, shaders o mundos |
| `--jugar <instancia> [servidor]` | Abre el juego (y entra al servidor) |
| `--probar-juego <instancia> [segundos]` | Abre el juego, espera al menú principal, lo cierra y resume los mods cargados |
| `--contenido <instancia>` | Lista los mods, packs y mundos |
| `--buscar <texto> [mod\|resourcepack\|shader] [loader] [versión]` | Busca en Modrinth |
| `--instalar <instancia> <proyecto> [versión]` | Instala de Modrinth con dependencias compatibles |
| `--actualizaciones <instancia> [aplicar]` | Lista (o aplica) las actualizaciones seguras |
| `--exportar <instancia> <carpeta> [con-mundos]` | Paquete para Drive o USB (modpack + base de Minecraft) |
| `--exportar-mrpack <instancia> <archivo.mrpack>` | Modpack en formato de Modrinth |
| `--importar <archivo> [nombre]` | Instala un .mrpack o un paquete de Parabellum |
| `--externas` | Instancias de CurseForge, Prism y MultiMC en este PC |
| `--importar-externa <carpeta>` | Copia una de ellas a Parabellum, con su loader |
| `--instalar-modpack <proyecto> [versión]` | Instala un modpack de Modrinth como instancia nueva |
| `--instalar-datapack <instancia> <mundo> <proyecto>` | Instala un data pack de Modrinth en un mundo |
| `--verificar <instancia> [rapido]` | Comprueba la huella de cada archivo del modpack y repone lo que falte o esté dañado (`rapido`: solo lo que falte) |
| `--catalogo` | Modpacks y versiones de Drive y de la carpeta paquetes, con su estado |
| `--instalar-catalogo <id> [reparar]` | Instala, actualiza o repara un paquete (bajando solo lo que cambió) |
| `--publicar-launcher <carpeta> [notas]` | Prepara _parabellum con esta versión para subirla a Drive |

## Hoja de ruta

Ver [ROADMAP.md](ROADMAP.md).

## Créditos

Fuente [Inter](https://github.com/rsms/inter) bajo la SIL Open Font License (`ui/fonts/OFL.txt`).
