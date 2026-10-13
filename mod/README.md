# Parabellum dentro del juego (el mod)

El panel de Parabellum dentro de Minecraft, con lo mismo que la parte social de la ventana (desde la 0.28):

- **Amigos:** agregar por nombre, solicitudes, la lista con filtros (todos, en línea, jugando) y búsqueda,
  y en cada amigo Unirse, Invitar, Mensaje y ⋯ (favorito, eliminar); a la derecha, los grupos.
- **Mensajes:** conversaciones (favoritas primero, el punto de en línea, fotos de grupo), responder,
  reaccionar (las seis de Parabellum, dibujadas en `Emojis`), editar y borrar, invitaciones e imágenes;
  crear grupos y, si eres el dueño, agregar, sacar, pasar el grupo y cambiarle el nombre.
- **Capturas:** mandarlas a una conversación, con «Abrir carpeta» y cómo sacarlas (F2, o Fn + F2 en
  muchas laptops).

Los avisos salen abajo a la derecha. Los iconos son los mismos trazos de `ui/index.html` (`Iconos`) y lo
que la fuente Inter no tiene (emojis) se escribe con una del sistema (`Lienzo.respaldo`). Se
abre con la tecla **O** (se cambia en Controles › Multijugador) o con el botón «Parabellum» del menú de
pausa. La primera vez que se entra a un mundo sale el aviso «Pulsa O para abrirlo», hasta que se abra
una vez (la marca es `datos/panel-visto.txt` de Parabellum).

El panel entra subiendo y aclarándose (0.18 s) y los avisos entran y salen deslizándose: solo se vuelven
a dibujar mientras algo se mueve.

No hay que instalarlo a mano: al jugar, Parabellum pone en la carpeta `mods` de la instancia el .jar que
toca según el loader y la versión, y lo quita al cerrar el juego (ver `src-tauri/src/juego.rs`). Así no
se cuela en los servidores, en los modpacks exportados ni en las instancias compartidas.

## Cómo está hecho

```
nucleo/                 Java 8, sin nada de Minecraft (lo mismo en todas las versiones):
                        Puente (conexión con Parabellum), Panel (dibujado con Java2D), Avisos,
                        Capturas, Campo (caja de texto), Lienzo (colores, fuente Inter, cabezas)
comun/recursos/         icono y nombre de la tecla (es/en)
comun/mc-<versión>/     lo de una versión de Minecraft que comparten los loaders: la pantalla del
                        panel, subir las imágenes como texturas, la tecla, entrar a un servidor
versiones/<loader>-<v>/ cada .jar: su build.gradle y lo propio del loader (eventos de Forge/NeoForge,
                        o mixins en Fabric para no depender de Fabric API)
compilar.py             compila todo, copia los .jar a src-tauri/juego/mods/ y genera lista.rs
```

El mod solo dibuja; la cuenta, el chat y el P2P los hace Parabellum. Se hablan por `127.0.0.1` con una
línea JSON por mensaje (`src-tauri/src/puente.rs`); Parabellum le pasa al juego el puerto y una clave con
`-Dparabellum.puente` y `-Dparabellum.clave`, y la carpeta de las fuentes con `-Dparabellum.fuentes`.

El panel se dibuja entero en una imagen (Java2D) y cada versión solo la sube como textura y le pasa el
ratón y el teclado. Por eso cada versión nueva de Minecraft es poco código.

## Versiones

| .jar | Minecraft |
|---|---|
| Fabric | 1.16.5, 1.18.2, 1.19–1.19.2, 1.20–1.20.1, 1.21–1.21.1, 1.21.2–1.21.4, 1.21.5, 1.21.6–1.21.8, 1.21.9–1.21.10, 1.21.11, 26.1.x, 26.2, 26.3 (también Quilt) |
| Forge | 1.16.5, 1.18.2, 1.19–1.19.2, 1.20–1.20.1 |
| NeoForge | 1.20.1 (el .jar de Forge), 1.21–1.21.1 |

Elegidas por popularidad en Modrinth (mods y modpacks por versión y loader). Los rangos de Fabric se
comprobaron compilando el mismo código contra cada versión del rango (`-Pmc=<versión>`).

## Compilar

Hacen falta los JDK de Temurin en `~/.jdks` (no van dentro de Parabellum): **25** (Fabric Loom y
ModDevGradle), **17** (Forge 1.18–1.20.1), **8** (Forge 1.16.5) y **21**.

```bash
python compilar.py                # todas
python compilar.py fabric-26.3    # solo esa
```

Luego se compila Parabellum (los .jar van dentro del .exe).

- Fabric: Fabric Loom (desde la 26.1 sin mappings: Minecraft ya no viene ofuscado).
- NeoForge: ModDevGradle. Forge 1.17–1.20.1: ModDevGradle Legacy (reofusca a SRG).
- Forge 1.16.5: ForgeGradle 5 con su propio Gradle 7 (`versiones/forge-1.16.5/gradlew`). Ojo: ahí las
  clases tienen nombres de MCP (`comun/mc-1.16.5-forge`), no los de Mojang.

## Agregar una versión

1. Copia la carpeta de la versión más parecida en `comun/` y en `versiones/`.
2. Cambia la versión en `build.gradle`, `settings.gradle` y el rango en `fabric.mod.json` o `mods.toml`.
3. Compila y corrige lo que cambió (con `javap` sobre el .jar de Minecraft que baja Gradle se ven las
   firmas nuevas).
4. Agrégala a `VERSIONES` en `compilar.py`.

## Probar

Con el JSON de una instancia (`extraJvmArgs`) o en la línea de comandos de Java:

- `-Dparabellum.prueba.abrir=1`: abre el panel solo en el menú principal (y deja
  `parabellum-prueba.txt` en la carpeta del juego).
- `-Dparabellum.prueba=true`: el panel se dibuja como conectado, con un amigo y un chat de ejemplo.
- `-Dparabellum.prueba.pestana=1`: abre la primera conversación; `=2`: la segunda (un grupo con
  respuestas y reacciones de ejemplo); `=grupo`: el diálogo «Nuevo grupo»; `=capturas`: Capturas.

Sin Minecraft: `python mod/vista/vista.py` compila el núcleo con `vista/Vista.java` (una `Plataforma`
de mentira), dibuja el panel en modo de prueba y guarda un PNG de cada estado en `vista/png` (amigos,
menú, conversación, grupo con citas y reacciones, herramientas, respuesta, diálogos y capturas): sirve
para revisar el diseño en segundos. Las fuentes las toma de `PARABELLUM_FUENTES` o de
`$PARABELLUM_ROOT/datos/fuentes`.
- `-Dparabellum.prueba.conectar=<dirección>`: entra a ese servidor como lo haría «Unirse».
- `-Dparabellum.prueba.lento=<N>`: el panel tarda N veces más en entrar (para verlo en una captura).

`Parabellum.exe --abrir-juego "<instancia>" [segundos] [con-mod|sin-mod] [servidor]` abre el juego con el
mod y lo deja abierto ese tiempo; con un servidor entra directo a él (para probar la pista, los avisos y
el botón del menú de pausa dentro de un mundo).
