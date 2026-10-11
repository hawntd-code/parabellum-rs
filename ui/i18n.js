// Idioma de la interfaz: español o inglés.
// En el código: t("texto en español", "text in English") devuelve el del idioma elegido.
// Los textos fijos de index.html se traducen solos al cargar con HTML_EN (clave: el texto en español).
// El idioma se recuerda también aquí (localStorage) para saberlo antes de pintar nada; el que manda es
// el de Ajustes (datos/ajustes.json): si no coinciden, la ventana se recarga una vez.

const LANG_KEY = "parabellum-idioma";
let LANG = (() => {
  try {
    return localStorage.getItem(LANG_KEY) === "en" ? "en" : "es";
  } catch {
    return "es";
  }
})();
document.documentElement.lang = LANG;

function t(es, en) {
  return LANG === "en" ? en : es;
}

/** Para fechas y números: "es-MX" o "en-US". */
function locale() {
  return LANG === "en" ? "en-US" : "es-MX";
}

/** Ajusta el idioma al de Ajustes. Devuelve true si hay que recargar para aplicarlo. */
function adoptLanguage(code) {
  const wanted = code === "en" ? "en" : "es";
  try {
    localStorage.setItem(LANG_KEY, wanted);
  } catch {
    // Sin almacenamiento se sigue igual: lo que se arma al momento sale en el idioma correcto.
  }
  if (wanted === LANG) return false;
  LANG = wanted;
  document.documentElement.lang = LANG;
  // Una sola recarga por sesión, por si el almacenamiento no guardara nada.
  try {
    if (sessionStorage.getItem("idioma-recargado") === wanted) return false;
    sessionStorage.setItem("idioma-recargado", wanted);
  } catch {
    return false;
  }
  return true;
}

const HTML_EN = {
  "Atrás": "Back",
  "Adelante": "Forward",
  "Inicio": "Home",
  "Descargando": "Downloading",
  "Hay una versión nueva de Parabellum": "There is a new version of Parabellum",
  "Actualizar": "Update",
  "Ir a tu biblioteca": "Go to your library",
  "Ningún juego abierto": "No game open",
  "Minimizar": "Minimize",
  "Maximizar": "Maximize",
  "Cerrar": "Close",
  "Explorar": "Explore",
  "Servidores": "Servers",
  "Mensajes": "Messages",
  "Nueva instancia (N)": "New instance (N)",
  "Ajustes": "Settings",
  "Tu perfil": "Your profile",
  "Continuar jugando": "Continue playing",
  "Biblioteca": "Library",
  "Buscar": "Search",
  "Nueva instancia": "New instance",
  "Jugadas recientemente": "Recently played",
  "Fecha de creación": "Date created",
  "Nombre": "Name",
  "Sin grupos": "No groups",
  "Versión de Minecraft": "Minecraft version",
  "Ninguna instancia coincide con la búsqueda.": "No instance matches the search.",
  "Bienvenido a Parabellum": "Welcome to Parabellum",
  "¿Listo para empezar a jugar?": "Ready to start playing?",
  "Crear una instancia": "Create an instance",
  "Pulsa": "Press",
  "para crear una instancia rápido": "to create an instance quickly",
  "Buscar mods…": "Search mods…",
  "Ordenar": "Sort",
  "Relevancia": "Relevance",
  "Más descargados": "Most downloaded",
  "Más seguidos": "Most followed",
  "Más nuevos": "Newest",
  "Actualizados": "Recently updated",
  "Cargar más": "Load more",
  "De dónde salen": "Where they come from",
  "Volver a leer": "Reload",
  "Instalar en": "Install to",
  "Todas": "All",
  "Instalar": "Install",
  "Descripción": "Description",
  "Versiones": "Versions",
  "Galería": "Gallery",
  "p. ej. UniversoCraft": "e.g. UniversoCraft",
  "p. ej. mc.universocraft.com": "e.g. mc.universocraft.com",
  "Dirección": "Address",
  "Agregar servidor": "Add server",
  "Busca si hay una versión nueva de Parabellum.": "Checks whether there is a new version of Parabellum.",
  "Publicar directo en Drive": "Publish straight to Drive",
  "Conectar Drive": "Connect Drive",
  "Subir a Drive": "Upload to Drive",
  "Los demás se actualizarán solos al abrir Parabellum.": "The others will update on their own when they open Parabellum.",
  "Agregar": "Add",
  "Todavía no tienes servidores guardados.": "You have no saved servers yet.",
  "Nuevo grupo": "New group",
  "Más": "More",
  "Enviar una imagen": "Send an image",
  "Escribe un mensaje…": "Write a message…",
  "Enviar (Enter)": "Send (Enter)",
  "Jugar": "Play",
  "Detener": "Stop",
  "Ajustes de la instancia": "Instance settings",
  "Más opciones": "More options",
  "Contenido": "Content",
  "Registro": "Log",
  "Servidor": "Server",
  "Detalles": "Details",
  "Mundos": "Worlds",
  "Actualizar todo": "Update all",
  "Abrir carpeta": "Open folder",
  "Agregar contenido": "Add content",
  "Filtrar líneas (p. ej. ERROR)": "Filter lines (e.g. ERROR)",
  "Seguir el final": "Follow the end",
  "Copiar": "Copy",
  "Carpeta": "Folder",
  "Entrar directo a un servidor": "Join a server directly",
  "Escribe la dirección de un servidor no premium y el juego se abrirá conectado a él.": "Type the address of a non-premium server and the game will open connected to it.",
  "p. ej. mc.universocraft.com": "e.g. mc.universocraft.com",
  "Jugar en el servidor": "Play on the server",
  "Jugando como": "Playing as",
  "Cuenta no premium": "Non-premium account",
  "Primeros pasos": "First steps",
  "Amigos": "Friends",
  "Inicia sesión con Google para agregar amigos, ver a qué están jugando y entrar a sus mundos con un clic.": "Sign in with Google to add friends, see what they are playing and join their worlds with one click.",
  "Iniciar sesión con Google": "Sign in with Google",
  "Nombre de tu amigo": "Your friend's name",
  "Enviar solicitud de amistad": "Send friend request",
  "Novedades": "What's new",
  "Parabellum, ahora en Rust": "Parabellum, now in Rust",
  "Un launcher nuevo desde cero: más ligero, más rápido y con un diseño renovado. Crea instancias de cualquier versión de Minecraft.": "A new launcher from scratch: lighter, faster and with a fresh design. Create instances of any Minecraft version.",
  "Octubre 2026": "October 2026",
  "Hostea partidas con tus amigos": "Host games with your friends",
  "Leer el EULA": "Read the EULA",
  "Grupos": "Groups",
  "De tus amigos": "From your friends",
  "Pausa › Abrir en LAN": "Pause › Open to LAN",
  "Ocultar el panel (Ctrl+B)": "Hide the panel (Ctrl+B)",
  "Crea un servidor con tu modpack o abre tu mundo a tus amigos, con una dirección fija y sin abrir puertos. Lo encuentras en «Hostear partida».": "Create a server with your modpack or open your world to your friends, with a fixed address and without opening ports. You will find it in «Host a game».",
  "Suelta para agregar": "Drop to add",
  "Mods, resource packs, shaders y mundos: cada uno va a su lugar.": "Mods, resource packs, shaders and worlds: each one goes to its place.",
  "Crear instancia": "Create instance",
  "Elige cómo empezar": "Choose how to start",
  "Configuración personalizada": "Custom setup",
  "Empieza desde cero eligiendo la versión de Minecraft y el loader.": "Start from scratch by choosing the Minecraft version and the loader.",
  "Desde un modpack de Drive": "From a Drive modpack",
  "Elige un modpack de tu carpeta de Google Drive. Funciona también en la escuela.": "Choose a modpack from your Google Drive folder. It also works at school.",
  "Subir un modpack": "Upload a modpack",
  "Instala un .mrpack o un paquete .zip de Parabellum desde tu PC o una USB.": "Install a .mrpack or a Parabellum .zip package from your PC or a USB drive.",
  "Importar de otro launcher": "Import from another launcher",
  "Copia una instancia de CurseForge, Prism o MultiMC con sus mods, mundos y opciones.": "Copy a CurseForge, Prism or MultiMC instance with its mods, worlds and options.",
  "Buscando instancias de CurseForge, Prism y MultiMC…": "Looking for CurseForge, Prism and MultiMC instances…",
  "Elegir otra carpeta…": "Choose another folder…",
  "Buscar versión (p. ej. 1.21)": "Search version (e.g. 1.21)",
  "Versión de Fabric": "Fabric version",
  "Cancelar": "Cancel",
  "Visualización": "Display",
  "Apariencia": "Appearance",
  "Comportamiento": "Behavior",
  "Cuenta": "Account",
  "Perfil": "Profile",
  "Instancias": "Instances",
  "Instalaciones de Java": "Java installations",
  "Recursos": "Resources",
  "Acerca de": "About",
  "Tema": "Theme",
  "Cómo se ve Parabellum.": "How Parabellum looks.",
  "Oscuro": "Dark",
  "Claro": "Light",
  "Sistema": "System",
  "Idioma": "Language",
  "El de Parabellum y el de Minecraft en las instancias nuevas.": "Parabellum's, and Minecraft's in new instances.",
  "Tamaño de la interfaz": "Interface size",
  "Letras y botones más grandes o más pequeños. También con Ctrl + y Ctrl − (Ctrl 0 vuelve al normal).": "Bigger or smaller text and buttons. Also with Ctrl + and Ctrl − (Ctrl 0 goes back to normal).",
  "Minimizar al abrir el juego": "Minimize when the game opens",
  "Parabellum se minimiza cuando Minecraft empieza a abrirse.": "Parabellum minimizes when Minecraft starts opening.",
  "Seguir junto al reloj al cerrar": "Stay next to the clock when closed",
  "Con la sesión de Google iniciada, la X deja Parabellum junto al reloj para que tus amigos te vean y puedan entrar a tu mundo. Para cerrarlo del todo: clic derecho en su icono › Salir.": "When signed in with Google, the X leaves Parabellum next to the clock so your friends can see you and join your world. To close it completely: right-click its icon › Quit.",
  "Avisos de amigos": "Friend notifications",
  "Un aviso de Windows cuando te escriben, te mandan solicitud de amistad o un amigo abre su mundo.": "A Windows notification when someone messages you, sends you a friend request or a friend opens their world.",
  "Sonido de mensajes": "Message sound",
  "Actualiza Parabellum": "Update Parabellum",
  "Cerrar Parabellum": "Close Parabellum",
  "Actualizar ahora": "Update now",
  "Hay cambios que se aplican al reiniciar el servidor.": "Some changes apply when the server restarts.",
  "Reiniciar": "Restart",
  "Icono del servidor": "Server icon",
  "Cambiar el nombre": "Change the name",
  "Cambiar el mensaje de la lista de servidores": "Change the server list message",
  "Suena cuando te escriben, aunque estés jugando o Parabellum esté junto al reloj.": "Plays when someone messages you, even while you play or Parabellum is next to the clock.",
  "Acceso directo en el Escritorio": "Desktop shortcut",
  "Para abrir Parabellum sin buscar su carpeta. Sigue siendo portable: no se instala nada.": "To open Parabellum without looking for its folder. It is still portable: nothing is installed.",
  "Acceso directo en el menú Inicio": "Start menu shortcut",
  "Aparece al buscar «Parabellum» en Inicio.": "It shows up when you search «Parabellum» in Start.",
  "Nombre de jugador": "Player name",
  "Guardado": "Saved",
  "Con tu nombre se genera tu identidad (UUID), igual que en los servidores no premium. Si juegas con amigos, usen nombres distintos.": "Your identity (UUID) is generated from your name, just like on non-premium servers. If you play with friends, use different names.",
  "Usar un UUID fijo": "Use a fixed UUID",
  "Solo si tus mundos o un servidor te conocen por otro UUID (por ejemplo, el que tenías en la versión anterior de Parabellum).": "Only if your worlds or a server know you by another UUID (for example, the one you had in the previous version of Parabellum).",
  "El UUID tiene 32 letras y números (0–9, a–f), con o sin guiones.": "The UUID has 32 letters and numbers (0–9, a–f), with or without dashes.",
  "Otras cuentas": "Other accounts",
  "Añadir cuenta": "Add account",
  "Cuenta de Google": "Google account",
  "Java detectados": "Detected Java",
  "Parabellum descarga automáticamente el Java que pide cada versión de Minecraft.": "Parabellum automatically downloads the Java each Minecraft version needs.",
  "Buscar actualizaciones": "Check for updates",
  "Carpeta para Drive": "Folder for Drive",
  "Todavía no eliges una": "You have not chosen one yet",
  "Cambiar": "Change",
  "Publicar esta versión": "Publish this version",
  "Prepara la carpeta _parabellum con este Parabellum para que la subas a tu Drive. Los demás se actualizarán solos al abrirlo.": "Prepares the _parabellum folder with this Parabellum for you to upload to your Drive. Everyone else will update on their own when they open it.",
  "Qué cambió (opcional, lo verán al actualizar)": "What changed (optional, shown when updating)",
  "Publicar en la carpeta": "Publish to the folder",
  "Abrir mi Drive": "Open my Drive",
  "Memoria": "Memory",
  "Ancho de la ventana del juego": "Game window width",
  "En píxeles, al abrir Minecraft.": "In pixels, when Minecraft opens.",
  "Alto de la ventana del juego": "Game window height",
  "Carpeta de Parabellum": "Parabellum folder",
  "Abrir": "Open",
  "Cambiar icono": "Change icon",
  "Quitar icono": "Remove icon",
  "Memoria propia": "Own memory",
  "Usar una cantidad distinta a la general solo en esta instancia.": "Use a different amount than the general one only in this instance.",
  "Argumentos de Java": "Java arguments",
  "Opcional, p. ej. -XX:+UseZGC": "Optional, e.g. -XX:+UseZGC",
  "Separados por espacios. Solo si sabes lo que haces.": "Separated by spaces. Only if you know what you are doing.",
  "Guardar": "Save",
  "Exportar modpack": "Export modpack",
  "Para Drive o USB": "For Drive or USB",
  "Completo: se instala sin internet, como en la escuela. Minecraft y Java van en un paquete aparte que comparten todos los modpacks de esa versión.": "Complete: it installs without internet, like at school. Minecraft and Java go in a separate package shared by all modpacks of that version.",
  "Ligero: los mods se descargan de Modrinth al instalarlo. Sirve en Prism, Modrinth App y otros launchers.": "Light: the mods are downloaded from Modrinth when installing it. It works in Prism, Modrinth App and other launchers.",
  "Versión": "Version",
  "Descripción (opcional)": "Description (optional)",
  "Para qué es, qué trae…": "What it is for, what it includes…",
  "Qué incluir": "What to include",
  "Exportar…": "Export…",
  "Nombre del grupo": "Group name",
  "Crear grupo": "Create group",
  "Es el nombre que verán en el juego y tus amigos. Tus mundos e instancias no cambian.": "It is the name the game and your friends will see. Your worlds and instances do not change.",
  "UUID fijo (opcional)": "Fixed UUID (optional)",
  "Vacío: el que sale de su nombre": "Empty: the one from their name",
  "Añadir y usar": "Add and use",
  "Jugar de todos modos": "Play anyway",
  "Lo que dice el registro": "What the log says",
  "Arreglar lo recomendado": "Fix the recommended",
  "Probar y arreglar solo": "Test and fix on its own",
  "Hostear partida": "Host a game",
  "Juega con tus amigos desde tu PC sin abrir puertos del router. Parabellum usa playit.gg (gratis) para darte una dirección pública que no cambia.": "Play with your friends from your PC without opening router ports. Parabellum uses playit.gg (free) to give you a public address that does not change.",
  "Crear un servidor": "Create a server",
  "Un servidor aparte con el modpack de una instancia. Sigue encendido aunque cierres el juego, pero usa más memoria.": "A separate server with an instance's modpack. It stays on even if you close the game, but it uses more memory.",
  "Abrir mi mundo a amigos": "Open my world to friends",
  "En el juego: Pausa › Abrir en LAN. Parabellum lo comparte solo y tus amigos pulsan «Unirse». No usa memoria extra.": "In the game: Pause › Open to LAN. Parabellum shares it on its own and your friends press «Join». It uses no extra memory.",
  "Compartir mi mundo abierto por playit.gg": "Share my open world through playit.gg",
  "Tus servidores": "Your servers",
  "Todavía no tienes servidores. Crea uno con «Crear un servidor».": "You have no servers yet. Create one with «Create a server».",
  "Encender": "Start",
  "Apagar": "Stop",
  "Consola": "Console",
  "Jugadores": "Players",
  "Escribe un comando (p. ej. say Hola, time set day, op TuAmigo)": "Type a command (e.g. say Hello, time set day, op YourFriend)",
  "Enviar": "Send",
  "Invitados": "Guests",
  "Solo ellos (y tú) pueden entrar: el servidor tiene lista blanca. Tus amigos de Parabellum ven la dirección actual en su página Servidores.": "Only they (and you) can join: the server has a whitelist. Your Parabellum friends see the current address on their Servers page.",
  "Nombre de Minecraft de alguien sin Parabellum": "Minecraft name of someone without Parabellum",
  "En el servidor ahora": "On the server now",
  "Los cambios se aplican la próxima vez que enciendas el servidor.": "Changes apply the next time you start the server.",
  "Mensaje en la lista de servidores": "Message in the server list",
  "Memoria del servidor": "Server memory",
  "Modo de juego": "Game mode",
  "Dificultad": "Difficulty",
  "Jugadores máximos": "Max players",
  "Distancia de visión": "View distance",
  "Distancia de simulación": "Simulation distance",
  "Jugadores pueden pelear entre ellos (PvP)": "Players can fight each other (PvP)",
  "Permitir volar (evita que los mods con alas o mochilas echen al jugador)": "Allow flying (prevents mods with wings or jetpacks from kicking the player)",
  "Solo se entra desde Parabellum con la cuenta de cada quien (nadie puede usar el nombre de otro)": "Players can only join from Parabellum with their own account (nobody can use someone else's name)",
  "Acepto el EULA de Minecraft": "I accept the Minecraft EULA",
  "No hacen falta en el servidor (son para gráficos, cámara o menús) o lo tumbaban. Tus amigos pueden seguir usándolos en su juego.": "The server does not need them (they are for graphics, camera or menus) or they crashed it. Your friends can keep using them in their game.",
  "Servidores a los que te invitaron: la dirección se actualiza sola cada vez que tu amigo lo enciende.": "Servers you were invited to: the address updates on its own every time your friend starts it.",
  "Guardados": "Saved",
  "Crear servidor": "Create server",
  "Instancia": "Instance",
  "Nombre del servidor": "Server name",
  "Mundo": "World",
  "Semilla (opcional)": "Seed (optional)",
  "Acepto el": "I accept the",
  "EULA de Minecraft": "Minecraft EULA",
  "Mojang pide aceptarlo para tener un servidor. Puedes crear el servidor sin aceptarlo, pero no encenderlo.": "Mojang asks you to accept it to run a server. You can create the server without accepting it, but not start it.",
  "Para encender un servidor, Mojang pide aceptar el acuerdo de licencia de Minecraft (EULA).": "To start a server, Mojang asks you to accept the Minecraft license agreement (EULA).",
  "Leer el EULA de Minecraft": "Read the Minecraft EULA",
  "Lo leí y lo acepto": "I have read it and I accept it",
  "Aceptar y encender": "Accept and start",
};

/** Traduce los textos fijos de la página (y sus title, placeholder y aria-label). */
function translateStatic(root = document.body) {
  if (LANG !== "en" || !root) return;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => (node.parentElement?.closest("script, style") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
  });
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const key = node.nodeValue.replace(/\s+/g, " ").trim();
    const english = key && HTML_EN[key];
    if (english) node.nodeValue = node.nodeValue.replace(/\S[\s\S]*\S|\S/, english);
  }
  for (const element of root.querySelectorAll("[title], [placeholder], [aria-label]")) {
    for (const attribute of ["title", "placeholder", "aria-label"]) {
      const value = element.getAttribute(attribute);
      if (value && HTML_EN[value.trim()]) element.setAttribute(attribute, HTML_EN[value.trim()]);
    }
  }
}

