# Parabellum: desarrollo

Cómo está hecho Parabellum, cómo compilarlo y cómo publicar una versión. Lo que ve quien juega está en el
[README](../README.md).

Launcher de Minecraft no premium. Motor en Rust (`src-tauri/`) e interfaz en HTML/CSS/JS sin dependencias (`ui/`).

## Plataformas

Un solo proyecto para **Windows** (la versión que ya funciona) y **Mac con chip Apple** (en
construcción). Casi todo es compartido; lo que cambia entre sistemas vive en `src-tauri/src/plataforma/`:

```
ui/                          interfaz (la misma en los dos)
src-tauri/src/               motor compartido
src-tauri/src/plataforma/
  mod.rs                     el contrato: lo que cada sistema tiene que dar
  windows/                   Windows          → guía: docs/windows.md
  macos/                     Mac (chip Apple) → guía y lista de tareas: docs/mac.md
.github/workflows/mac.yml    compila en una Mac de GitHub (ramas mac/… o a mano)
```

## Compilar

Windows (para Mac, ver [mac.md](mac.md)). Requiere Rust (rustup, toolchain MSVC) y las Build
Tools de Visual Studio con C++.

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
servidores/   los de «Hostear partida» (con su mundo y su configuración)
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

## Primera vez (lo que ve quien juega)

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
  en los dos temas y tamaño de la interfaz de 90 a 150 % (Ctrl + / Ctrl − / Ctrl 0). El panel de la derecha se oculta y
  se muestra con el botón de arriba o con Ctrl B.

## Parabellum dentro del juego

Con la tecla **O** dentro de Minecraft (o el botón «Parabellum» del menú de pausa) se abre el panel de
Parabellum: tus amigos (unirte a su partida, invitarlos a la tuya, escribirles), tus mensajes y grupos, y
tus capturas para mandarlas a un chat. Los avisos de mensajes e invitaciones salen abajo a la derecha; la
primera vez que entras a un mundo, uno te dice cómo abrirlo. Funciona en Fabric, Forge y NeoForge de
la 1.16.5 a la 26.3 (las versiones más usadas); Parabellum pone el mod solo al jugar. Se apaga en
Ajustes › Comportamiento. El código y cómo compilarlo: [mod/README.md](../mod/README.md).

## Hostear partida

Juega con amigos desde tu PC sin abrir puertos ni pagar: un **servidor** hecho a partir de una
instancia (vanilla, Fabric, Quilt, Forge o NeoForge, sin los mods que solo son de cliente) o **tu
mundo abierto en LAN**. Lo creas, lo enciendes e invitas a tus amigos: a ellos les aparece en
Servidores y pulsan **Unirse**. Nadie ve ni copia una IP. El EULA de Minecraft se acepta marcándolo al
crear o encender el servidor.

Cada servidor tiene sus **Conexiones**:

- **P2P** (activada): conexión directa entre las dos PCs, sin túnel. QUIC con perforación de NAT
  (biblioteca [iroh](https://iroh.computer)) y, si las redes no lo permiten, un relevo. Solo pasan tus
  amigos y solo a lo que está encendido.
- **Red local** (activada): para quien está en tu casa o en tu escuela.
- **[playit.gg](https://playit.gg)** (desactivada): una dirección pública fija para quien no usa
  Parabellum. Se vincula una vez (Parabellum abre su página para aceptar y baja su agente oficial).

Al unirse, Parabellum prueba las rutas activas a la vez y usa la más rápida. El mundo abierto en LAN se
comparte solo con tus amigos; por playit.gg, si se elige en Ajustes.

Los invitados se siguen por su cuenta de Google: si alguien se cambia el nombre, la lista blanca, el
permiso de administrador y sus cosas en el mundo pasan al nombre nuevo. Y quien entra con el nombre de
un invitado tiene que estar jugando desde Parabellum con esa cuenta (si no, el servidor lo saca).

En **Ajustes** de cada servidor, todo se cambia sin comandos, como en Aternos: cada opción de
server.properties y las reglas del juego (conservar el inventario, ciclo de día, phantoms, fuego…) en
tarjetas con su nombre claro y, debajo, el nombre real (`max-players=20`, `gamerule keepInventory true`).
Se guarda al cambiarla. Con el servidor encendido, las reglas, la dificultad, el modo de juego y la lista
blanca se aplican al momento; lo demás pide «Reiniciar». Arriba se ve como en la lista de servidores de
Minecraft: icono (cualquier imagen, se recorta a 64×64), nombre y mensaje con colores (§).

## Actualizaciones

Al abrir Parabellum (y cada media hora) se busca una versión nueva en el catálogo de Drive o en la
carpeta `paquetes`. Si la hay, hay que actualizar para seguir usándolo: se descarga, reemplaza el
.exe y se reinicia solo (los juegos abiertos siguen; con un servidor encendido, espera a que se apague).

## Publicar en Drive

Quien mantiene el catálogo conecta Drive una vez en Ajustes › Acerca de (Google pide el permiso de
escribir en Drive; los demás jugadores nunca lo ven). Después, «Subir a Drive» al exportar un modpack y
«Publicar en Drive» con una versión nueva de Parabellum suben los paquetes a la carpeta del catálogo con
la misma estructura que la carpeta local; lo que ya está igual no se vuelve a subir. La cuenta tiene
que poder editar esa carpeta.

**El paquete ligero (desde la 0.35.0).** Al exportar un modpack (y al publicar el paquete oficial de una
comunidad) junto al zip completo sale `<Modpack> (ligero).zip`: el mismo paquete sin los mods,
resource packs y shaders que están en Modrinth. El `.json` los lista en `remote` (ruta, enlace de
`cdn.modrinth.com`, SHA-1, SHA-512 y tamaño) y nombra el ligero en `liteArchive`. Al instalar desde
Drive se bajan a la vez el zip chico y esos archivos del CDN (los que ya están iguales en la instancia se
copian); si Modrinth no responde, se usa el completo. Por qué: Drive revisa por virus los zips de menos
de 100 MB, y lo que lo hace tardar son los jars. Uno con 21 tardó ~30 s en empezar a bajar; el ligero de
«RETORCIDOS | MODPACK», que aún traía los 7 del loader, de 13 a 15 s (medido el 2026-10-20 y 21). Las versiones
anteriores a la 0.35.0 ignoran el ligero y siguen con el completo, así que el completo se sigue subiendo.
Cada instalación ligera queda en `datos/logs/paquetes.log` con su tiempo.

**Desde la 0.35.1, el ligero tampoco trae el loader de Fabric.** Sus bibliotecas (fabric-loader,
sponge-mixin y asm) se listan en `remoteBase`, con rutas dentro de `base/` y enlaces de
`maven.fabricmc.net`, que pasa el filtro de la escuela. Al exportar solo entran las que el maven da con la
misma huella que el archivo de este PC (`<enlace>.sha1`); Fabric anota su espejo `fabricmc.forgecdn.net`,
pero se usa el maven. Un ligero así se nombra en `liteZip`, no en `liteArchive`, porque la 0.35.0 no sabe
bajar `remoteBase`: lo instalaría sin el loader. Así la 0.35.0 sigue con el completo. Si el loader no es de
Fabric (o no hay `remoteBase`), el ligero se sigue nombrando en `liteArchive` como antes.

## CurseForge (desde la 0.36.0)

Explorar conserva las mismas pantallas y deja elegir Modrinth o CurseForge. Los ids `cf:<número>` se
resuelven en `src-tauri/src/curseforge.rs`: categorías, búsqueda, proyecto, versiones y planes con
dependencias. La versión de Minecraft y el loader se comprueban incluso si se elige un archivo concreto.
`manifest.json` permite importar el `.zip` de CurseForge, instalando su loader y copiando `overrides`.
Los archivos quedan con su SHA-1 y procedencia en `.lanzador/archivos.json` y `curseforge.json`.

La clave **no va en el ejecutable ni en el repo**: vive en la variable del puente de Cloud Run.
El despliegue y sus rutas están en [puente-curseforge/README.md](../puente-curseforge/README.md).
El puente está publicado, escala de 0 a 3 instancias y se comprobó con categorías y búsquedas reales.
Los jars y zips se bajan directamente del CDN de CurseForge. Cuando el autor no permite descargas
externas, la interfaz ofrece su página; no se inventa ni se fuerza un enlace de descarga.

Desde la **0.36.2**, `curseforge_manual.rs` abre esa página en una ventana de Tauri sin permisos
de comandos del launcher. El usuario descarga desde el portal; `on_download` recibe el archivo en
`.lanzador/curseforge-entrada`, comprueba el tamaño y SHA-1 esperado y lo coloca en su categoría.
Los pendientes se guardan en `.lanzador/curseforge-pendientes.json`, con su proyecto y archivo exactos.
El asistente permite retomarlos desde Biblioteca o la instancia, y elegir un archivo ya descargado.
También reconoce copias de Descargas con `(1)` en el nombre si coinciden por su huella. No se cambian
archivos personales de Descargas ni se construyen enlaces alternativos de descarga.
La recepción comprueba también el temporal después de la descarga iniciada por el usuario: algunas
versiones de WebView2 no entregan `Finished`. Solo acepta el tamaño y SHA-1 exactos, nunca un archivo
parcial. Al reabrir recupera los temporales completos de una sesión anterior; tras diez minutos sin
completarse permite reintentar. Cerrar la ventana del portal no oculta el launcher en la bandeja.

Recorrido reproducible con el ejecutable final, **siempre en una carpeta de prueba**:

```powershell
python pruebas/curseforge.py --exe src-tauri/target/release/Parabellum.exe --root ../_verificacion-curseforge --base ../Parabellum/base
```

`--base` es opcional: copia archivos compartidos de Minecraft 1.21.1 para ahorrar descargas; nunca
enlaza ni modifica las instancias reales. El informe queda en `resultado.txt` dentro de la carpeta
de prueba. Las actualizaciones automáticas y los favoritos del contenido de CurseForge quedan para
otra versión; los favoritos y actualizaciones de Modrinth siguen como antes.

## Publicar una versión

1. **Drive** (lo hace Juan): Ajustes › Acerca de › «Publicar esta versión» deja `_parabellum` con
   `Parabellum-<versión>.zip`, su `.json` y `LEEME.txt`; «Subir a Drive» la sube y manda la anterior a la
   papelera de Drive. Con línea de comandos: `--publicar-launcher <carpeta> [notas] [pruebas]`. Si la
   versión necesita reglas de Firebase nuevas, se publican antes.
2. **GitHub** (en el mismo momento): una release con el zip **exacto** que se subió a Drive, para que la
   tabla de versiones del README tenga su enlace. Desde la raíz del repo:

   ```
   gh release create v<versión> ../_parabellum/Parabellum-<versión>.zip --target <commit> --title "Parabellum para Windows <versión>" --notes-file <notas.md> --latest
   ```

   Las notas, para quien juega (lo mismo que se anota en `docs/mac-cambios.md`), con el SHA-256 del zip.
3. **README**: en «Descargar», la tarjeta de Windows (el botón, la fecha, el tamaño y «Lo nuevo en la
   <versión>»). El README muestra solo la última versión de cada sistema. El botón de descarga de arriba
   se pone al día solo (toma la release `v*` más nueva).
4. **`docs/mac-cambios.md`** › «Versiones publicadas en Drive»: número, fecha, commit y qué cambió.

**Mac** (`_parabellum-mac`, la publica Diego desde su Mac): el mismo zip de Drive va como release
`mac-v<versión>` con el nombre `Parabellum-Mac-<versión>.zip` (así no se confunde con el de Windows),
título «Parabellum para Mac <versión>» y `--latest=false`, para que «la más nueva» siga siendo la de
Windows (de ella sale solo el botón de Windows). Las de Windows se titulan «Parabellum para Windows
<versión>». En el README, a mano: el botón «Descargar para Mac» de arriba y la tarjeta de Mac en
«Descargar» (botón, fecha, tamaño y «Lo nuevo»).

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
| `--exportar <instancia> <carpeta> [con-mundos] [sin-base]` | Paquete para Drive o USB (modpack, su versión ligera y la base de Minecraft; `sin-base`, sin la base) |
| `--instalar-ligero <id>` | Instala con el paquete ligero de la carpeta `paquetes` y los mods del CDN de Modrinth, como desde Drive (para medir) |
| `--exportar-mrpack <instancia> <archivo.mrpack>` | Modpack en formato de Modrinth |
| `--importar <archivo> [nombre]` | Instala un .mrpack, un .zip de CurseForge o un paquete de Parabellum |
| `--externas` | Instancias de CurseForge, Prism y MultiMC en este PC |
| `--importar-externa <carpeta>` | Copia una de ellas a Parabellum, con su loader |
| `--instalar-modpack <proyecto> [versión]` | Instala un modpack de Modrinth como instancia nueva |
| `--instalar-datapack <instancia> <mundo> <proyecto>` | Instala un data pack de Modrinth en un mundo |
| `--revisar <instancia>` | La revisión de antes de jugar (lo que abriría el doctor) |
| `--diagnosticar <instancia>` | Qué falla: revisión de mods (dependencias, loader, versión del juego, repetidos) y el último informe de fallo, con sus arreglos |
| `--arreglar <instancia>` | Probar y arreglar: abre el juego hasta el menú principal y, si no llega, aplica los arreglos recomendados y repite (hasta 5 veces) |
| `--verificar <instancia> [rapido]` | Comprueba la huella de cada archivo del modpack y repone lo que falte o esté dañado (`rapido`: solo lo que falte) |
| `--crear-servidor <instancia> [nombre] [mundo]` | Servidor dedicado a partir de la instancia (sin aceptar el EULA) |
| `--probar-servidor <servidor> [segundos]` | Lo enciende, espera a que esté listo y lo apaga; si no arranca, explica por qué |
| `--playit` | Estado de la cuenta y del túnel de playit.gg |
| `--probar-receta <instancia>` | Comparte la instancia como por el chat y la vuelve a armar como otra instancia (compara los archivos) |
| `--probar-sonido [archivo.wav]` | Toca el sonido de mensaje nuevo (y lo guarda si se da un archivo) |
| `--ping <dirección>` | Estado de un servidor como en la lista de Multijugador: icono, mensaje, jugadores, versión (con SRV) |
| `--abrir-juego <instancia> [segundos] [con-mod\|sin-mod] [servidor]` | Abre el juego con el mod de Parabellum y lo deja abierto ese tiempo (para probar el panel dentro del juego; ver `mod/README.md`). Con un servidor, entra directo a él |
| `--probar-p2p <puerto> [segundos]` | Abre un túnel P2P en este PC hacia un servidor encendido y le hace ping; con segundos lo deja abierto (puerto en `datos/logs/p2p-puerto.txt`) |
| `--convertir-imagen <entrada> <salida.png>` | Abre una imagen con Windows (HEIC, TIFF…) y la guarda como PNG |
| `--probar-favoritos` | Con la sesión guardada: lee los favoritos de la cuenta, escribe uno de prueba (y lo borra) y comprueba las reglas de Firebase |
| `--probar-tiempo-real [veces]` | Con la sesión de Google guardada: cuánto tarda en llegar por la escucha de Firebase algo recién escrito |
| `--catalogo` | Modpacks y versiones de Drive y de la carpeta paquetes, con su estado |
| `--espejo [filtro]` | Qué tiene el espejo de Minecraft en Drive (paquetes, capas y el espacio del Drive) |
| `--espejo-subir <todo\|1.20.1,1.21.1…> [sin-loaders] [sin-servidores]` | Arma y sube el espejo (quien publica, con «Conectar Drive»); el avance va a `datos/logs/espejo.txt` |
| `--instalar-catalogo <id> [reparar]` | Instala, actualiza o repara un paquete (bajando solo lo que cambió) |
| `--publicar-launcher <carpeta> [notas]` | Prepara _parabellum con esta versión para subirla a Drive |

## Hoja de ruta

Ver [ROADMAP.md](../ROADMAP.md) y, hacia dónde va, [plan-diciembre.md](plan-diciembre.md).

## Créditos

Fuente [Inter](https://github.com/rsms/inter) bajo la SIL Open Font License (`ui/fonts/OFL.txt`).
