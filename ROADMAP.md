# Hoja de ruta de Parabellum

Cada fase termina con una prueba concreta; no se avanza a la siguiente hasta que pasa.
El orden importa: cada fase usa lo que construyó la anterior.

## ✅ Hecho

### Fase 0: proyecto
- [x] Rust + Tauri v2, interfaz HTML/CSS/JS sin Node, exe único de ~6 MB
- [x] Repositorio y compilación (`cargo build --release`)

### Fase 1: launcher mínimo (vanilla)
- [x] Nombre de jugador y UUID offline (mismo algoritmo que Minecraft)
- [x] Lista oficial de versiones de Mojang (estables y snapshots)
- [x] Descarga paralela con verificación SHA-1: juego, bibliotecas, assets, log4j y el Java de Mojang
- [x] Jugar sin consola, proceso independiente del launcher
- [x] Entrar directo a un servidor al abrir el juego (Quick Play)
- [x] *Prueba:* instalación limpia de 1.21.1 y entrada a un servidor de prueba no premium

### Fase 2: servidores
- [x] Entrar a servidores públicos no premium (probado por Juan en UniversoCraft)
- (El servidor de sesión local para mundos LAN/e4mc de amigos pasa a la fase 9, donde se usa)

### Fase 3: gestión de instancias
- [x] Ajustes de la instancia: nombre, icono propio, memoria propia y argumentos de Java
- [x] Duplicar y eliminar (a la papelera de reciclaje)
- [x] Pestaña Contenido: mods (con nombre, versión, autores e icono leídos del propio mod), resource packs, shaders y mundos; activar/desactivar y mandar a la papelera
- [x] Pestaña Registro: registro del juego dentro del launcher, coloreado, con filtro y actualización en vivo
- [x] Seguir el proceso del juego, botón Detener y aviso con el motivo cuando se cierra con error
- [x] *Prueba:* `--probar-gestion` (duplicar, editar, icono, activar/desactivar, papelera y motivo del error)

### Fase 4: Fabric y Quilt
- [x] Crear instancias Fabric o Quilt eligiendo la versión de Minecraft (solo las compatibles) y la del loader (la recomendada por defecto)
- [x] Cambiar la versión del loader desde los ajustes de la instancia (algunos mods piden una más nueva)
- [x] Sin conexión: se pueden crear instancias con loaders ya descargados
- [x] Agregar archivos a una instancia con un botón o arrastrándolos a la ventana; cada uno va a su categoría y se rechazan los mods de otro loader
- [x] *Prueba:* Fabric 0.19.5 y Quilt 0.30.1 en 1.21.1 con Fabric API y Mod Menu llegan al menú principal sin errores (`--probar-juego`)

### Fase 5: explorar e instalar de Modrinth (base de datos de mods)
- [x] Página Explorar: mods, resource packs y shaders de **Modrinth** con búsqueda, orden y filtros (loader, versión, categorías)
- [x] Página de cada proyecto: descripción (Markdown limpiado: sin scripts ni enlaces peligrosos), versiones, galería, compatibilidad y enlaces
- [x] Instalar con un clic eligiendo la versión compatible con la instancia ("Instalar en")
- [x] **Dependencias**: las obligatorias se instalan solas, las opcionales se ofrecen y las incompatibles se bloquean
- [x] **Versiones que funcionan juntas**: se leen los rangos de versión de cada mod (fabric.mod.json) y, si una dependencia no encaja, se prueban otras (p. ej. Iris 1.8.8 pide Sodium 0.6.x); si no hay forma, se explica por qué
- [x] Lo instalado se reconoce por SHA-1, también los mods agregados a mano
- [x] Actualizaciones en la pestaña Contenido: sin bajar de estable a beta y sin aplicar las que romperían algo (se avisa por qué)
- [x] Al cerrarse el juego por mods incompatibles se muestra la explicación de Fabric
- [x] *Prueba:* Lithium, Iris (con su Sodium compatible) y Mod Menu instalados desde la interfaz llegan al menú principal sin errores

### Fase 6: modpacks
- [x] **Exportar para Drive o USB**: paquete completo con el mismo formato que la versión C# (modpack + base con Minecraft y Java, y su .json con SHA-256); se instala sin internet
- [x] **Exportar .mrpack** (estándar de Modrinth): ligero, los mods se descargan al instalarlo
- [x] Elegir qué incluir (mods, configuración, mundos, opciones…)
- [x] **Subir un modpack** desde Crear instancia o soltándolo en la ventana: .mrpack, paquetes de Parabellum y de la versión C# (también sus instancias de CurseForge con Fabric); reinstalar un paquete lo actualiza conservando mundos, opciones y memoria propia
- [x] Instalar **modpacks de Modrinth** como instancia nueva, con su icono
- [x] **Data packs** de Modrinth dentro del mundo que elijas
- [x] *Prueba:* escuela simulada (carpeta vacía: base y modpack, sin Mojang, Fabric ni Modrinth) llega al menú; el paquete real "Minecraft++" de la versión C# (147 mods) funciona; .mrpack de ida y vuelta; Fabulously Optimized desde Modrinth

### Fase 7: escuela y distribución
- [x] **Catálogo de Drive** en Explorar (pestaña Drive): modpacks y versiones de Minecraft de la carpeta pública de Drive y de la carpeta «paquetes» junto al launcher (USB); mismo formato que la versión C#
- [x] Instalar con un clic: si falta Minecraft, se instala antes su paquete base; todo se verifica con SHA-256
- [x] **Actualizar bajando solo lo que cambió** (o reparar lo dañado): se lee el índice del zip en Drive por rangos y se descargan solo los archivos distintos (CRC32)
- [x] En la escuela, si Modrinth está bloqueado, Explorar pasa solo a la pestaña Drive
- [x] Exportar para Drive recuerda la carpeta; botón para abrir tu Drive y arrastrarla
- [x] **Auto-actualización del launcher**: Acerca de › Publicar esta versión prepara _parabellum; los demás ven «Actualizar a X» y se reinicia solo
- [x] Configuración integrada en el exe (config-integrada.json fuera del repositorio; lanzador-config.json junto al exe la sustituye)
- [x] La ventana se abre maximizada
- [x] **Versión C# retirada** (2026-10-08): sus instancias, mundos y bases pasaron a la carpeta de Parabellum; Parabellum lee sus ajustes (nombre, UUID fijo, memoria) y su actualizador lleva a quienes la usen a esta versión en cuanto se publique en Drive
- [x] (Fase 9) Subir a Drive directamente desde Parabellum (0.16)

### Fase 8: Forge, NeoForge e importación
- [x] **Instancias Forge y NeoForge** con su instalador oficial: bibliotecas y "processors" (las herramientas que preparan el juego parcheado), también para Minecraft 26; cambiar su versión desde la instancia
- [x] Forge 1.17–1.20 y NeoForge arrancan sin el .jar original en el classpath (-DignoreList), como el launcher oficial
- [x] **Modpacks de Forge y NeoForge**: .mrpack, de Modrinth y de Drive; los paquetes para Drive incluyen lo que Forge genera y no nombra en su JSON (juego parcheado, fmlcore, mapeos)
- [x] **Importar de otro launcher**: CurseForge, Prism y MultiMC se encuentran solos (o se elige la carpeta); se copian mods, mundos y opciones y el loader lo instala Parabellum
- [x] *Prueba:* Better MC BMC4 (Forge 47.4.20, 444 mods) llega al menú; desde cero, Forge 1.20.1, NeoForge 1.21.1, Forge 26.1.2 y NeoForge 26.1.2 se instalan y llegan al menú; Better MC [BMC6] (NeoForge, 228 mods) importado de CurseForge llega al menú; paquetes de Forge y NeoForge exportados e instalados en una carpeta vacía sin internet llegan al menú
- [ ] (Opcional, en pausa) CurseForge: requiere solicitar una clave de API y respetar qué autores permiten descargas externas. Juan lo dejó de lado el 2026-10-12: los archivos de CurseForge que piden los modpacks se siguen bajando como hasta ahora
- [x] Probar Forge 1.12.2: abre (0.16.0, con Java 8 los argumentos van en la línea de comandos)

### Modpacks completos e íntegros (0.10.1)
- [x] **Mods que el modpack pide a CurseForge**: algunos modpacks de Modrinth (Better MC BMC2) no pueden traer los mods que solo están en CurseForge y los piden con Missing Mods Checker, que abre una ventana para bajarlos a mano y cierra el juego. Parabellum los baja solo, de la CDN de CurseForge, al instalar el modpack (y al importar o instalar desde Drive)
- [x] **Lista de lo que trae cada modpack** (`.lanzador/archivos.json`): mods, resource packs y shaders con su huella (SHA-1 de Modrinth o CRC32 del paquete) y de dónde volver a bajarlos
- [x] **Antes de jugar** se comprueba que no falte nada; lo que falte se baja o se repone del modpack (de Modrinth o del paquete de Drive/USB). Si falta un mod y no hay forma de reponerlo, se explica en lugar de abrir un juego roto
- [x] **Verificar y reparar archivos** (menú ⋯ de la instancia): comprueba la huella de cada archivo y repone lo dañado; en instancias instaladas antes, arma la lista con el modpack original
- [x] Lo que el jugador quita o actualiza desde Contenido deja de vigilarse; lo desactivado cuenta como presente
- [x] Exportar revisa antes que el modpack esté completo; «Actualizar todo» en un modpack pide confirmación (rompe la combinación que probó su autor)
- [x] *Prueba:* Better MC BMC2 desde Modrinth llega completo (356 mods y los 34 archivos de CurseForge; el mod dice «All mods found!» y el juego llega al menú); borrar o dañar mods y repararlos (rápido, completo, sin lista, desde el paquete de Drive conservando mundos)

### Diseño
- [x] Interfaz al estilo de la app de Modrinth con identidad morada (tema oscuro y claro, fuente Inter)
- [x] Biblioteca, continuar jugando, crear instancia por tipo, ajustes por secciones, página Servidores

## ⏭️ Pendiente, en orden

### Plan desde el 2026-10-19 (pedido de Juan): primero la interfaz, después lo demás

**1. 0.32.0: pulido completo de la interfaz (publicada por Juan el 2026-10-20; detalle en `docs/auditoria-ui.md`).** Que todo se vea pulcro, tenga sentido, esté bien
centrado y animado. Se revisa absolutamente cada vista, formulario y ventana.
- [x] **Inventario con capturas** de cada vista en todos sus estados (vacía, cargando, con error, llena), en tema oscuro y claro, en español e inglés, y con la ventana chica y grande
- [x] **Sistema de diseño**:
  - medidas comunes de espaciado, radios, tipografía, sombras y capas;
  - un solo componente por cosa: botones, campos, listas desplegables, segmentados, interruptores, tarjetas, chips, filas y diálogos;
  - quitar estilos repetidos o que se contradicen.
- [x] **Ventanas y formularios** (21), al nivel de los de Comunidades:
  - instancias: crear, editar, exportar, instalar, icono y versiones de Drive;
  - servidores: crear servidor, EULA, servidor guardado y favoritos;
  - social: grupo, imagen al chat, reportar, agregar amigo y cuenta;
  - generales: actualización, confirmar, doctor y Ajustes.
- [x] **Ajustes**: sus 6 secciones (Apariencia, Comportamiento, Java, Perfil, Recursos, Acerca de)
- [x] **Páginas** (9) y sus pestañas:
  - Inicio, Explorar, Proyecto, Servidores, Servidor hosteado, Comunidades, Amigos, Mensajes;
  - Instancia, con sus 7 pestañas, y las 3 del servidor.
- [x] **El marco**:
  - barra de arriba y barra lateral;
  - paneles flotantes (descargas, juego e información), menús, avisos y pantalla de carga.
- [x] **Estados**: esqueletos mientras carga; vacíos que explican qué hacer; los errores internos ya no salen tal cual
- [ ] Después: más errores con una acción para resolverlos (hoy la tienen el modo escuela y el doctor)
- [x] **Movimiento**:
  - entradas y transiciones coherentes y botones que se hunden;
  - cambio de tema con un círculo;
  - números y minigráficas vivas en el servidor (lo que quedaba del «Resto del pulido»).
- [x] **Forma**: iconos y textos centrados, tamaños consistentes, arte y figuras bien hechas, nada cortado ni encimado
- [x] **Uso**: todo con teclado y foco visible, textos claros y completos en los dos idiomas
- [x] Prueba final en la ventana real (WebView2): sin errores de JavaScript; capturas de antes y después de cada tanda
- [ ] *Prueba:* Juan: recorrer la 0.32.0 en su PC (en especial el cambio de tema y las ventanas largas)

**2. Después de la interfaz, en este orden:**
- [x] **E1, base compartida (0.33.0)** (`docs/plan-diciembre.md`):
  - **huella del paquete** (`identidad.rs`):
    - al instalar, actualizar, reparar o exportar un paquete se guarda qué mods trae, por su contenido (SHA-1);
    - la instancia se compara con eso: «igual» o «modificada» (mods de más, que faltan, otro loader);
    - las instalaciones anteriores se migran solas con lo que guardó `integridad.rs`.
  - **Comunidades**: el paquete oficial dice «Modificado» con «Restaurar», y «Jugar» ofrece «Restaurar y jugar» o «Jugar así». Quien publicó ve «Cambió desde que la publicaste».
  - **Detalles de la instancia**: si es igual a su paquete (y qué cambió), y sus copias de seguridad.
  - **Respaldos** (`respaldos.rs`):
    - antes de actualizar o restaurar un paquete, lo que se reemplaza o se quita queda en `datos/respaldos/`;
    - «Restaurar» lo vuelve a poner, y lo de ese momento queda en otra copia;
    - se guardan 3 por instancia y 4 GB en total.
  - **Servicio de ajustes** (`ajustes.rs`):
    - `guardar_ajustes` y las tarjetas del soporte (E3) usan los mismos efectos;
    - registro de los 7 ajustes compartibles con su validación;
    - una operación se aplica solo si el valor sigue siendo el esperado, y repetirla no cambia nada.
- [ ] *Prueba:* Juan: en la escuela o en casa, quitar un mod del paquete oficial y ver que «Jugar» lo detecta y lo restaura
- [x] **E3, soporte y tarjetas (0.34.0)** (`soporte.rs`, `ui/soporte.js`):
  - **Pedir ayuda** desde la comunidad o desde «Reportar problema»: tema, qué pasó, la instancia y su
    diagnóstico, que se ve antes de mandarlo y va sin la carpeta del usuario, correos ni claves.
  - **Lo lee todo el equipo de soporte** (dueño, administradores y quien tenga «Soporte»), en su propio
    espacio de Firebase (`supportTickets`, `supportMessages`, `supportByUser`), no en el chat.
  - El equipo ve **la cola** con filtros (abiertas, mías, cerradas), la toma, contesta y la cierra; el
    jugador ve sus solicitudes con un punto si le respondieron. Todo llega solo, sin recargar.
  - **Tarjetas de ajustes**: el equipo propone uno de los 7 ajustes compartibles con su valor. El
    jugador ve el valor de su PC y el recomendado, y decide si lo aplica:
    - se vuelve a comprobar todo al aplicarla (que la solicitud sea suya y que quien la mandó siga en el
      equipo de esa comunidad);
    - se aplica con `ajustes::apply`; repetirla no cambia nada;
    - queda en `datos/tarjetas.json`, con **Deshacer**, que no pisa un cambio hecho después;
    - el equipo ve el acuse: «la aplicó en su PC».
  - Las reglas de Firebase validan cada tarjeta (de la lista, con un valor de su tipo y solo del equipo)
    y no dejan editar ni borrar mensajes. Probadas en el emulador (`docs/probar-reglas.py`).
- [ ] *Prueba:* Juan: pedir ayuda desde otra cuenta en su comunidad y mandarle una tarjeta (la 0.34.0 y sus reglas ya están publicadas)
- [ ] **E4, mundos en Drive** (opcional antes de diciembre): copias, revisiones y restaurar con su instancia
- [ ] **Pruebas reales pendientes**:
  - en la escuela: Modrinth sin modo escuela;
  - con amigos en sus casas: P2P y los «teletransportes»;
  - el panel en 26.3;
  - las demás *Prueba:* de abajo.
- [ ] **Detalles**:
  - el zip de «RETORCIDOS | EL PACK» tarda ~30 s en empezar a bajar de Drive;
  - Mac al día con la 0.34 (Diego; la Mac 0.33.0 ya está publicada);
  - una sola conexión a Firebase por persona.
- [ ] **P2P desde la escuela** (después de E3, pedido de Juan el 2026-10-20). Medido en la UABC:
  - el filtro bloquea los 4 relevos de n0;
  - la red deja salir UDP con un NAT que conserva el puerto, y TCP a cualquier puerto. Los servidores
    públicos de Minecraft responden, así que el servidor oficial se podrá jugar desde la escuela.
  - Hace falta un relevo propio en un dominio que no esté bloqueado, idealmente en la nube del
    servidor de Juan.
- [ ] **Sin fecha**:
  - cuentas premium de Microsoft (depende de que Microsoft apruebe la app);
  - CurseForge (en pausa);
  - Forge 1.8–1.12.1, 1.13.2, 1.14.2 y 1.14.3.

### 0.25.0: que se sienta fluido (pedido de Juan)
- [x] **Un sistema de movimiento** en `styles.css`: cuatro duraciones (respuesta al pulsar, colores, aparecer, páginas) y tres curvas (entrar frena suave, salir acelera, énfasis con un pequeño rebote) en vez de 104 valores sueltos; los interruptores y el botón de las tarjetas rebotan un poco
- [x] **Cambio de página**: la anterior se desvanece debajo mientras entra la nueva (sin View Transitions: funciona igual en la ventana de Mac). La **portada** de una instancia (desde la Biblioteca o la barra) o de un servidor (desde Hostear) viaja de su tarjeta a la cabecera
- [x] **Listas vivas** (`setHtml` con `data-key`): amigos, solicitudes, grupos, conversaciones, mensajes, contenido de la instancia, Biblioteca y servidores se deslizan a su nuevo lugar; lo nuevo aparece y lo que se va se desvanece en su sitio. Solo se animan las filas que se ven; si cambia la instancia, el tipo o el chat, la lista entra de nuevo
- [x] **Chat**: el mensaje nuevo entra y los demás suben; el borrado se desvanece; «Mensajes nuevos» si estás leyendo más arriba; al mandar algo, baja solo
- [x] **Progreso**: las barras avanzan suave y con un brillo mientras trabajan; arriba, junto al porcentaje, cuánto falta («· 2 min»). **Jugar** se vuelve «Abriendo…» (y se llena si falta bajar algo) y «Detener» aparece con un rebote. Los avisos de la ventana se van sin que los demás salten
- [x] **Dentro del juego**: el panel entra subiendo y aclarándose (0.18 s); los avisos entran y se van deslizándose desde la orilla y ahora salen **abajo a la derecha** (arriba los tapaban los del juego: logros, recetas, tutorial). Solo se redibujan mientras se mueven
- [x] **Que encuentren el panel**: botón «Parabellum» en el menú de pausa (las 18 versiones; mixin en Fabric, evento en Forge/NeoForge) y, la primera vez que se entra a un mundo, el aviso «Pulsa O para abrirlo» hasta que se abra una vez (`datos/panel-visto.txt`). La tarjeta de Novedades de Inicio lo cuenta
- [x] Reglas: 0.4 s como mucho, nunca hacer esperar, sin texto extra, solo movimiento y opacidad; con «reducir movimiento» de Windows o la ventana minimizada no se anima nada
- [x] `--abrir-juego <instancia> [segundos] [con-mod|sin-mod] [servidor]` entra directo a un servidor; `-Dparabellum.prueba.lento=N` para capturar la entrada del panel
- [x] Probado en la ventana real (WebView2): la página anterior se va, la portada vuela y se suelta, al buscar en la Biblioteca las tarjetas que sobran se desvanecen (solo las que se ven), los avisos se cierran sin saltos, «Abriendo…» se llena, sin errores de JavaScript. En el juego: el panel a medio entrar (semitransparente) en las cinco formas de dibujar (1.16.5, 1.18.2, 1.21.1/NeoForge, 1.21.4, 1.21.8–26.3) y abierto en 15 versiones; dentro de un mundo (Fabric y NeoForge 1.21.1) sale la pista, el botón «Parabellum» del menú de pausa abre el panel y nuestro mod no impide entrar a servidores sin mods
- [ ] *Prueba:* Juan: que se sienta bien en la ventana real (páginas, listas, chat, descargas)

### 0.26.3: un solo Parabellum abierto
- [x] **Causa de fondo** de los P2P rechazados y del «No hay permiso» al actualizar: Parabellum se podía abrir dos veces (uno escondido junto al reloj y otro nuevo). Cada copia publicaba su propia dirección P2P en la misma presencia (el anfitrión no reconocía a quien entraba aunque releyera), duplicaba las conexiones a Firebase y la copia vieja dejaba bloqueado el exe al actualizar
- [x] Ahora hay un candado por carpeta (`datos/parabellum.lock`, con el bloqueo de archivos de Rust: igual en Windows y Mac). El segundo que se abre deja `datos/mostrar-ventana`, el primero se muestra y el segundo se cierra. Tras actualizar (`--reiniciar`), la versión nueva espera hasta 20 s a que la anterior se cierre
- [x] Probado: el segundo se cierra en 1 s y el primero se muestra; con `--reiniciar` espera al viejo y luego abre
- [x] Telemetría de la sesión real con David (19:38–19:49, anfitrión Juan): P2P directo todo el tiempo, ping mediana 36 ms (p95 42, máx 78), 0 paquetes perdidos de 43 617. Servidor: 6 ms por tick, pero 10 trabones de 50–126 ms con TPS en 20: esos son los «teletransportes»
- [x] Después: menos trabones del servidor cuando el anfitrión también juega (0.29.0: guardado sin forzar el disco y pregenerar el mundo)

### 0.26.2: lo que vieron los amigos de Juan con la 0.26.1
- [x] **P2P rechazado**: un amigo llegaba por P2P directo (32 ms) pero el anfitrión rechazaba cada intento porque no reconocía su dirección P2P (cambia cada vez que abre Parabellum y se reconoce por su presencia en Firebase). Ahora, si no lo reconoce, vuelve a leer la presencia de todos (como mucho cada 10 s) y el siguiente intento entra; `p2p.log` anota el motivo de cada rechazo y, al releer, quién no tiene dirección P2P o no se deja leer
- [x] **Actualizar**: fallaba con «No hay permiso para cambiar la carpeta» si no se podía borrar el `Parabellum.exe.old` de la actualización anterior (un Parabellum viejo abierto, un antivirus). Ahora el exe actual se aparta con otro nombre (`.old-<hora>`), se limpian todos al abrir y el error dice el motivo de Windows
- [x] **Escribir en el panel en 26.3**: desde la 26.3 (SDL3) las letras solo llegan si la pantalla pide entrada de texto; el panel la pide mientras está abierto
- [ ] *Prueba:* Juan: escribir en el panel en 26.3; el amigo que fue rechazado vuelve a pulsar «Unirse»

### 0.26.1: de dónde viene el lag (pedido de Juan: «a ratos se teletransporta el personaje»)
- [x] **Telemetría P2P cada segundo** (los dos lados): ping, camino (directo o relevo) y paquetes perdidos en `datos/logs/p2p-latencia.csv`; en `p2p.log` se anotan los picos de ping (más de 2.5 veces lo normal), el paso al relevo y las pérdidas. Los primeros 10 s no cuentan (al conectar se prueban caminos). La tarjeta P2P muestra «picos de X ms» o «pierde X %» del último minuto
- [x] **Tiempo por tick del servidor** (1.20.3+ y 26.x): «tick query» cada 4 s, promedio y percentiles en `servidores/<servidor>/.lanzador/rendimiento.csv`; debajo del TPS sale «X ms por tick» y «trabón de X ms» si con alguien jugando un tick tardó más de 50 ms (el TPS promedio de 6 s no los ve). Las preguntas no ensucian la consola
- [x] **Firebase**: los errores ya no muestran la dirección con la clave de la sesión (se veía en pantalla) y el 402 (plan lleno) se explica. El proyecto pasó a Blaze (Juan): el límite de 100 conexiones a la vez del plan gratis se había llenado con 5 o 6 Parabellum abiertos
- [x] Probado: la prueba P2P por internet escribe la telemetría de los dos lados; «Prueba vanilla» encendido desde Parabellum mide 1.8 ms por tick y guarda rendimiento.csv
- [ ] *Prueba:* Juan con un amigo 15–20 min; cuando se teletransporte, anotar la hora para cruzarla con p2p-latencia.csv (de los dos) y rendimiento.csv
- [ ] Después (opcional): una sola conexión a Firebase por persona (hoy 10–25), con el protocolo de WebSocket de la base

### 0.26.0: lo que encontró Juan probando con sus amigos (pedido de Juan)
- [x] **Panel en 26.3**: no se podía cambiar de pestaña ni pulsar «Mensaje». Desde la 26.3 Minecraft numera los botones del ratón como SDL3 (izquierdo 1, derecho 3; antes 0 y 1) y el panel tomaba cada clic izquierdo como derecho. Ahora se traducen, como ya se hacía con el teclado
- [x] **Servidores 26.x**: no salía quién estaba conectado ni los TPS. Desde 26.x la consola escribe «System chat: Juan joined the game» y «The game time is 98 tick(s)»; se leen los dos formatos (con pruebas)
- [x] **Grupos en el panel del juego**: sección «Grupos» debajo de los amigos (como en el panel derecho de Parabellum); al pulsar uno se abre su conversación
- [x] **Capturas en el panel**: cuándo se sacó cada una, «Mandar» al pasar el ratón, «Enviar a» también a amigos sin conversación (se crea), los grupos con su inicial y la lista se desplaza
- [x] **P2P visto desde el anfitrión**: el registro anota «se conectó: directo (N ms)» o «relevo», y la tarjeta P2P del servidor (y la línea del mundo abierto) muestra quién está jugando por P2P, si va directo o por relevo y su ping, en vivo. Para comparar con playit.gg
- [x] Probado: la prueba P2P por internet ve al invitado (destino, directo, ping) y lo quita al salir; Grupos y Capturas capturados en Fabric 1.21.1 (clic en una captura → «Enviar a»); el panel de 26.3 abre (sus clics no se pueden simular: los revisa Juan)
- [ ] *Prueba:* Juan: en 26.3 cambiar de pestaña, «Mensaje» y Capturas; ver los jugadores del servidor; con un amigo, la tarjeta P2P (directo o relevo y ping)

### 0.31.3: Modrinth en la escuela, sin modo escuela (pedido de Juan)
- [x] **Medida la red de la escuela** (UABC): un FortiGate bloquea por nombre la API de Modrinth, Mojang, la info de Fabric, Forge, Quilt, CurseForge, playit y el P2P. Deja pasar Google, Drive, Firebase, Cloud Run, el CDN de Modrinth, `maven.fabricmc.net`, NeoForge y GitHub. Drive da ~12 MB/s con 6 conexiones
- [x] **Puente de Modrinth** (`puente-modrinth/`, Cloud Run en `parabellum52`):
  - Parabellum va directo y, si la red lo bloquea, por el puente, sin preguntar;
  - los archivos siguen bajando directo del CDN;
  - solo 10 rutas permitidas, con caché y gzip (las categorías pasan de 55 KB a 5.6 KB);
  - máximo 3 instancias; costo estimado: centavos al mes.
- [x] Explorar abre en Modrinth también en modo escuela
- [x] Un modpack de Modrinth cuyo loader exacto no está en el espejo usa el del espejo para esa versión de Minecraft. Antes fallaba en la escuela
- [ ] *Prueba:* Juan en la escuela: buscar e instalar mods y un modpack de Modrinth sin modo escuela

### 0.31.0: Comunidades (plan de diciembre, etapa E2; pedido de Juan)
- [x] **Reglas de Firebase** de comunidades, con 45 pruebas en el emulador (`docs/probar-reglas.py`):
  - se entra con un código vigente;
  - solo el dueño nombra administradores;
  - un administrador da permisos a los miembros, pero no a sí mismo ni a otros administradores;
  - quien no es miembro solo ve lo público.
- [x] **Motor** (`comunidad.rs`): crear, unirse, invitar, anuncios, reglas, servidores (Java y Bedrock), staff con sus permisos (soporte, publicar paquetes, servidores) y el paquete oficial (un modpack del catálogo de Drive)
- [x] **Página Comunidades** (`ui/comunidad.js`):
  - unirse con un código o con «Unirse» en la invitación por mensaje;
  - anuncios, reglas, servidores con su estado y miembros;
  - «Jugar» instala o actualiza el paquete oficial y entra directo al servidor.
- [x] **0.31.1 (pedido de Juan):**
  - «Pedir la versión que está ahora en Drive» ya no duplica el paquete; tampoco se duplican los servidores al editarlos.
  - El paquete muestra si está instalado, con «Instalar» o «Actualizar».
  - **El paquete oficial sale de una de tus instancias**: se elige en tarjetas, se exporta sin Minecraft y se sube solo a Drive. También puede ser un modpack que ya está en Drive.
  - Formularios rediseñados:
    - comunidad: la imagen se cambia con un clic, también desde arriba;
    - anuncios y reglas, con vista previa;
    - servidor, con «Probar»;
    - invitar a varios **amigos y grupos** a la vez, con el código grande;
    - permisos explicados.
  - La columna derecha es ~30 % más angosta.
- [x] Reglas de Firebase publicadas (Juan, 2026-10-18), y después la 0.31.1 en Drive
- [x] Paquete oficial conectado desde una instancia de Juan («RETORCIDOS | MODPACK OFICIAL», 26.2, Fabric, 11 mods): la primera publicación real con «Mis instancias»
- [x] **0.31.2:** el catálogo reintenta los `.json` que Drive no entrega a la primera. Antes, 2 de 5 lecturas perdían uno, a veces el de la versión nueva de Parabellum

### 0.30.1: canal de pruebas y el logo nuevo en el exe (pedido de Juan)
- [x] **Canal de pruebas abierto a todos** (`canal.rs`): Ajustes › Acerca de › Estable o Pruebas. En pruebas llegan también las versiones de `_parabellum-pruebas`; al volver a estable con una de pruebas más nueva, «Volver a la versión estable» (sin perder datos). Quien publica elige el canal
- [x] El logo nuevo también en el exe y en los accesos directos que ya había (ver la 0.30.0)

### 0.30.0: todo Minecraft en Drive, modo escuela, pantalla de carga y chat (pedido de Juan) — publicada el 2026-10-18
- [x] **Espejo de Minecraft en Google Drive** (`espejo.rs`, carpeta `_espejo` del catálogo): las 75 versiones que abre Parabellum (1.8 a 26.3), 155 loaders (Fabric, Quilt, Forge y NeoForge, su versión recomendada) y 248 servidores (vanilla y con cada loader, más los de los modpacks del catálogo con su versión exacta), con sus Java. Cada archivo se guarda **una sola vez** por su SHA-1 en capas de ~2 GB: todo ocupa **12.8 GB** (un zip por versión habrían sido más de 50). Cada PC baja por partes solo lo que le falta
- [x] Al crear una instancia, cambiar de loader, instalar un modpack de Drive sin su base o crear un servidor, si no hay Mojang (o con el modo escuela) todo sale del espejo. Probado con un proxy que solo deja pasar Google (como la escuela) y una carpeta vacía: Fabric 1.20.1 en 53 s (al menú en 27 s), su servidor en 24 s (listo en 45 s), Forge 1.20.1 desde la ventana en 63 s y el modpack «Fabric 26.3» con su Minecraft en 70 s
- [x] **«Subir versiones de Minecraft»** (quien publica) ahora sube al espejo, con sus loaders y servidores, y tiene «Todas». También `--espejo` y `--espejo-subir` (`docs/desarrollo.md`)
- [x] **Modo escuela** (botón de arriba, junto a Descargas): todo se baja de Google Drive y no se intenta Mojang ni los servidores de los loaders (cada intento bloqueado tardaba en fallar); la lista de versiones y de loaders es la del espejo, y Explorar abre en los paquetes de Drive. Desactivado: Mojang y cada loader, y Drive solo si no responden
- [x] **Pantalla de carga**: al abrir, primero busca una versión nueva (si la hay, pide actualizar antes de mostrar lo demás) y después espera a tu sesión de Google hasta que llegan tus amigos y tus mensajes (15 s como mucho; a los 6 s, «Entrar sin esperar»). Medido: ~5 s en total. La sesión de Google ya no espera a la conexión directa (P2P)
- [x] El catálogo de Drive se lee con las carpetas y los .json de cada nivel a la vez: de ~15 s a 3–5 s; buscar la versión nueva al abrir mira solo la carpeta del launcher (~1.4 s)
- [x] **Chat**: tu mensaje sale al momento («Enviando…») y, si falla, «Reintentar» o «Descartar»; un borrador por conversación; los enlaces se abren en el navegador; buscar conversaciones (Enter abre la primera); emojis al escribir; «Hoy» y «Ayer» en los separadores (y la fecha en el idioma de la ventana); **«está escribiendo…»** arriba y en la lista (va en la presencia, sin reglas nuevas de Firebase)
- [x] Servidores Quilt de 1.14 a 1.17: su instalador pide Java 17 y se le daba el de la versión (fallaba siempre); ahora usa el más nuevo
- [x] De Diego (PR #15): en 26.x la «Lista de amigos» de Mojang también venía en la O: pasa a «]» mientras la del panel siga en la O; los paquetes exportados ya no llevan tu tecla del panel; una imagen pegada tiene que ser imagen por dentro
- [x] **Una actualización que no arranca se deshace sola** (`arranque.rs`, mejora 3 del plan): la versión anterior se conserva hasta que la nueva confirma que cargó su ventana (sin depender de internet); la abre un vigía (el exe anterior con `--vigilar-actualizacion`) que, si en 90 s no hay confirmación, la cierra, vuelve a la anterior, la abre y no vuelve a ofrecer esa versión hasta que salga otra. Probado: con un exe que se cierra al abrir, volvió a la 0.30.0 y avisó; con uno bueno, confirmó y borró el anterior a los 7 s
- [x] **«Verificar» revisa por huella también Minecraft** (mejora 2 del plan): bibliotecas, el juego, el índice y los sonidos contra las huellas de Mojang (`launch::known_hashes`), y su Java contra el espejo o la lista de Mojang. Antes un archivo dañado con el mismo tamaño pasaba. Lo normal sigue siendo por tamaño: lo que se baja ya se comprueba antes de quedar con su nombre. Probado: con un sonido, una biblioteca, el juego y `jvm.dll` dañados sin cambiar su tamaño, los 4 se repusieron (del espejo en modo escuela en 47 s; de Mojang en 13 s) y Forge 1.20.1 abrió; una instalación desde Drive cortada a la mitad (2392 archivos) no quedó como instalada, al reintentar terminó en 30 s y «Verificar» no encontró nada dañado
- [x] **Si Mojang o un loader no responden**, el aviso trae «Activar modo escuela», y «Crear instancia» lo ofrece en una franja encima de las versiones (que ya salen de Drive)
- [x] **Firebase medido** (mejora 5): cada minuto una línea en `datos/logs/firebase.csv` (escuchas, conexiones, reconexiones, eventos, bytes) y cuánto tardó en sincronizar. Con 7 amigos y 8 conversaciones: 24 conexiones, sincroniza en 3.1 s, ~1.3 KB por minuto
- [x] Pruebas automáticas en Windows (`.github/workflows/windows.yml`): en cada PR a `main` (los de Mac) y a mano; compila sin advertencias y corre las pruebas
- [x] **Logo nuevo** en la ventana, el exe, la barra de tareas, la bandeja y los avisos. El exe seguía con el viejo porque Cargo no volvía a meter el icono (`build.rs` ahora lo vigila); los accesos directos que ya había lo toman la primera vez que se abre la versión nueva (`accesos::refresh_icons`)
- [ ] Fuera del espejo: Forge para jugar en 1.8 a 1.12.1 (instalador antiguo que Parabellum no maneja; sus servidores sí están) y Forge 1.13.2, 1.14.2 y 1.14.3 (su instalador falla)
- [ ] *Prueba:* Juan en la escuela: activar el modo escuela, crear una instancia y un servidor, instalar un modpack; con un amigo, «está escribiendo…» y mandar con la red lenta

### 0.29.0: gestor de descargas, imágenes al chat, descargas más rápidas y Minecraft desde Drive (pedido de Juan)
- [x] **Gestor de descargas** (`descargas.rs`): cada instalación es una tarea (crear instancia, modpacks de Modrinth o de un archivo, mods y sus actualizaciones, el catálogo de Drive, instancias compartidas, importar de otro launcher, verificar y lo que falta al jugar). La flecha de arriba dice cuánto va y cuántas hay, y abre el panel: progreso, velocidad, cuánto falta, Cancelar, Reintentar (aprovecha lo ya bajado), Abrir la instancia y Limpiar terminadas
- [x] **Cola**: los modpacks, versiones y paquetes de Drive esperan su turno (1 a la vez por defecto; 2 o 3 en las opciones del panel). Ya no sale «Espera a que termine…»: en el catálogo se pueden pedir varios y quedan «En cola». Los mods y lo que falta al jugar no esperan
- [x] **Límite de velocidad** (opciones del panel: 1 a 20 MB/s) para todas las descargas juntas, para no dejar sin internet la casa
- [x] Cancelar es seguro: las descargas lo notan entre pedazos y fallan como si se cayera la conexión, así cada instalación limpia lo suyo como siempre (un modpack a medias no deja instancia)
- [x] Si cierras la ventana de crear o subir un modpack, sigue en Descargas y al terminar no te saca de donde estás; soltar otro .mrpack mientras tanto lo pone en la cola
- [x] **Pegar una imagen** en cualquier parte de Mensajes (no solo en el cuadro de texto). Lo que la ventana no sabe pegar (un archivo copiado en el Explorador, un .heic, un dibujo de Paint) lo lee Parabellum del portapapeles
- [x] **Arrastrar una imagen** al chat también desde el navegador, Discord o un gestor del portapapeles (en Windows llegaban sin archivo y se perdían: `plataforma/windows/arrastre.rs`), con el aviso «Suelta para mandarla a…»; como siempre, sale la vista previa antes de mandarla
- [x] **Descargas más rápidas** (pedido de Juan: «a veces está muy lenta, en algunos dispositivos»). Medido en su red: Drive da 23 MB/s con una conexión (partirla no ayudó), pero los miles de archivos chicos de Mojang esperan ~100 ms cada uno: con 8 a la vez salían ~80 por segundo y con 32, ~180. Ahora van **32 a la vez y los grandes primero**; la lista de sonidos y la de Java se piden a la vez
- [x] **Las descargas largas ya no se cortan**: el cliente cortaba cualquier descarga que pasara de 300 s (un modpack de Drive de 1 GB en una red lenta se cortaba y podía fallar); ahora solo si deja de llegar algo 60 s
- [x] **Menos trabajo para la ventana**: el progreso llegaba una vez por archivo (miles por instalación) y en una PC lenta trababa la interfaz; ahora como mucho ~10 por segundo (una etapa nueva y el final, siempre)
- [x] **Minecraft desde Google Drive** (pedido de Juan): al crear una instancia (o completar una versión), si su base está en el catálogo se baja de Drive, solo lo que falte en el PC (por partes con el zip remoto; si falta casi todo, el zip completo). Si Drive falla, de Mojang. En «Crear instancia» las versiones de Drive salen con «En Drive», también donde Mojang está bloqueado
- [x] **Drive por partes, 6 a la vez**: completar una versión o actualizar un modpack de Drive pedía cada pedazo del zip de uno en uno (Drive tarda ~0.3 s en responder cada petición): faltando 200 archivos sueltos de 26.3 tardaba 58 s; ahora 12 s
- [x] Probado en la ventana real con una carpeta vacía: 26.3 desde Drive (679 MB a ~42 MB/s, lista en 40 s); borrando 200 sonidos, la siguiente instancia bajó solo lo que faltaba (26 MB) y quedó completa (5147 de 5147); 1.20.4 de Mojang con Java 17 (785 MB) en 29 s
- [x] Subir un modpack a Drive ya subía su base (`base-minecraft-<versión>`): en Drive están 1.20.1, 1.21.1, 26.1.2, 26.2 y 26.3
- [x] **«Subir versiones de Minecraft»** (catálogo de Drive, solo quien publica): elegir versiones (botón «Las más jugadas»), ver el espacio libre del Drive y cuánto ocupan (~650 MB cada una); cada una se baja de Mojang, se empaqueta, se sube y se borra de este PC. Es una tarea de Descargas
- [x] Probado en la ventana real (carpeta de prueba): con 1 MB/s la segunda instalación esperó en cola, al cancelar la primera no quedó carpeta y la segunda terminó (111 MB, con «Abrir»); con 2 MB/s el promedio fue 1.99 MB/s; pegar con Ctrl+V una imagen copiada y un archivo copiado abre la vista previa
- [x] **Menos trabones del servidor** (los «teletransportes»): en la telemetría de Juan, buena parte de los trabones de 50–125 ms caían **cada 300 s exactos** (el autoguardado escribía cada chunk esperando al disco) y el resto al entrar o ir a zonas nuevas (generar terreno). Ahora los servidores van con `sync-chunk-writes=false` (los nuevos y, una vez, los que ya había: `servidor::apply_tuning`)
- [x] **Pregenerar el mundo** (Ajustes del servidor, solo con mods): agrega Chunky de Modrinth para su loader y versión, enciende o reinicia el servidor y genera 500, 1000 o 2000 bloques alrededor del spawn, con el avance en vivo (porcentaje, cuánto falta, chunks por segundo) y «Pausar». Probado: «Prueba fabric» generó 4225 chunks en 100 s y quedó anotado
- [x] **Doctor: «Buscar el mod culpable»** (`culpable.rs`), cuando ya se probó y el error no dice qué mod es: primero sin ningún mod (si tampoco abre, no es un mod) y después por mitades, desactivando junto con cada mitad los mods que dependen de ella (para no fallar por una dependencia que falta); con 150 mods, unas 8 pruebas. Al final todo queda como estaba (un registro deshace lo desactivado aunque se cierre Parabellum a la mitad: al jugar o volver a buscar). «Desactivar» quita al culpable y a los que lo necesitan; se puede detener
- [x] El doctor ya no espera 8 minutos cuando Fabric o Quilt no arrancan por mods incompatibles (dejan su ventana de error abierta): reconoce «Incompatible mods found!» en el registro. Y antes de cada prueba aparta el latest.log anterior (`set_aside_log`): al probar seguido leía el de la prueba anterior y se equivocaba
- [x] Probado: en «Fabric 1.21.1» con 7 mods (uno que depende de otro y «Roto», que pide Minecraft 1.19.2) encontró a «Roto» en 4 pruebas y 3 minutos; una búsqueda cortada a la mitad dejó 4 mods desactivados y la siguiente los restauró
- [x] De Mac (PR #13, Diego): un Java que se cortó a medias ya no se da por instalado (el juego no abría: «missing server JVM»); «Jugar» lo completa. Y un Java cortado en casa y completado desde Drive en la escuela vuelve a contar (`java::package_installed`), sin pedir Mojang; una versión ya no se da por completa con un Java de reemplazo (la 1.20.1 se quedaba con el 25, y Forge 1.20.1 no abre con él), ni se empaqueta con él
- [x] De Mac (PR #14, Diego): en Mac, una base de Drive (armada en Windows) se completa con Mojang (su Java y sus nativos). Probado en Windows con Mojang bloqueado (solo Google): 1.20.1 desde Drive en 38 s y al menú en 14 s, sin pasar por Mojang
- [ ] *Prueba:* Juan: instalar dos modpacks seguidos (el segundo en cola), cancelar uno y reintentarlo; arrastrar una imagen desde el navegador al chat; subir unas versiones a Drive y crear una instancia de una de ellas en la escuela; pregenerar SESE y jugar con un amigo para ver si siguen los trabones

### 0.28.0: el panel del juego con lo mismo que la ventana (pedido de Juan)
- [x] **Imágenes:** al adjuntar, pegar o soltar una imagen sale una vista previa (ya reducida) y se confirma antes de mandarla
- [x] **Mensajes:** el punto de en línea (verde) o jugando (morado) en las conversaciones directas y en la cabecera
- [x] **Panel del juego (tecla O), rehecho para parecerse a la ventana:** mismos iconos y colores; Amigos con filtros, búsqueda, favoritos, ⋯ y grupos; Mensajes con favoritos, «Escribir a un amigo», punto de en línea, fotos de grupo, días, citas, reacciones (con quién reaccionó), herramientas al pasar el ratón, menú del clic derecho con reacciones, barra «Respondiendo a…», crear grupos, miembros (sacar, pasar el grupo) y cambiar el nombre; Capturas con «Abrir carpeta»
- [x] Lo que Inter no tiene (emojis en los mensajes) ya no sale como cuadritos: se escribe con una fuente del sistema
- [x] El puente le da al juego favoritos, miembros, dueño y foto de cada grupo, y las reacciones; operaciones nuevas: reaccionar, responder, favoritos, crear/editar/salir de grupos, sacar y pasar el grupo, foto del grupo y abrir la carpeta de capturas
- [x] **Capturas «vacías» de un amigo:** el código buscaba bien la carpeta (`screenshots` del juego); lo más probable es que en su laptop F2 cambie el brillo y haga falta Fn + F2. Ahora el panel lo explica y tiene «Abrir carpeta»
- [ ] *Prueba:* Juan con un amigo dentro del juego: responder, reaccionar, crear un grupo y sacar a alguien

### 0.27.1: Servidores y Hostear en una sola página (pedido de Juan)
- [x] «Hostear partida» ya no es un botón aparte: es la sección «Tus servidores» de Servidores (con «Crear servidor», el aviso de tu mundo abierto y lo de playit.gg). El punto verde de «servidor encendido» pasó al botón de Servidores
- [x] Tres secciones con su encabezado: Tus servidores, De tus amigos y Guardados. Cada una es una lista de filas compactas (unos 52 px: icono, nombre, de quién o qué instancia, estado, jugadores y su botón) en vez de tarjetas grandes
- [x] De tus amigos: arriba a donde puedes entrar ahora; los servidores apagados a los que te invitaron, al final y plegados («Ver 3 apagados»)

### 0.27.0: amigos, mensajes y grupos (pedido de Juan)
- [x] **Inicio sin tarjetas**: se quitaron «Tu servidor», «Amigos» y «Novedades»; arriba solo quedan los primeros pasos mientras falten
- [x] **Página de Amigos** en la barra (con aviso de solicitudes): agregar por nombre, solicitudes, la lista con filtros (todos, en línea, jugando) y búsqueda, y en cada amigo Unirse, Invitar (si tienes un servidor encendido o tu mundo abierto), Mensaje y Más; a la derecha, los grupos. También se agrega desde Mensajes (botón junto a «Nuevo grupo»)
- [x] **Responder** a un mensaje (pasar el ratón › ↩, o clic derecho): la cita va arriba de la burbuja y lleva al original; si lo borran dice «Mensaje borrado». Esc cancela
- [x] **Reaccionar** con 👍 ❤️ 😂 😮 😢 🔥: una por persona (otra la reemplaza, la misma la quita); se ve al momento y al pasar el ratón dice quién
- [x] **Grupos con foto y dueño**: quien lo crea es el dueño y solo él cambia nombre, foto y miembros (puede sacar a alguien o pasarle el grupo). Si el dueño sale, el grupo pasa a otro. Los grupos de antes no tienen dueño: quien los edite primero queda como dueño. Clic en el avatar del grupo: sus miembros
- [x] **Invitaciones del juego en la ventana**: antes se veía el texto crudo; ahora una tarjeta con «Unirse»
- [x] **«Reportar problema»** (Ajustes › Acerca de, menú ⋯ de la instancia y el doctor): versión, sistema, instancia, error, lo que dice el doctor y lo último del P2P, con lo que escribe el jugador arriba. Va por chat a quien da soporte (`supportName` en config-integrada.json: Juan) o a quien elijas; sin sesión, se copia
- [x] Reglas de Firebase (`firebase-reglas.json`): `reply` en los mensajes, `reactions/`, `owner` y `photo` en los grupos, y solo el dueño cambia el grupo. Probadas con el emulador oficial de Firebase (`docs/probar-reglas.py`): 50 casos (crear como la 0.26 y la 0.27, editar, sacar, pasar el grupo, responder, reaccionar, borrar). De paso se cerró un hueco: cualquier miembro podía esconderle una conversación a otro
- [x] **Juan: publicar las reglas ANTES de publicar la 0.27** (publicadas el 2026-10-13)
- [x] Después: responder y reaccionar también desde el panel del juego (0.28.0)
- [ ] *Prueba:* con compañeros en sus casas: P2P, mensajes, invitar y «Unirse» desde el juego, y el teclado del panel en 26.3

### Después
- [x] Doctor: buscar el mod culpable por mitades cuando el error no lo dice (0.29.0)
- [ ] Resto del pulido: esqueletos de carga en Explorar, telemetría del servidor viva (números suaves y minigráficas), botones que se hunden, cambio de tema con un círculo
- [ ] Mod: pulsar O justo después de F2 abre esa captura lista para mandar
- [ ] Mac (Diego): `docs/mac.md` y `docs/mac-cambios.md`

### Fase 9: social
- [x] **Servidor de sesión local**: mundos LAN/e4mc de amigos sin "Invalid session" (el juego arranca usándolo; e4mc siempre por su relé)
- [x] **Inicio de sesión con Google** (la sesión guardada por la versión C# sigue valiendo) y **nombres únicos**: una cuenta de Google reserva los nombres de sus cuentas de Parabellum
- [x] Perfil guardado en la cuenta (nombre, UUID, memoria y otras cuentas): se recupera en cualquier PC
- [x] **Amigos**, solicitudes y presencia: a qué juegan, en qué servidor y si tienen el mundo abierto (leyendo el registro del juego, sin mods)
- [x] **Unirse** al mundo de un amigo con un clic (e4mc por Internet o LAN), con el mismo modpack
- [x] Bandeja del sistema: con la sesión iniciada, la X deja Parabellum junto al reloj
- [x] Cuentas de Parabellum: cambiar de jugador con un clic y UUID fijo
- [x] Avisos de Windows (solicitud nueva, un amigo abrió su mundo) sin instalador: Parabellum se registra solo para el usuario; se pueden apagar en Comportamiento
- [ ] *Prueba:* iniciar sesión, agregarse con un compañero y entrar a su mundo con Unirse
- [x] Subir a Drive directamente desde Parabellum (0.16): ver «Publicar en Drive» más abajo

### Fase 10: chat
- [x] **Mensajes** con amigos y **grupos** (crear, agregar amigos, cambiar nombre, salir); página Mensajes con conversaciones recientes y marca de no leídos
- [x] **Imágenes** reducidas en la propia base de datos (máx. 1280 px y ~330 KB en JPEG): botón, pegar una captura o soltarla en la ventana; se guardan aparte de los mensajes y se descargan solo al verlas
- [x] Avisos de Windows de mensajes nuevos (salvo que esa conversación esté a la vista)
- [x] Reglas de Firebase: solo los miembros leen y escriben cada conversación; solo se puede meter a amigos; tamaños limitados
- [x] Reglas publicadas en Firebase (Juan, 2026-10-10)
- [ ] *Prueba:* chatear con un compañero
- [ ] (Más adelante) overlay dentro del juego

### 0.24.1: el mod gasta lo mínimo y se aparta si da problemas
- [x] Medido: dibujar el panel entero tarda 7–11 ms (solo cuando algo cambia: ratón sobre un botón, escribir, llega un mensaje); un cuadro sin cambios 0.0005 ms; el tick con el panel cerrado 0.0001 ms. En el juego (Fabric 1.21.1, menú principal, 1 min): procesador 9.7 % sin el mod y 9.7 % con él; memoria igual
- [x] Al cerrar el panel se sueltan su imagen y su textura (8–24 MB según la ventana)
- [x] Si el juego se cae y el error es del mod (código suyo en la pila o el cargador que no pudo con él), Parabellum ya no lo pone en esa instancia hasta la siguiente versión (`sin-panel.txt` en .lanzador)

### 0.24.0: Parabellum dentro del juego (pedido de Juan: «como Essential»)
- [x] **Panel dentro de Minecraft** (tecla O): amigos (unirse con la ruta más rápida, invitar, mensaje, solicitudes, quitar), mensajes (chats y grupos, enviar, editar con clic derecho o flecha arriba, borrar; fotos; tarjetas de invitación con «Unirse»), capturas (F2 → mandarlas a un chat). Avisos arriba a la derecha (mensajes, invitaciones, solicitudes, amigo que abrió su mundo); dentro del juego reemplazan a los de Windows
- [x] Hecho como mod pequeño (`mod/`): núcleo en Java 8 que dibuja todo con Java2D (igual en todas las versiones) y una capa por versión. Parabellum lo pone al jugar y lo quita al cerrar; se apaga en Ajustes › Comportamiento
- [x] **Puente** (`puente.rs`): 127.0.0.1 con clave; estado de amigos y chats al momento, mensajes, imágenes, editar/borrar, solicitudes, invitar (al servidor encendido —entra a la lista blanca— o al mundo abierto) y unirse (P2P, red local o playit.gg; avisa si es otro modpack)
- [x] **18 .jar**: Fabric 1.16.5, 1.18.2, 1.19–1.19.2, 1.20–1.20.1, 1.21–1.21.1, 1.21.2–1.21.4, 1.21.5, 1.21.6–1.21.8, 1.21.9–1.21.10, 1.21.11, 26.1.x, 26.2, 26.3 (y Quilt); Forge 1.16.5, 1.18.2, 1.19–1.19.2, 1.20–1.20.1; NeoForge 1.20.1 y 1.21–1.21.1. Elegidas por popularidad en Modrinth. Cada una abierta en el juego con el panel; clics y escritura probados en 1.16.5, 1.21.1, 1.21.10 y 26.2; «Unirse» entró a un servidor 1.21.1
- [x] Arreglo: Minecraft 1.18.2 no arrancaba (bibliotecas duplicadas por sistema: la de Mac tapaba a la de Windows y se perdían los nativos de LWJGL)
- [x] `--abrir-juego <instancia> [segundos]`; JDK 8/17/21/25 en ~/.jdks solo para compilar el mod (no van en Parabellum)
- [ ] *Prueba:* con un compañero (mensajes, invitar y unirse desde el juego). En 26.3 (usa SDL3) revisar a mano el teclado del panel
- [ ] Después: NeoForge 1.21.11/26.x si alguien lo usa, Fabric 1.20.2–1.20.6

### 0.23.0: conexiones por servidor y editar/borrar mensajes (pedido de Juan)
- [x] **Conexiones** en cada servidor (tarjetas con interruptor): **P2P** y **red local** activadas (no muestran ninguna IP; «Unirse» elige), **playit.gg** desactivado (dirección pública para quien no usa Parabellum; al activarlo se vincula si hace falta). `servidor::Connections` en parabellum-servidor.json; solo se publica y se deja pasar lo activado
- [x] Hostear sin la tarjeta de playit.gg ni el interruptor «Abrir mi mundo»: el mundo abierto en LAN se comparte solo con los amigos (P2P y red local); por playit.gg, en Ajustes › Comportamiento (desactivado). playit.gg solo aparece arriba mientras se vincula o si algo falta
- [x] Página del servidor: telemetría en una fila (jugadores, TPS, encendido, procesador, memoria) y las tres conexiones debajo; el permiso del firewall vive en «Red local». Sin la ruta IPv6 aparte (el P2P ya usa IPv6)
- [x] La consola del servidor apagado ya no se llena de «The time is…» (las preguntas del TPS no se guardan)
- [x] **Mensajes:** editar los propios de texto (lápiz, clic derecho o flecha arriba con la caja vacía; «editado») y borrarlos para todos (también fotos e instancias compartidas, que no se editan). Reglas de Firebase: solo el autor cambia o borra; al editar no cambia la hora
- [ ] Juan: publicar `firebase-reglas.json` (mensajes e imágenes) en la consola de Firebase
- [ ] Siguiente: interfaz dentro del juego (mod ligero tipo Essential: chats, capturas, invitaciones y amigos)

### 0.22.0: conexión directa entre PCs, P2P (pedido de Juan: «como Essential, pero no premium»)
- [x] **P2P dentro de Parabellum** (`p2p.rs`, biblioteca iroh): QUIC sobre UDP con perforación de NAT, también por IPv6; si las redes no lo permiten, pasa por un relevo de n0 (gratis, para uso personal, con límite de velocidad). Funciona en todas las versiones y loaders sin mods: quien se une abre un puerto en 127.0.0.1 y Minecraft entra ahí; cada conexión del juego es un flujo QUIC hacia el anfitrión
- [x] Cada Parabellum publica su dirección P2P en la presencia (identidad nueva en cada arranque; sin cambiar las reglas de Firebase). El anfitrión solo deja pasar a sus amigos y solo a lo que está encendido (un servidor o el mundo abierto); la lista blanca sigue decidiendo quién juega
- [x] «Unirse» (servidores y mundos abiertos, también desde el chat y la lista de amigos) suma la ruta P2P a la carrera: red local, P2P, IPv6 y playit.gg. En el detalle: «P2P directo» o «P2P por relevo» con su tiempo. Un servidor sin playit.gg ya se puede unir por P2P
- [x] `--probar-p2p <puerto> [segundos]`; registro en `datos/logs/p2p.log`. Probado en este PC: entrar a «Prueba vanilla» por el túnel P2P (directo)
- [ ] *Prueba:* Juan y un amigo en otra casa (los dos con la 0.22): ver en el detalle si sale «P2P directo» y el ping
- [ ] Después: playit.gg opcional (hostear sin vincular nada), relevo propio cerca si hace falta, mod ligero para la interfaz dentro del juego (invitar/unirse sin salir de Minecraft)

### 0.21.0: menos lag al jugar en los servidores (pedido de Juan)
- [x] Medido: el túnel de playit.gg de Juan entra por un centro de datos a 140–180 ms (el agente se conecta a uno a 16 ms); su servidor respondía en ~665 ms por playit.gg y 3–14 ms en su PC, sin «Can't keep up» (no era la PC). Golpear entidades espera al servidor; romper bloques no (por eso solo se notaba al pelear)
- [x] **«Unirse» elige la ruta más rápida:** el anfitrión publica, además de playit.gg, su dirección de la misma red y su IPv6 pública; al unirse se prueban todas a la vez (ping del juego) y se usa la más rápida que responda y sea el mismo servidor (mismo máximo de jugadores). También con los mundos abiertos. En el detalle de Servidores: cada ruta con su tiempo
- [x] **«Jugar» para el anfitrión** en la página del servidor: entra por 127.0.0.1 (sin túnel)
- [x] **Conexión directa:** botón «Permitir» que agrega una regla del firewall de Windows (puertos 25565–25600, pide administrador una vez); plataforma::firewall_open / open_firewall
- [x] **TPS del servidor** (con «time query gametime» cada ~6 s, sirve en todas las versiones, sin ensuciar la consola) y aviso si se atrasa; se publica a los amigos y se ve en el detalle
- [x] ~~Siguiente: abrir el puerto del módem con UPnP~~ → en su lugar, P2P (0.22.0; iroh también usa UPnP/PCP del módem cuando hay)

### 0.20.1: un proyecto, dos plataformas (pedido de Juan)
- [x] Lo que depende del sistema se movió a `src-tauri/src/plataforma/` con un contrato común (`plataforma/mod.rs`): `windows/` (lo que ya funcionaba: DPAPI, job de Windows, ANSI/8.3 de Java, WIC, DnsQuery, PlaySound, avisos, accesos directos, Explorador…) y `macos/` (lo fácil hecho con herramientas de macOS y lo demás con `TODO(mac)`). Lo compartido ya pregunta por la plataforma donde Mojang distingue sistemas (reglas `os`, nativos, Java de Mojang, separador del classpath)
- [x] Dependencias de Windows en `[target.'cfg(windows)'.dependencies]`; guía para el amigo que hará la versión de Mac: `docs/mac.md` (estructura, reglas, compilar, lista de tareas por prioridad); `docs/windows.md`; `.github/workflows/mac.yml` (cargo check/test en macos-14, ramas `mac/…` o a mano)
- [x] Windows sigue igual: juego Fabric 1.21.1 al menú, servidor vanilla, ping, imagen TIFF, sonido y las 76 pruebas
- [ ] Mac: ver la lista de tareas de docs/mac.md

### 0.20.0: servidores con su icono e imágenes de cualquier formato (pedido de Juan)
- [x] **Servidores:** tarjetas compactas (icono del propio servidor, nombre grande, quién lo hostea, estado y jugadores); clic → detalle con el mensaje (MOTD) en colores, modpack, versión, jugadores, dirección, respuesta y «Unirse»/«Jugar». Igual en «Guardados». El estado sale del servidor con el Server List Ping del juego (ping.rs; direcciones de playit.gg por su registro SRV con DnsQuery_W), cada 20 s con la página abierta; el último icono se recuerda para cuando está apagado. `--ping <dirección>`
- [x] **Imágenes del chat de cualquier formato:** lo que la ventana no abre (HEIC del celular, TIFF, ICO, JPEG XR…) lo convierte Windows (WIC, imagen.rs) a PNG; también al soltarla y en los iconos. `--convertir-imagen`
- [x] Crear instancia: «Con mis favoritos» ya no se encima (el texto tomaba el estilo del interruptor)
- [x] Tus compañeros seguían sin imágenes ni «compartir instancia» porque en Drive estaba la 0.17.1: hay que publicar esta versión en Drive (Ajustes › Acerca de › Publicar esta versión) y se actualizan solos

### 0.19.1: los favoritos siguen a la cuenta (pedido de Juan)
- [x] favoritos.rs: con la sesión de Google, los favoritos (conversaciones y mods) viven en favorites/{uid} de la base de datos y manda lo que hay ahí (quitar uno en otro PC también lo quita aquí); la primera vez se sube lo de este PC, salvo que fueran de otra cuenta (PC compartido: favorites_owner). Los iconos locales no suben (solo los https de Modrinth)
- [x] Juan publicó las reglas con «favorites» (2026-10-11). Probado con su cuenta (`--probar-favoritos` y en la ventana): sube los del PC la primera vez, marcar/quitar llega a la cuenta, los cambios de otro PC se ven al momento y las reglas rechazan tipos, iconos, campos y cuentas ajenas

### 0.19.0: favoritos (pedido de Juan)
- [x] **Conversaciones y grupos favoritos:** estrella en el encabezado del chat, clic derecho en la lista o «Agregar a favoritos» en el menú de un amigo. Salen primero: «Favoritos» y «Recientes» en Mensajes, y una sección «Favoritos» arriba en el panel de amigos y en grupos. Se guardan en ajustes.json de este PC (favorite_chats), sin tocar las reglas de Firebase
- [x] **Mods favoritos** (también resource packs y shaders de Modrinth): estrella en su página de Explorar y en la lista de Contenido. Botón «Favoritos» en Contenido para instalar los que falten en esa instancia (cada uno con su versión compatible y dependencias; los que no son compatibles se dicen). Al crear una instancia, «Con mis favoritos» los instala de una vez (en vanilla, solo los resource packs)

### 0.18.0: más usos del chat e icono del servidor (pedido de Juan)
- [x] **Imágenes en el chat:** el botón de imagen y pegar una captura no funcionaban: la ventana bloqueaba las imágenes abiertas como blob: (faltaba en la CSP de tauri.conf.json). Lo mismo rompía el icono del servidor («Esta imagen no se pudo abrir») y el de las instancias
- [x] **Primer mensaje con alguien:** la conversación nueva todavía no llegaba de Firebase y la lista la cerraba; ahora se abre al momento con el nombre del amigo (state.pendingChat) hasta que llega
- [x] **Compartir instancias por el chat** (compartir.rs): viaja una receta corta en un mensaje de texto (cabe en las reglas actuales de Firebase): modpack de Modrinth (proyecto y versión), paquete del catálogo de Drive (su id) o, si se hizo a mano, versión de Minecraft, loader y las versiones exactas de sus mods, resource packs y shaders de Modrinth (8 letras cada una; hasta ~200). Tarjeta con «Instalar» (o «Abrir» si ya la tienes). Botón en el chat y «Compartir con un amigo» en el menú de la instancia. Probado: una instancia hecha a mano se rearma con los mismos archivos (`--probar-receta`)
- [x] **Amigos:** clic en un amigo abre su conversación; arriba, lo que está jugando y «Unirse» (a su mundo abierto o al servidor donde está)
- [x] **Editor del icono del servidor:** cualquier imagen (PNG, JPG, GIF, WEBP…): se recorta, se mueve arrastrando, se acerca (rueda o barra), opción pixelado, vista previa como en la lista de servidores; se guarda en 64×64 PNG

### 0.17.1: vincular playit.gg sin trabas y actualización obligatoria (pedido de Juan)
- [x] **playit.gg:** al aceptar en el navegador, la página de playit.gg espera a que el agente se conecte; Parabellum solo lo encendía al compartir algo, así que la página se quedaba «esperando», no volvía a Parabellum y no había dirección. Ahora, al aceptar: el agente se conecta en ese momento, se crea la dirección y Parabellum vuelve al frente con ella lista
- [x] Mientras se vincula se ve el paso que dice playit.gg (abrir la página, aceptar, terminando); si playit.gg dejó a alguien en otra página tras iniciar sesión, «Abrir de nuevo» lo regresa. Hasta 30 min para crear la cuenta
- [x] Cuenta nueva sin correo verificado (playit.gg no da dirección): estado propio «Verifica tu correo» con «Ya lo verifiqué», también en la página del servidor encendido; «Reintentar» vuelve a dejar todo listo sin apagar el servidor ni volver a vincular
- [x] **Actualización obligatoria:** al abrir Parabellum (y cada 30 min) se busca una versión nueva; si la hay, una ventana que no se cierra (ni con Esc ni con clic afuera ni con atajos) pide actualizar: «Actualizar ahora» (descarga, reemplaza el exe y se reinicia) o «Cerrar Parabellum». Con un servidor encendido espera a que se apague (se cerraría al reiniciar)

### 0.17.0: ajustes del servidor como en Aternos, sonido de mensajes (pedido de Juan)
- [x] **Vista previa del chat al día:** al escribir varias rutas a la vez, Firebase avisa con claves como «meta/lastText» y Parabellum las guardaba tal cual; ahora las pone en su sitio (firebase.rs). Por lo mismo tampoco llegaban los avisos de mensajes nuevos
- [x] **Sonido de mensajes** (sonido.rs): dos notas cortas generadas por Parabellum y tocadas por Windows, aunque se esté jugando o Parabellum esté junto al reloj; no si esa conversación está a la vista; una vez si llegan varios seguidos. Interruptor en Ajustes › Comportamiento; el aviso de Windows del mensaje sale sin su sonido
- [x] **Ajustes del servidor** (opciones.rs): tarjetas por grupos (Jugadores, Partida, Reglas del juego, Mundo, Paquete de recursos, Servidor) con interruptores, +/− y listas; debajo, la línea real de server.properties o el /gamerule. Se guarda al cambiar. Solo se muestran las que existen en la versión (las que escribió el servidor o las que tiene su mundo). No se pueden tocar online-mode, puertos, rcon ni level-name
- [x] Reglas del juego: hasta 1.21.10 en level.dat › Data › GameRules (texto, keepInventory); desde 26.1 en data/minecraft/game_rules.dat (minecraft:keep_inventory, 0/1). Encendido: /gamerule al momento; apagado: se escriben en el mundo (comprobando que se lee igual antes de reemplazarlo); sin mundo: pendientes hasta que el servidor esté listo
- [x] Desde 1.21.11 PvP, Nether y monstruos son reglas del juego: el interruptor de PvP de antes no hacía nada en 26.x (también al crear el servidor)
- [x] «Reiniciar» cuando hay cambios que el servidor solo lee al arrancar; icono del servidor (server-icon.png) y MOTD con colores y formato (§)

### 0.16.1: herramientas de publicar solo para el creador (pedido de Juan)
- [x] En Ajustes › Acerca de los demás solo ven la versión y «Buscar actualizaciones». La carpeta para Drive, «Conectar Drive», «Publicar esta versión» y «Abrir mi Drive» (también los botones de Drive al exportar) se ven solo en el PC de quien publica: el que ya tiene Drive conectado para publicar (datos/drive-publicar.bin). En otro PC se activan (o se ocultan) con siete clics seguidos en la versión; queda en datos/creador.txt. Los textos para los demás ya no hablan de «tu Drive»

### 0.16.0: publicar en Drive y batería de modpacks
- [x] **Publicar en Drive desde Parabellum** (fin de la fase 9): en Ajustes › Acerca de, «Conectar Drive» pide en el navegador el permiso de escribir en Drive (aparte del inicio de sesión normal: solo lo da quien publica; queda cifrado con DPAPI en datos/drive-publicar.bin). Después:
  - Al exportar un modpack «Para Drive o USB», el botón **Subir a Drive** sube el modpack (zip, icono y .json) y Minecraft de esa versión a la carpeta del catálogo, con la misma estructura (versión/modpack/…); las carpetas que falten se crean
  - «Publicar esta versión» de Parabellum sube la carpeta _parabellum y manda a la papelera de Drive las versiones anteriores: quien abra Parabellum se actualiza
  - Lo que ya está igual en Drive (mismo tamaño y huella MD5) no se vuelve a subir; lo que cambió se reemplaza en el mismo archivo (el enlace no cambia). Subida en trozos de 8 MB que sigue donde iba si se corta la conexión; los .json van al final para que el catálogo no anuncie un paquete a medio subir
  - Si la cuenta no puede escribir en la carpeta del catálogo, lo dice (compartirla como Editor con esa cuenta)
- [x] *Prueba de Juan:* publicar versiones de Parabellum en Drive desde Parabellum (0.17.1 y 0.20.1, 2026-10-12; los compañeros se actualizan solos)
- [x] **Batería de modpacks** (los 11 más descargados de Modrinth que cubren cada loader y versión: instalar como el botón Instalar, revisar como antes de jugar y abrir hasta el menú con capturas). Antes de los arreglos: 1 no abría, 1 «abría» sin sus mods y 4 tenían avisos falsos que habrían detenido el juego. Ahora los 11 llegan al menú y ninguno muestra avisos falsos:

  | Modpack | Loader | Resultado |
  |---|---|---|
  | Fabulously Optimized | Fabric 26.2 | Menú en 79 s |
  | Cobblemon Official | Fabric 1.21.1 | Menú en 129 s |
  | Zombie Invade 100 Days | Forge 1.20.1 | Menú en 208 s |
  | Better MC BMC2 | Fabric 1.20.1 | Menú en 125 s (aviso falso corregido) |
  | The Pixelmon Modpack | NeoForge 1.21.1 | Menú en 51 s |
  | Create+ | Forge 1.19.2 | Menú en 83 s (2 avisos falsos corregidos) |
  | Better MC BMC5 | NeoForge 1.21.1 | Menú en 87 s |
  | Prominence II | Fabric 1.20.1 | Menú en 161 s con sus 678 mods (antes abría con 4) |
  | Cave Horror Project | Forge 1.20.1 | Menú en 72 s |
  | Cobbleverse | Fabric 1.21.1 | Menú en 112 s |
  | Parasites: Reloaded | Forge 1.12.2 | Menú en 97 s (antes no abría) |

  Lo que se arregló (para todos los modpacks, no solo para estos):
  - **Rutas con «™», acentos o «ñ»**: Java lee su archivo de argumentos en la codificación de Windows (ANSI), no en UTF-8; con UTF-8 «Prominence™» llegaba roto y el juego abría otra carpeta sin mods (pasaría igual con una cuenta de Windows como «José»). Ahora va en ANSI y, si una letra no cabe, la ruta usa su nombre corto de Windows
  - **Java 8 (Minecraft hasta 1.16) no entiende el archivo de argumentos**: los argumentos van en la línea de comandos. Forge 1.12.2 ya abre
  - **Copias de un mod incluidas en otros (jar-in-jar)**: el doctor revisa la copia que usa el loader (la suelta en la carpeta, o la más nueva que encaja con el juego y con lo que piden los demás), y lee hasta cuatro niveles de jars dentro de jars (LambDynamicLights trae así sus bibliotecas). Lo que le falta a una biblioteca incluida no se da por perdido
  - **Correcciones de dependencias del modpack** (config/fabric_loader_dependencies.json de Fabric): se aplican como lo hace Fabric
  - **Mods repetidos en Forge/NeoForge 1.17+** ya no impiden abrir (el loader usa la copia más nueva): queda como nota con «quitar la vieja». Un mod de otro loader (un Geckolib de Fabric en Forge) es un aviso: el loader se lo salta
  - **Rangos de versión mal escritos** («[1.19.0,1.19.1,1.19.2)») ya no son un problema: Forge los acepta. Forge antes de 1.13 no lee mods.toml: ahí no se revisan
  - La pantalla de título también se reconoce por la revisión de Realms (1.20) para terminar la prueba antes
- [x] Forge 1.12.2 probado (Parasites: Reloaded)

### 0.15.2: Servidores más simple (pedido de Juan)
- [x] Página Servidores sin descripciones (solo «Servidores», «De tus amigos» y «Guardados»); el formulario de cuatro campos se cambió por un botón «Agregar servidor» que abre un diálogo con nombre, dirección e instancia (Enter guarda), para que no llame más la atención que los servidores de tus amigos

### 0.15.1: los modpacks abren a la primera (prueba de Juan con Better MC BMC4)
Instalando Better MC [FORGE] - BMC4 desde Modpacks en el PC de Juan (Iris Xe) salían avisos de mods que «faltaban» que el doctor no podía arreglar, y el doctor daba por bueno el juego aunque se cayera después. Lo que pasaba y lo que se hizo:
- [x] **Avisos falsos antes de jugar**: los mods «multiloader» traen mods.toml (Forge) y neoforge.mods.toml (NeoForge 1.21) con dependencias distintas y el doctor leía el de NeoForge incluso en Forge (Blended Compat «pedía 1.21», Immersive Lanterns «necesitaba» accessories, Sodium Options API reeses_sodium_options). Ahora cada loader lee el suyo: Forge y NeoForge < 1.20.5 mods.toml, NeoForge ≥ 1.20.5 neoforge.mods.toml
- [x] **Versiones como las compara Forge**: los rangos de mods.toml se comparan con las reglas de Maven (`ComparableVersion`), no de semver: «1.20.1-1.5.2-neoforge» cumple «[1.20.1,]» (The Aether)
- [x] **«Llega al menú» de verdad**: al terminar de cargar se sigue mirando 45 s (o hasta que FancyMenu anote la pantalla de título); un cierre en ese tiempo es un fallo. BMC4 se caía 16 s después de «Sound engine started» y el doctor decía que abría
- [x] **Cierres de Java en código nativo** (hs_err_pid…log): el doctor lee el marco que falló y el mod de la pila. El controlador de Intel (igxelpicd64.dll) se cae al preparar un shader de Twilight Forest (fallo conocido de los controladores recientes) → instala **TF-Intel-Fix** de Modrinth; con otros controladores explica cuál se cayó y con qué mod
- [x] **Pantallas de aviso dentro del juego**: Memory Settings («Less memory allocated than recommended») y Missing Mods Checker se detectan tras abrir; «Probar y arreglar solo» les da la memoria que pide el modpack o repone los archivos
- [x] **Crash Assistant** (lo trae BMC4) se relanza suelto para vigilar el juego: cuando Parabellum cierra el juego (fin de la prueba del doctor o «Detener») también se cierra, para que no aparezca su ventana de «crashed» cuando todo estaba bien
- [x] **Arreglos conocidos al instalar un modpack**: con gráficos Intel y Twilight Forest se agrega TF-Intel-Fix, y si trae Memory Settings la instancia recibe la memoria que pide para ese PC (su tabla, interpolada). Al jugar, si la memoria nunca se eligió a mano, también
- [x] *Prueba:* BMC4 (365 mods) desde Modpacks en este PC: antes, 5 avisos falsos y el juego se caía al cargar; ahora la revisión solo muestra el problema real (Twilight Forest con Intel), «Arreglar y jugar» lo arregla y el juego llega al menú de Better Minecraft sin pantallas de aviso; «Probar y arreglar solo» desde cero: instala TF-Intel-Fix, abre, ve el aviso de memoria y le da 7 GB en un solo intento

### 0.15.0: nombres que siguen a la cuenta, Forge/NeoForge y doctor nuevo (pedido de Juan)
- [x] **Los servidores siguen a la cuenta de Google, no al nombre**: si el anfitrión o un amigo invitado se cambia el nombre en Parabellum, la lista blanca se actualiza sola (con el servidor encendido se recarga en caliente), el permiso de administrador pasa al nombre nuevo (`deop` del anterior, `op` del nuevo) y su **inventario, posición, logros y estadísticas** pasan al UUID del nombre nuevo (si estaba conectado, al salir; si el nombre nuevo ya tenía cosas propias no se pisa nada). Los servidores de antes de la 0.15 reconocen al anfitrión por el administrador que había puesto Parabellum
- [x] **Nadie entra con el nombre de otro**: quien entra con el nombre del anfitrión o de un amigo invitado tiene que estar jugando desde Parabellum con esa cuenta (su Parabellum publica en la presencia con qué nombre juega); si no, se le saca a los pocos segundos con «Entra desde Parabellum con tu cuenta». Los invitados por nombre (sin cuenta) no se comprueban. Se puede apagar en Ajustes del servidor
- [x] **Mensajes en orden**: se ordenan por la hora del servidor de Firebase y no por la del reloj de cada PC (si el reloj de un amigo iba atrasado, su respuesta salía arriba de tus mensajes); los ids nuevos llevan la hora del servidor
- [x] **Forge y NeoForge sin confusiones**: Parabellum abre cada .jar para ver para qué loader es de verdad (mods.toml, neoforge.mods.toml, fabric.mod.json…) en vez de fiarse de la etiqueta de Modrinth; si una versión está mal etiquetada busca otra del mismo mod que sí sirva. Las dependencias que piden una versión exacta de otro loader se cambian por la buena. En Minecraft 1.20.1 se aceptan los de los dos (NeoForge 1.20.1 todavía era Forge) pero se prefiere el del loader de la instancia. Con Sinytra Connector también valen los de Fabric. La revisión de rangos de versión ahora también se hace al instalar en Forge y NeoForge (NeoForge no exige al pie de la letra el rango de Minecraft: se acepta la misma línea, p. ej. JEI «[1.21, 1.21.1)» en 1.21.1)
- [x] **El doctor reconoce mods de otro loader** ya instalados (uno de Forge en NeoForge 1.21, uno de Fabric en Forge…): ofrece cambiarlo por el bueno desde Modrinth o, si ya está, quitar la copia que sobra
- [x] **Doctor rediseñado**: arriba la imagen de la instancia, su estado («1 problema impide abrirlo», «Todo en orden», «Ya abre») y loader, versión y mods revisados; los hallazgos en una lista con su marca (impide abrir, posible causa, para saber), los archivos como fichas y la línea del registro aparte; el arreglo recomendado resaltado sin competir con el botón principal; «Probar y arreglar solo» muestra los pasos mientras trabaja. Los cierres con error también se explican aquí (ya no hay una ventana aparte)
- [x] *Prueba:* servidor vanilla de prueba con un cliente falso de Minecraft 1.21.1: al renombrar al anfitrión con el servidor encendido se recarga la lista blanca, el nombre viejo queda fuera (`not_whitelisted`), el nuevo entra y es administrador, su inventario pasó al nombre nuevo, y con la comprobación activa quien usa el nombre sin jugar desde Parabellum sale a los 10 s. NeoForge 1.21.1: JEI, Sophisticated Backpacks y Create se instalan con sus versiones de NeoForge; un JEI de Forge metido a mano lo detecta el doctor y «Probar y arreglar solo» lo quita y llega al menú
- [ ] Inicio de sesión con **Microsoft** (premium): falta que Microsoft apruebe a Parabellum para usar la API de Minecraft (registro de la app en Azure + solicitud; hoy casi no aprueban launchers independientes)
- [ ] **CurseForge** junto a Modrinth (buscar e instalar de los dos): en pausa por decisión de Juan (2026-10-12); haría falta la clave de API, que se solicita a CurseForge con un formulario

### 0.14.0: amigos, Inicio y Servidores (pedido de Juan)
- [x] **Servidores › De tus amigos**: los servidores a los que te invitaron, los mundos abiertos de tus amigos, los servidores públicos donde están jugando (Unirse abre el mismo modpack y entra) y los que hostean sin haberte invitado (la presencia dice qué hostea; la dirección solo la ven los invitados). Se actualiza solo
- [x] **Panel derecho = amigos**: por secciones (Jugando, En línea, Desconectados), botón de mensaje al pasar el mouse, y debajo los **grupos** de chat con su último mensaje. «Jugando como», «Primeros pasos» y «Novedades» salen del panel (la cuenta sigue en el botón de abajo a la izquierda)
- [x] **Inicio**: sin «Continuar jugando»; tarjetas de Tu servidor (encender/apagar), Amigos (quién está en línea y cuántas partidas abiertas) y Primeros pasos mientras falten (después, Novedades). Sin el panel, el contenido se centra en vez de quedar pegado a la izquierda
- [x] Menú del perfil (abajo a la izquierda) dentro de la ventana: se abre a un lado del botón; ningún menú se sale de la ventana
- [x] «Tus servidores» ya no parpadea: las listas solo se repintan si cambió algo y la animación de entrada solo sale la primera vez
- [x] **Mensajes y estados al momento**: las escuchas de Firebase se renuevan antes de que caduque el permiso (cada hora), reconectan enseguida tras un corte (antes podían esperar hasta 60 s) y no dañan letras con tilde ni emojis partidos entre trozos; al volver a la ventana se piden los datos al momento. Medido: lo escrito llega por la escucha en ~80 ms (`--probar-tiempo-real`)

### Fase 13: hostear partida (0.13.0)
- [x] **Página «Hostear partida»** con dos formas de jugar con amigos desde tu PC, sin abrir puertos del router y sin depender de un mod (sirve para vanilla, Fabric, Quilt, Forge y NeoForge):
  - **Crear un servidor** a partir de una instancia: se elige la instancia, el mundo (nuevo o una copia de uno de la instancia, con tu inventario y posición), la memoria, los argumentos de Java, el modo de juego, la dificultad, los jugadores y el PvP. El **EULA de Minecraft** se acepta marcándolo (sin eso se crea pero no enciende)
  - **Abrir mi mundo a amigos**: al abrir el mundo en LAN dentro del juego, Parabellum lo comparte solo y tus amigos pulsan «Unirse» (un solo Java: lo mejor en un PC de 8 GB)
- [x] **Dirección pública fija con playit.gg** (gratis): se vincula una vez desde Parabellum (la página de playit.gg se abre sola), su agente oficial firmado se baja la primera vez (4,9 MB, comprobando su huella) y corre oculto mientras haya algo compartido; la clave queda cifrada para tu usuario de Windows. Un único túnel con la misma dirección siempre: solo cambia a qué apunta (el servidor encendido o tu mundo)
- [x] **Servidor de verdad para cada loader**: vanilla y Fabric con sus servidores oficiales, Quilt y Forge/NeoForge con sus instaladores (también Forge antiguo); Java el de la instancia; banderas de Java para servidores sin reservar toda la memoria al arrancar
- [x] **Mods que no van en el servidor**: los que Modrinth marca como solo de cliente, los que Parabellum conoce y los que dependen de ellos se quedan fuera (BattleArmory: 43 de 99). Si aun así uno tumba el servidor al encender, el doctor lo encuentra («ModID», «Mod File» o la traza), lo quita y vuelve a encender solo
- [x] **Consola integrada** en vivo (con colores, historial de comandos y lo que hizo Parabellum), **jugadores en línea** (dar admin, quitar, echar), **memoria y procesador** del servidor y la memoria libre del PC, tiempo encendido; apagar con «stop» (guarda el mundo)
- [x] **Invitados**: lista blanca (servidor no premium con `online-mode=false`, `enforce-whitelist`), amigos de Parabellum con un clic o cualquier nombre de Minecraft. Los amigos invitados ven el servidor en su página Servidores con la **dirección actual** y «Unirse» (Firebase `servers/` e `invites/`, solo lo leen los invitados)
- [x] **Plan de memoria**: barra con Windows, tu juego, el servidor y lo libre, con consejo (bien, justo o no alcanza; en 8 GB recomienda «Abrir mi mundo» para modpacks grandes)
- [x] Con un servidor encendido, la X deja Parabellum junto al reloj; al salir se apagan guardando el mundo. El servidor y el agente van atados a Parabellum (si se cierra de golpe, no quedan abiertos)
- [x] *Prueba:* servidores de vanilla, Fabric (con un mundo dañado: el doctor ofrece empezar uno nuevo), Quilt y BattleArmory TACZ (Forge 1.20.1, 56 mods, mundo copiado con nombre chino): enciende en 33 s, `list` responde, se apaga guardando; con Oculus metido a propósito el servidor cae, Parabellum lo quita solo y enciende en el 2.º intento; invitados, lista blanca y ajustes en la ventana real, en español e inglés
- [x] 0.13.1 (tras la prueba de Juan con Fabulously Optimized 26.2): los mods que abren su propio túnel (e4mc, World Host) ya no van al servidor y, si un servidor los tiene, se quitan solos al encender (e4mc daba otra dirección en la consola y su 6.2.1 tumba los servidores de 26.2 cuando alguien entra: `NoSuchMethodError CommandSourceStack.hasPermission(int)`). En «Abrir mi mundo», aviso con botón para desactivar e4mc en las instancias que lo tengan. Botones y campos alineados (tarjeta de dirección, ajustes, consola, invitados) y el interruptor del EULA al crear ya no sale doble
- [x] 0.13.2 (pedido de Juan): «Hostear partida» compacta, sin título ni descripción: playit.gg en una tarjeta baja, «Abrir mi mundo a amigos» en una línea con su interruptor, botón «+ Crear servidor» y la lista igual. El panel de la derecha (jugador, amigos, novedades) se oculta y se muestra con un botón junto a «Ningún juego abierto» o con Ctrl+B; se recuerda en los ajustes
- [x] *Prueba de Juan:* ¡funciona! playit.gg vinculado, servidor encendido con dirección fija (cairo-take.tun.ply.gg, con registro SRV: no hace falta puerto)
- [ ] *Prueba:* entrar desde otra red con la dirección de playit.gg; invitar a un compañero y que entre con «Unirse» (las reglas de `servers/` e `invites/` ya están publicadas)

### Fase 12: doctor (0.12.0)
- [x] **Revisión antes de jugar**: lo que declara cada mod (fabric.mod.json y también mods.toml de Forge/NeoForge, con los mods que traen dentro) contra la instancia: dependencias que faltan, versión del loader o de Minecraft que no encaja, choques y mods repetidos. Si algo impide abrir, se ofrece «Arreglar y jugar» o «Jugar de todos modos»
- [x] **Después de un fallo** (también cuando Forge muestra su pantalla de error y sale con código 0): se lee el informe y los registros y se explica en palabras simples, con la línea que lo demuestra: argumento de Java que pide el propio mod, choque de módulos de Java, Java equivocado, memoria, configuración dañada, mod sospechoso (mixins, «Suspected Mods»), video
- [x] **Arreglos de un clic**: activar/desactivar un mod, instalar la dependencia desde Modrinth (comprobando el .jar), cambiar la versión del loader a una que cumpla todo, agregar el argumento de Java, otra versión de Java para la instancia (se baja la de Mojang), más memoria, mandar la configuración dañada a la papelera, reponer archivos
- [x] **Probar y arreglar solo**: abre el juego hasta el menú principal y lo cierra; si no llega, aplica los arreglos recomendados y repite (hasta 5 intentos)
- [x] «Actualizar» y «Actualizar todo» ya no instalan versiones que piden otro Forge/NeoForge, otra versión de Minecraft u otra dependencia (antes solo se revisaba Fabric)
- [x] *Prueba:* BattleArmory TACZ (Forge 1.20.1, 100 mods, el que falló a Juan): Probar y arreglar desactivó CustomSkinLoader y llegó al menú en el 2.º intento; «Actualizar todo» dejó fuera ParCool 4.0.1.2 (pide Forge 47.4.23) y Shoulder Surfing 5.2 (choca con Third Person Shooting) y el modpack sigue abriendo; en la ventana real: antes de jugar (Connector desactivado → «Activar»), después del fallo y desde el menú
- [ ] (Más adelante) buscar el mod culpable por mitades cuando el error no lo dice

### Fase 11: pulido (0.11.0)
- [x] **Sin instalador** (sigue portable): accesos directos en el Escritorio y en el menú Inicio desde Ajustes › Comportamiento; avisan si apuntan a otra carpeta
- [x] **Firma**: sin certificado de pago (decisión de Juan). El exe lleva su versión real, nombre y editor en sus propiedades; al publicar se deja `LEEME.txt` en `_parabellum` con los pasos de la primera vez y qué hacer con el aviso de SmartScreen (las actualizaciones automáticas no lo muestran)
- [x] **Idiomas**: español e inglés en toda la interfaz y en los mensajes del motor; la primera vez se usa el de Windows; Minecraft abre en ese idioma en las instancias nuevas (también en modpacks que traen inglés de EE. UU.)
- [x] **Accesibilidad**: foco visible, interruptores y tarjetas usables con el teclado, diálogos con el foco dentro, menús con flechas, avisos para lector de pantalla, contraste AA en los dos temas, tamaño de la interfaz 90–150 %
- [x] **Rendimiento**: la pestaña Contenido guarda lo leído de cada mod y carga los iconos al verse (355 mods: de 14 MB y segundos a 120 KB y ~0,1 s); en reposo, menos de 1 % de un núcleo
- [x] *Prueba:* todo con teclado real por CDP; recorrido de todas las páginas y diálogos en inglés sin textos en español; mensajes del motor en ambos idiomas; accesos directos creados y quitados; instancia nueva con `lang:es_mx`
- [ ] (Si algún día se quiere) certificado de firma de código
