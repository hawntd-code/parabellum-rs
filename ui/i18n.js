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
  "Guarda tus servidores no premium y entra con un clic: Parabellum abre la instancia elegida y se conecta directamente.": "Save your non-premium servers and join with one click: Parabellum opens the chosen instance and connects directly.",
  "Nombre (p. ej. UniversoCraft)": "Name (e.g. UniversoCraft)",
  "Dirección (p. ej. mc.universocraft.com)": "Address (e.g. mc.universocraft.com)",
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
  "Un aviso de Windows cuando alguien te manda solicitud de amistad o un amigo abre su mundo (solo si Parabellum no está al frente).": "A Windows notification when someone sends you a friend request or a friend opens their world (only if Parabellum is not in front).",
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
  "Busca si hay una versión nueva en tu Drive o en la carpeta paquetes.": "Checks whether there is a new version in your Drive or in the packages folder.",
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
  "El juego se cerró con un error": "The game closed with an error",
  "Esto es lo que dice el registro del juego:": "This is what the game log says:",
  "Ver registro completo": "See full log",
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

