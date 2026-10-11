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
- [ ] (Fase 9) Subir a Drive directamente desde Parabellum, con la sesión de Google

### Fase 8: Forge, NeoForge e importación
- [x] **Instancias Forge y NeoForge** con su instalador oficial: bibliotecas y "processors" (las herramientas que preparan el juego parcheado), también para Minecraft 26; cambiar su versión desde la instancia
- [x] Forge 1.17–1.20 y NeoForge arrancan sin el .jar original en el classpath (-DignoreList), como el launcher oficial
- [x] **Modpacks de Forge y NeoForge**: .mrpack, de Modrinth y de Drive; los paquetes para Drive incluyen lo que Forge genera y no nombra en su JSON (juego parcheado, fmlcore, mapeos)
- [x] **Importar de otro launcher**: CurseForge, Prism y MultiMC se encuentran solos (o se elige la carpeta); se copian mods, mundos y opciones y el loader lo instala Parabellum
- [x] *Prueba:* Better MC BMC4 (Forge 47.4.20, 444 mods) llega al menú; desde cero, Forge 1.20.1, NeoForge 1.21.1, Forge 26.1.2 y NeoForge 26.1.2 se instalan y llegan al menú; Better MC [BMC6] (NeoForge, 228 mods) importado de CurseForge llega al menú; paquetes de Forge y NeoForge exportados e instalados en una carpeta vacía sin internet llegan al menú
- [ ] (Opcional) CurseForge: requiere solicitar una clave de API y respetar qué autores permiten descargas externas
- [ ] Probar Forge 1.12.2 (su instalador más nuevo no tiene processors; los muy antiguos usan otro formato)

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
- [ ] Subir a Drive directamente desde Parabellum (pide más permisos de Drive que los de ahora)

### Fase 10: chat
- [x] **Mensajes** con amigos y **grupos** (crear, agregar amigos, cambiar nombre, salir); página Mensajes con conversaciones recientes y marca de no leídos
- [x] **Imágenes** reducidas en la propia base de datos (máx. 1280 px y ~330 KB en JPEG): botón, pegar una captura o soltarla en la ventana; se guardan aparte de los mensajes y se descargan solo al verlas
- [x] Avisos de Windows de mensajes nuevos (salvo que esa conversación esté a la vista)
- [x] Reglas de Firebase: solo los miembros leen y escriben cada conversación; solo se puede meter a amigos; tamaños limitados
- [x] Reglas publicadas en Firebase (Juan, 2026-10-10)
- [ ] *Prueba:* chatear con un compañero
- [ ] (Más adelante) overlay dentro del juego

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
- [ ] **CurseForge** junto a Modrinth (buscar e instalar de los dos): falta la clave de API, que se solicita a CurseForge con un formulario

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
