# Parabellum, cliente oficial del servidor (plan hacia diciembre de 2026)

Estado: **aprobado con las respuestas de Juan del 2026-10-18** (sección 11). Sale de lo que pidió Juan
y de una auditoría de otro agente. Lo que todavía depende de una respuesta está marcado.

El servidor es **Paper**, alojado por Juan en una nube aparte: Parabellum es solo el cliente para
entrar; no lo aloja ni lo administra.
Cada etapa se entrega en incrementos pequeños, probados en carpetas de prueba (nunca en los mundos ni
servidores reales de nadie), con sus textos en español e inglés y su entrada en `mac-cambios.md`.

Meta de diciembre: alguien instala Parabellum, inicia sesión, se une a la comunidad, prepara la
instancia oficial y entra al servidor con la menor intervención posible. Sin dejar de servir para sus
partidas personales: instancias, mundos y seguir jugando desde otra computadora.

## 1. Diagnóstico (revisado en el código el 2026-10-18)

La auditoría fue de lectura, sin ejecutar nada. Esto es lo que se comprobó encima de ella.

| Área | Estado | Cómo se comprobó |
|---|---|---|
| Versiones | `main` está en **0.30.0** (`53ba562`, `270476a`). En Drive: Windows 0.28.0 y Mac 0.28.0. La 0.29.0 nunca salió sola: va dentro de la 0.30.0. | `git log`; catálogo leído con `--catalogo` el 2026-10-17 |
| «14 archivos sin registrar» | Ya no: eran el espejo, el modo escuela y el arranque, y están en `53ba562`. El árbol está limpio. | `git status` |
| Espejo de Minecraft en Drive y modo escuela | **Hecho y probado** con un proxy que solo deja pasar Google: instancia, loader, servidor y modpack desde una carpeta vacía. Falta probarlo en la red real de la escuela. | `ROADMAP.md` (0.30.0), pruebas del 2026-10-17 y 18 |
| Integridad del espejo | **Limitación comprobada.** `espejo::Remote::install`, `catalog::base_delta` y `download::FileDownload::already_present` dan por bueno un archivo local si tiene el mismo tamaño. Un archivo dañado del mismo tamaño no se repara. Lo que se baja sí se comprueba (CRC32 del zip o SHA-1 de Mojang). | lectura de esas funciones |
| Actualizar el launcher | **Limitación comprobada.** El exe anterior queda como `.old` y el nuevo lo borra al arrancar (`lanzador::cleanup`), antes de saber si funciona. No hay confirmación de arranque ni vuelta atrás automática. | `plataforma/windows/lanzador.rs` |
| Identidad de la instancia | **Comprobado:** la instancia se identifica por el nombre de su carpeta (`Instance::id`). No hay un id de instalación ni una huella del contenido. | `instances.rs` |
| Base para saber si una instancia cambió | **Ya existe en parte:** al instalar un paquete se guardan su manifiesto (`.lanzador/pack.json`) y la lista de archivos con su huella (`.lanzador/archivos.json`, `integridad.rs`). No hay una huella única ni comparación «exacta / modificada». | `packs.rs`, `integridad.rs` |
| Recetas para compartir | Cubren modpack de Modrinth, paquete del catálogo o mods de Modrinth. Lo que no es de Modrinth solo se **cuenta** (`left_out`), no se nombra, y la receta no lleva huellas: no se puede saber si lo reconstruido quedó igual. | `compartir.rs` |
| Chat | Los mensajes aceptan solo campos conocidos (`$other: false`); el texto tiene un máximo de 2000. Para meter a alguien en un chat hay que ser su amigo. Invitaciones y recetas viajan como texto con prefijo. | `firebase-reglas.json` |
| Reportar problema | Arma un resumen de hasta 1500 letras (versión, sistema, instancia, error, doctor) y lo manda por chat a la cuenta de soporte. **Requiere validación**: no filtra datos sensibles de forma explícita y depende de la amistad con soporte. | `reporte.rs` |
| Google | El inicio de sesión ya pide `drive.appdata` (ahí se guarda el perfil). El permiso de quien publica el catálogo (Drive completo) va aparte (`drive-publicar.bin`). | `google.rs`, `drive.rs` |
| Ajustes | **No hay un servicio común.** `guardar_ajustes`, `ajustes_descargas`, `modo_escuela` y otros guardan por su cuenta y cada uno hace sus efectos (descargas, idioma, zoom, bandeja). | `main.rs` |
| Respaldos | Solo temporales: al actualizar un modpack, la carpeta vieja va a un respaldo que se borra al terminar. Lo reemplazado en servidores va a la papelera. No hay copias conservadas con retención ni restauración. | `packs::replace_keeping_player_data`, `servidor.rs` |
| Firebase | Cada escucha es una conexión abierta: 7 fijas, más una por amigo y una por conversación, más 2 del chat abierto. Para Juan (7 amigos, 8 chats) son unas 22. No se ha medido el tráfico. | `amigos.rs`, `chat.rs` |
| Pruebas | 97 pruebas unitarias. Solo hay automatización para Mac (`mac.yml`). Los recorridos completos se hacen con scripts que viven fuera del repositorio. | `.github/workflows`, `cargo test` |
| Tamaño de archivos | `ui/app.js` 7190 líneas, `main.rs` 3887 (118 comandos), `ui/hostear.js` 1962. | `wc -l` |
| Mac | PR #1 a #15 mezclados. A Mac le falta probar la 0.30.0 y publicarla cuando salga la de Windows. | `docs/mac-cambios.md` |

## 2. Principios de la arquitectura

1. **Una sola base de identidad.** Comunidades, mundos y tarjetas de ajustes preguntan lo mismo: ¿esta
   instalación es la misma que aquella? Lo responde un módulo (`identidad.rs`), no cada función por su
   cuenta.
2. **Una sola forma de cambiar un ajuste.** La ventana, una tarjeta del chat y una comunidad cambian
   ajustes a través del mismo servicio, que valida y aplica los efectos.
3. **Pendiente no es confirmado.** Cada estado se muestra como es: «pendiente», «subiendo»,
   «verificando», «guardado en Drive». Lo último solo aparece después de comprobarlo.
4. **Operaciones idempotentes.** Todo lo que viaja (tarjetas, subidas, restauraciones) lleva un id y
   valores explícitos. Repetir una operación no cambia el resultado.
5. **Nada destructivo sin copia recuperable.** Antes de reemplazar un mundo, actualizar un paquete o
   cambiar el software de un servidor, queda un respaldo local con retención.
6. **Los permisos los decide Firebase.** Las reglas validan roles y pertenencia con la identidad
   autenticada. La interfaz solo los muestra; un indicador local nunca da permisos.
7. **Crecer sin engordar `app.js` ni `main.rs`.** Cada área nueva va en su módulo de Rust y su archivo de
   interfaz; en `main.rs` solo queda el registro de comandos.

## 3. Módulos nuevos

| Rust (`src-tauri/src/`) | Interfaz (`ui/`) | Para qué |
|---|---|---|
| `identidad.rs` | — | id de instalación, manifiesto de contenido, huella, diferencias |
| `ajustes.rs` (servicio) y `ajustes_compartibles.rs` (registro) | `ajustes.js` | leer y aplicar ajustes con sus efectos; catálogo de lo que se puede compartir |
| `respaldos.rs` | `respaldos.js` | copias locales recuperables, retención y restauración |
| `drive_personal.rs` | — | el Drive del jugador (separado del de quien publica) |
| `mundos.rs` | `mundos.js` | identidad de mundos, copias, cola de subida, revisiones y conflictos |
| `comunidad.rs` | `comunidad.js` | comunidad, roles, servidores, paquete oficial, «Preparar y jugar» |
| `soporte.rs` y `tarjetas.rs` | `soporte.js`, `tarjetas.js` | conversaciones de soporte, diagnóstico revisable, tarjetas de ajustes |
| `ajustes_minecraft.rs` | — | adaptadores por versión de Minecraft (options.txt) |
| `canal.rs` | — | canal estable o de pruebas; confirmación de arranque tras actualizar |

`main.rs` y `app.js` se adelgazan al mismo tiempo: al tocar el chat, pasa a `ui/chat.js`; al tocar
ajustes, sus comandos pasan a `ajustes.rs`.

## 4. Identidad y compatibilidad de instancias

### 4.1 Cuatro conceptos distintos

| Concepto | Qué es | Dónde vive |
|---|---|---|
| **Instalación** | Una carpeta concreta en un PC. `installationId` es un UUID que nace la primera vez y no cambia al renombrar. El nombre de la carpeta sigue siendo la clave local. | `.lanzador/instalacion.json` |
| **Familia de paquete** | De dónde viene: `drive:<id del paquete>`, `modrinth:<proyecto>`, `comunidad:<comunidad>/<paquete>` o `local:<installationId>` si es hecha a mano. | en el manifiesto |
| **Versión del paquete** | La versión publicada de esa familia («1.4.0», la versión de Modrinth). | en el manifiesto |
| **Huella de contenido** | `contentHash`: SHA-256 del contenido que importa para la compatibilidad, calculada aquí y en cualquier PC igual. | se calcula; se guarda en caché |

Dos «Cobbleverse» son la misma instancia solo si tienen la misma familia, la misma versión **y** la
misma huella. Si solo coinciden la familia y la versión pero no la huella, el jugador cambió algo.

### 4.2 El manifiesto de contenido

```json
{
  "format": 1,
  "installationId": "7f3c…",
  "package": { "family": "comunidad:oficial/principal", "version": "1.2.0", "contentHash": "sha256:…" },
  "minecraft": "1.21.1",
  "loader": { "name": "NeoForge", "version": "21.1.228" },
  "mods": [
    { "sha1": "…", "sha512": "…", "id": "create", "version": "6.0.4", "source": "modrinth:LNytGWDc/abc12345" },
    { "sha1": "…", "id": "mod-propio", "source": "drive:oficial-principal" },
    { "sha1": "…", "id": "algo-curseforge", "source": "desconocido" }
  ],
  "structural": [ { "path": "config/create-common.toml", "sha256": "…" } ],
  "rebuild": { "complete": false, "missing": ["mods/algo-curseforge.jar"] }
}
```

### 4.3 Qué entra en la huella y qué no

| Entra (cambiarlo es otra instancia) | No entra (personal o propio del sistema) |
|---|---|
| Versión de Minecraft | Nombre y carpeta de la instancia |
| Loader y su versión | Rutas absolutas |
| Cada mod activo, por su **contenido** (SHA-1), no por su nombre de archivo | Mods desactivados (`.disabled`) |
| Archivos de configuración **estructurales** que declare el paquete (los que deben coincidir con el servidor) | El mod de Parabellum (`juego.jar`), que se pone en cada partida y cambia por sistema |
| | Nativos, Java y bibliotecas de la carpeta base (dependen de Windows o Mac) |
| | `options.txt`, sonido, controles, distancia de renderizado, memoria asignada, argumentos de Java |
| | Mundos, capturas, registros, `servers.dat` |
| | Resource packs y shaders (no impiden entrar a un servidor; se informan aparte) |

La huella es el SHA-256 de un JSON canónico (claves ordenadas, mods ordenados por SHA-1). Así da lo
mismo en Windows y en Mac, y renombrar un archivo no la cambia.

### 4.4 Diferencias y estado

`identidad::compare(instalación, referencia)` devuelve:
- **exacta**: misma huella;
- **modificada**: con la lista exacta de lo que cambió (mods agregados, quitados o de otra versión;
  loader o Minecraft distintos; archivo estructural distinto);
- **desconocida**: no hay referencia con qué comparar (instancia hecha a mano sin paquete).

### 4.5 Reconstruir en otro PC

`rebuild.complete` dice si se puede rehacer igual en otra computadora. Lo que no viene de Modrinth ni
de un paquete de Drive se nombra en `missing`. Las recetas del chat pasan a una versión 2: llevan la
huella y la lista de lo que falta. La versión 1 se sigue leyendo.

### 4.6 Migración

Nada se reescribe de golpe. `instalacion.json` se crea la primera vez que hace falta. La familia y la
versión salen de `.lanzador/pack.json` (paquetes de Drive) o de `modpack_project` (Modrinth). La huella
se calcula al pedirla y se guarda junto con el tamaño y la fecha de cada mod: solo se recalcula lo que
cambió.

## 5. Servicio de ajustes y registro de ajustes compartibles

- `ajustes::read(id, alcance)` y `ajustes::apply(operación)`. Una operación es
  `{ id, valor, esperado }`: se aplica solo si el valor actual sigue siendo `esperado`; si no, devuelve
  el actual para mostrarlo. Aplicar hace lo mismo que la ventana de Ajustes: guarda, configura las
  descargas, cambia el idioma, emite `ajustes`, etc. `guardar_ajustes` pasa a usarlo.
- **Registro** (`ajustes_compartibles.rs`): cada ajuste compartible tiene `id`, tipo (sí/no, número,
  opción), rango o valores permitidos, alcance (Parabellum o instancia), desde qué versión existe, en
  qué sistemas, texto en los dos idiomas y la función que lo aplica. Para la primera entrega:
  - Parabellum: `launcher.message_sound`, `launcher.notifications`, `launcher.ram_gb` (1–64),
    `launcher.download_limit_mb` (0, 1, 2, 5, 10, 20), `launcher.download_parallel` (1–3),
    `launcher.game_panel`, `launcher.school_mode`.
  - Minecraft (etapa 5): `mc.render_distance` (2–32), `mc.max_fps`, `mc.vsync`, `mc.sound.master`
    (0–100 %), solo en las versiones donde el adaptador sabe escribirlos.
- La validación está en el motor; la interfaz solo ayuda.

## 6. Comunidades

### 6.1 Datos en Firebase (primera versión)

```
communities/{cid}             nombre, descripción, imagen, dueño
communityMembers/{cid}/{uid}  rol y permisos: { role: "owner"|"admin"|"member", caps: { support, publish, servers } }
userCommunities/{uid}/{cid}   true (para listar las tuyas con una sola escucha)
communityInvites/{cid}/{code} quién la creó, caducidad, usos
communityServers/{cid}/{sid}  nombre, dirección, paquete requerido
communityPosts/{cid}/{pid}    anuncios (título, texto, autor, fecha)
communityRules/{cid}          texto de las reglas y su versión
communityPackages/{cid}/{pk}  familia, versión vigente por canal, huella, id del paquete en el catálogo
```

Permisos separados, validados en las reglas con `auth.uid`:
- **administrar la comunidad** (anuncios, reglas, miembros): `owner` o `admin`;
- **publicar paquetes**: `caps.publish`;
- **editar los servidores de la comunidad** (nombre, dirección, paquete requerido): `caps.servers`. El
  servidor de Minecraft en sí se administra fuera de Parabellum;
- **atender soporte**: `caps.support`.

Solo el dueño o un administrador dan permisos; nadie se los da a sí mismo; el dueño solo cambia por
transferencia. Al inicio el dueño es Juan; desde la página de la comunidad nombra a otros como staff y
elige qué puede hacer cada uno.

**Unirse** (decisión 3): con el código de la comunidad, en la sección Comunidades, o con una invitación
que llega por mensaje directo y tiene el botón «Unirse». Las dos usan `communityInvites`; no hay
comunidades abiertas en la primera versión.

### 6.2 «Preparar y jugar»

1. Lee el paquete vigente de la comunidad (`communityPackages`).
2. Busca instalaciones locales de esa familia. Si hay varias, se elige una; nunca por el nombre.
3. **Si falta**, la instala desde el catálogo (o el espejo en modo escuela). **Si está desactualizada**,
   la actualiza por partes, como hoy. **Si está modificada**, dice exactamente qué cambió y ofrece
   reparar solo lo administrado.
4. Comprueba Java y la base de Minecraft.
5. Abre el juego y entra directo al servidor (ya existe `plan.join_server`).

**Archivos administrados y personales.** El paquete declara qué administra: mods, configuración
estructural, resource packs obligatorios. El resto es personal: `options.txt`, controles, mundos,
capturas, `servers.dat`, mapas de minimapas. Hoy esa lista es fija en el código
(`PRESERVED_ON_UPDATE`); pasa a ser parte del paquete, con la fija como mínimo.

### 6.3 El servidor

Paper, en una nube de Juan. Parabellum no exige ser el launcher para entrar (decisión 2): es el cliente
recomendado. Con Paper el paquete oficial es del lado del cliente (vanilla o con mods que no hace falta
tener en el servidor, como los de rendimiento); la versión de Minecraft todavía está por confirmar. El
estado y los jugadores salen del ping del servidor (`ping.rs`), sin acceso a su consola.

## 7. Soporte por chat y tarjetas de ajustes

### 7.1 Conversaciones de soporte

No van dentro de `chats`, para no cambiar las reglas de amistad. Van en su propio espacio:

```
supportTickets/{cid}/{tid}   quién pidió, tema, estado (abierto, atendido, cerrado), a quién se asignó
supportMessages/{cid}/{tid}  mensajes (los mismos campos que el chat, más tarjetas)
```

- Cualquiera de la comunidad abre una desde la comunidad o la instancia.
- **Todo el equipo de soporte** (`caps.support`) ve la cola y lee los mensajes de cada solicitud
  (decisión 7). El jugador lo ve antes de escribir: «Lo leerá el equipo de soporte de <comunidad>».
- Un miembro del staff puede tomarla para que se sepa quién la atiende.
- Ser staff no da acceso a los mensajes privados del chat normal.

**El diagnóstico** reutiliza `reporte.rs` y se ve antes de mandarlo. Se filtran:
- tokens, claves y correos;
- la carpeta del usuario, que pasa a `%USERPROFILE%` o `~`;
- todo lo que no sea de la instancia afectada.

### 7.2 Tarjetas: un mensaje estructurado y versionado

```json
{ "from": "<uid>", "at": 1760000000000,
  "text": "🛠 Ajuste propuesto: Sonido de mensajes → Activado (actualiza Parabellum para aplicarlo)",
  "card": { "v": 1, "op": "c3b1…", "kind": "setting", "setting": "launcher.message_sound",
            "value": true, "scope": "launcher", "official": "<cid>",
            "target": { "family": "…", "version": "…", "contentHash": "…" } } }
```

- **`text` siempre va.** Los clientes viejos y el panel del juego muestran el resumen con la acción
  apagada.
- **Las reglas validan** el formato:
  - `v` conocida, `setting` de la lista y `value` del tipo correcto;
  - remitente igual a `auth.uid` y miembro de esa conversación;
  - si es oficial, rol de soporte vigente en esa comunidad;
  - **un mensaje con tarjeta no se edita.**
- **No hay campos libres:** ni comandos, ni argumentos de Java, ni rutas.

### 7.3 Aplicar una tarjeta

1. La tarjeta muestra:
   - quién la mandó y qué ajuste propone;
   - si es de Parabellum o de una instancia (y cuál);
   - el valor actual en **este** PC y el recomendado;
   - qué efecto tendrá y cuándo se aplica.
2. **Recibir o leer la tarjeta no cambia nada.** El jugador pulsa el control.
3. Al pulsarlo, el motor:
   - vuelve a validar el remitente, el rol vigente, la compatibilidad y el valor actual;
   - aplica `{ id, valor, esperado }` con el servicio de ajustes.
4. Queda un registro local en `datos/tarjetas.json`: `op → { previo, aplicado, fecha, resultado }`.
   - Repetir la misma `op` no hace nada nuevo, y el valor es explícito (nunca «invertir»), así que un
     doble clic no lo deshace.
5. Estados: aplicada, pendiente (Minecraft abierto), falló o ya no es compatible. Es lo de **este** PC;
   no dice nada de los demás.
6. **Deshacer** vuelve al valor anterior solo si nadie lo cambió después; si lo cambiaron, avisa.
7. Opcional: un acuse («aplicada en este PC») para que el staff lo vea.

## 8. Mundos en el Drive personal de cada jugador

### 8.1 Carpeta visible o carpeta privada de la app

**Recomendación: una carpeta visible «Parabellum/Mundos» con el permiso `drive.file`**:
- solo deja tocar lo que creó Parabellum;
- es un permiso no sensible, como el `drive.appdata` que ya se pide, así que no pide la verificación de Google;
- el jugador ve sus copias en Drive, las puede bajar sin Parabellum y entiende qué ocupa su espacio.

La carpeta privada (`appDataFolder`) se descarta porque es invisible y no se puede compartir. Si
Parabellum falla, el jugador no puede sacar sus mundos, y el espacio que ocupa no se explica en ningún
lado.

Riesgo de la carpeta visible: el jugador puede mover, renombrar o borrar archivos. Por eso las copias
se buscan por sus **propiedades de la app** (`appProperties`), no por su ruta, y lo que esté en la
papelera se ignora.

Va con su propio permiso, pedido la primera vez que alguien activa un respaldo. No es el del inicio de
sesión ni el de quien publica.

### 8.2 Identidad del mundo

Al activar el respaldo de un mundo se crea `parabellum-mundo.json` dentro de su carpeta, con
`worldId`, el dueño y la instalación de origen. Si aparecen dos carpetas con el mismo `worldId` (una
copia), la segunda recibe uno nuevo y se avisa.

### 8.3 Revisiones sin sobrescribir nunca

- Cada copia es un **archivo inmutable** en Drive, `<worldId>-<rev>.zip`, con estas propiedades:
  `worldId`, `rev`, `parent` (la revisión de la que deriva), `device`, `account`, `sha256`, `size`,
  `playerUuid`, la huella de la instancia y `state`.
- Primero se sube con `state=pendiente`. Después se comprueba la huella MD5 que da Drive contra la
  local. Solo entonces pasa a `state=confirmada`.
- **Nunca hay un archivo «último» compartido que dos PC sobrescriban.** La última copia se calcula: es
  la revisión confirmada de la que no deriva ninguna otra.
- Si hay **dos o más** así, es un **conflicto**: las dos ramas se conservan y el jugador elige con
  cuál seguir. La otra queda para restaurarla o convertirla en otro mundo.

### 8.4 La primera versión

- El jugador elige qué mundos respaldar, en la instancia o en el mundo.
- Al cerrar Minecraft (o apagar el servidor propio), se comprueba que el proceso terminó y que
  `session.lock` está libre. Entonces se hace la copia: un zip completo con todas las dimensiones,
  `playerdata` y `data`.
- La subida es reanudable. La sesión de subida se guarda en disco, así que sobrevive a cerrar
  Parabellum o apagar la computadora.
- **En otro PC** se consulta la última revisión confirmada. Se prepara la instancia compatible (con la
  identidad de la sección 4) o se explica qué falta. Antes de reemplazar el mundo local, queda en
  `respaldos.rs`.
- **Identidad del jugador.** Cada copia guarda el UUID del jugador. Si en el otro PC entras con otro
  nombre (otro UUID sin premium), se avisa antes de abrir: si no, el inventario parecería perdido.
- **Límites:** se conservan las 5 últimas revisiones confirmadas y siempre las cabezas de las ramas en
  conflicto. Hay un tope configurable por cuenta, de 5 GB por defecto (decisión 5).
- **Exportar e importar:** el zip de una revisión se puede guardar o cargar a mano.
- Los mundos de los servidores propios entran igual, con el servidor apagado. El mundo del servidor
  comunitario se respalda desde el servidor, no desde aquí.

### 8.5 Casos difíciles

| Caso | Qué pasa |
|---|---|
| Se va el internet en plena subida | La copia queda **pendiente** y se reanuda donde quedó; la última confirmada sigue disponible |
| Se cierra Parabellum o se apaga el PC | Al abrir, la cola retoma la sesión guardada (Drive la guarda una semana) o empieza otra |
| Drive sin espacio | Pendiente, con el aviso «Drive lleno» y el espacio que falta; nada se borra solo |
| La sesión caduca o se quita el permiso | Pendiente, con «Vuelve a conectar Drive» |
| Cambia de cuenta | Las copias son de la cuenta que las hizo; lo pendiente de otra cuenta se pausa y se explica |
| Juega sin conexión | La copia se hace igual y espera en la cola |
| El mismo mundo en dos PC | Cada uno sube su rama desde la revisión que tenía; quedan dos cabezas → conflicto visible, ninguna se pierde |
| Una subida termina mientras otro PC juega sobre una revisión anterior | Ese PC subirá con un `parent` viejo → otra rama → conflicto, sin perder nada |

## 9. Las siete mejoras de la auditoría

| Mejora | Tipo | Qué se hace |
|---|---|---|
| 1. Respaldos recuperables | **Amplía** (ya hay respaldos temporales) | `respaldos.rs`: copia antes de actualizar un modpack, restaurar un mundo o cambiar el software de un servidor; con lista, restauración, retención (las 3 últimas por elemento) y tope de espacio |
| 2. Integridad del espejo y modo escuela | **Corrige una limitación comprobada** (archivo dañado del mismo tamaño) | «Reparar» y «Verificar» comprueban SHA-1 (el espejo ya lo tiene en sus listas; Mojang en su JSON). En lo normal, tamaño más una marca «verificado» (fecha y tamaño de cuando se comprobó): solo se recalcula lo que cambió. Pruebas: un archivo cambiado del mismo tamaño, cancelar a la mitad y reanudar tras reiniciar |
| 3. Recuperación de actualizaciones del launcher | **Corrige una limitación comprobada** | El `.old` se conserva hasta que la versión nueva confirma su arranque: la ventana cargó y lleva 30 s viva. El exe viejo espera esa confirmación (como mucho 90 s) antes de cerrarse. Si no llega, vuelve a ponerse y anota la versión fallida para no reintentarla hasta que salga otra. Un fallo de internet no cuenta: la confirmación no depende de la red |
| 4. Pruebas de recorridos completos | **Amplía** | Flujo de Windows en GitHub (`cargo test` y compilar). Los scripts de recorrido pasan al repo (`pruebas/`): instalación limpia, descarga cortada, actualizar conservando datos, abrir el cliente. Más los casos nuevos de conflictos, restauración y tarjetas. Las pruebas entre dos PC quedan escritas con lo comprobado y lo pendiente |
| 5. Medir y reducir Firebase | **Amplía** (limitación conocida) | **Medido** (2026-10-18, `datos/logs/firebase.csv`, cuenta de Juan con 7 amigos y 8 conversaciones): 24 escuchas = 24 conexiones abiertas; sincroniza en 3.1 s y baja ~13 KB; después ~1.3 KB y ~8 peticiones por minuto. El tráfico es mínimo: lo que cuesta es abrir 24 conexiones (la espera al abrir). Consolidar después: un resumen de conversaciones en `userChats` (una escucha en vez de una por chat) y la presencia de amigos en un buzón propio. Comunidades y soporte se diseñan con una escucha por comunidad |
| 6. Dividir los archivos grandes | **Amplía** | Gradual, al tocar cada área (sección 3) |
| 7. Canales estable y de pruebas | **Nuevo** | Carpetas `_parabellum-pruebas` y `_parabellum-mac-pruebas`; quien está en `testers/{uid}` ve el selector de canal. Volver a estable no borra datos. Lista de requisitos para publicar en la sección 10.1 |

## 10. Etapas y orden

El orden sigue las dependencias. Lo que se dice que «cabe» es lo razonable, no una promesa.

| Etapa | Contenido | Depende de |
|---|---|---|
| **E0. Consolidar** (ya) | Probar la 0.30.0 en la escuela y publicarla; logo nuevo (hecho); integridad del espejo (mejora 2); recuperación de actualizaciones (mejora 3); flujo de Windows en GitHub y scripts de prueba en el repo (mejora 4); medir Firebase (mejora 5); canal de pruebas (mejora 7) | — |
| **E1. Base compartida** | `identidad.rs` (manifiesto, huella, diferencias, recetas v2); servicio de ajustes y su registro; `respaldos.rs` (mejora 1) | E0 |
| **E2. Comunidad oficial** | Datos y reglas de comunidades; página de la comunidad (unirse con código o invitación, anuncios, reglas, servidores con su estado, nombrar staff); paquete oficial y «Preparar y jugar» | E1; la versión de Minecraft para el paquete |
| **E3. Soporte y tarjetas de Parabellum** | Tickets, diagnóstico revisable, tarjetas de los 7 ajustes de Parabellum, reglas y sus pruebas | E1, E2 (roles) |
| **E4. Mundos en Drive** | Copias completas, cola reanudable, revisiones, conflictos, restaurar con su instancia | E1 (identidad, respaldos) |
| **E5. Tarjetas de Minecraft** | Adaptadores por versión, compatibilidad estricta, pendientes hasta cerrar el juego | E1, E3 |
| **E6. Después** | Subir solo lo que cambió de un mundo, configuraciones de mods por formato, aplicar en vivo con el mod, más comunidades y eventos, consolidar Firebase | lo anterior |

**Antes de diciembre**: E0, E1, E2 y E3 caben si las decisiones 1–3 llegan pronto. E4 es posible en el
canal de pruebas. E5 y E6 quedan para después. Si algo se atrasa, se recorta E3 a soporte sin tarjetas
antes de tocar E2: entrar al servidor es lo primero.

### 10.1 Lista para publicar (vale para cada versión)

- [ ] `cargo test` pasa en Windows y en Mac (los dos flujos de GitHub).
- [ ] Recorridos de `pruebas/` pasan en una carpeta de prueba.
- [ ] Reglas de Firebase probadas con el emulador (`docs/probar-reglas.py`) **y publicadas antes** que la
      versión.
- [ ] Formatos compartidos compatibles: manifiesto, tarjetas (`v`), índice del espejo (`format`), mundos.
      Un cliente anterior entiende lo nuevo o lo ignora sin romperse.
- [ ] Probada en el canal de pruebas.
- [ ] `mac-cambios.md` al día y la versión anotada en «Versiones publicadas en Drive».

## 11. Decisiones

Contestadas por Juan el 2026-10-18:

1. **Servidor:** Paper, alojado por Juan en una nube aparte. Parabellum solo es el cliente. *Falta:* la
   versión de Minecraft y si el paquete oficial lleva mods (del lado del cliente).
2. **Exclusividad:** no se exige Parabellum para entrar. Queda fuera del plan.
3. **Unirse:** con código en la sección Comunidades, o con una invitación por mensaje directo con el
   botón «Unirse».
4. **Mundos:** carpeta visible «Parabellum/Mundos» en el Drive de cada jugador (`drive.file`).
5. **Límites:** 5 revisiones por mundo y 5 GB por cuenta.
6. **Staff:** Juan es el dueño y administrador; desde Parabellum nombra a otros y elige sus permisos
   (soporte, publicar paquetes, editar servidores).
7. **Soporte:** todo el equipo de soporte lee los mensajes de las solicitudes.
8. **Canal de pruebas:** *falta* quiénes lo prueban.
9. **Firebase:** sin tope de gasto. Igual se mide y se reduce (mejora 5), por velocidad.

Prioridad de Juan: **dejar el launcher lo más pulido posible** mientras él prepara el servidor.

## 12. Pruebas de aceptación y en qué etapa se cubren

| Situación | Etapa | Cómo se prueba |
|---|---|---|
| Un jugador nuevo prepara el cliente oficial desde cero y entra al servidor | E2 | carpeta vacía y un servidor de prueba local; también con el proxy de la escuela |
| Actualizar el paquete conserva preferencias y datos | E2 | instancia con `options.txt`, mundo y capturas; actualizar; comparar |
| Respaldar un mundo, cambiar de PC y restaurarlo con su instancia | E4 | dos carpetas de prueba con distinto `device` |
| Dos PC modifican el mismo mundo y no se pierde ninguna partida | E4 | dos ramas desde la misma revisión; quedan dos cabezas |
| Se corta una subida y la última confirmada sigue disponible | E4 | matar el proceso a la mitad; reanudar |
| El staff manda un ajuste y el resultado coincide con la configuración normal | E3 | comparar `ajustes.json` y los efectos con los de la ventana de Ajustes |
| Un doble clic o un reintento no invierte el ajuste | E3 | aplicar la misma `op` dos veces |
| Una tarjeta de Minecraft se aplica a la instancia exacta y se rechaza en otra | E5 | dos instancias con distinta huella |
| Falsificar una tarjeta oficial o cambiar permisos falla | E2/E3 | emulador: remitente falso, rol falso, editar tarjeta, darse permisos, leer soporte sin serlo |
| Una versión nueva del launcher no arranca y hay recuperación | E0 | exe de prueba que se cierra al iniciar; el anterior vuelve solo |

## 13. Coordinación con Mac

- Lo de E0 a E5 es código compartido y Mac lo compila igual. Lo propio del sistema va en
  `plataforma/` con su contrato.
  - La confirmación de arranque reemplaza la `.app` entera en Mac (`lanzador::install`).
  - La huella excluye nativos y Java, así que una instancia es la misma en Windows y en Mac.
- El logo nuevo llega solo a la `.app`: `empaquetar.sh` arma el `.icns` desde `ui/logo.png`.
- Cada etapa agrega su entrada en «Cambios de Windows» con qué probar en Mac.
