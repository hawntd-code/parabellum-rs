# Plan de noviembre: listo para el lanzamiento, E4, E5 y E6

Pedido de Juan el 2026-10-20, después de publicar la 0.34.0 (E3). Sigue a [plan-diciembre.md](plan-diciembre.md),
que tiene el porqué de cada etapa. Aquí está el **cómo**: qué se hace, en qué archivos, con qué pruebas y
en qué orden.

**Entra:**
1. el zip del paquete que tarda ~30 s;
2. los avisos de soporte desde que se abre Parabellum;
3. una sola conexión a Firebase por persona;
4. los detalles menores: errores con una acción, y la O después de F2;
5. E4, mundos en Drive;
6. E5, tarjetas de Minecraft;
7. E6.

**No entra** (decisión de Juan):
- el P2P desde la escuela (relevo propio);
- lo que depende de terceros: cuentas de Microsoft, CurseForge, Forge viejo y la firma de código.

Cada versión pasa la lista de la sección 10.1 de plan-diciembre.md. Si una versión necesita reglas de Firebase
nuevas, Juan las publica antes.

**Avance:** la 0.35.0 (puntos 1 y 2) está hecha el 2026-10-21 y falta publicarla. Lo que cambió respecto de
este plan:
- el «botón del escudo» es el de Comunidades en la barra lateral;
- el paquete ligero es otro zip junto al completo (`<Modpack> (ligero).zip`);
- para el equipo, el número cuenta las solicitudes en las que lo último lo escribió quien pidió ayuda (las
  nuevas y las que el jugador contestó), y «esperando» en la cola usa la misma regla.

---

## 1. El zip del paquete oficial tarda ~30 s en empezar

### Qué pasa (medido el 2026-10-20)

Primer byte de cada zip del catálogo, pedido igual que Parabellum (`drive.usercontent.google.com`, con
`confirm=t`):

| Archivo | Tamaño | Primer byte |
|---|---|---|
| Los 11 de más de 70 MB (bases, Better MC, Cobbleverse, Fabulously Optimized…) | 74 MB a 1.4 GB | **0.9 a 1.2 s** |
| `26.3/RETORCIDOS _ EL PACK.zip` | 24 MB | **29.6 s** (y una vez, **error 500** a los 30.2 s) |
| `26.3/Fabric 26.3.zip` | 24 MB | **29.4 s** |

Por la API (`files/{id}?alt=media`, con y sin `acknowledgeAbuse`) tarda lo mismo: 28.9 s. Pedir solo un
pedazo (`Range`) no cambia nada.

Por dentro no tienen nada raro. Fabulously Optimized tiene 157 jars o zips adentro y arranca en 0.9 s; el pack
tiene 21. La explicación que cuadra es **el revisor de virus de Google**:
- Drive revisa los archivos de menos de 100 MB antes de entregarlos (los más grandes no los revisa);
- con estos dos llega a su límite de ~30 s;
- a veces, en vez de soltar el archivo, responde 500.

No depende de la red: pasa en casa y en la escuela, y todos los días.

**Además:** el pack publicado en Drive no es igual a la instancia de Juan. Trae `ForgeConfigAPIPort` y no
trae el shader Complementary Reimagined. Juan decide cuál es el bueno y lo vuelve a publicar.

### Qué se hace

**a) Ya (en la versión que lo incluya):**
- **Reintentar los 500 y los cortes** de la descarga del paquete: dos veces, con 2 s y 5 s de espera, como
  ya hace `catalog::download_text`. Hoy un 500 a los 30 s deja al jugador con un error.
- **Pedir el zip antes:** al abrir la página de la comunidad, si el paquete oficial está en «falta» o «hay
  versión nueva», empezar a bajarlo en segundo plano. Tiene un tope: un paquete a la vez, y solo si pesa
  menos de 200 MB y no se está jugando. Cuando el jugador pulsa «Jugar», ya va avanzado o terminó.
- **Decir la verdad mientras espera:** en Descargas, «Google está revisando el archivo (hasta 30 s)» si pasan
  más de 3 s sin el primer byte, en vez de una barra quieta.

**b) La solución de fondo: los mods del paquete salen del CDN de Modrinth**

Al **publicar** un paquete (`packs::export`), cada archivo que exista en Modrinth (se busca por su SHA-1,
como ya hace `modrinth.json` de cada instancia) **no entra al zip**. Va en el manifiesto con:
- su URL directa de `cdn.modrinth.com`;
- su SHA-1, su SHA-512 y su tamaño;
- su ruta dentro de la instancia.

Al **instalar o actualizar**:
- esos archivos se bajan del CDN, 8 a la vez (el CDN pasa el filtro de la escuela y no hace falta la API ni
  el puente);
- se comprueban por su huella;
- el zip de Drive trae solo lo demás: configuraciones, `options.txt`, lo que no es de Modrinth y el `base/`
  del loader.

Así:
- el zip queda en unos pocos MB;
- casi todo baja en paralelo de un CDN rápido;
- una actualización baja solo los mods que cambiaron (ya se compara por huella con `identidad.rs`).

**Si el CDN falla** con un archivo (o el jugador está en una red que lo bloquea), se usa el **zip completo
de respaldo**: al publicar se sube también, como hoy, y solo se baja si hace falta.

**Archivos:**
- `packs.rs`: el manifiesto, `Manifest.remote: Vec<RemoteFile>`, y la exportación;
- `catalog.rs`: instalar y `delta_update` con archivos remotos;
- `modrinth.rs`: buscar por SHA-1 en lote, `version_files` con el puente;
- `descargas.rs`: el progreso y el texto de la espera.

**Formato:** `lanzador-pack.json` gana `remote` (opcional). Las versiones viejas de Parabellum lo ignoran y
siguen bajando el zip completo. Por eso, mientras haya jugadores con versiones viejas, el zip completo se
sigue publicando.

**Pruebas:**
- unitarias: el manifiesto con y sin `remote`, y la huella que no coincide (que use el respaldo);
- la ventana real: instalar el pack nuevo en una carpeta de prueba, medido de punta a punta;
- con el CDN bloqueado (`HTTPS_PROXY` como en la escuela, sin `cdn.modrinth.com` en `NO_PROXY`): que use el
  zip;
- el `delta_update` con un mod cambiado.

**Juan:** volver a publicar el paquete oficial con la versión que lo incluya, y elegir antes cuál es el bueno
(lo de `ForgeConfigAPIPort` y el shader).

---

## 2. Avisos de soporte desde que se abre Parabellum

### Qué pasa

La cola del soporte (`soporte_escuchar_cola`) solo empieza cuando se abre la página Comunidades.

- **Si no entraste en esa sesión:** no te enteras de que te respondieron, y el equipo no ve llegar
  solicitudes nuevas.
- **No hay contador** en el botón del escudo, como el de Mensajes.
- **Para el jugador hay que escuchar cada solicitud** (hasta 8 escuchas por comunidad), porque nada que
  pueda leer cambia cuando el staff contesta.

### Qué se hace

**Datos (reglas nuevas):** `supportNotify/{cid}/{uid}/{tid} = lastAt`.
- Lo escribe quien contesta una solicitud: el equipo, en el mismo `patch` del mensaje.
- Lo lee solo la persona dueña.
- Así el jugador escucha **una rama por comunidad** en vez de una por solicitud.
- El equipo sigue escuchando `supportTickets/{cid}`, que ya es la cola resumida.
- Las reglas, con sus pruebas en `docs/probar-reglas.py`:
  - validan que `{tid}` exista en `supportTickets/{cid}` con `by == {uid}`;
  - validan que quien escribe sea del equipo o la persona dueña;
  - aceptan solo números.

**Motor:** un `soporte::Vigia` que arranca con la sesión social (junto a `amigos::start` y `chat::start`):
- **lee** `userCommunities/{uid}`;
- **escucha**, por cada comunidad, `supportNotify/{cid}/{uid}` si eres jugador y también
  `supportTickets/{cid}` si eres del equipo;
- **calcula** las respuestas sin leer (las del jugador) y las solicitudes abiertas sin tomar (las del
  equipo);
- **manda** el evento `soporte-avisos` con `{ total, porComunidad, nuevas }`;
- **avisa con una notificación de Windows o Mac** (`plataforma::avisos`) si la ventana no está al frente y
  los avisos están activados (`settings.notifications`), una por solicitud, sin repetir;
- **se deja de escuchar** una comunidad al salir de ella, y se vuelve a leer la lista cuando cambia
  `userCommunities`.

`soporte_escuchar_cola` se va: la ventana usa el evento del vigía.

**Interfaz:**
- un número en el botón del escudo (como el de Mensajes): respuestas sin leer más, si eres del equipo,
  solicitudes sin tomar;
- en la lista de comunidades de arriba, un punto en la que tiene algo;
- al abrir la solicitud se marca leída, como ya hace `soporte_visto`;
- en el panel del juego (más adelante): el mismo aviso abajo a la derecha.

**Pruebas:**
- `f67_soporte.py` ampliada: el aviso llega sin abrir Comunidades, el número sube y baja, y la notificación
  sale una sola vez;
- reglas: un jugador no puede escribir en el `supportNotify` de otro; un ex miembro del staff, tampoco.

**Juan:** publicar las reglas nuevas antes de esa versión.

---

## 3. Una sola conexión a Firebase por persona

### Qué pasa (medido)

- **Cada escucha es una conexión aparte** (`firebase::Database::listen`, SSE por HTTP/1.1). Con 7 amigos
  y 8 conversaciones son **24 conexiones** TLS abiertas a la vez. El tráfico es mínimo (~1.3 KB por
  minuto); lo que pesa es abrirlas: al iniciar se ve la espera, y con comunidades y soporte son más.
- **HTTP/2 no sirve:** Firebase Realtime Database solo ofrece HTTP/1.1 (ALPN, comprobado el 2026-10-20), así
  que no se pueden juntar varias escuchas en una conexión.

### Qué se hace

**Paso 1, menos escuchas (sin cambiar de protocolo):**
- **`userChats/{uid}/{chatId}`:** un resumen por conversación (último mensaje, a qué hora y de quién) que
  escribe quien manda el mensaje, en el mismo `patch`. La lista de Mensajes escucha **una** rama en vez de
  una por conversación; los mensajes completos se escuchan solo en la conversación abierta.
- **La presencia de los amigos en un buzón propio:** `presenceInbox/{uid}`, escrito por cada amigo al
  cambiar su estado. Una escucha en vez de una por amigo.
- Las reglas, con sus pruebas, y una migración: si el resumen no existe, se arma la primera vez.

**Paso 2, el protocolo de WebSocket de Firebase:** el que usan los SDK oficiales. Es un solo
`wss://<base>/.ws?v=5&ns=<ns>` por persona y, por dentro, todas las escuchas (`q`) y escrituras (`p`, `m`):
- **Módulo nuevo `firebase_ws.rs`:**
  - el saludo (`h`) y la redirección de host;
  - `auth` con el token, que se renueva antes de vencer;
  - pedir y soltar escuchas (`q` y `n`) y las respuestas (`d` y `m`);
  - mensajes partidos en varios marcos y el *keep-alive*;
  - reconectar con espera creciente y volver a pedir lo que estaba escuchando.
- **`Database::listen` conserva su firma:** por dentro usa el WebSocket, y si no conecta (una red que lo
  bloquee), vuelve a SSE solo. Lo demás del código no cambia.
- **Dependencia:** `tokio-tungstenite` con rustls (la misma pila TLS que ya se usa).
- **El emulador habla el mismo protocolo:** todo se prueba ahí primero.

**Medir antes y después** con `datos/logs/firebase.csv` y `metrics::LISTENERS`: conexiones abiertas,
tiempo hasta tener amigos y mensajes al arrancar, y bytes por minuto. **Meta:** 1 conexión y menos de 1.5 s
hasta tener todo.

**Pruebas:**
- unitarias del protocolo (marcos partidos, redirección y reconexión), con un servidor de prueba;
- `--probar-tiempo-real` contra el emulador y contra la base real (con la sesión de Juan, solo lectura y una
  escritura de prueba que se borra, como ya hace `--probar-favoritos`);
- cortar la red a la mitad y ver que vuelve sola y no pierde mensajes;
- la escuela: que el WebSocket pase el filtro y, si no, que vuelva a SSE.

**Riesgo:** el protocolo no está documentado como API pública, aunque es estable porque lo usan todos los SDK
en producción. Por eso SSE se queda como respaldo y el cambio va primero al canal de pruebas.

---

## 4. Detalles menores

### 4.1 Errores con una acción para resolverlos

**Hoy** solo el modo escuela pone un botón en el aviso de error (`suggestsSchool`), además del doctor.

**Qué se hace:**
- **Errores con su acción:** el motor puede agregarle al mensaje una marca
  `⟦accion:<id>[:<argumento>]⟧`. `friendlyError` la quita y pone el botón.
- **Inventario primero:** los `Err(t!(…))` más comunes, con los registros de `datos/logs/` de Juan y del
  canal de pruebas.
- **Primera tanda:**

| Error | Acción |
|---|---|
| Sin internet, Drive o Modrinth no responden | **Reintentar** (vuelve a lanzar lo mismo) |
| Mojang o el loader bloqueados | **Activar modo escuela** (ya existe) |
| No hay espacio en el disco | **Ver qué ocupa espacio** (Ajustes › Recursos) y **Limpiar descargas terminadas** |
| Java no está o está dañado | **Reparar Java** (`verificar_instancia`) |
| Un archivo de la instancia está dañado o no coincide | **Verificar la instancia** |
| El juego está abierto y no deja hacer algo | **Cerrar el juego** |
| La sesión de Google venció | **Volver a iniciar sesión** |
| Windows no deja escribir (antivirus o permisos) | **Abrir la carpeta** y qué revisar |
| El puerto del servidor está ocupado | **Usar otro puerto** |
| El paquete pide una versión más nueva de Parabellum | **Buscar actualización** |

**Pruebas:** un caso por fila en la demostración (`demoInvoke` con el error) y capturas.

### 4.2 Pulsar O justo después de F2 abre esa captura lista para mandar

- **El puente (`puente.rs`)** ya conoce la carpeta `screenshots` de la instancia abierta. Anota la última
  captura y su hora: la ve aparecer con un vigía de carpeta, o al pedirla.
- **El mod (`mod/nucleo`, común a todas las versiones):** al pulsar O, pregunta al puente si hay una captura
  de hace menos de 10 s. Si la hay, el panel abre en **Capturas** con esa captura elegida y el selector de
  conversación abierto. Si no, abre como siempre.
- No hace falta tocar cada versión (`mod/versiones`): todo pasa en el núcleo y el puente. Se compilan los 18
  jars con `mod/compilar.py`.
- **Pruebas:**
  - `--abrir-juego` en 1.20.1, 1.21.1 y 26.3: F2 y luego O antes y después de los 10 s;
  - que una captura vieja no se vuelva a ofrecer.

---

## 5. E4: mundos en Drive

El diseño está en plan-diciembre.md, sección 8. Aquí va cómo se construye.

### 5.1 Permiso y cuenta

- **Permiso:** `drive.file`, pedido **la primera vez** que alguien activa el respaldo de un mundo. Se pide
  con `include_granted_scopes=true` sobre la misma cuenta de Google de la sesión, y su token se guarda
  aparte (`datos/drive-mundos.bin`), igual que hoy `drive-publicar.bin`.
- **Por qué `drive.file`:** no es un permiso sensible, así que Google no pide verificar la app.
- **Juan:** agregarlo a la pantalla de consentimiento en Google Cloud (proyecto parabellum52) antes de la
  versión.

### 5.2 Módulos

- **`mundos.rs` (nuevo):**
  - la identidad del mundo (`parabellum-mundo.json` con `worldId`, dueño e instalación);
  - armar la copia: un zip con todas las dimensiones, `playerdata`, `data` y `level.dat`, sin `session.lock`;
  - las revisiones y sus propiedades, y el cálculo de «la última» y de los conflictos;
  - la retención: 5 confirmadas, las cabezas de los conflictos y el tope de 5 GB.
- **`drive_mundos.rs` (nuevo):**
  - la carpeta «Parabellum/Mundos», buscada por `appProperties` y no por nombre;
  - la subida reanudable: la sesión de subida se guarda en `datos/mundos/subidas.json` y sobrevive a cerrar
    Parabellum o apagar la PC;
  - confirmar la copia con la huella MD5 que da Drive, y después `state=confirmada`;
  - listar, bajar y mandar a la papelera.
- **`descargas.rs`:** las subidas también salen en el panel de Descargas, con su cola, pausa y reintento.
- **El cuándo:**
  - al cerrar el juego (el aviso `juego-cerrado` de `main.rs`), si `session.lock` está libre;
  - al apagar un servidor propio, igual;
  - y a mano, con «Respaldar ahora».

### 5.3 Interfaz

- **En la pestaña Mundos de la instancia:**
  - un interruptor «Respaldar en Drive» por mundo;
  - el estado: «Respaldado hace 5 min», «Subiendo 40 %» o «Conflicto».
- **En el detalle de un mundo:** sus revisiones (fecha, PC y tamaño), con **Restaurar** y **Guardar como
  zip**.
- **Conflicto:** las dos ramas lado a lado (de qué PC y cuándo), con **Seguir con esta**; la otra se puede
  convertir en otro mundo.
- **En otra PC:** «Mundos en tu Drive» lista los que no están en este PC. **Bajar** prepara la instancia
  compatible (con `identidad.rs`) o dice qué falta. Lo que se reemplace queda en `respaldos.rs`.
- **Aviso del jugador:** si la copia es de otro UUID (otro nombre, sin premium), se avisa antes de abrir,
  porque el inventario «desaparecería».
- **Ajustes › Recursos:** cuánto ocupan los mundos en Drive y el tope.

### 5.4 Pruebas

- **Unitarias:** las revisiones (la última, conflictos y retención) y la identidad duplicada.
- **Subida reanudable:** cortar a la mitad, cerrar Parabellum y que siga. Con una cuenta de Google de prueba
  y en una carpeta de prueba de Drive, nunca la del catálogo.
- **Dos PC (o dos carpetas `PARABELLUM_ROOT` con la misma cuenta):** jugar en una, respaldar, restaurar en la
  otra; jugar en las dos sin subir y comprobar que sale el conflicto.
- **Mundo grande (2 GB):** el tiempo, la memoria (sin cargar el zip entero) y que la ventana no se trabe.

**Va primero al canal de pruebas.** Una sola versión con todo, o dos (respaldar en una, restaurar y
conflictos en la siguiente).

---

## 6. E5: tarjetas de Minecraft

Hoy las tarjetas solo cambian ajustes de Parabellum (`ajustes.rs`). E5 agrega ajustes **del juego** de una
instancia, que viven en `options.txt`.

### 6.1 El registro

- **`ajustes_juego.rs` (nuevo):** los ajustes que se pueden mandar, con un **adaptador por versión**, porque
  la clave y el formato de `options.txt` cambian.

| Ajuste | Clave y notas |
|---|---|
| Distancia de renderizado | `renderDistance` (2 a 32) |
| Distancia de simulación | `simulationDistance` (desde 1.18) |
| Calidad gráfica | `graphicsMode` (0, 1, 2) desde 1.19; antes, `fancyGraphics` |
| FPS máximos | `maxFps` |
| VSync | `enableVsync` |
| Campo de visión | `fov`, guardado como -1 a 1 |
| Escala de la interfaz | `guiScale` |
| Partículas | `particles` |
| Sombras de entidades | `entityShadows` |
| Brillo | `gamma` |

- Cada ajuste dice desde qué versión de Minecraft existe. Si la instancia es de una versión que no lo tiene,
  la tarjeta lo dice y no se aplica.

### 6.2 La tarjeta

- **El formato `v: 1` crece**, sin romper las de hoy:
  - `scope: "instancia"`;
  - `target: { package, minecraft, contentHash }`, como en plan-diciembre.md 7.2;
  - `setting: "minecraft.renderDistance"`.
- **Reglas:** la lista de ajustes del juego y sus valores, con sus pruebas.
- **Al aplicarla**, se revisa todo como en `aplicar_tarjeta`, y además:
  - que la instancia sea la del paquete oficial de esa comunidad;
  - que su huella coincida, o se avisa «tu paquete cambió» con la opción de restaurar.
- **Con el juego abierto queda pendiente:** Minecraft reescribe `options.txt` al cerrar. Se aplica en cuanto
  el juego se cierra (`juego-cerrado`) y se avisa «Se aplicará al cerrar el juego».
- **El registro (`datos/tarjetas.json`)** guarda la instancia, el valor de antes y el estado «pendiente».
- **Deshacer**, igual que hoy: solo si nadie lo cambió después.

### 6.3 Interfaz

- **El staff:** en «Proponer un ajuste», un segundo grupo «Del juego (paquete oficial)» con los ajustes de la
  versión del paquete.
- **El jugador:** la tarjeta dice a qué instancia aplica, el valor actual de **su** `options.txt` y el
  recomendado, y el estado «pendiente hasta cerrar el juego».

### 6.4 Pruebas

- adaptadores: los `options.txt` reales de 1.8.9, 1.16.5, 1.20.1, 1.21.1 y 26.3 de las instancias de prueba;
- aplicar con el juego cerrado y abierto (pendiente), deshacer y la huella distinta;
- reglas en el emulador;
- `f67_soporte.py` ampliada con una tarjeta de Minecraft.

---

## 7. E6: más adelante (en este orden)

1. **Subir solo lo que cambió de un mundo.** Cada revisión deja de ser un zip y pasa a ser un **manifiesto**:
   la lista de archivos con su SHA-256. Los archivos se guardan una sola vez en `Parabellum/Mundos/<worldId>/objetos/`.
   Como al jugar solo cambian unas regiones (`.mca`), una copia pasa de subir todo el mundo a subir unos pocos MB.
   Restaurar arma el mundo desde los objetos. La limpieza borra los objetos que ya no usa ninguna revisión
   conservada. Necesita E4.
2. **Configuraciones de mods por formato:** tarjetas para el `config/` de los mods, con adaptadores para
   TOML, JSON y JSON5, `.properties` e INI. Empieza por los del paquete oficial: Sodium, Xaero's Minimap,
   Jade y JEI. Usa el registro de E5.
3. **Aplicar en vivo con el mod:** con el juego abierto, el puente le pasa el ajuste al mod y el mod lo cambia
   en las opciones de Minecraft sin reiniciar. Lo que no se pueda en vivo queda pendiente, como en E5.
4. **Más comunidades y eventos:**
   - eventos con fecha y hora en la comunidad (apertura, torneos);
   - un aviso antes de que empiecen;
   - «Me apunto», con su contador;
   - aparecen en la landing si Juan quiere.
   - Reglas nuevas.
5. **Consolidar Firebase:** lo que quede del punto 3 (por ejemplo, el soporte y las comunidades dentro del
   WebSocket) y medir otra vez.

---

## 8. Orden y versiones

| Versión | Qué | Reglas nuevas | Juan |
|---|---|---|---|
| **0.35.0** | 1a (reintentos, pedir antes, «revisando»), 1b (mods del CDN de Modrinth) y 2 (avisos de soporte) | `supportNotify` | Publicar las reglas; volver a publicar el paquete oficial (elegir antes cuál es el bueno) |
| **0.36.0** | 3, paso 1 (`userChats` y buzón de presencia) | `userChats`, `presenceInbox` | Publicar las reglas |
| **0.37.0** | 3, paso 2 (WebSocket con respaldo SSE), primero en el canal de pruebas | — | Probarlo en la escuela |
| **0.38.0** | 4 (errores con acción y la O después de F2) | — | — |
| **0.39.0 – 0.40.0** | E4 (mundos en Drive), en el canal de pruebas y después estable | — | Agregar `drive.file` en Google Cloud |
| **0.41.0** | E5 (tarjetas de Minecraft) | Lista de ajustes del juego | Publicar las reglas |
| **Después** | E6, en el orden de la sección 7 | Eventos | Según cada uno |

**Cómo se trabaja cada versión:**
1. código con sus pruebas unitarias;
2. la ventana real contra el emulador;
3. la entrada en `docs/mac-cambios.md`, el ROADMAP y el número de versión;
4. compilar e instalar en la carpeta de Juan.

**Lo que hace solo Juan:**
- publicar en Drive;
- crear la release de GitHub (el agente la arma);
- publicar las reglas;
- los cambios en Google Cloud.

**Mac:** todo es código compartido. Lo único de sistema son las notificaciones del vigía de soporte
(`plataforma::avisos`, que ya existe en los dos) y el vigía de carpeta del punto 4.2. Diego compila y prueba
cada versión como siempre.
