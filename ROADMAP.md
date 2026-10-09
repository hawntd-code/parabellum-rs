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
- [ ] *Prueba:* publicar las reglas nuevas en Firebase y chatear con un compañero
- [ ] (Más adelante) overlay dentro del juego

### Fase 11: pulido
- [ ] Instalador y firma del exe, idiomas, accesibilidad y rendimiento
