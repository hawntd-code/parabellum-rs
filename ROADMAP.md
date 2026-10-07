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

### Diseño
- [x] Interfaz al estilo de la app de Modrinth con identidad morada (tema oscuro y claro, fuente Inter)
- [x] Biblioteca, continuar jugando, crear instancia por tipo, ajustes por secciones, página Servidores

## ⏭️ Pendiente, en orden

### Fase 2: multijugador sin problemas de sesión
- [ ] Servidor de sesión local (ya probado en la versión C#): mundos LAN/e4mc sin "Invalid session"
- [ ] Prueba automática con servidor oficial en modo offline y online
- [ ] Prueba real en un servidor público (UniversoCraft)

### Fase 3: gestión de instancias
- [ ] Editar instancia: nombre, icono propio (imagen), memoria propia, argumentos de Java
- [ ] Duplicar y borrar (a la papelera)
- [ ] Registro del juego dentro del launcher y aviso con el motivo cuando el juego se cierra con error
- [ ] Pestaña Contenido: ver mods, resource packs, shaders y mundos de la instancia; activar/desactivar
- [ ] Saber qué juego está abierto siguiendo el proceso (sin consultar a Windows cada 3 s)

### Fase 4: Fabric (y Quilt)
- [ ] Instancias Fabric eligiendo versión de Minecraft y del loader (meta.fabricmc.net)
- [ ] *Prueba:* Fabric con un mod sencillo arranca

### Fase 5: explorar e instalar mods (base de datos de mods)
- [ ] Conexión con la API de **Modrinth** (abierta, sin clave, permite launchers de terceros)
- [ ] Buscar mods con filtros (versión, loader, categoría) y página de cada mod
- [ ] Instalar eligiendo automáticamente la versión compatible con la instancia
- [ ] **Dependencias**: las obligatorias se instalan solas, las opcionales se ofrecen, las incompatibles se bloquean
- [ ] Verificación por hash, detectar mods ya instalados y actualizar mods
- [ ] Resource packs, shaders y data packs
- [ ] (Opcional) CurseForge: requiere solicitar una clave de API y respetar qué autores permiten descargas externas

### Fase 6: modpacks
- [ ] Crear un modpack a partir de una instancia
- [ ] Exportar en formato `.mrpack` (estándar de Modrinth) y en zip completo
- [ ] Importar `.mrpack` y zip ("Subir un modpack")
- [ ] Instalar modpacks de Modrinth

### Fase 7: escuela y distribución
- [ ] Catálogo de Google Drive en Explorar (modpacks y versiones), para redes donde Mojang/Modrinth están bloqueados
- [ ] Subir a Drive versiones y modpacks desde casa con un botón
- [ ] Auto-actualización del launcher
- [ ] Configuración integrada en el exe

### Fase 8: Forge, NeoForge e importación
- [ ] Instancias Forge y NeoForge (instaladores con "processors")
- [ ] Importar instancias de CurseForge, Prism y de la versión C# (por ejemplo Better MC)

### Fase 9: social
- [ ] Inicio de sesión con Google y nombres únicos
- [ ] Amigos, solicitudes y presencia (a qué juegan, mundo abierto)
- [ ] Unirse al mundo de un amigo con un clic (e4mc por el relé, LAN)
- [ ] Bandeja del sistema y avisos

### Fase 10: chat
- [ ] Mensajes con amigos, grupos e imágenes
- [ ] (Más adelante) overlay dentro del juego

### Fase 11: pulido
- [ ] Instalador y firma del exe, idiomas, accesibilidad y rendimiento
