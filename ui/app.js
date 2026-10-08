"use strict";

// ======================================================================
// Puente con Rust (Tauri). Fuera de Tauri (navegador) se usan datos de ejemplo para revisar el diseño.
// ======================================================================
const tauri = window.__TAURI__;
const invoke = tauri ? tauri.core.invoke : demoInvoke;
const listen = tauri ? tauri.event.listen : () => Promise.resolve(() => {});
const appWindow = tauri ? tauri.window.getCurrentWindow() : null;

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

const state = {
  settings: null,
  instances: [],
  running: new Set(),
  page: "home",
  current: null,
  back: [],
  forward: [],
  sort: "played",
  group: "none",
  systemRam: 8,
  versions: null,
  selectedVersion: null,
  nameEdited: false,
  creating: false,
  tab: "content",
  kind: "mods",
  content: null,
  contentFor: null,
  log: null,
  editing: null,
  loader: "vanilla",
  /** Versiones de Minecraft que soporta cada loader: { fabric: { online, games, message } } */
  loaderGames: {},
  /** Versiones de cada loader por versión de Minecraft: { "fabric:1.21.1": { online, versions, recommended, installed } } */
  loaderVersions: {},
  explore: {
    kind: "mod",
    query: "",
    sort: "relevance",
    loader: "",
    version: "",
    categories: new Set(),
    categoryList: {},
    target: null,
    hits: [],
    total: 0,
    requestId: 0,
    installed: new Set(),
    installedVersions: new Set(),
    installing: new Set(),
    initialized: false,
    versionsLoaded: false,
  },
  projectId: null,
  projectData: null,
  projectVersions: null,
  /** Actualizaciones de Modrinth de la instancia abierta. */
  updates: { for: null, list: [], at: 0 },
  /** Actualizaciones que se dejaron fuera por romper algo: id de versión -> motivo. */
  heldBack: new Map(),
  /** Catálogo de Drive y de la carpeta paquetes. */
  drive: { data: null, loading: false, installing: null },
  launcherUpdate: null,
  version: "",
  driveUrl: null,
};

const pages = {
  home: { title: "Inicio", icon: "home", sidebar: true },
  explore: { title: "Explorar", icon: "compass", sidebar: false },
  project: { title: "Proyecto", icon: "compass", sidebar: false },
  servers: { title: "Servidores", icon: "server", sidebar: true },
  instance: { title: "", icon: "cubes", sidebar: false },
};

// ======================================================================
// Utilidades
// ======================================================================
function hashOf(text) {
  let h = 2166136261;
  for (const c of String(text)) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  return h >>> 0;
}

function ago(ms) {
  if (!ms) return "Nunca jugada";
  const minutes = (Date.now() - ms) / 60000;
  if (minutes < 1) return "Hace un momento";
  if (minutes < 60) return `Hace ${Math.floor(minutes)} min`;
  if (minutes < 1440) return `Hace ${Math.floor(minutes / 60)} h`;
  if (minutes < 2880) return "Ayer";
  if (minutes < 43200) return `Hace ${Math.floor(minutes / 1440)} días`;
  return new Date(ms).toLocaleDateString("es-MX");
}

const escapeHtml = (text) =>
  String(text ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
const icon = (name) => `<svg><use href="#i-${name}"/></svg>`;
const isTyping = () => ["INPUT", "SELECT", "TEXTAREA"].includes(document.activeElement?.tagName);

/** Arte de instancia: un bloque isométrico con colores elegidos por el nombre (como los iconos de Modrinth). */
const palettes = [
  { bg: ["#58a6ff", "#2563eb"], top: "#7ccf4f", topDark: "#5aa936", left: "#a8723f", right: "#7d522b" }, // pasto
  { bg: ["#c084fc", "#7c3aed"], top: "#e9d5ff", topDark: "#c4a5f5", left: "#8b5cf6", right: "#6d28d9" }, // amatista
  { bg: ["#f472b6", "#db2777"], top: "#f6c26b", topDark: "#e0a54a", left: "#c9883a", right: "#9c6427" }, // mesa de trabajo
  { bg: ["#34d399", "#0f9f6e"], top: "#f1f5f9", topDark: "#d4dbe3", left: "#8b9bb0", right: "#66778d" }, // nieve
  { bg: ["#fbbf24", "#ea8a0c"], top: "#ef4444", topDark: "#cf2e2e", left: "#f8fafc", right: "#cbd5e1" }, // TNT
  { bg: ["#6366f1", "#3730a3"], top: "#1f1b2e", topDark: "#151223", left: "#2d2742", right: "#211c33" }, // obsidiana
  { bg: ["#22d3ee", "#0e7490"], top: "#7dd3fc", topDark: "#4fb8ea", left: "#3b82f6", right: "#1d4ed8" }, // diamante
  { bg: ["#a78bfa", "#5b21b6"], top: "#4c1d95", topDark: "#3b1677", left: "#6d28d9", right: "#4c1d95" }, // gengar
];

function art(instance, size = "") {
  if (instance.icon) return `<div class="art has-icon ${size}"><img src="${instance.icon}" alt=""></div>`;
  const h = hashOf(instance.name);
  const p = palettes[h % palettes.length];
  const id = `g${h}`;
  // Pixeles de textura en la cara superior
  let pixels = "";
  for (let i = 0; i < 7; i++) {
    const x = (h >>> (i * 3)) & 3;
    const y = (h >>> (i * 2 + 5)) & 3;
    const px = 50 + (x - y) * 8;
    const py = 18 + (x + y) * 4.2;
    pixels += `<path d="M${px} ${py}l8 4.2-8 4.2-8-4.2z" fill="${p.topDark}"/>`;
  }
  const svg = `<svg viewBox="0 0 100 100">
      <path d="M50 18 82 34.8 50 51.6 18 34.8z" fill="${p.top}"/>${pixels}
      <path d="M18 34.8 50 51.6V86L18 69.2z" fill="${p.left}"/>
      <path d="M82 34.8 50 51.6V86l32-16.8z" fill="${p.right}"/>
      <path d="M18 34.8 50 51.6V58L18 41.2z" fill="${p.topDark}" opacity=".55"/>
      <path d="M82 34.8 50 51.6V58l32-16.8z" fill="${p.topDark}" opacity=".45"/>
    </svg>`;
  return `<div class="art ${size}" style="background: radial-gradient(circle at 30% 20%, rgba(255,255,255,.25), transparent 55%), linear-gradient(135deg, ${p.bg[0]}, ${p.bg[1]})" data-art="${id}">${svg}</div>`;
}

/** Cabeza de jugador pixelada (8×8) generada a partir del nombre. */
function drawHead(canvas, name) {
  const ctx = canvas.getContext("2d");
  const h = hashOf(name || "Jugador");
  const skins = ["#f2c7a5", "#e0ac83", "#c68a5e", "#a8714a", "#8d5a3b", "#5c3a26"];
  const hairs = ["#2b1d14", "#4a2f1e", "#7a4b2a", "#c79a4a", "#1d1a2b", "#6b3fa0", "#b5332e"];
  ctx.fillStyle = skins[h % skins.length];
  ctx.fillRect(0, 0, 8, 8);
  ctx.fillStyle = hairs[(h >> 3) % hairs.length];
  ctx.fillRect(0, 0, 8, 2);
  ctx.fillRect(0, 2, 1, (h >> 9) % 2 ? 3 : 1);
  ctx.fillRect(7, 2, 1, (h >> 10) % 2 ? 3 : 1);
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(1, 4, 2, 1);
  ctx.fillRect(5, 4, 2, 1);
  ctx.fillStyle = ["#3b82f6", "#22a06b", "#7c4a24", "#8b5cf6"][(h >> 6) % 4];
  ctx.fillRect(2, 4, 1, 1);
  ctx.fillRect(5, 4, 1, 1);
  ctx.fillStyle = "rgba(0,0,0,.28)";
  ctx.fillRect(3, 6, 2, 1);
}

function toast(message, kind = "info") {
  const element = document.createElement("div");
  element.className = `toast ${kind}`;
  element.innerHTML = `${icon(kind === "error" ? "alert" : "check")}<div>${escapeHtml(message)}</div>`;
  $("#toasts").appendChild(element);
  setTimeout(() => {
    element.classList.add("leaving");
    setTimeout(() => element.remove(), 220);
  }, kind === "error" ? 7000 : 3500);
}

function contextMenu(anchor, items) {
  $$(".menu").forEach((m) => m.remove());
  const menu = document.createElement("div");
  menu.className = "menu";
  menu.innerHTML = items.map((item, i) => `<button data-i="${i}" class="${item.danger ? "danger" : ""}">${icon(item.icon)}${escapeHtml(item.label)}</button>`).join("");
  document.body.appendChild(menu);
  const rect = anchor.getBoundingClientRect();
  menu.style.top = `${Math.min(rect.bottom + 6, innerHeight - menu.offsetHeight - 10)}px`;
  menu.style.left = `${Math.min(rect.right - menu.offsetWidth, innerWidth - menu.offsetWidth - 10)}px`;
  menu.onclick = (event) => {
    const button = event.target.closest("[data-i]");
    if (button) items[button.dataset.i].action();
    menu.remove();
  };
  setTimeout(() => document.addEventListener("click", () => menu.remove(), { once: true }), 0);
}

// ======================================================================
// Navegación (con historial Atrás/Adelante)
// ======================================================================
/** Lo que identifica la página actual: la instancia o el proyecto abiertos. */
function currentParam() {
  if (state.page === "instance") return state.current?.id ?? null;
  if (state.page === "project") return state.projectId ?? null;
  return null;
}

function navigate(page, param = null, record = true) {
  const same = state.page === page && currentParam() === param;
  if (record && !same) {
    state.back.push({ page: state.page, param: currentParam() });
    state.forward = [];
  }
  if (page === "instance") {
    const instance = state.instances.find((i) => i.id === param);
    if (!instance) return navigate("home", null, false);
    state.current = instance;
    renderInstance();
  } else {
    state.current = null;
  }
  state.page = page;
  if (page === "project") {
    if (state.projectId !== param || !state.projectData) loadProject(param);
    else renderProjectInstall();
  }
  if (page === "explore") enterExplore();
  for (const section of $$(".page")) {
    const active = section.id === `page-${page}`;
    section.classList.toggle("hidden", !active);
    if (active) {
      section.classList.remove("enter");
      void section.offsetWidth;
      section.classList.add("enter");
      section.scrollTop = 0;
    }
  }
  const railPage = page === "project" ? "explore" : page;
  for (const button of $$(".rail-btn[data-page]")) button.classList.toggle("active", button.dataset.page === railPage);
  $(".workspace").classList.toggle("no-sidebar", !pages[page].sidebar);
  $("#crumb-icon").innerHTML = `<use href="#i-${pages[page].icon}"/>`;
  $("#crumb-text").textContent =
    page === "instance" ? state.current.name : page === "project" ? (state.projectData?.title ?? pages.project.title) : pages[page].title;
  $("#nav-back").disabled = state.back.length === 0;
  $("#nav-forward").disabled = state.forward.length === 0;
  renderRail();
}

function goBack() {
  const previous = state.back.pop();
  if (!previous) return;
  state.forward.push({ page: state.page, param: currentParam() });
  navigate(previous.page, previous.param, false);
}

function goForward() {
  const next = state.forward.pop();
  if (!next) return;
  state.back.push({ page: state.page, param: currentParam() });
  navigate(next.page, next.param, false);
}

/** Primera vez en Explorar: versiones, la instancia por defecto y la primera búsqueda. */
function enterExplore() {
  const e = state.explore;
  fillTargetSelect($("#explore-instance"));
  if (e.initialized) return;
  e.initialized = true;
  loadExploreVersions();
  if (e.target === null) {
    const modded = [...state.instances]
      .filter((i) => i.loader !== "Vanilla")
      .sort((a, b) => (b.lastPlayed ?? b.created ?? 0) - (a.lastPlayed ?? a.created ?? 0))[0];
    if (modded) setExploreTarget(modded.id);
  }
  selectExploreKind(e.kind);
}

function applyTheme(theme) {
  const resolved = theme === "sistema" ? (matchMedia("(prefers-color-scheme: light)").matches ? "claro" : "oscuro") : theme;
  document.documentElement.dataset.theme = resolved;
  for (const button of $$("#theme button")) button.classList.toggle("active", button.dataset.theme === theme);
}

// ======================================================================
// Inicio: continuar jugando y biblioteca
// ======================================================================
const isNew = (i) => !i.lastPlayed && i.created && Date.now() - i.created < 3 * 86400e3;
const subtitle = (i) => `${i.loader} ${i.minecraftVersion}`;

function renderHome() {
  const empty = state.instances.length === 0;
  $("#home-filled").classList.toggle("hidden", empty);
  $("#home-empty").classList.toggle("hidden", !empty);
  if (empty) return;

  const recent = [...state.instances].sort((a, b) => (b.lastPlayed ?? b.created ?? 0) - (a.lastPlayed ?? a.created ?? 0)).slice(0, 3);
  $("#jump-list").innerHTML = recent
    .map((i, index) => {
      const running = state.running.has(i.id);
      return `<div class="jump" data-open="${escapeHtml(i.id)}" style="animation-delay:${index * 40}ms">
        ${art(i, "small")}
        <div class="grow">
          <div class="title">${escapeHtml(i.name)}${isNew(i) ? `<span class="badge-new">${icon("sparkle")}Nueva instancia</span>` : ""}</div>
          <div class="sub">${escapeHtml(subtitle(i))}<i></i>${running ? "Jugando ahora" : escapeHtml(ago(i.lastPlayed))}</div>
        </div>
        <button class="btn primary play" data-play="${escapeHtml(i.id)}" ${running ? "disabled" : ""}>${icon("play")}${running ? "En juego" : "Jugar"}</button>
        <button class="circle-btn" data-more="${escapeHtml(i.id)}" title="Más opciones">${icon("more")}</button>
      </div>`;
    })
    .join("");
  renderLibrary();
}

function renderLibrary() {
  const query = $("#library-search").value.trim().toLowerCase();
  const sorters = {
    played: (a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.name.localeCompare(b.name),
    created: (a, b) => (b.created ?? 0) - (a.created ?? 0),
    name: (a, b) => a.name.localeCompare(b.name, "es", { sensitivity: "base" }),
  };
  const visible = state.instances
    .filter((i) => !query || `${i.name} ${subtitle(i)}`.toLowerCase().includes(query))
    .sort(sorters[state.sort]);

  const groups = new Map();
  for (const instance of visible) {
    const key = state.group === "version" ? `Minecraft ${instance.minecraftVersion}` : state.group === "loader" ? instance.loader : "";
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(instance);
  }
  let index = 0;
  $("#library").innerHTML = [...groups]
    .map(([title, list]) => {
      const tiles = list
        .map((i) => {
          const running = state.running.has(i.id);
          return `<div class="tile" data-open="${escapeHtml(i.id)}" style="animation-delay:${Math.min(index++, 14) * 30}ms">
            <div style="position:relative">${art(i)}
              <button class="tile-play ${running ? "running" : ""}" data-play="${escapeHtml(i.id)}" title="${running ? "En juego" : "Jugar"}">${icon("play")}</button>
            </div>
            <div class="name">${escapeHtml(i.name)}</div>
            <div class="sub">${escapeHtml(subtitle(i))}</div>
          </div>`;
        })
        .join("");
      return `${title ? `<div class="group-title">${escapeHtml(title)}<span>${list.length}</span></div>` : ""}<div class="library-grid">${tiles}</div>`;
    })
    .join("");
  $("#no-results").classList.toggle("hidden", visible.length > 0);
}

function renderRail() {
  const recent = [...state.instances].sort((a, b) => (b.lastPlayed ?? b.created ?? 0) - (a.lastPlayed ?? a.created ?? 0)).slice(0, 6);
  $("#rail-instances").innerHTML = recent
    .map((i) => art(i).replace('class="art ', `title="${escapeHtml(i.name)}" data-open="${escapeHtml(i.id)}" class="art ${state.current?.id === i.id ? "active " : ""}`))
    .join("");
}

async function reloadInstances() {
  state.instances = await invoke("listar_instancias");
  renderHome();
  renderRail();
  renderSteps();
  renderServers();
}

// ======================================================================
// Detalle de instancia
// ======================================================================
function renderInstance() {
  const i = state.current;
  $("#detail-art").outerHTML = art(i, "xl").replace('class="art xl"', 'class="art xl" id="detail-art"');
  $("#detail-name").textContent = i.name;
  $("#detail-meta").innerHTML = `
    <span>${icon("cubes")}${escapeHtml(subtitle(i))}</span>
    <span>${icon("play-outline")}${escapeHtml(state.running.has(i.id) ? "Jugando ahora" : ago(i.lastPlayed))}</span>`;
  $("#detail-rows").innerHTML = `
    <dt>Versión de Minecraft</dt><dd>${escapeHtml(i.minecraftVersion)}</dd>
    ${i.modpack ? `<dt>Modpack</dt><dd>${escapeHtml(i.modpack)}</dd>` : ""}
    <dt>Loader</dt><dd>${escapeHtml(i.loader)}${i.loaderVersion ? " " + escapeHtml(i.loaderVersion) : ""}</dd>
    <dt>Versión del perfil</dt><dd>${escapeHtml(i.versionId)}</dd>
    <dt>Carpeta</dt><dd>instancias\\${escapeHtml(i.id)}</dd>
    <dt>Memoria</dt><dd>${i.ramGb ? `${i.ramGb} GB (propia de esta instancia)` : `${state.settings.ramGb} GB (la general de Ajustes)`}</dd>
    <dt>Argumentos de Java</dt><dd>${i.jvmArgs.length ? escapeHtml(i.jvmArgs.join(" ")) : "—"}</dd>
    <dt>Creada</dt><dd>${i.created ? new Date(i.created).toLocaleString("es-MX") : "—"}</dd>`;
  const running = state.running.has(i.id);
  $("#detail-play").classList.toggle("hidden", running);
  $("#detail-stop").classList.toggle("hidden", !running);
  $("#detail-join").disabled = running;
  // Al abrir otra instancia se vuelve a la pestaña Contenido con su contenido.
  if (state.contentFor !== i.id) {
    state.contentFor = i.id;
    state.content = null;
    selectTab("content");
  }
}

function selectTab(tab) {
  state.tab = tab;
  for (const button of $$("#detail-tabs .tab")) button.classList.toggle("active", button.dataset.tab === tab);
  for (const name of ["content", "logs", "server", "overview"]) $(`#tab-${name}`).classList.toggle("hidden", tab !== name);
  if (tab === "content" && !state.content) loadContent();
  if (tab === "logs") loadLog(true);
}

// ---------- Contenido (mods, resource packs, shaders y mundos) ----------
const kindNames = { mods: "mods", resourcepacks: "resource packs", shaderpacks: "shaders", worlds: "mundos" };
const kindIcons = { mods: "puzzle", resourcepacks: "image", shaderpacks: "sun-sparkle", worlds: "globe" };

async function loadContent() {
  const id = state.current?.id;
  if (!id) return;
  $("#content-list").innerHTML = `<div class="content-loading"><div class="spinner"></div></div>`;
  try {
    const content = await invoke("contenido", { id });
    if (state.current?.id !== id) return;
    state.content = content;
    renderContent();
    // Buscar actualizaciones en segundo plano (como mucho cada 10 minutos por instancia).
    if (state.updates.for !== id || Date.now() - state.updates.at > 600e3) checkUpdates(id);
  } catch (error) {
    $("#content-list").innerHTML = `<div class="content-empty">${escapeHtml(error)}</div>`;
  }
}

function renderContent() {
  const content = state.content;
  if (!content) return;
  for (const button of $$("#content-kinds .kind")) {
    button.classList.toggle("active", button.dataset.kind === state.kind);
    $("span", button).textContent = content[button.dataset.kind].length;
  }
  const updates = availableUpdates();
  $("#content-update-all").classList.toggle("hidden", !updates.length);
  $("#content-update-all span").textContent = `Actualizar todo (${updates.length})`;
  const query = $("#content-search").value.trim().toLowerCase();
  const items = content[state.kind].filter((i) => !query || `${i.name} ${i.file} ${i.authors ?? ""}`.toLowerCase().includes(query));
  if (!items.length) {
    const none = content[state.kind].length === 0;
    const vanillaMods = state.kind === "mods" && state.current?.loader === "Vanilla";
    $("#content-list").innerHTML = `<div class="content-empty">${
      !none
        ? "Nada coincide con la búsqueda."
        : vanillaMods
          ? "Las instancias vanilla no cargan mods. Crea una instancia con Fabric o Quilt para usarlos."
          : `Esta instancia no tiene ${kindNames[state.kind]}. Arrástralos a la ventana o usa "Agregar archivos".`
    }</div>`;
    return;
  }
  const world = state.kind === "worlds";
  $("#content-list").innerHTML = items
    .slice(0, 600)
    .map((item, index) => {
      const sub = world ? `Jugado ${ago(item.modified).toLowerCase()}${item.version ? ` · ${item.version}` : ""}` : item.authors ? `por ${item.authors}` : item.file;
      return `<div class="content-item ${item.enabled ? "" : "disabled"}" style="animation-delay:${Math.min(index, 20) * 15}ms">
        <div class="content-icon">${item.icon ? `<img src="${item.icon}" alt="">` : icon(kindIcons[state.kind])}</div>
        <div class="grow">${
          item.projectId
            ? `<button class="c-name link" data-project-open="${escapeHtml(item.projectId)}" title="Ver su página">${escapeHtml(item.name)}</button>`
            : `<div class="c-name">${escapeHtml(item.name)}</div>`
        }<div class="c-sub">${escapeHtml(sub)}</div></div>
        ${updateChip(item.file)}
        ${world ? "" : `<div class="c-version">${escapeHtml(item.version ?? "")}</div>
          <label class="toggle" title="${item.enabled ? "Desactivar" : "Activar"}"><input type="checkbox" data-toggle="${escapeHtml(item.file)}" ${item.enabled ? "checked" : ""}><span></span></label>`}
        <button class="circle-btn" data-remove="${escapeHtml(item.file)}" title="Mandar a la papelera">${icon("trash")}</button>
      </div>`;
    })
    .join("");
}

/** Botón de actualizar (o aviso de que la nueva versión rompería algo) junto a un elemento. */
function updateChip(file) {
  if (state.updates.for !== state.current?.id) return "";
  const update = state.updates.list.find((u) => u.replaces === file);
  if (!update) return "";
  const reason = state.heldBack.get(update.versionId);
  return reason
    ? `<span class="update-chip held" title="${escapeHtml(`${update.versionNumber} no se instaló: ${reason}`)}">${icon("alert")}No compatible</span>`
    : `<button class="update-chip" data-update="${escapeHtml(file)}" title="Actualizar a ${escapeHtml(update.versionNumber)}">${icon("update")}${escapeHtml(update.versionNumber)}</button>`;
}

async function toggleContent(file, enabled) {
  try {
    const renamed = await invoke("activar_contenido", { id: state.current.id, tipo: state.kind, archivo: file, activo: enabled });
    const item = state.content[state.kind].find((i) => i.file === file);
    if (item) Object.assign(item, { file: renamed, enabled });
    renderContent();
  } catch (error) {
    toast(String(error), "error");
    renderContent();
  }
}

async function removeContent(file) {
  const item = state.content[state.kind].find((i) => i.file === file);
  const ok = await confirmAction({
    title: state.kind === "worlds" ? "¿Eliminar este mundo?" : "¿Eliminar este archivo?",
    text: `"${item?.name ?? file}" se moverá a la papelera de reciclaje; desde ahí lo puedes recuperar.`,
    ok: "Mandar a la papelera",
  });
  if (!ok) return;
  try {
    await invoke("eliminar_contenido", { id: state.current.id, tipo: state.kind, archivo: file });
    state.content[state.kind] = state.content[state.kind].filter((i) => i.file !== file);
    renderContent();
  } catch (error) {
    toast(String(error), "error");
  }
}

// ---------- Agregar archivos (botón o arrastrando a la ventana) ----------
const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;

/** Avisa qué se agregó ("2 mods y 1 mundo") y qué no, y muestra la categoría donde quedó. */
function reportAdded(added) {
  const counts = { mods: ["mod", "mods"], resourcepacks: ["resource pack", "resource packs"], shaderpacks: ["shader", "shaders"], worlds: ["mundo", "mundos"] };
  const parts = Object.entries(counts).filter(([kind]) => added[kind]).map(([kind, [one, many]]) => plural(added[kind], one, many));
  const total = Object.keys(counts).reduce((sum, kind) => sum + added[kind], 0);
  if (parts.length) toast(`Se ${total === 1 ? "agregó" : "agregaron"} ${parts.length > 1 ? `${parts.slice(0, -1).join(", ")} y ${parts.at(-1)}` : parts[0]}.`);
  if (added.skipped.length) {
    const more = added.skipped.length > 3 ? ` (y ${added.skipped.length - 3} más)` : "";
    toast(`No se ${added.skipped.length === 1 ? "agregó" : "agregaron"}: ${added.skipped.slice(0, 3).join(" · ")}${more}`, "error");
  }
  if (!total) return;
  if (!added[state.kind]) state.kind = Object.keys(counts).find((kind) => added[kind]);
  state.content = null;
  if (state.tab === "content") loadContent();
  else selectTab("content");
}

async function addContentFiles(paths) {
  const id = state.current?.id;
  if (!id) return;
  try {
    reportAdded(await invoke("agregar_contenido", { id, rutas: paths, tipo: state.kind }));
  } catch (error) {
    toast(String(error), "error");
  }
}

async function pickContentFiles() {
  const id = state.current?.id;
  if (!id) return;
  try {
    const added = await invoke("elegir_contenido", { id, tipo: state.kind });
    if (added) reportAdded(added);
  } catch (error) {
    toast(String(error), "error");
  }
}

// ---------- Registro del juego ----------
let logTimer = null;

async function loadLog(scrollToEnd = false) {
  clearTimeout(logTimer);
  const id = state.current?.id;
  if (!id || state.page !== "instance" || state.tab !== "logs") return;
  try {
    const log = await invoke("registro", { id });
    state.log = log;
    renderLog(scrollToEnd);
  } catch (error) {
    $("#log").textContent = String(error);
  }
  // Mientras el juego está abierto el registro se actualiza solo.
  if (state.running.has(id)) logTimer = setTimeout(() => loadLog(false), 2000);
}

function renderLog(scrollToEnd) {
  const box = $("#log");
  const query = $("#log-search").value.trim().toLowerCase();
  const lines = (state.log?.text ?? "").split("\n").filter((l) => !query || l.toLowerCase().includes(query));
  $("#log-file").textContent = state.log?.file ? `Mostrando ${state.log.file}` : "";
  box.innerHTML = lines.length && state.log?.text
    ? lines
        .slice(-4000)
        .map((line) => {
          const css = /\/(ERROR|FATAL)\]|Exception|^\s+at /.test(line) ? "l-error" : /\/WARN\]/.test(line) ? "l-warn" : /\[CHAT\]/.test(line) ? "l-chat" : /\/DEBUG\]/.test(line) ? "l-dim" : "";
          return css ? `<span class="${css}">${escapeHtml(line)}</span>` : escapeHtml(line);
        })
        .join("\n")
    : `<span class="l-dim">${query ? "Ninguna línea coincide." : "Esta instancia todavía no tiene registro: se crea al abrir el juego."}</span>`;
  if (scrollToEnd || $("#log-follow").checked) box.scrollTop = box.scrollHeight;
}

// ---------- Acciones sobre instancias ----------
function instanceMenuItems(id) {
  return [
    { icon: "info", label: "Ver instancia", action: () => navigate("instance", id) },
    { icon: "settings", label: "Ajustes de la instancia", action: () => openEdit(id) },
    { icon: "folder", label: "Abrir carpeta", action: () => invoke("abrir_carpeta", { id }) },
    { icon: "duplicate", label: "Duplicar", action: () => duplicateInstance(id) },
    { icon: "upload", label: "Exportar modpack", action: () => openExport(id) },
    { icon: "trash", label: "Eliminar", danger: true, action: () => deleteInstance(id) },
  ];
}

async function duplicateInstance(id) {
  const instance = state.instances.find((i) => i.id === id);
  toast(`Duplicando ${instance?.name}… (se copian también sus mundos y mods)`);
  try {
    const copy = await invoke("duplicar_instancia", { id });
    await reloadInstances();
    toast(`Se creó ${copy.name}.`);
  } catch (error) {
    toast(String(error), "error");
  }
}

async function deleteInstance(id) {
  const instance = state.instances.find((i) => i.id === id);
  const ok = await confirmAction({
    title: `¿Eliminar ${instance?.name}?`,
    text: "La instancia, con sus mundos y mods, se moverá a la papelera de reciclaje. Desde ahí la puedes recuperar.",
    ok: "Mandar a la papelera",
  });
  if (!ok) return;
  try {
    await invoke("eliminar_instancia", { id });
    if (state.current?.id === id) navigate("home");
    state.back = state.back.filter((h) => !(h.page === "instance" && h.param === id));
    state.forward = state.forward.filter((h) => !(h.page === "instance" && h.param === id));
    await reloadInstances();
    toast(`${instance?.name} se movió a la papelera.`);
  } catch (error) {
    toast(String(error), "error");
  }
}

function openEdit(id) {
  const instance = state.instances.find((i) => i.id === id);
  if (!instance) return;
  state.editing = { id, icon: instance.icon ?? null, iconChanged: false };
  $("#edit-name").value = instance.name;
  $("#edit-ram-own").checked = !!instance.ramGb;
  $("#edit-ram").max = $("#ram").max || 16;
  $("#edit-ram").value = instance.ramGb ?? state.settings.ramGb;
  $("#edit-jvm").value = instance.jvmArgs.join(" ");
  const usesLoader = ["Fabric", "Quilt"].includes(instance.loader);
  $("#edit-loader-box").classList.toggle("hidden", !usesLoader);
  if (usesLoader) loadEditLoaderVersions(instance);
  updateEditTexts();
  openModal("#edit-dialog");
}

async function loadEditLoaderVersions(instance) {
  const select = $("#edit-loader");
  const loader = instance.loader.toLowerCase();
  const current = instance.loaderVersion ?? "";
  $("#edit-loader-label").textContent = `Versión de ${instance.loader}`;
  $("#edit-loader-hint").textContent = "Algunos mods piden una versión más nueva del loader.";
  select.innerHTML = `<option value="${escapeHtml(current)}">${escapeHtml(current)} (actual)</option>`;
  select.disabled = true;
  const key = `${loader}:${instance.minecraftVersion}`;
  try {
    state.loaderVersions[key] ??= await invoke("versiones_loader", { loader, minecraft: instance.minecraftVersion });
  } catch (error) {
    $("#edit-loader-hint").textContent = String(error);
    return;
  }
  if (state.editing?.id !== instance.id) return;
  const info = state.loaderVersions[key];
  fillLoaderSelect(select, info, current);
  select.disabled = false;
  if (!info.online) $("#edit-loader-hint").textContent = `${info.message} Solo se muestran las versiones ya descargadas.`;
}

function updateEditTexts() {
  const name = $("#edit-name").value.trim() || "Instancia";
  $("#edit-art").outerHTML = art({ name, icon: state.editing?.icon }, "large").replace('class="art', 'id="edit-art" class="art');
  $("#edit-icon-remove").classList.toggle("hidden", !state.editing?.icon);
  $("#edit-ram-box").classList.toggle("hidden", !$("#edit-ram-own").checked);
  $("#edit-ram-value").textContent = `${$("#edit-ram").value} GB`;
}

/** Recorta la imagen elegida a un cuadrado de 256 px y la convierte a PNG. */
function imageToPng(file) {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const image = new Image();
    image.onload = () => {
      const size = Math.min(image.width, image.height);
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 256;
      const ctx = canvas.getContext("2d");
      ctx.imageSmoothingEnabled = size > 64; // los iconos pixelados se mantienen nítidos
      ctx.drawImage(image, (image.width - size) / 2, (image.height - size) / 2, size, size, 0, 0, 256, 256);
      URL.revokeObjectURL(url);
      resolve(canvas.toDataURL("image/png"));
    };
    image.onerror = () => reject("No se pudo leer esa imagen.");
    image.src = url;
  });
}

async function saveEdit() {
  const { id, icon: newIcon, iconChanged } = state.editing;
  const instance = state.instances.find((i) => i.id === id);
  const wantedLoader = $("#edit-loader-box").classList.contains("hidden") ? "" : $("#edit-loader").value;
  $("#edit-save").disabled = true;
  try {
    if (wantedLoader && wantedLoader !== instance?.loaderVersion) {
      $("#edit-loader-hint").textContent = `Preparando ${instance.loader} ${wantedLoader}…`;
      await invoke("cambiar_loader", { id, versionLoader: wantedLoader });
      state.loaderVersions = {};
    }
    let updated = await invoke("editar_instancia", {
      id,
      nombre: $("#edit-name").value,
      ramGb: $("#edit-ram-own").checked ? Number($("#edit-ram").value) : null,
      jvmArgs: $("#edit-jvm").value.split(/\s+/).filter(Boolean),
    });
    if (iconChanged) updated = await invoke("icono_instancia", { id, png: newIcon });
    closeModal("#edit-dialog");
    await reloadInstances();
    if (state.current?.id === id) {
      state.current = state.instances.find((i) => i.id === id) ?? updated;
      renderInstance();
      $("#crumb-text").textContent = state.current.name;
    }
    toast("Cambios guardados.");
  } catch (error) {
    toast(String(error), "error");
  } finally {
    $("#edit-save").disabled = false;
  }
}

/** Ventana de confirmación propia (en lugar de la del sistema). */
function confirmAction({ title, text, ok, primary = false }) {
  return new Promise((resolve) => {
    $("#confirm-title").textContent = title;
    $("#confirm-text").textContent = text;
    $("#confirm-ok").textContent = ok;
    $("#confirm-ok").className = primary ? "btn primary" : "btn danger";
    const finish = (value) => {
      closeModal("#confirm-dialog");
      resolve(value);
    };
    $("#confirm-ok").onclick = () => finish(true);
    $("#confirm-cancel").onclick = () => finish(false);
    state.confirmCancel = () => finish(false);
    openModal("#confirm-dialog");
  });
}

/** Aviso del backend: un juego se cerró. Si fue por un error, se explica el motivo. */
function onGameClosed({ id, code, reason }) {
  state.running.delete(id);
  refreshRunning();
  if (state.page === "instance" && state.current?.id === id && state.tab === "logs") loadLog(false);
  if (code === null || code === undefined || code === 0) return;
  const instance = state.instances.find((i) => i.id === id);
  state.crashFor = id;
  $("#crash-title").textContent = `${instance?.name ?? "El juego"} se cerró con un error`;
  $("#crash-reason").textContent = reason ?? `Java terminó con el código ${code}. Revisa el registro completo para ver el motivo.`;
  appWindow?.unminimize?.();
  openModal("#crash-dialog");
}

async function play(id, server = null) {
  const instance = state.instances.find((i) => i.id === id);
  if (!instance) return;
  try {
    toast(server ? `Abriendo ${instance.name} y entrando a ${server}…` : `Abriendo ${instance.name}…`);
    await invoke("jugar", { id, servidor: server });
    state.running.add(id);
    refreshRunning();
    if (state.settings.minimizeOnLaunch) setTimeout(() => appWindow?.minimize(), 3500);
  } catch (error) {
    toast(String(error), "error");
  }
}

// ======================================================================
// Juego abierto
// ======================================================================
function refreshRunning() {
  const running = state.instances.find((i) => state.running.has(i.id));
  $("#status").classList.toggle("running", !!running);
  $("#status-text").textContent = running ? `Jugando ${running.name}` : "Ningún juego abierto";
  renderHome();
  if (state.page === "instance") renderInstance();
}

async function pollRunning() {
  try {
    const ids = await invoke("juegos_abiertos");
    if (ids.length !== state.running.size || ids.some((id) => !state.running.has(id))) {
      state.running = new Set(ids);
      refreshRunning();
    }
  } catch {
    /* se reintenta en la siguiente vuelta */
  }
}

// ======================================================================
// Panel derecho
// ======================================================================
function renderSidebar() {
  $("#side-name").textContent = state.settings.username;
  drawHead($("#side-head"), state.settings.username);
  drawHead($("#rail-head"), state.settings.username);
}

function renderSteps() {
  const steps = [
    { done: state.instances.length > 0, label: "Crea tu primera instancia", action: openCreate },
    { done: state.settings.username !== "Jugador", label: "Elige tu nombre de jugador", action: () => openSettings("profile") },
    { done: false, label: "Inicia sesión con Google", disabled: true, soon: true },
  ];
  $("#steps-list").innerHTML = `<div>${steps
    .map(
      (s, i) => `<button class="step ${s.done ? "done" : ""}" data-step="${i}" ${s.done || s.disabled ? "disabled" : ""}>
        ${icon(s.done ? "check-circle" : "circle")}<span class="grow">${s.label}</span>${s.soon ? `<span class="soon">Pronto</span>` : ""}</button>`,
    )
    .join("")}</div>`;
  $$("#steps-list .step").forEach((button) => (button.onclick = () => steps[button.dataset.step].action?.()));
}

// ======================================================================
// Servidores
// ======================================================================
function renderServers() {
  const select = $("#server-instance");
  const previous = select.value;
  select.innerHTML = state.instances.length
    ? state.instances.map((i) => `<option value="${escapeHtml(i.id)}">${escapeHtml(i.name)} · ${escapeHtml(subtitle(i))}</option>`).join("")
    : `<option value="">Primero crea una instancia</option>`;
  if (previous) select.value = previous;
  const servers = state.settings.servers ?? [];
  $("#server-list").innerHTML = servers
    .map((s, index) => {
      const instance = state.instances.find((i) => i.id === s.instance);
      return `<div class="server" style="animation-delay:${index * 40}ms">
        <div class="server-icon">${icon("server")}</div>
        <div class="grow">
          <div class="title">${escapeHtml(s.name || s.address)}</div>
          <div class="sub">${escapeHtml(s.address)} · ${instance ? escapeHtml(instance.name) : "instancia no encontrada"}</div>
        </div>
        <button class="btn primary play" data-server-play="${index}" ${instance ? "" : "disabled"}>${icon("play")}Jugar</button>
        <button class="circle-btn" data-server-delete="${index}" title="Quitar">${icon("trash")}</button>
      </div>`;
    })
    .join("");
  $("#servers-empty").classList.toggle("hidden", servers.length > 0);
}

async function saveServers(servers) {
  try {
    state.settings = await invoke("guardar_ajustes", { ajustes: { ...state.settings, servers } });
    renderServers();
  } catch (error) {
    toast(String(error), "error");
  }
}

// ======================================================================
// Modales
// ======================================================================
function openModal(id) {
  const overlay = $(id);
  overlay.classList.remove("hidden", "closing");
}

function closeModal(id) {
  const overlay = $(id);
  if (overlay.classList.contains("hidden")) return;
  overlay.classList.add("closing");
  setTimeout(() => overlay.classList.add("hidden"), 150);
}

// ---------- Crear instancia ----------
function openCreate() {
  if (state.creating || state.importing) return openModal("#create-dialog");
  $("#create-step-import").classList.add("hidden");
  $("#create-step-type").classList.remove("hidden");
  $("#create-step-custom").classList.add("hidden");
  $("#create-back").classList.add("hidden");
  $("#create-title").textContent = "Crear instancia";
  openModal("#create-dialog");
}

async function showCustomStep() {
  $("#create-step-type").classList.add("hidden");
  $("#create-step-custom").classList.remove("hidden");
  $("#create-back").classList.remove("hidden");
  $("#create-title").textContent = "Configuración personalizada";
  state.selectedVersion = null;
  state.nameEdited = false;
  state.loader = "vanilla";
  state.loaderVersions = {};
  for (const button of $$("#loader-choice [data-loader]")) button.classList.toggle("active", button.dataset.loader === "vanilla");
  $("#loader-version-box").classList.add("hidden");
  $("#create-step-custom").classList.remove("with-loader");
  $("#new-name").value = "";
  $("#new-search").value = "";
  $("#new-status").textContent = "";
  $("#new-create").disabled = true;
  updateCreateArt();
  $("#new-versions").innerHTML = `<div class="version-empty">Cargando versiones…</div>`;
  state.versions = await invoke("listar_versiones");
  if (!state.versions.mojang) $("#new-status").textContent = `${state.versions.message} Puedes usar las versiones ya descargadas en este PC.`;
  renderVersions();
}

function updateCreateArt() {
  $("#create-art").outerHTML = art({ name: $("#new-name").value.trim() || "Nueva" }, "large").replace('class="art large"', 'class="art large" id="create-art"');
}

const loaderNames = { vanilla: "Vanilla", fabric: "Fabric", quilt: "Quilt" };

async function selectLoader(loader) {
  state.loader = loader;
  for (const button of $$("#loader-choice [data-loader]")) button.classList.toggle("active", button.dataset.loader === loader);
  $("#loader-version-box").classList.toggle("hidden", loader === "vanilla");
  $("#create-step-custom").classList.toggle("with-loader", loader !== "vanilla");
  $("#loader-version-label").textContent = `Versión de ${loaderNames[loader]}`;
  if (loader !== "vanilla" && !state.loaderGames[loader]?.online) {
    $("#new-versions").innerHTML = `<div class="version-empty">Buscando las versiones compatibles con ${loaderNames[loader]}…</div>`;
    state.loaderGames[loader] = await invoke("versiones_compatibles", { loader });
    if (state.loader !== loader) return;
  }
  // Se conserva la versión elegida si este loader también la soporta.
  const previous = state.selectedVersion;
  state.selectedVersion = null;
  if (previous && versionOptions().some((o) => o.id === previous)) selectVersion(previous, false);
  renderVersions();
  if (!state.selectedVersion) {
    $("#new-create").disabled = true;
    $("#new-status").textContent = "";
  }
}

function versionOptions() {
  const v = state.versions;
  if (!v) return [];
  const installed = new Set(v.installed);
  let options = v.mojang
    ? v.releases.map((r) => ({ ...r, installed: installed.has(r.id) }))
    : v.installed.map((id) => ({ id, type: "release", releaseTime: null, installed: true }));
  if (state.loader !== "vanilla") {
    const games = state.loaderGames[state.loader];
    // Sin conexión con el loader solo sirven las versiones donde ya está descargado.
    const supported = new Set(
      games?.online ? games.games : v.loaders.filter((l) => l.loader === loaderNames[state.loader]).map((l) => l.minecraft),
    );
    options = options.filter((o) => supported.has(o.id));
  }
  return options;
}

function emptyVersionsText() {
  const games = state.loaderGames[state.loader];
  if (state.loader !== "vanilla" && games && !games.online) return `${games.message} No hay ${loaderNames[state.loader]} descargado en este PC.`;
  if (state.versions?.mojang === false && !state.versions.installed.length) return "No hay versiones descargadas en este PC.";
  return "Ninguna versión coincide.";
}

function renderVersions() {
  const query = $("#new-search").value.trim().toLowerCase();
  const snapshots = $("#new-snapshots").checked;
  const options = versionOptions().filter((o) => (snapshots || o.type === "release") && (!query || o.id.toLowerCase().includes(query)));
  if (!state.selectedVersion && options.length && !query) selectVersion(options[0].id, false);
  $("#new-versions").innerHTML = options.length
    ? options
        .slice(0, 300)
        .map((o) => {
          const date = o.releaseTime ? new Date(o.releaseTime).toLocaleDateString("es-MX", { day: "numeric", month: "short", year: "numeric" }) : "";
          return `<div class="version ${o.id === state.selectedVersion ? "selected" : ""}" data-version="${escapeHtml(o.id)}">
            <div><div class="v-name">Minecraft ${escapeHtml(o.id)}</div><div class="v-sub">${o.type === "release" ? "Versión estable" : "Snapshot"}${date ? " · " + date : ""}</div></div>
            ${o.installed ? `<span class="badge">Descargada</span>` : ""}
          </div>`;
        })
        .join("")
    : `<div class="version-empty">${escapeHtml(emptyVersionsText())}</div>`;
}

function selectVersion(id, rerender = true) {
  state.selectedVersion = id;
  if (!state.nameEdited) {
    $("#new-name").value = `${state.loader === "vanilla" ? "Minecraft" : loaderNames[state.loader]} ${id}`;
    updateCreateArt();
  }
  $("#new-create").disabled = state.creating;
  updateCreateStatus();
  if (state.loader !== "vanilla") loadLoaderVersions(id);
  if (rerender) renderVersions();
}

/** Qué se va a descargar con lo elegido. */
function updateCreateStatus() {
  if (state.creating) return;
  const option = versionOptions().find((o) => o.id === state.selectedVersion);
  if (!option) return;
  if (state.loader === "vanilla") {
    $("#new-status").textContent = option.installed ? "Ya está descargada: la instancia se crea al momento." : "Se descargará de los servidores de Mojang, con su Java.";
    return;
  }
  const name = loaderNames[state.loader];
  const chosen = $("#loader-version").value;
  if (!chosen) return;
  const ready = state.loaderVersions[`${state.loader}:${option.id}`]?.installed.includes(chosen);
  $("#new-status").textContent =
    option.installed && ready
      ? "Ya está descargada: la instancia se crea al momento."
      : option.installed
        ? `Se descargará ${name} ${chosen} (tarda unos segundos).`
        : `Se descargará Minecraft ${option.id} con su Java y ${name} ${chosen}.`;
}

async function loadLoaderVersions(minecraft) {
  const loader = state.loader;
  const key = `${loader}:${minecraft}`;
  const select = $("#loader-version");
  if (!state.loaderVersions[key]) {
    select.innerHTML = `<option value="">Cargando versiones…</option>`;
    select.disabled = true;
    $("#new-create").disabled = true;
    try {
      state.loaderVersions[key] = await invoke("versiones_loader", { loader, minecraft });
    } catch (error) {
      state.loaderVersions[key] = { online: false, versions: [], installed: [], recommended: null, message: String(error) };
    }
  }
  if (state.loader !== loader || state.selectedVersion !== minecraft) return;
  const info = state.loaderVersions[key];
  fillLoaderSelect(select, info, null);
  select.disabled = state.creating || !info.versions.length;
  $("#new-create").disabled = state.creating || !select.value;
  updateCreateStatus();
  if (!info.versions.length) $("#new-status").textContent = info.message ?? `${loaderNames[loader]} todavía no está disponible para Minecraft ${minecraft}.`;
}

/** Lista de versiones del loader; marca la recomendada, las beta, las descargadas y la actual. */
function fillLoaderSelect(select, info, current) {
  const selected = current || info.recommended;
  const versions = [...info.versions];
  if (current && !versions.some((v) => v.version === current)) versions.unshift({ version: current, stable: !current.includes("-") });
  select.innerHTML = versions.length
    ? versions
        .map((v) => {
          const tags = [
            v.version === current ? "actual" : "",
            v.version === info.recommended ? "recomendada" : "",
            v.stable ? "" : "beta",
            info.installed.includes(v.version) ? "descargada" : "",
          ].filter(Boolean);
          return `<option value="${escapeHtml(v.version)}" ${v.version === selected ? "selected" : ""}>${escapeHtml(v.version)}${tags.length ? ` (${tags.join(", ")})` : ""}</option>`;
        })
        .join("")
    : `<option value="">No disponible</option>`;
}

async function createInstance() {
  const name = $("#new-name").value.trim();
  const loader = state.loader;
  const versionLoader = loader === "vanilla" ? null : $("#loader-version").value;
  if (!name || !state.selectedVersion || versionLoader === "") return;
  state.creating = true;
  $("#new-create").disabled = true;
  $("#loader-version").disabled = true;
  $("#new-progress").classList.remove("hidden");
  $("#new-progress div").style.width = "2%";
  $("#download-indicator").classList.remove("hidden");
  try {
    const instance = await invoke("crear_instancia", { nombre: name, version: state.selectedVersion, loader, versionLoader });
    state.creating = false;
    closeModal("#create-dialog");
    await reloadInstances();
    toast(`${instance.name} está lista para jugar.`);
    navigate("instance", instance.id);
  } catch (error) {
    $("#new-status").textContent = String(error);
  } finally {
    state.creating = false;
    state.loaderVersions = {};
    $("#new-create").disabled = false;
    $("#loader-version").disabled = false;
    $("#new-progress").classList.add("hidden");
    $("#download-indicator").classList.add("hidden");
  }
}

// ---------- Ajustes ----------
let saveTimer = null;

function openSettings(section = "appearance") {
  selectSettingsSection(section);
  openModal("#settings-dialog");
  renderSettings();
  loadJava();
}

function selectSettingsSection(section) {
  if (section === "about") refreshAbout();
  for (const button of $$(".settings-nav button")) button.classList.toggle("active", button.dataset.section === section);
  for (const panel of $$(".settings-content section")) panel.classList.toggle("hidden", panel.dataset.section !== section);
}

function renderSettings() {
  const s = state.settings;
  $("#username").value = s.username;
  $("#minimize-on-launch").checked = s.minimizeOnLaunch;
  $("#ram").max = Math.max(4, Math.min(32, state.systemRam - 2));
  $("#ram").value = s.ramGb;
  $("#ram-max").textContent = `${$("#ram").max} GB`;
  $("#win-width").value = s.windowWidth;
  $("#win-height").value = s.windowHeight;
  updateSettingsTexts();
  applyTheme(s.theme);
}

async function updateSettingsTexts() {
  const name = $("#username").value.trim();
  const valid = /^[A-Za-z0-9_]{1,16}$/.test(name);
  $("#username-error").textContent = "El nombre debe tener de 1 a 16 letras, números o guiones bajos (sin espacios ni acentos).";
  $("#username-error").classList.toggle("hidden", valid);
  $("#uuid-text").textContent = valid ? await invoke("uuid_offline", { nombre: name }) : "—";
  drawHead($("#settings-head"), name);
  const ram = Number($("#ram").value);
  $("#ram-value").textContent = `${ram} GB`;
  let hint = `Tu PC tiene ${state.systemRam} GB. Vanilla funciona bien con 2–4 GB; los modpacks grandes piden de 6 a 10 GB.`;
  if (ram > state.systemRam - 3) hint += " Deja al menos 3 GB libres para Windows.";
  $("#ram-hint").textContent = hint;
}

function queueSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveSettings, 400);
}

async function saveSettings(extra = {}) {
  const name = $("#username").value.trim();
  if (!/^[A-Za-z0-9_]{1,16}$/.test(name)) return;
  try {
    state.settings = await invoke("guardar_ajustes", {
      ajustes: {
        ...state.settings,
        username: name,
        ramGb: Number($("#ram").value),
        windowWidth: Number($("#win-width").value) || state.settings.windowWidth,
        windowHeight: Number($("#win-height").value) || state.settings.windowHeight,
        minimizeOnLaunch: $("#minimize-on-launch").checked,
        ...extra,
      },
    });
    renderSidebar();
    renderSteps();
  } catch (error) {
    toast(String(error), "error");
  }
}

async function loadJava() {
  const list = await invoke("listar_java");
  $("#java-list").innerHTML = list.length
    ? list
        .map((j) => `<div class="java"><div class="java-major">${j.major}</div><div class="grow"><b>Java ${escapeHtml(j.version)}</b><div class="java-path">${escapeHtml(j.path)}</div></div></div>`)
        .join("")
    : `<p class="muted">Todavía no hay ningún Java: se descargará al crear tu primera instancia.</p>`;
}

// ======================================================================
// Eventos
// ======================================================================
function wireEvents() {
  $("#win-min").onclick = () => appWindow?.minimize();
  $("#win-max").onclick = () => appWindow?.toggleMaximize();
  // Se abre maximizada; el botón cambia entre maximizar y restaurar.
  const updateMaxButton = async () => {
    const maximized = await appWindow?.isMaximized();
    $("#win-max use").setAttribute("href", maximized ? "#i-restore" : "#i-max");
    $("#win-max").title = maximized ? "Restaurar" : "Maximizar";
  };
  updateMaxButton();
  appWindow?.onResized(updateMaxButton);
  $("#win-close").onclick = () => appWindow?.close();
  $("#nav-back").onclick = goBack;
  $("#nav-forward").onclick = goForward;

  for (const button of $$(".rail-btn[data-page]")) button.onclick = () => navigate(button.dataset.page);
  for (const button of [$("#rail-new"), $("#home-new"), $("#empty-new")]) button.onclick = openCreate;
  $("#rail-settings").onclick = () => openSettings("appearance");
  $("#rail-avatar").onclick = () => openSettings("profile");
  $("#account-card").onclick = () => openSettings("profile");
  $("#status").onclick = () => {
    const running = state.instances.find((i) => state.running.has(i.id));
    navigate(running ? "instance" : "home", running?.id ?? null);
  };

  // Clics en tarjetas, filas e iconos de instancia (delegados)
  document.addEventListener("click", (event) => {
    const playButton = event.target.closest("[data-play]");
    if (playButton) {
      event.stopPropagation();
      return play(playButton.dataset.play);
    }
    const more = event.target.closest("[data-more]");
    if (more) {
      event.stopPropagation();
      const id = more.dataset.more;
      return contextMenu(more, instanceMenuItems(id));
    }
    const open = event.target.closest("[data-open]");
    if (open) navigate("instance", open.dataset.open);
  });

  $("#library-search").oninput = renderLibrary;
  for (const dropdown of $$(".dropdown")) {
    $(".btn", dropdown).onclick = (event) => {
      event.stopPropagation();
      const wasOpen = dropdown.classList.contains("open");
      $$(".dropdown").forEach((d) => d.classList.remove("open"));
      dropdown.classList.toggle("open", !wasOpen);
    };
    $(".dd-menu", dropdown).onclick = (event) => {
      const option = event.target.closest("[data-value]");
      if (!option) return;
      if (dropdown.id === "sort-dropdown") state.sort = option.dataset.value;
      else state.group = option.dataset.value;
      $(".dd-label", dropdown).textContent = option.textContent;
      $$("[data-value]", dropdown).forEach((b) => b.classList.toggle("selected", b === option));
      dropdown.classList.remove("open");
      renderLibrary();
    };
  }
  document.addEventListener("click", () => $$(".dropdown").forEach((d) => d.classList.remove("open")));

  // Detalle
  $("#detail-play").onclick = () => state.current && play(state.current.id);
  $("#detail-stop").onclick = async () => {
    try {
      await invoke("detener", { id: state.current.id });
    } catch (error) {
      toast(String(error), "error");
    }
  };
  $("#detail-edit").onclick = () => state.current && openEdit(state.current.id);
  $("#detail-more").onclick = (event) => {
    event.stopPropagation();
    if (state.current) contextMenu($("#detail-more"), instanceMenuItems(state.current.id).slice(2));
  };
  for (const tab of $$("#detail-tabs .tab")) tab.onclick = () => selectTab(tab.dataset.tab);

  // Contenido
  for (const button of $$("#content-kinds .kind")) {
    button.onclick = () => {
      state.kind = button.dataset.kind;
      renderContent();
    };
  }
  $("#content-search").oninput = renderContent;
  $("#content-folder").onclick = () => invoke("abrir_carpeta_contenido", { id: state.current.id, tipo: state.kind });
  $("#content-list").onchange = (event) => {
    const toggle = event.target.closest("[data-toggle]");
    if (toggle) toggleContent(toggle.dataset.toggle, toggle.checked);
  };
  $("#content-list").onclick = (event) => {
    const remove = event.target.closest("[data-remove]");
    if (remove) return removeContent(remove.dataset.remove);
    const update = event.target.closest("[data-update]");
    if (update) {
      const found = availableUpdates().find((u) => u.replaces === update.dataset.update);
      return found && applyUpdates([found]);
    }
    const project = event.target.closest("[data-project-open]");
    if (project) {
      // Abierto desde Contenido: el tipo lo dice el propio proyecto.
      state.projectKind = null;
      setExploreTarget(state.current?.id);
      navigate("project", project.dataset.projectOpen);
    }
  };

  // Registro
  $("#log-search").oninput = () => renderLog(false);
  $("#log-follow").onchange = () => renderLog(true);
  $("#log-copy").onclick = async () => {
    await navigator.clipboard.writeText(state.log?.text ?? "");
    toast("Registro copiado: pégalo donde quieras pedir ayuda.");
  };
  $("#log-folder").onclick = () => invoke("abrir_registro", { id: state.current.id });

  // Clic derecho sobre una instancia (mosaicos, filas y barra lateral)
  document.addEventListener("contextmenu", (event) => {
    const target = event.target.closest("[data-open]");
    if (!target) return;
    event.preventDefault();
    contextMenu(target, instanceMenuItems(target.dataset.open));
  });

  // Ajustes de la instancia
  $("#edit-close").onclick = () => closeModal("#edit-dialog");
  $("#edit-cancel").onclick = () => closeModal("#edit-dialog");
  $("#edit-dialog").onmousedown = (event) => event.target.id === "edit-dialog" && closeModal("#edit-dialog");
  $("#edit-name").oninput = updateEditTexts;
  $("#edit-ram-own").onchange = updateEditTexts;
  $("#edit-ram").oninput = updateEditTexts;
  $("#edit-icon-button").onclick = () => $("#edit-icon-file").click();
  $("#edit-icon-file").onchange = async () => {
    const file = $("#edit-icon-file").files[0];
    $("#edit-icon-file").value = "";
    if (!file) return;
    try {
      state.editing.icon = await imageToPng(file);
      state.editing.iconChanged = true;
      updateEditTexts();
    } catch (error) {
      toast(String(error), "error");
    }
  };
  $("#edit-icon-remove").onclick = () => {
    state.editing.icon = null;
    state.editing.iconChanged = true;
    updateEditTexts();
  };
  $("#edit-save").onclick = saveEdit;
  $("#edit-name").onkeydown = (event) => event.key === "Enter" && saveEdit();

  // Confirmación y error del juego
  $("#confirm-dialog").onmousedown = (event) => event.target.id === "confirm-dialog" && state.confirmCancel?.();
  for (const id of ["#crash-close", "#crash-dismiss"]) $(id).onclick = () => closeModal("#crash-dialog");
  $("#crash-logs").onclick = () => {
    closeModal("#crash-dialog");
    navigate("instance", state.crashFor);
    selectTab("logs");
  };
  $("#detail-join").onclick = () => {
    const address = $("#detail-server").value.trim();
    if (!address) return toast("Escribe la dirección del servidor.", "error");
    play(state.current.id, address);
  };
  $("#detail-server").onkeydown = (event) => event.key === "Enter" && $("#detail-join").click();

  // Servidores
  $("#server-add").onclick = () => {
    const address = $("#server-address").value.trim();
    const instance = $("#server-instance").value;
    if (!address) return toast("Escribe la dirección del servidor.", "error");
    if (!instance) return toast("Primero crea una instancia para entrar al servidor.", "error");
    saveServers([...(state.settings.servers ?? []), { name: $("#server-name").value.trim(), address, instance }]);
    $("#server-name").value = "";
    $("#server-address").value = "";
  };
  $("#server-list").onclick = (event) => {
    const playButton = event.target.closest("[data-server-play]");
    if (playButton) {
      const server = state.settings.servers[playButton.dataset.serverPlay];
      return play(server.instance, server.address);
    }
    const remove = event.target.closest("[data-server-delete]");
    if (remove) saveServers(state.settings.servers.filter((_, i) => i !== Number(remove.dataset.serverDelete)));
  };

  // Crear instancia
  $("#create-close").onclick = () => closeModal("#create-dialog");
  $("#create-back").onclick = () => !state.creating && !state.importing && openCreate();
  $("#create-dialog").onmousedown = (event) => event.target.id === "create-dialog" && closeModal("#create-dialog");
  $("[data-type='custom']").onclick = showCustomStep;
  $("[data-type='import']").onclick = () => importModpack();
  $("[data-type='drive']").onclick = () => {
    closeModal("#create-dialog");
    state.explore.kind = "drive";
    navigate("explore");
    selectExploreKind("drive");
  };
  $("#new-cancel").onclick = () => closeModal("#create-dialog");
  $("#loader-choice").onclick = (event) => {
    const button = event.target.closest("[data-loader]");
    if (button && !button.disabled && !state.creating && button.dataset.loader !== state.loader) selectLoader(button.dataset.loader);
  };
  $("#loader-version").onchange = updateCreateStatus;
  $("#new-search").oninput = renderVersions;
  $("#new-snapshots").onchange = renderVersions;
  $("#new-versions").onclick = (event) => {
    const item = event.target.closest("[data-version]");
    if (item && !state.creating) selectVersion(item.dataset.version);
  };
  $("#new-name").oninput = () => {
    state.nameEdited = $("#new-name").value.length > 0;
    updateCreateArt();
  };
  $("#new-create").onclick = createInstance;

  // Ajustes
  $("#settings-close").onclick = () => closeModal("#settings-dialog");
  $("#settings-dialog").onmousedown = (event) => event.target.id === "settings-dialog" && closeModal("#settings-dialog");
  for (const button of $$(".settings-nav button")) button.onclick = () => selectSettingsSection(button.dataset.section);
  $("#username").oninput = () => {
    updateSettingsTexts();
    queueSave();
  };
  $("#ram").oninput = () => {
    updateSettingsTexts();
    queueSave();
  };
  for (const input of [$("#win-width"), $("#win-height")]) input.onchange = () => saveSettings();
  $("#minimize-on-launch").onchange = () => saveSettings();
  for (const button of $$("#theme button")) {
    button.onclick = () => {
      applyTheme(button.dataset.theme);
      saveSettings({ theme: button.dataset.theme });
    };
  }
  matchMedia("(prefers-color-scheme: light)").onchange = () => state.settings?.theme === "sistema" && applyTheme("sistema");
  $("#uuid-copy").onclick = async () => {
    await navigator.clipboard.writeText($("#uuid-text").textContent);
    toast("UUID copiado.");
  };
  $("#open-root").onclick = () => invoke("abrir_carpeta", { id: null });

  // Colapsables y teclado
  for (const collapsible of $$(".collapsible")) $(".collapsible-head", collapsible).onclick = () => collapsible.classList.toggle("open");
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeModal("#create-dialog");
      closeModal("#settings-dialog");
      closeModal("#edit-dialog");
      closeModal("#crash-dialog");
      closeModal("#install-dialog");
      if (!state.exporting?.busy) closeModal("#export-dialog");
      $("#lightbox").classList.add("hidden");
      if (!$("#confirm-dialog").classList.contains("hidden")) state.confirmCancel?.();
    } else if ((event.key === "n" || event.key === "N") && !isTyping() && !event.ctrlKey && !event.altKey) {
      event.preventDefault();
      openCreate();
    } else if (event.altKey && event.key === "ArrowLeft") {
      goBack();
    } else if (event.altKey && event.key === "ArrowRight") {
      goForward();
    }
  });
  document.addEventListener("mouseup", (event) => {
    if (event.button === 3) goBack();
    if (event.button === 4) goForward();
  });

  wireExploreEvents();
  wireModpackEvents();
  wireDriveEvents();
  listen("juego-cerrado", ({ payload }) => onGameClosed(payload));
  // Arrastrar archivos a la ventana: se agregan a la instancia abierta.
  // Fuera de una instancia (o con un .mrpack) lo que se suelta se instala como modpack.
  const dropsModpack = (paths) => !(state.page === "instance" && state.current) || (paths ?? []).some((p) => /\.mrpack$/i.test(p));
  tauri?.webview?.getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (payload.type === "enter") {
      const modpack = dropsModpack(payload.paths);
      $("#drop-title").textContent = modpack ? "Suelta para instalar el modpack" : `Suelta para agregar a ${state.current.name}`;
      $("#drop-text").textContent = modpack
        ? "Un .mrpack o un paquete .zip de Parabellum (de Drive o una USB)."
        : "Mods, resource packs, shaders y mundos: cada uno va a su lugar.";
      $("#drop-overlay").classList.remove("hidden");
    } else if (payload.type === "drop" || payload.type === "leave") {
      $("#drop-overlay").classList.add("hidden");
      if (payload.type !== "drop" || !payload.paths?.length) return;
      if (dropsModpack(payload.paths)) importModpack(payload.paths[0]);
      else addContentFiles(payload.paths);
    }
  });

  listen("progreso", ({ payload }) => {
    if (payload.task === "catalogo") {
      const line = state.drive.installing && $(`[data-progress-for="${CSS.escape(state.drive.installing)}"]`);
      if (line) line.textContent = payload.stage;
      $("#download-percent").textContent = `${Math.round(payload.fraction * 100)}%`;
      return;
    }
    if (payload.task === "actualizacion") {
      $("#update-text").textContent = `Descargando… ${Math.round(payload.fraction * 100)}%`;
      return;
    }
    if (payload.task === "exportar") {
      $("#export-progress").classList.remove("hidden");
      $("#export-progress div").style.width = `${Math.max(2, payload.fraction * 100)}%`;
      $("#export-status").textContent = payload.stage;
      return;
    }
    if (payload.task === "importar") {
      $("#import-progress div").style.width = `${Math.max(2, payload.fraction * 100)}%`;
      $("#import-status").textContent = payload.stage;
      $("#download-percent").textContent = `${Math.round(payload.fraction * 100)}%`;
      return;
    }
    if (payload.task === "instalar") {
      $("#download-percent").textContent = `${Math.round(payload.fraction * 100)}%`;
      return;
    }
    if (payload.task === "loader") {
      $("#edit-loader-hint").textContent = payload.stage;
      return;
    }
    $("#new-status").textContent = payload.stage;
    const percent = Math.max(2, payload.fraction * 100);
    $("#new-progress div").style.width = `${percent}%`;
    $("#download-percent").textContent = `${Math.round(percent)}%`;
  });
}

async function init() {
  wireEvents();
  const initial = await invoke("estado_inicial");
  state.settings = initial.settings;
  state.instances = initial.instances;
  state.systemRam = initial.systemRamGb;
  state.version = initial.version;
  state.driveUrl = initial.driveUrl;
  // Buscar una versión nueva de Parabellum sin estorbar el arranque.
  setTimeout(() => checkLauncherUpdate(false, true), 4000);
  $("#root-text").textContent = initial.root;
  $("#version-text").textContent = `Parabellum ${initial.version}`;
  applyTheme(state.settings.theme);
  renderSidebar();
  renderSteps();
  renderHome();
  renderServers();
  navigate("home", null, false);
  pollRunning();
  setInterval(pollRunning, 3000);
}

// ======================================================================
// Explorar: mods, resource packs y shaders de Modrinth
// ======================================================================
const exploreKinds = {
  mod: { placeholder: "Buscar mods…", contentKind: "mods", noun: "mods" },
  resourcepack: { placeholder: "Buscar resource packs…", contentKind: "resourcepacks", noun: "resource packs" },
  shader: { placeholder: "Buscar shaders…", contentKind: "shaderpacks", noun: "shaders" },
  datapack: { placeholder: "Buscar data packs…", contentKind: "worlds", noun: "data packs" },
  modpack: { placeholder: "Buscar modpacks…", contentKind: null, noun: "modpacks" },
  drive: { placeholder: "Buscar en tus paquetes…", contentKind: null, noun: "paquetes" },
};
const contentToExplore = { mods: "mod", resourcepacks: "resourcepack", shaderpacks: "shader", worlds: "datapack" };
/** Qué es el proyecto abierto: lo dice la pestaña desde la que se abrió (los data packs en Modrinth son "mod"). */
const projectKind = () => state.projectKind ?? state.projectData?.projectType;

const categoryNames = {
  adventure: "Aventura", cursed: "Raros", decoration: "Decoración", economy: "Economía", equipment: "Equipo", food: "Comida",
  "game-mechanics": "Mecánicas", library: "Bibliotecas", magic: "Magia", management: "Gestión", minigame: "Minijuegos",
  mobs: "Criaturas", optimization: "Optimización", social: "Social", storage: "Almacenamiento", technology: "Tecnología",
  transportation: "Transporte", utility: "Utilidades", worldgen: "Generación de mundo",
  fabric: "Fabric", quilt: "Quilt", forge: "Forge", neoforge: "NeoForge", iris: "Iris", optifine: "OptiFine", canvas: "Canvas",
  vanilla: "Vanilla", minecraft: "Minecraft", datapack: "Data pack",
  audio: "Sonido", blocks: "Bloques", combat: "Combate", "core-shaders": "Shaders básicos", entities: "Criaturas",
  environment: "Entorno", fonts: "Fuentes", gui: "Interfaz", items: "Objetos", locale: "Idioma", modded: "Para mods",
  models: "Modelos", realistic: "Realista", simplistic: "Sencillo", themed: "Temático", tweaks: "Ajustes", "vanilla-like": "Estilo vanilla",
  atmosphere: "Atmósfera", bloom: "Resplandor", "colored-lighting": "Luces de colores", foliage: "Vegetación",
  "path-tracing": "Trazado de rayos", pbr: "PBR", reflections: "Reflejos", shadows: "Sombras", cartoon: "Caricatura",
  fantasy: "Fantasía", "semi-realistic": "Semirrealista", screenshot: "Capturas", high: "Exigente", medium: "Medio",
  low: "Ligero", potato: "Muy ligero", "8x-": "8x o menos", "512x+": "512x o más",
};
const categoryHeaders = { categories: "Categorías", features: "Características", resolutions: "Resolución", "performance impact": "Rendimiento" };
const loaderCategories = new Set(["fabric", "quilt", "forge", "neoforge", "iris", "optifine", "canvas", "vanilla", "minecraft", "datapack", "liteloader", "modloader", "rift", "bukkit", "paper", "spigot", "velocity"]);
const categoryName = (name) => categoryNames[name] ?? name.charAt(0).toUpperCase() + name.slice(1);
const compact = new Intl.NumberFormat("es-MX", { notation: "compact", maximumFractionDigits: 1 });

function relativeDate(iso) {
  const ms = Date.parse(iso);
  if (!ms) return "";
  const days = (Date.now() - ms) / 86400e3;
  if (days < 1) return "hoy";
  if (days < 2) return "ayer";
  if (days < 31) return `hace ${Math.floor(days)} días`;
  if (days < 365) return `hace ${Math.floor(days / 30)} ${Math.floor(days / 30) === 1 ? "mes" : "meses"}`;
  return `hace ${Math.floor(days / 365)} ${Math.floor(days / 365) === 1 ? "año" : "años"}`;
}

function projectIcon(url, size = "") {
  return url
    ? `<img src="${escapeHtml(url)}" alt="" loading="lazy" referrerpolicy="no-referrer">`
    : `<svg class="${size}"><use href="#i-puzzle"/></svg>`;
}

/** Loaders de Modrinth que sirven en una instancia (Quilt también carga mods de Fabric). */
function modLoadersFor(instance) {
  return { Fabric: ["fabric"], Quilt: ["quilt", "fabric"], Forge: ["forge"], NeoForge: ["neoforge"] }[instance.loader] ?? [];
}

const exploreTarget = () => state.instances.find((i) => i.id === state.explore.target) ?? null;

function installLabel(instance) {
  return `${instance.name} · ${instance.loader} ${instance.minecraftVersion}`;
}

function fillTargetSelect(select) {
  const target = state.explore.target;
  select.innerHTML =
    `<option value="">Ninguna (solo ver)</option>` +
    state.instances.map((i) => `<option value="${escapeHtml(i.id)}" ${i.id === target ? "selected" : ""}>${escapeHtml(installLabel(i))}</option>`).join("");
}

async function setExploreTarget(id) {
  state.explore.target = id || null;
  state.explore.installed = new Set();
  state.explore.installedVersions = new Set();
  fillTargetSelect($("#explore-instance"));
  fillTargetSelect($("#project-target"));
  renderExploreFilters();
  const instance = exploreTarget();
  if (instance) {
    try {
      const installed = await invoke("proyectos_instalados", { id: instance.id });
      if (state.explore.target !== instance.id) return;
      state.explore.installed = new Set(installed.projects);
      state.explore.installedVersions = new Set(installed.versions);
    } catch {
      // Sin conexión: simplemente no se marca lo instalado.
    }
  }
  refreshInstallButtons();
}

/** Abre Explorar para agregar contenido a una instancia (desde su pestaña Contenido). */
function openExploreFor(instanceId, contentKind) {
  state.explore.kind = contentToExplore[contentKind] ?? "mod";
  state.explore.categories.clear();
  navigate("explore");
  setExploreTarget(instanceId);
  selectExploreKind(state.explore.kind);
}

function selectExploreKind(kind) {
  const e = state.explore;
  if (e.kind !== kind) e.categories.clear();
  e.kind = kind;
  for (const tab of $$("#explore-kinds .tab[data-kind]")) tab.classList.toggle("active", tab.dataset.kind === kind);
  $("#explore-search").placeholder = exploreKinds[kind].placeholder;
  renderExploreFilters();
  if (kind === "drive") {
    $("#explore-categories").innerHTML = "";
    $("#explore-more").classList.add("hidden");
    return state.drive.data ? renderDrive() : loadDriveCatalog();
  }
  loadCategories(kind);
  runExplore();
}

function renderExploreFilters() {
  const e = state.explore;
  const instance = exploreTarget();
  // Con instancia, el loader y la versión salen de ella; los modpacks crean su propia instancia.
  const modpack = e.kind === "modpack";
  const drive = e.kind === "drive";
  $("#filter-drive").classList.toggle("hidden", !drive);
  $("#explore-sort").classList.toggle("hidden", drive);
  $("#explore-categories").classList.toggle("hidden", drive);
  $("#filter-target").classList.toggle("hidden", modpack || drive);
  $("#filter-loader").classList.toggle("hidden", drive || !(modpack || (e.kind === "mod" && !instance)));
  $("#filter-version").classList.toggle("hidden", drive || (!!instance && !modpack));
  for (const chip of $$("#explore-loaders [data-loader]")) chip.classList.toggle("active", e.loader === chip.dataset.loader);
  const note = $("#explore-instance-note");
  if (!instance) note.textContent = "Elige una instancia para ver solo lo compatible e instalar con un clic.";
  else if (e.kind === "mod" && instance.loader === "Vanilla") note.textContent = "Es vanilla: no carga mods. Elige una con Fabric o Quilt.";
  else if (e.kind === "datapack") note.textContent = `Se muestra solo lo compatible con Minecraft ${instance.minecraftVersion}. Al instalar eliges el mundo.`;
  else note.textContent = `Se muestra solo lo compatible con ${e.kind === "mod" ? instance.loader + " " : "Minecraft "}${instance.minecraftVersion}.`;
  const showNote = instance && e.kind === "mod" && instance.loader === "Vanilla";
  $("#explore-note").classList.toggle("hidden", !showNote);
  $("#explore-note").textContent = showNote ? `${instance.name} es vanilla y no puede cargar mods. Crea una instancia con Fabric o Quilt, o explora resource packs y shaders.` : "";
}

async function loadExploreVersions() {
  if (state.explore.versionsLoaded) return;
  state.explore.versionsLoaded = true;
  try {
    const versions = state.versions ?? (await invoke("listar_versiones"));
    state.versions ??= versions;
    const releases = versions.mojang ? versions.releases.filter((r) => r.type === "release").map((r) => r.id) : versions.installed;
    $("#explore-version").innerHTML = `<option value="">Todas</option>` + releases.map((v) => `<option>${escapeHtml(v)}</option>`).join("");
  } catch {
    state.explore.versionsLoaded = false;
  }
}

async function loadCategories(kind) {
  const box = $("#explore-categories");
  if (!state.explore.categoryList[kind]) {
    try {
      state.explore.categoryList[kind] = await invoke("categorias", { tipo: kind });
    } catch {
      box.innerHTML = "";
      return;
    }
  }
  if (state.explore.kind !== kind) return;
  const groups = {};
  for (const category of state.explore.categoryList[kind]) {
    if (loaderCategories.has(category.name)) continue;
    (groups[category.header] ??= []).push(category);
  }
  box.innerHTML = Object.entries(groups)
    .map(
      ([header, list]) => `<div class="filter-block"><h4>${escapeHtml(categoryHeaders[header] ?? header)}</h4><div class="filter-list">${list
        .sort((a, b) => categoryName(a.name).localeCompare(categoryName(b.name), "es"))
        .map(
          (c) => `<button class="filter-item ${state.explore.categories.has(c.name) ? "active" : ""}" data-category="${escapeHtml(c.name)}">
            <span class="cat-icon" style="--icon:url('${c.icon}')"></span>${escapeHtml(categoryName(c.name))}</button>`,
        )
        .join("")}</div></div>`,
    )
    .join("");
}

let exploreTimer = null;
function queueExplore() {
  clearTimeout(exploreTimer);
  exploreTimer = setTimeout(() => runExplore(), 280);
}

async function runExplore(append = false) {
  const e = state.explore;
  if (e.kind === "drive") return renderDrive();
  const id = ++e.requestId;
  const instance = exploreTarget();
  const modpack = e.kind === "modpack";
  const loaders = modpack ? (e.loader ? [e.loader] : []) : e.kind !== "mod" ? [] : instance ? modLoadersFor(instance) : e.loader ? [e.loader] : [];
  const gameVersion = !modpack && instance ? instance.minecraftVersion : e.version || null;
  if (!append) $("#explore-results").innerHTML = `<div class="content-loading"><div class="spinner"></div></div>`;
  $("#explore-more").disabled = true;
  try {
    const result = await invoke("buscar_contenido", {
      busqueda: { query: e.query, kind: e.kind, loaders, gameVersion, categories: [...e.categories], sort: e.sort, offset: append ? e.hits.length : 0 },
    });
    if (id !== e.requestId) return;
    e.hits = append ? [...e.hits, ...result.hits] : result.hits;
    e.total = result.total;
    renderExploreResults(append ? e.hits.length - result.hits.length : 0);
  } catch (error) {
    if (id !== e.requestId) return;
    if (String(error).includes("No se puede conectar con Modrinth") && !e.autoDrive) {
      e.autoDrive = true;
      toast("Modrinth está bloqueado en esta red: aquí tienes los modpacks de tu Drive.");
      return selectExploreKind("drive");
    }
    $("#explore-results").innerHTML = `<div class="content-empty">${escapeHtml(error)}</div>`;
    $("#explore-more").classList.add("hidden");
  } finally {
    $("#explore-more").disabled = false;
  }
}

const modpackInstalled = (projectId) => state.instances.some((i) => i.modpackProject === projectId);

function installButton(projectId, extraClass = "") {
  const e = state.explore;
  if (e.installing.has(projectId)) return `<button class="btn ${extraClass}" disabled><span class="spinner tiny"></span>Instalando…</button>`;
  const installed = e.kind === "modpack" ? modpackInstalled(projectId) : e.kind !== "datapack" && e.installed.has(projectId);
  if (installed) return `<button class="btn installed ${extraClass}" disabled><svg><use href="#i-check"/></svg>Instalado</button>`;
  return `<button class="btn primary ${extraClass}" data-install="${escapeHtml(projectId)}"><svg><use href="#i-download"/></svg>Instalar</button>`;
}

function renderExploreResults(from = 0) {
  const e = state.explore;
  const list = $("#explore-results");
  if (!e.hits.length) {
    list.innerHTML = `<div class="content-empty">No se encontró nada${e.query ? ` para "${escapeHtml(e.query)}"` : ""}. Prueba con otras palabras o quita filtros.</div>`;
    $("#explore-more").classList.add("hidden");
    return;
  }
  const html = e.hits
    .slice(from)
    .map((hit, index) => {
      const tags = hit.categories.filter((c) => !loaderCategories.has(c)).slice(0, 4);
      return `<div class="result" data-project="${escapeHtml(hit.id)}" style="animation-delay:${Math.min(index, 12) * 25}ms">
        <div class="result-icon">${projectIcon(hit.iconUrl)}</div>
        <div class="grow result-body">
          <div class="r-title"><b>${escapeHtml(hit.title)}</b><span class="muted">por ${escapeHtml(hit.author)}</span></div>
          <div class="r-desc">${escapeHtml(hit.description)}</div>
          <div class="r-tags">${tags.map((t) => `<span class="tag">${escapeHtml(categoryName(t))}</span>`).join("")}</div>
        </div>
        <div class="r-side">
          <div class="r-stats">
            <span title="Descargas">${icon("download")}${compact.format(hit.downloads)}</span>
            <span title="Seguidores">${icon("heart")}${compact.format(hit.follows)}</span>
          </div>
          <div class="faint r-updated">${icon("clock")}Actualizado ${escapeHtml(relativeDate(hit.updated))}</div>
          <div class="r-action">${installButton(hit.id)}</div>
        </div>
      </div>`;
    })
    .join("");
  if (from === 0) list.innerHTML = html;
  else list.insertAdjacentHTML("beforeend", html);
  $("#explore-more").classList.toggle("hidden", e.hits.length >= e.total);
}

function refreshInstallButtons() {
  for (const result of $$("#explore-results .result[data-project]")) $(".r-action", result).innerHTML = installButton(result.dataset.project);
  if (state.page === "project" && state.projectData) renderProjectInstall();
}

// ---------- Instalar ----------
async function installProject(projectId, versionId = null) {
  const e = state.explore;
  const instance = exploreTarget();
  if (!instance) {
    toast('Elige en qué instancia instalarlo en "Instalar en".', "error");
    ($("#page-project").classList.contains("hidden") ? $("#explore-instance") : $("#project-target")).focus();
    return;
  }
  if (e.installing.has(projectId)) return;
  e.installing.add(projectId);
  refreshInstallButtons();
  $("#download-indicator").classList.remove("hidden");
  $("#download-percent").textContent = "…";
  try {
    const plan = await invoke("preparar_instalacion", { id: instance.id, proyecto: projectId, version: versionId });
    if (plan.installed) {
      toast(`${plan.title} ${plan.versionNumber} ya está instalado en ${instance.name}.`);
    } else if (plan.problem) {
      showInstallProblem(plan, projectId);
    } else {
      await invoke("instalar_contenido", { id: instance.id, archivos: plan.files });
      const dependencies = plan.files.filter((f) => f.dependency).map((f) => f.title);
      toast(`Se instaló ${plan.title} ${plan.versionNumber} en ${instance.name}${dependencies.length ? `, con ${dependencies.join(", ")}` : ""}.`);
      for (const file of plan.files) {
        e.installed.add(file.projectId);
        e.installedVersions.add(file.versionId);
      }
      if (state.contentFor === instance.id) {
        state.content = null;
        state.updates = { for: null, list: [] };
      }
      if (plan.optional.length) offerOptional(plan);
    }
  } catch (error) {
    toast(String(error), "error");
  } finally {
    e.installing.delete(projectId);
    $("#download-indicator").classList.add("hidden");
    refreshInstallButtons();
    if (state.page === "project" && state.projectVersions) renderProjectVersions();
  }
}

function showInstallProblem(plan, projectId) {
  openInstallDialog({
    alert: true,
    title: `No se puede instalar ${plan.title}`,
    text: plan.problem,
    items: [],
    ok: state.page === "project" ? "" : "Ver versiones",
    secondary: "Entendido",
    onOk: () => {
      navigate("project", projectId);
      selectProjectTab("versions");
    },
  });
}

function offerOptional(plan) {
  openInstallDialog({
    alert: false,
    title: `${plan.title} está instalado`,
    text: "También funciona con estos mods opcionales. ¿Quieres alguno?",
    items: plan.optional,
    ok: "Instalar los marcados",
    secondary: "No, gracias",
    onOk: async (chosen) => {
      for (const id of chosen) await installProject(id);
    },
  });
}

function openInstallDialog({ alert, title, text, items, ok, secondary, onOk, single = false }) {
  $("#install-alert").classList.toggle("hidden", !alert);
  $("#install-title").textContent = title;
  $("#install-text").textContent = text;
  $("#install-list").innerHTML = items
    .map(
      (p, index) => `<label class="install-item"><input type="${single ? "radio" : "checkbox"}" name="install-choice" value="${escapeHtml(p.id)}" ${single && index === 0 ? "checked" : ""}>
        <span class="content-icon">${projectIcon(p.iconUrl)}</span><b class="grow">${escapeHtml(p.title)}</b></label>`,
    )
    .join("");
  $("#install-ok").textContent = ok;
  $("#install-ok").classList.toggle("hidden", !ok);
  $("#install-secondary").textContent = secondary;
  $("#install-ok").onclick = () => {
    const chosen = $$("#install-list input:checked").map((i) => i.value);
    closeModal("#install-dialog");
    onOk(chosen);
  };
  openModal("#install-dialog");
}

// ======================================================================
// Página de un proyecto
// ======================================================================
async function loadProject(id) {
  state.projectId = id;
  state.projectData = null;
  state.projectVersions = null;
  selectProjectTab("description");
  $("#project-icon").innerHTML = `<div class="spinner"></div>`;
  $("#project-title").textContent = "Cargando…";
  $("#project-summary").textContent = "";
  $("#project-meta").innerHTML = "";
  $("#project-body").replaceChildren();
  $("#project-aside").innerHTML = "";
  $("#project-gallery").innerHTML = "";
  fillTargetSelect($("#project-target"));
  try {
    const project = await invoke("proyecto", { id });
    if (state.projectId !== id) return;
    state.projectData = project;
    renderProject();
  } catch (error) {
    if (state.projectId !== id) return;
    $("#project-icon").innerHTML = icon("alert");
    $("#project-title").textContent = "No se pudo abrir";
    $("#project-summary").textContent = String(error);
  }
}

function renderProject() {
  const p = state.projectData;
  if (state.page === "project") $("#crumb-text").textContent = p.title;
  $("#project-icon").innerHTML = projectIcon(p.iconUrl, "large");
  $("#project-title").textContent = p.title;
  $("#project-summary").textContent = p.description;
  $("#project-meta").innerHTML = `
    <span>${icon("download")}${compact.format(p.downloads)} descargas</span>
    <span>${icon("heart")}${compact.format(p.followers)} seguidores</span>
    <span>${icon("clock")}Actualizado ${escapeHtml(relativeDate(p.updated))}</span>`;
  renderProjectInstall();
  const body = $("#project-body");
  body.replaceChildren(Markdown.toFragment(p.body || p.description));
  const releases = p.gameVersions.filter((v) => /^\d+(\.\d+)+$/.test(v));
  const sides = { required: "Necesario", optional: "Opcional", unsupported: "No se usa", unknown: "Sin datos" };
  $("#project-aside").innerHTML = `
    <div class="card-panel"><h4>Compatibilidad</h4>
      <dl class="mini-details">
        <dt>Minecraft</dt><dd>${releases.length ? `${escapeHtml(releases[0])} – ${escapeHtml(releases.at(-1))}` : escapeHtml(p.gameVersions.at(-1) ?? "—")}</dd>
        <dt>Plataformas</dt><dd>${escapeHtml(p.loaders.map(categoryName).join(", ") || "—")}</dd>
        <dt>En el cliente</dt><dd>${sides[p.clientSide] ?? "—"}</dd>
        <dt>En el servidor</dt><dd>${sides[p.serverSide] ?? "—"}</dd>
      </dl></div>
    <div class="card-panel"><h4>Detalles</h4>
      <dl class="mini-details">
        <dt>Autor</dt><dd>${escapeHtml(p.author || "—")}</dd>
        <dt>Licencia</dt><dd>${escapeHtml(p.license || "—")}</dd>
        <dt>Publicado</dt><dd>${p.published ? new Date(p.published).toLocaleDateString("es-MX") : "—"}</dd>
      </dl>
      <div class="r-tags">${p.categories.filter((c) => !loaderCategories.has(c)).map((c) => `<span class="tag">${escapeHtml(categoryName(c))}</span>`).join("")}</div>
    </div>
    <div class="card-panel"><h4>Enlaces</h4>
      <div class="link-list">
        <a href="https://modrinth.com/${escapeHtml(p.projectType)}/${escapeHtml(p.slug)}">${icon("external")}Ver en Modrinth</a>
        ${p.links.map(([label, url]) => `<a href="${escapeHtml(url)}">${icon("external")}${escapeHtml(label)}</a>`).join("")}
      </div>
    </div>`;
  $("#project-gallery").innerHTML = p.gallery.length
    ? p.gallery
        .map(
          (g, i) => `<figure data-image="${i}"><img src="${escapeHtml(g.url)}" alt="" loading="lazy" referrerpolicy="no-referrer">
          ${g.title ? `<figcaption>${escapeHtml(g.title)}</figcaption>` : ""}</figure>`,
        )
        .join("")
    : `<div class="content-empty">Este proyecto no tiene imágenes.</div>`;
  $('#project-tabs [data-tab="gallery"]').classList.toggle("hidden", !p.gallery.length);
}

function renderProjectInstall() {
  const p = state.projectData;
  const button = $("#project-install");
  const instance = exploreTarget();
  const installing = state.explore.installing.has(p.id);
  const installed = state.explore.installed.has(p.id);
  button.disabled = installing || installed;
  button.className = `btn big ${installed ? "installed" : "primary"}`;
  const kind = projectKind();
  const modpack = kind === "modpack";
  const done = modpack ? modpackInstalled(p.id) : kind !== "datapack" && installed;
  button.disabled = installing || done;
  button.className = `btn big ${done ? "installed" : "primary"}`;
  $("span", button).textContent = installing
    ? "Instalando…"
    : done
      ? "Instalado"
      : modpack
        ? "Instalar modpack"
        : instance
          ? `Instalar en ${instance.name}`
          : "Instalar";
  $("use", button).setAttribute("href", done ? "#i-check" : "#i-download");
  $("#project-target-box").classList.toggle("hidden", modpack);
  $("#project-install-note").classList.toggle("hidden", !modpack);
  $("#project-install-note").textContent = "Se instala como una instancia nueva, con su versión de Minecraft y su loader.";
  $(".project-install").classList.toggle("hidden", !exploreKinds[kind]);
}

function selectProjectTab(tab) {
  for (const button of $$("#project-tabs .tab")) button.classList.toggle("active", button.dataset.tab === tab);
  for (const name of ["description", "versions", "gallery"]) $(`#ptab-${name}`).classList.toggle("hidden", name !== tab);
  if (tab === "versions" && !state.projectVersions) loadProjectVersions();
}

async function loadProjectVersions() {
  const id = state.projectId;
  const p = state.projectData;
  if (!id || !p) return;
  const kind = projectKind();
  const instance = exploreKinds[kind] && kind !== "modpack" ? exploreTarget() : null;
  $("#project-versions").innerHTML = `<div class="content-loading"><div class="spinner"></div></div>`;
  $("#project-versions-note").textContent =
    kind === "modpack"
      ? "Versiones para Fabric y Quilt. Cada una se instala como una instancia nueva."
      : instance
        ? `Versiones compatibles con ${instance.name} (${kind === "mod" ? instance.loader + " " : "Minecraft "}${instance.minecraftVersion}).`
        : "Todas las versiones. Elige una instancia en \"Instalar en\" para ver solo las compatibles.";
  try {
    const versions = await invoke("versiones_proyecto", { id, tipo: kind, instancia: instance?.id ?? null });
    if (state.projectId !== id) return;
    state.projectVersions = versions;
    renderProjectVersions();
  } catch (error) {
    $("#project-versions").innerHTML = `<div class="content-empty">${escapeHtml(error)}</div>`;
  }
}

function renderProjectVersions() {
  const versions = state.projectVersions ?? [];
  const types = { release: ["Estable", "release"], beta: ["Beta", "beta"], alpha: ["Alfa", "alpha"] };
  const instance = exploreTarget();
  $("#project-versions").innerHTML = versions.length
    ? versions
        .slice(0, 150)
        .map((v) => {
          const [typeName, typeClass] = types[v.versionType] ?? [v.versionType, "alpha"];
          const games = v.gameVersions.length > 3 ? `${v.gameVersions[0]} – ${v.gameVersions.at(-1)}` : v.gameVersions.join(", ");
          const kind = projectKind();
          const installed = kind !== "modpack" && kind !== "datapack" && state.explore.installedVersions.has(v.id);
          const action = !exploreKinds[kind] || (!instance && kind !== "modpack")
            ? ""
            : installed
              ? `<button class="btn installed small" disabled><svg><use href="#i-check"/></svg>Instalada</button>`
              : `<button class="btn small" data-install-version="${escapeHtml(v.id)}" ${state.explore.installing.has(state.projectId) ? "disabled" : ""}><svg><use href="#i-download"/></svg>Instalar</button>`;
          return `<div class="content-item">
            <span class="version-type ${typeClass}">${typeName}</span>
            <div class="grow"><div class="c-name">${escapeHtml(v.name || v.versionNumber)}</div>
              <div class="c-sub">${escapeHtml(v.loaders.map(categoryName).join(", "))} · Minecraft ${escapeHtml(games)}</div></div>
            <div class="c-version">${new Date(v.published).toLocaleDateString("es-MX")} · ${compact.format(v.downloads)}</div>
            ${action}
          </div>`;
        })
        .join("")
    : `<div class="content-empty">No hay versiones${instance ? ` para ${escapeHtml(instance.name)}` : ""}.</div>`;
}

// ======================================================================
// Actualizaciones (pestaña Contenido)
// ======================================================================
async function checkUpdates(id) {
  try {
    const list = await invoke("buscar_actualizaciones", { id });
    if (state.current?.id !== id) return;
    state.updates = { for: id, list };
    renderContent();
  } catch {
    // Sin conexión con Modrinth: no se muestran actualizaciones.
  }
}

const availableUpdates = () => (state.updates.for === state.current?.id ? state.updates.list.filter((u) => !state.heldBack.has(u.versionId)) : []);

async function applyUpdates(files) {
  const id = state.current?.id;
  if (!id || !files.length) return;
  const button = $("#content-update-all");
  button.disabled = true;
  $("#download-indicator").classList.remove("hidden");
  $("#download-percent").textContent = "…";
  try {
    const heldBack = await invoke("actualizar_contenido", { id, archivos: files });
    for (const held of heldBack) state.heldBack.set(held.versionId, held.reason);
    const done = files.length - heldBack.length;
    if (done) toast(`Se ${done === 1 ? "actualizó" : "actualizaron"} ${plural(done, "elemento", "elementos")}.`);
    if (heldBack.length) {
      const one = heldBack.length === 1;
      toast(`No se ${one ? "actualizó" : "actualizaron"} porque ${one ? "rompería" : "romperían"} algo: ${heldBack.map((h) => `${h.title} (${h.reason})`).join(" · ")}`, "error");
    }
    state.content = null;
    state.updates = { for: null, list: [] };
    await loadContent();
  } catch (error) {
    toast(String(error), "error");
  } finally {
    button.disabled = false;
    $("#download-indicator").classList.add("hidden");
  }
}

function contentAddMenu() {
  const id = state.current?.id;
  if (!id) return;
  contextMenu($("#content-add"), [
    { label: "Buscar en Modrinth", icon: "compass", action: () => openExploreFor(id, state.kind) },
    { label: "Desde un archivo de la PC", icon: "upload", action: pickContentFiles },
  ]);
}

function openLightbox(index) {
  const image = state.projectData?.gallery[index];
  if (!image) return;
  $("#lightbox-image").src = image.url;
  $("#lightbox-caption").textContent = [image.title, image.description].filter(Boolean).join(" — ");
  $("#lightbox").classList.remove("hidden");
}

function wireExploreEvents() {
  for (const tab of $$("#explore-kinds .tab[data-kind]")) tab.onclick = () => selectExploreKind(tab.dataset.kind);
  $("#explore-search").oninput = () => {
    state.explore.query = $("#explore-search").value.trim();
    queueExplore();
  };
  $("#explore-sort").onchange = () => {
    state.explore.sort = $("#explore-sort").value;
    runExplore();
  };
  $("#explore-instance").onchange = () => {
    setExploreTarget($("#explore-instance").value);
    runExplore();
  };
  $("#project-target").onchange = () => {
    setExploreTarget($("#project-target").value);
    state.projectVersions = null;
    if (!$("#ptab-versions").classList.contains("hidden")) loadProjectVersions();
  };
  $("#explore-version").onchange = () => {
    state.explore.version = $("#explore-version").value;
    runExplore();
  };
  $("#explore-loaders").onclick = (event) => {
    const chip = event.target.closest("[data-loader]");
    if (!chip) return;
    state.explore.loader = state.explore.loader === chip.dataset.loader ? "" : chip.dataset.loader;
    renderExploreFilters();
    runExplore();
  };
  $("#explore-categories").onclick = (event) => {
    const item = event.target.closest("[data-category]");
    if (!item) return;
    const set = state.explore.categories;
    set.has(item.dataset.category) ? set.delete(item.dataset.category) : set.add(item.dataset.category);
    item.classList.toggle("active", set.has(item.dataset.category));
    runExplore();
  };
  $("#explore-more").onclick = () => runExplore(true);
  $("#explore-results").onclick = (event) => {
    const driveInstall = event.target.closest("[data-drive-install]");
    if (driveInstall) return installFromDrive(driveInstall.dataset.driveInstall);
    const driveRepair = event.target.closest("[data-drive-repair]");
    if (driveRepair) return installFromDrive(driveRepair.dataset.driveRepair, true);
    const driveOpen = event.target.closest("[data-drive-open]");
    if (driveOpen) return driveOpen.dataset.driveOpen && navigate("instance", driveOpen.dataset.driveOpen);
    if (event.target.closest("[data-drive]")) return;
    const install = event.target.closest("[data-install]");
    if (install) return installAny(state.explore.kind, install.dataset.install);
    const result = event.target.closest("[data-project]");
    if (result && !event.target.closest("button")) {
      state.projectKind = state.explore.kind;
      navigate("project", result.dataset.project);
    }
  };

  // Página del proyecto
  for (const tab of $$("#project-tabs .tab")) tab.onclick = () => selectProjectTab(tab.dataset.tab);
  $("#project-install").onclick = () => state.projectData && installAny(projectKind(), state.projectData.id);
  $("#project-versions").onclick = (event) => {
    const button = event.target.closest("[data-install-version]");
    if (button) installAny(projectKind(), state.projectId, button.dataset.installVersion);
  };
  $("#project-gallery").onclick = (event) => {
    const figure = event.target.closest("[data-image]");
    if (figure) openLightbox(Number(figure.dataset.image));
  };
  $("#lightbox").onclick = () => $("#lightbox").classList.add("hidden");

  // Ventana de instalar
  for (const id of ["#install-close", "#install-secondary"]) $(id).onclick = () => closeModal("#install-dialog");
  $("#install-dialog").onmousedown = (event) => event.target.id === "install-dialog" && closeModal("#install-dialog");

  // Contenido: agregar, actualizar y abrir el proyecto de un mod
  $("#content-add").onclick = (event) => {
    event.stopPropagation();
    contentAddMenu();
  };
  $("#content-update-all").onclick = () => applyUpdates(availableUpdates());

  // Los enlaces (descripciones, "Ver en Modrinth") se abren en el navegador, nunca dentro del launcher.
  const openLink = (event) => {
    const link = event.target.closest("a[href]");
    if (!link) return;
    event.preventDefault();
    if (/^https?:/i.test(link.href)) invoke("abrir_enlace", { url: link.href }).catch((error) => toast(String(error), "error"));
  };
  document.addEventListener("click", openLink);
  document.addEventListener("auxclick", openLink);
}

// ======================================================================
// Modpacks y data packs: instalar, subir y exportar
// ======================================================================

/** Instala según el tipo: un modpack crea su instancia; un data pack va dentro de un mundo. */
function installAny(kind, projectId, versionId = null) {
  if (kind === "modpack") return installModpack(projectId, versionId);
  if (kind === "datapack") return installDatapack(projectId, versionId);
  return installProject(projectId, versionId);
}

function setBusy(projectId, busy) {
  if (busy) state.explore.installing.add(projectId);
  else state.explore.installing.delete(projectId);
  $("#download-indicator").classList.toggle("hidden", !busy);
  $("#download-percent").textContent = "…";
  refreshInstallButtons();
  if (state.page === "project" && state.projectVersions) renderProjectVersions();
}

async function installModpack(projectId, versionId = null) {
  if (state.explore.installing.has(projectId)) return;
  setBusy(projectId, true);
  try {
    const result = await invoke("instalar_modpack", { proyecto: projectId, version: versionId });
    await reloadInstances();
    toast(`${result.name} está lista para jugar. La encuentras en tu biblioteca.`);
  } catch (error) {
    toast(String(error), "error");
  } finally {
    setBusy(projectId, false);
  }
}

async function installDatapack(projectId, versionId = null) {
  const instance = exploreTarget();
  if (!instance) {
    toast('Elige en qué instancia instalarlo en "Instalar en".', "error");
    return;
  }
  let worlds;
  try {
    worlds = (await invoke("contenido", { id: instance.id })).worlds;
  } catch (error) {
    return toast(String(error), "error");
  }
  if (!worlds.length) {
    return openInstallDialog({
      alert: true,
      title: "Todavía no hay mundos",
      text: `Los data packs van dentro de un mundo y ${instance.name} aún no tiene ninguno. Abre el juego, crea un mundo y vuelve.`,
      items: [],
      ok: "",
      secondary: "Entendido",
    });
  }
  openInstallDialog({
    alert: false,
    title: "¿En qué mundo?",
    text: "Los data packs se instalan dentro de un mundo. Si el mundo ya existe, se activan la próxima vez que lo abras.",
    items: worlds.map((w) => ({ id: w.file, title: w.name, iconUrl: w.icon })),
    ok: "Instalar",
    secondary: "Cancelar",
    single: true,
    onOk: async ([world]) => {
      if (!world) return;
      setBusy(projectId, true);
      try {
        toast(await invoke("instalar_datapack", { id: instance.id, mundo: world, proyecto: projectId, version: versionId }));
        if (state.contentFor === instance.id) state.content = null;
      } catch (error) {
        toast(String(error), "error");
      } finally {
        setBusy(projectId, false);
      }
    },
  });
}

/** Subir un modpack: .mrpack o paquete de Parabellum. Sin ruta, abre la ventana para elegirlo. */
async function importModpack(path = null) {
  if (state.importing) return toast("Ya se está instalando un modpack; espera a que termine.", "error");
  state.importing = true;
  $("#create-step-type").classList.add("hidden");
  $("#create-step-custom").classList.add("hidden");
  $("#create-step-import").classList.remove("hidden");
  $("#create-back").classList.add("hidden");
  $("#create-title").textContent = "Subir un modpack";
  $("#import-title").textContent = path ? path.split(/[\\/]/).pop() : "Elige el archivo del modpack…";
  $("#import-status").textContent = path ? "Preparando…" : "Se abrió una ventana para elegirlo.";
  $("#import-progress div").style.width = "2%";
  openModal("#create-dialog");
  $("#download-indicator").classList.remove("hidden");
  $("#download-percent").textContent = "…";
  try {
    const result = await invoke("importar_modpack", { ruta: path });
    state.importing = false;
    if (!result) return openCreate();
    closeModal("#create-dialog");
    await reloadInstances();
    if (result.kind === "base") {
      toast(`Se instaló ${result.name}. Ya puedes instalar los modpacks de esa versión.`);
    } else {
      toast(result.updated ? `${result.name} se actualizó: tus mundos y opciones se conservaron.` : `${result.name} está lista para jugar.`);
      navigate("instance", result.id);
    }
  } catch (error) {
    $("#import-title").textContent = "No se pudo instalar";
    $("#import-status").textContent = String(error);
    $("#create-back").classList.remove("hidden");
  } finally {
    state.importing = false;
    $("#download-indicator").classList.add("hidden");
  }
}

// ---------- Exportar ----------
const exportHints = {
  mods: "Mods",
  config: "Configuración de los mods",
  resourcepacks: "Resource packs",
  shaderpacks: "Shaders",
  saves: "Mundos (tus partidas)",
  "options.txt": "Opciones del juego (teclas, video, sonido)",
  "servers.dat": "Lista de servidores",
  screenshots: "Capturas de pantalla",
  defaultconfigs: "Configuración inicial",
  kubejs: "Scripts de KubeJS",
};

function formatSize(bytes) {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(bytes < 10 * 1024 ** 2 ? 1 : 0)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

async function openExport(id) {
  const instance = state.instances.find((i) => i.id === id);
  if (!instance) return;
  state.exporting = { id, format: "drive", entries: [], busy: false, file: null, folder: await invoke("carpeta_exportar").catch(() => null), choose: false };
  $("#export-drive").classList.add("hidden");
  $("#export-title").textContent = `Exportar ${instance.name}`;
  $("#export-name").value = instance.name;
  $("#export-version").value = "1.0.0";
  $("#export-description").value = "";
  $("#export-status").textContent = "";
  $("#export-progress").classList.add("hidden");
  resetExportButton();
  selectExportFormat("drive");
  $("#export-list").innerHTML = `<div class="content-loading"><div class="spinner"></div></div>`;
  openModal("#export-dialog");
  try {
    state.exporting.entries = await invoke("contenido_exportable", { id });
    renderExportList();
  } catch (error) {
    $("#export-list").innerHTML = `<div class="content-empty">${escapeHtml(error)}</div>`;
  }
}

function resetExportButton() {
  $("#export-ok").disabled = false;
  $("#export-ok span").textContent = "Exportar…";
  $("#export-ok use").setAttribute("href", "#i-download");
  state.exporting.file = null;
}

function selectExportFormat(format) {
  if (state.exporting.busy) return;
  state.exporting.format = format;
  for (const option of $$("#export-formats [data-format]")) option.classList.toggle("active", option.dataset.format === format);
  $("#export-mrpack-fields").classList.toggle("hidden", format !== "mrpack");
  $("#export-drive").classList.add("hidden");
  updateExportDestination();
  resetExportButton();
  $("#export-status").textContent = "";
}

function renderExportList() {
  $("#export-list").innerHTML = state.exporting.entries
    .map(
      (e) => `<label class="export-item">
        <input type="checkbox" data-entry="${escapeHtml(e.name)}" ${e.checked ? "checked" : ""}>
        ${icon(e.folder ? "folder" : "file")}
        <span class="grow"><b>${escapeHtml(e.name)}</b>${exportHints[e.name.toLowerCase()] ? `<span class="muted"> · ${escapeHtml(exportHints[e.name.toLowerCase()])}</span>` : ""}</span>
        <span class="faint">${formatSize(e.size)}</span>
      </label>`,
    )
    .join("");
  updateExportSize();
}

function updateExportSize() {
  const chosen = new Set($$("#export-list [data-entry]:checked").map((i) => i.dataset.entry));
  const total = state.exporting.entries.filter((e) => chosen.has(e.name)).reduce((sum, e) => sum + e.size, 0);
  $("#export-size").textContent = `${formatSize(total)} de la instancia`;
}

async function runExport() {
  const exporting = state.exporting;
  if (exporting.file) return invoke("mostrar_archivo", { ruta: exporting.file }).catch((error) => toast(String(error), "error"));
  if (exporting.busy) return;
  const include = $$("#export-list [data-entry]:checked").map((i) => i.dataset.entry);
  if (!include.length) return toast("Marca al menos una cosa para incluir.", "error");
  exporting.busy = true;
  $("#export-ok").disabled = true;
  $("#export-status").textContent = exporting.format === "mrpack" ? "Elige dónde guardar el .mrpack…" : "Elige la carpeta donde guardar el paquete…";
  try {
    const result = await invoke("exportar_modpack", {
      id: exporting.id,
      formato: exporting.format,
      opciones: { include, description: $("#export-description").value, name: $("#export-name").value, version: $("#export-version").value },
      elegir: exporting.choose || !exporting.folder,
    });
    if (!result) {
      $("#export-status").textContent = "";
      return;
    }
    $("#export-progress").classList.add("hidden");
    if (exporting.format === "mrpack") {
      exporting.file = result.file;
      $("#export-status").textContent = `Listo: ${plural(result.downloads, "archivo se descargará", "archivos se descargarán")} de Modrinth y ${plural(result.included, "va", "van")} dentro del .mrpack.${result.warning ? ` ${result.warning}` : ""}`;
    } else {
      exporting.file = result.modpack;
      exporting.folder = result.folder.replace(/[\\/][^\\/]+$/, "");
      exporting.choose = false;
      updateExportDestination();
      $("#export-drive").classList.toggle("hidden", !state.driveUrl);
      const folder = result.folder.split(/[\\/]/).pop();
      $("#export-status").textContent = `Listo (${formatSize(result.size)}). Sube la carpeta «${folder}» a tu Drive tal cual o cópiala a una USB: dentro van el modpack y Minecraft ${folder} con su Java.${
        result.baseReused ? " Minecraft ya estaba en esa carpeta y no se volvió a empaquetar." : ""
      }`;
    }
    $("#export-ok span").textContent = "Mostrar en la carpeta";
    $("#export-ok use").setAttribute("href", "#i-folder");
  } catch (error) {
    $("#export-progress").classList.add("hidden");
    $("#export-status").textContent = String(error);
  } finally {
    exporting.busy = false;
    $("#export-ok").disabled = false;
  }
}

function wireModpackEvents() {
  $("#export-formats").onclick = (event) => {
    const option = event.target.closest("[data-format]");
    if (option) selectExportFormat(option.dataset.format);
  };
  $("#export-list").onchange = updateExportSize;
  $("#export-ok").onclick = runExport;
  for (const id of ["#export-close", "#export-cancel"]) $(id).onclick = () => !state.exporting?.busy && closeModal("#export-dialog");
  $("#export-dialog").onmousedown = (event) => event.target.id === "export-dialog" && !state.exporting?.busy && closeModal("#export-dialog");
}

// ======================================================================
// Drive: modpacks y versiones de Minecraft (también en la escuela), publicar y actualizar Parabellum
// ======================================================================

async function loadDriveCatalog(reload = false) {
  const drive = state.drive;
  if (drive.loading) return;
  drive.loading = true;
  if (!drive.data || reload) {
    $("#explore-results").innerHTML = `<div class="content-loading column"><div class="spinner"></div><span class="muted">Leyendo tus paquetes de Drive…</span></div>`;
  }
  try {
    drive.data = await invoke("catalogo", { recargar: reload });
  } catch (error) {
    drive.data = null;
    if (state.explore.kind === "drive") $("#explore-results").innerHTML = `<div class="content-empty">${escapeHtml(error)}</div>`;
    return;
  } finally {
    drive.loading = false;
  }
  if (state.explore.kind === "drive") renderDrive();
}

function driveAction(entry) {
  const drive = state.drive;
  if (drive.installing === entry.id) return `<button class="btn" disabled><span class="spinner tiny"></span>Instalando…</button>`;
  if (entry.unsupported) return `<button class="btn" disabled title="${escapeHtml(entry.unsupported)}">Fase 8</button>`;
  const busy = drive.installing ? "disabled" : "";
  if (entry.kind === "base") {
    return entry.status === "instalado"
      ? `<button class="btn installed" disabled><svg><use href="#i-check"/></svg>Instalada</button>`
      : `<button class="btn primary" data-drive-install="${escapeHtml(entry.id)}" ${busy}><svg><use href="#i-download"/></svg>Instalar</button>`;
  }
  if (entry.status === "actualizar") {
    return `<button class="btn primary" data-drive-install="${escapeHtml(entry.id)}" ${busy}><svg><use href="#i-update"/></svg>Actualizar</button>`;
  }
  if (entry.status === "instalado") {
    const repair = entry.source === "drive"
      ? `<button class="circle-btn" data-drive-repair="${escapeHtml(entry.id)}" ${busy} title="Reparar: vuelve a bajar solo lo que falte o esté dañado">${icon("refresh")}</button>`
      : "";
    return `${repair}<button class="btn" data-drive-open="${escapeHtml(entry.instance ?? "")}"><svg><use href="#i-play"/></svg>Abrir</button>`;
  }
  return `<button class="btn primary" data-drive-install="${escapeHtml(entry.id)}" ${busy}><svg><use href="#i-download"/></svg>Instalar</button>`;
}

function driveCard(entry, index) {
  const base = entry.kind === "base";
  const subtitle = base
    ? `Minecraft ${entry.minecraftVersion}`
    : `${entry.loader} · Minecraft ${entry.minecraftVersion}${entry.modCount ? ` · ${plural(entry.modCount, "mod", "mods")}` : ""}`;
  const description = entry.description || (base ? "Minecraft con sus sonidos, idiomas y Java. Sirve para crear instancias aunque Mojang esté bloqueado." : "");
  const date = entry.exportedAt ? relativeDate(entry.exportedAt) : "";
  return `<div class="result drive-item" data-drive="${escapeHtml(entry.id)}" style="animation-delay:${Math.min(index, 12) * 25}ms">
    <div class="result-icon">${entry.icon ? `<img src="${entry.icon}" alt="">` : icon(base ? "cubes" : "puzzle")}</div>
    <div class="grow result-body">
      <div class="r-title"><b>${escapeHtml(entry.name)}</b><span class="muted">${escapeHtml(subtitle)}</span></div>
      ${description ? `<div class="r-desc">${escapeHtml(description)}</div>` : ""}
      <div class="r-tags">
        <span class="tag">${entry.source === "drive" ? "Google Drive" : "Carpeta paquetes"}</span>
        <span class="tag">${formatSize(entry.size)}</span>
        ${date ? `<span class="tag">Subido ${escapeHtml(date)}</span>` : ""}
        ${entry.status === "actualizar" ? `<span class="tag accent">Versión nueva</span>` : ""}
      </div>
      <div class="r-progress faint" data-progress-for="${escapeHtml(entry.id)}"></div>
    </div>
    <div class="r-side"><div class="r-action">${driveAction(entry)}</div></div>
  </div>`;
}

function renderDrive() {
  const data = state.drive.data;
  if (!data) return;
  const sources = [];
  sources.push(
    data.configured
      ? `<a href="${escapeHtml(data.folderUrl)}">${icon("external")}Carpeta de Google Drive</a>`
      : `<span class="muted">No hay carpeta de Drive configurada.</span>`,
  );
  sources.push(`<button class="link-btn" data-open-packages>${icon("folder")}Carpeta «paquetes» de este PC</button>`);
  if (data.warnings.length) {
    sources.push(`<details class="drive-warnings"><summary>${plural(data.warnings.length, "aviso", "avisos")}</summary>${data.warnings.map((w) => `<p>${escapeHtml(w)}</p>`).join("")}</details>`);
  }
  $("#drive-sources").innerHTML = sources.join("");

  const query = state.explore.query.toLowerCase();
  const matches = (e) => !query || `${e.name} ${e.description} ${e.minecraftVersion} ${e.loader}`.toLowerCase().includes(query);
  const modpacks = data.entries.filter((e) => e.kind === "modpack" && matches(e));
  const bases = data.entries.filter((e) => e.kind === "base" && matches(e));
  // Primero lo que tiene versión nueva, luego lo no instalado.
  const order = { actualizar: 0, nuevo: 1, instalado: 2 };
  // Lo que todavía no se puede instalar (Forge) va al final.
  const rank = (e) => (e.unsupported ? 3 : order[e.status]);
  modpacks.sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name, "es"));
  bases.sort((a, b) => b.minecraftVersion.localeCompare(a.minecraftVersion, "es", { numeric: true }));
  if (!modpacks.length && !bases.length) {
    $("#explore-results").innerHTML = `<div class="content-empty">${
      query
        ? "Nada coincide con la búsqueda."
        : data.entries.length
          ? "No hay modpacks."
          : "Todavía no hay paquetes. Exporta un modpack «Para Drive o USB» y súbelo a la carpeta de Drive, o copia los zips a la carpeta «paquetes» junto a Parabellum."
    }</div>`;
    return;
  }
  let html = "";
  if (modpacks.length) html += `<h3 class="results-title">Modpacks</h3>${modpacks.map(driveCard).join("")}`;
  if (bases.length) {
    html += `<h3 class="results-title">Versiones de Minecraft</h3>
      <p class="muted small-text results-note">Los modpacks instalan la suya solos. Instálalas aparte para crear instancias vanilla aunque Mojang esté bloqueado.</p>
      ${bases.map(driveCard).join("")}`;
  }
  $("#explore-results").innerHTML = html;
  $("#explore-more").classList.add("hidden");
}

async function installFromDrive(id, repair = false) {
  const drive = state.drive;
  if (drive.installing) return toast("Espera a que termine la instalación que está en curso.", "error");
  drive.installing = id;
  renderDrive();
  $("#download-indicator").classList.remove("hidden");
  $("#download-percent").textContent = "…";
  try {
    const result = await invoke("instalar_de_catalogo", { id, reparar: repair });
    await reloadInstances();
    if (result.kind === "base") {
      toast(`${result.name} quedó instalado: ya puedes crear instancias de esa versión.`);
    } else if (result.downloadedFiles != null) {
      const what = result.downloadedFiles ? `se bajaron ${plural(result.downloadedFiles, "archivo", "archivos")} (${formatSize(result.downloadedBytes)})` : "todo estaba bien, no hizo falta bajar nada";
      toast(repair ? `${result.name} revisado: ${what}.` : `${result.name} actualizado: solo ${what}. Tus mundos se conservaron.`);
    } else {
      toast(result.updated ? `${result.name} se actualizó; tus mundos y opciones se conservaron.` : `${result.name} está lista para jugar.`);
    }
  } catch (error) {
    toast(String(error), "error");
  } finally {
    drive.installing = null;
    $("#download-indicator").classList.add("hidden");
    await loadDriveCatalog(false);
  }
}

// ---------- Actualizar y publicar Parabellum ----------
async function checkLauncherUpdate(reload = false, quiet = true) {
  try {
    const update = await invoke("actualizacion_launcher", { recargar: reload });
    state.launcherUpdate = update;
    $("#update-pill").classList.toggle("hidden", !update);
    $("#update-pill").disabled = false;
    if (update) $("#update-text").textContent = `Actualizar a ${update.version}`;
    $("#about-update").textContent = update
      ? `Hay una versión nueva: ${update.version}${update.notes ? ` — ${update.notes.replace(/\.$/, "")}` : ""}.`
      : `Tienes la versión más nueva.`;
    if (!quiet) toast(update ? `Hay una versión nueva de Parabellum: ${update.version}.` : `Ya tienes la versión más nueva (${state.version}).`);
  } catch (error) {
    if (!quiet) toast(String(error), "error");
  }
}

async function applyLauncherUpdate() {
  const update = state.launcherUpdate;
  if (!update) return;
  const ok = await confirmAction({
    title: `Actualizar a Parabellum ${update.version}`,
    text: `${update.notes ? `${update.notes}\n\n` : ""}Se descargará (${formatSize(update.size)}) y Parabellum se reiniciará solo. Los juegos abiertos no se cierran.`,
    ok: "Actualizar y reiniciar",
    primary: true,
  });
  if (!ok) return;
  $("#update-pill").disabled = true;
  $("#update-text").textContent = "Descargando…";
  try {
    await invoke("aplicar_actualizacion");
  } catch (error) {
    toast(String(error), "error");
    $("#update-pill").disabled = false;
    $("#update-text").textContent = `Actualizar a ${update.version}`;
  }
}

async function refreshAbout() {
  $("#about-version").textContent = `Parabellum ${state.version}`;
  const folder = await invoke("carpeta_exportar").catch(() => null);
  $("#about-folder").textContent = folder ?? "Todavía no eliges una (se pide al publicar o exportar).";
  $("#about-open-drive").classList.toggle("hidden", !state.driveUrl);
}

async function publishLauncher() {
  const button = $("#about-publish");
  button.disabled = true;
  try {
    const folder = await invoke("publicar_launcher", { notas: $("#about-notes").value, elegir: false });
    if (folder) {
      toast(`Listo: sube la carpeta «_parabellum» (está en ${folder.replace(/[\\/]_parabellum$/, "")}) a tu Drive. Quien abra Parabellum se actualizará.`);
      refreshAbout();
    }
  } catch (error) {
    toast(String(error), "error");
  } finally {
    button.disabled = false;
  }
}

function wireDriveEvents() {
  $("#drive-reload").onclick = () => loadDriveCatalog(true);
  $("#drive-sources").onclick = (event) => {
    if (event.target.closest("[data-open-packages]")) invoke("abrir_carpeta_paquetes").catch((error) => toast(String(error), "error"));
  };
  $("#update-pill").onclick = applyLauncherUpdate;
  $("#about-check").onclick = () => checkLauncherUpdate(true, false);
  $("#about-folder-change").onclick = async () => {
    try {
      if (await invoke("elegir_carpeta_exportar")) refreshAbout();
    } catch (error) {
      toast(String(error), "error");
    }
  };
  $("#about-publish").onclick = publishLauncher;
  $("#about-open-drive").onclick = () => state.driveUrl && invoke("abrir_enlace", { url: state.driveUrl });
  $("#export-drive").onclick = () => state.driveUrl && invoke("abrir_enlace", { url: state.driveUrl });
  $("#export-folder-change").onclick = () => {
    state.exporting.choose = true;
    updateExportDestination();
  };
}

function updateExportDestination() {
  const exporting = state.exporting;
  $("#export-destination").classList.toggle("hidden", exporting.format !== "drive");
  $("#export-folder-text").textContent =
    exporting.choose || !exporting.folder ? "Elegirás la carpeta al exportar (la de tus paquetes para Drive, o una USB)." : `Se guardará en ${exporting.folder}`;
  $("#export-folder-change").classList.toggle("hidden", exporting.choose || !exporting.folder);
}

// ======================================================================
// Datos de ejemplo (solo al abrir index.html en un navegador, para revisar el diseño)
// ======================================================================
function demoInvoke(command, args = {}) {
  const now = Date.now();
  const instances = [
    { id: "Vanilla 1.21.1", name: "Vanilla 1.21.1", versionId: "1.21.1", minecraftVersion: "1.21.1", loader: "Vanilla", jvmArgs: [], icon: null, ramGb: null, lastPlayed: now - 3600e3, created: now - 86400e3 * 4 },
    { id: "Survival con amigos", name: "Survival con amigos", versionId: "26.2", minecraftVersion: "26.2", loader: "Vanilla", jvmArgs: [], icon: null, ramGb: null, lastPlayed: null, created: now - 3600e3 },
    { id: "Better MC BMC4", name: "Better MC BMC4", versionId: "forge-47.4.20", minecraftVersion: "1.20.1", loader: "Forge", jvmArgs: [], icon: null, ramGb: 8, lastPlayed: now - 86400e3 * 2, created: now - 86400e3 * 10 },
    { id: "Minecraft++", name: "Minecraft++", versionId: "fabric-loader-0.19.5-26.1.2", minecraftVersion: "26.1.2", loader: "Fabric", loaderVersion: "0.19.5", jvmArgs: [], icon: null, ramGb: null, lastPlayed: null, created: now - 86400e3 * 9 },
    { id: "Creativo", name: "Creativo", versionId: "1.21.11", minecraftVersion: "1.21.11", loader: "Vanilla", jvmArgs: [], icon: null, ramGb: null, lastPlayed: now - 86400e3 * 6, created: now - 86400e3 * 20 },
    { id: "Skyblock", name: "Skyblock", versionId: "1.20.1", minecraftVersion: "1.20.1", loader: "Vanilla", jvmArgs: [], icon: null, ramGb: null, lastPlayed: now - 86400e3 * 15, created: now - 86400e3 * 30 },
  ];
  const settings = demoInvoke.settings ?? (demoInvoke.settings = {
    username: "GxxseS", uuid: "", ramGb: 6, theme: "oscuro", windowWidth: 1280, windowHeight: 720, minimizeOnLaunch: true,
    servers: [{ name: "UniversoCraft", address: "mc.universocraft.com", instance: "Vanilla 1.21.1" }],
  });
  const demo = {
    estado_inicial: { settings, offlineUuid: "8b67db88-c6fb-3d4d-bbee-71f84a0e752d", instances, version: "0.1.0 (demostración)", root: "C:\\Users\\Juan\\Desktop\\Parabellum", systemRamGb: 16 },
    listar_instancias: instances,
    uuid_offline: "8b67db88-c6fb-3d4d-bbee-71f84a0e752d",
    juegos_abiertos: [],
    listar_java: [
      { name: "java-runtime-delta", version: "21.0.7", major: 21, path: "C:\\Parabellum\\base\\java\\java-runtime-delta" },
      { name: "java-runtime-gamma", version: "17.0.8", major: 17, path: "C:\\Parabellum\\base\\java\\java-runtime-gamma" },
    ],
    listar_versiones: {
      mojang: true,
      installed: ["1.21.1", "26.1.2"],
      loaders: [{ id: "fabric-loader-0.19.5-26.1.2", loader: "Fabric", loaderVersion: "0.19.5", minecraft: "26.1.2" }],
      releases: [
        { id: "26.3", type: "release", releaseTime: "2026-09-15T10:00:00+00:00" },
        { id: "26.3-pre2", type: "snapshot", releaseTime: "2026-09-08T10:00:00+00:00" },
        { id: "26.2", type: "release", releaseTime: "2026-06-16T10:00:00+00:00" },
        { id: "26.1.2", type: "release", releaseTime: "2026-04-09T10:00:00+00:00" },
        { id: "1.21.11", type: "release", releaseTime: "2025-12-09T10:00:00+00:00" },
        { id: "1.21.1", type: "release", releaseTime: "2024-08-08T10:00:00+00:00" },
        { id: "1.20.1", type: "release", releaseTime: "2023-06-12T10:00:00+00:00" },
      ],
    },
  };
  if (command === "versiones_compatibles")
    return new Promise((r) => setTimeout(() => r({ online: true, games: ["26.3", "26.3-pre2", "26.2", "26.1.2", "1.21.11", "1.21.1", "1.20.1"], message: null }), 300));
  if (command === "versiones_loader")
    return new Promise((r) =>
      setTimeout(() => r({
        online: true,
        versions: args.loader === "quilt"
          ? [{ version: "0.31.0-beta.4", stable: false }, { version: "0.30.1", stable: true }, { version: "0.29.2", stable: true }]
          : [{ version: "0.19.5", stable: true }, { version: "0.19.4", stable: true }, { version: "0.18.2", stable: true }, { version: "0.16.14", stable: true }],
        recommended: args.loader === "quilt" ? "0.30.1" : "0.19.5",
        installed: args.minecraft === "26.1.2" && args.loader === "fabric" ? ["0.19.5"] : [],
        message: null,
      }), 300),
    );
  // Modrinth de ejemplo
  const later = (value, ms = 350) => new Promise((resolve) => setTimeout(() => resolve(value), ms));
  if (command === "buscar_contenido") {
    const names = [
      ["AANobbMI", "Sodium", "jellysquid3", "Un motor de renderizado que mejora muchísimo los FPS y reduce los tirones.", ["fabric", "optimization"], 238896364, 41530],
      ["YL57xq9U", "Iris Shaders", "coderbot", "Un mod de shaders moderno para Fabric, compatible con los packs de OptiFine.", ["fabric", "decoration", "utility"], 120123456, 30120],
      ["P7dR8mSH", "Fabric API", "modmuss50", "Biblioteca esencial para casi todos los mods de Fabric.", ["fabric", "library"], 140000000, 25000],
      ["mOgUt4GM", "Mod Menu", "Prospector", "Agrega un menú para ver y configurar tus mods.", ["fabric", "utility"], 90000000, 18000],
      ["gvQqBUqZ", "Lithium", "jellysquid3", "Optimiza la lógica del juego sin cambiar cómo funciona.", ["fabric", "optimization"], 80000000, 16000],
    ];
    const hits = names.map(([id, title, author, description, categories, downloads, follows]) => ({
      id, slug: title.toLowerCase().replace(/\W+/g, "-"), title, author, description, categories, downloads, follows,
      iconUrl: null, updated: new Date(now - 86400e3 * 9).toISOString(), clientSide: "required", serverSide: "optional",
    }));
    const query = args.busqueda.query.toLowerCase();
    return later({ hits: hits.filter((h) => !query || h.title.toLowerCase().includes(query)), total: hits.length });
  }
  if (command === "categorias")
    return later(["optimization", "utility", "library", "decoration", "adventure", "technology"].map((name) => ({
      name, header: "categories",
      icon: "data:image/svg+xml;base64," + btoa('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="8"/></svg>'),
    })), 100);
  if (command === "proyectos_instalados") return later({ projects: ["P7dR8mSH"], versions: ["v1"] }, 100);
  if (command === "proyecto")
    return later({
      id: args.id, slug: "sodium", title: "Sodium", description: "Un motor de renderizado que mejora muchísimo los FPS.", projectType: "mod",
      author: "CaffeineMC", downloads: 238896364, followers: 41530, categories: ["fabric", "optimization"], loaders: ["fabric", "quilt", "neoforge"],
      gameVersions: ["1.16.5", "1.20.1", "1.21.1", "26.3"], clientSide: "required", serverSide: "unsupported", license: "Polyform Shield 1.0.0",
      updated: new Date(now - 86400e3 * 17).toISOString(), published: "2021-01-03T00:53:34Z", iconUrl: null,
      links: [["Código fuente", "https://github.com/CaffeineMC/sodium"], ["Discord", "https://caffeinemc.net/discord"]],
      gallery: [],
      body: "## Sodium\n\nSodium es un **motor de renderizado** gratuito y de código abierto.\n\n- Mejora los *FPS*\n- Reduce los tirones\n  - también en servidores\n\n| Versión | Estado |\n|---|---|\n| 1.21.1 | Estable |\n\n> Nota: necesita Fabric.\n\n<p align=\"center\"><b>HTML permitido</b></p>\n\n<script>alert('esto no se ejecuta')</script>\n<img src=x onerror=alert(1)>\n<a href=\"javascript:alert(1)\">enlace peligroso</a>\n\n[Página oficial](https://modrinth.com/mod/sodium) y `código`.",
    });
  if (command === "versiones_proyecto")
    return later([
      { id: "v2", projectId: args.id, name: "Sodium 0.6.13", versionNumber: "mc1.21.1-0.6.13-fabric", versionType: "release", published: new Date(now - 86400e3 * 20).toISOString(), downloads: 1200000, loaders: ["fabric", "quilt"], gameVersions: ["1.21", "1.21.1"], filename: "sodium.jar", size: 1, dependencies: [] },
      { id: "v1", projectId: args.id, name: "Sodium 0.6.12 beta", versionNumber: "mc1.21.1-0.6.12-beta.1", versionType: "beta", published: new Date(now - 86400e3 * 40).toISOString(), downloads: 30000, loaders: ["fabric"], gameVersions: ["1.21.1"], filename: "sodium.jar", size: 1, dependencies: [] },
    ]);
  if (command === "preparar_instalacion")
    return later({ projectId: args.proyecto, title: "Mod de ejemplo", versionNumber: "1.0.0", files: [], optional: [{ id: "x", title: "Mod opcional", iconUrl: null }], installed: false, problem: args.proyecto === "YL57xq9U" ? "Iris necesita Sodium 0.6.x, pero la versión es 0.8.13." : null });
  if (command === "instalar_contenido") return later(null, 600);
  if (command === "buscar_actualizaciones")
    return later([{ projectId: "AANobbMI", versionId: "nueva", title: "Sodium", versionNumber: "0.6.13", kind: "mods", filename: "sodium-0.6.13.jar", replaces: "sodium-0.6.0.jar", dependency: false }], 300);
  if (command === "actualizar_contenido") return later([], 500);
  // Drive de ejemplo
  if (command === "catalogo")
    return later({
      configured: true,
      folderUrl: "https://drive.google.com/drive/folders/ejemplo",
      localFolder: "C:\\Parabellum\\paquetes",
      warnings: [],
      entries: [
        { id: "minecraft-plus-plus", kind: "modpack", name: "Minecraft++", description: "Optimización y mejoras visuales para jugar con amigos.", version: "202610042111", minecraftVersion: "26.1.2", loader: "Fabric 0.19.5", modCount: 47, size: 356e6, installedSize: 373e6, exportedAt: "2026-10-04T21:11:56-07:00", icon: null, source: "drive", location: "26.1.2/Minecraft++", status: "actualizar", instance: "Minecraft++", unsupported: null },
        { id: "fabulously-optimized", kind: "modpack", name: "Fabulously Optimized", description: "", version: "202610050000", minecraftVersion: "26.2", loader: "Fabric 0.19.5", modCount: 48, size: 73e6, installedSize: 75e6, exportedAt: "2026-10-05T10:00:00-07:00", icon: null, source: "drive", location: "26.2/Fabulously Optimized", status: "nuevo", instance: null, unsupported: null },
        { id: "better-mc-bmc4", kind: "modpack", name: "Better MC BMC4", description: "", version: "202610010000", minecraftVersion: "1.20.1", loader: "Forge 47.4.20", modCount: 371, size: 1.29e9, installedSize: 1.4e9, exportedAt: "2026-10-01T10:00:00-07:00", icon: null, source: "drive", location: "1.20.1/Better MC BMC4", status: "nuevo", instance: null, unsupported: "Los modpacks de Forge llegan en la fase 8." },
        { id: "minecraft-26.1.2", kind: "base", name: "Minecraft 26.1.2 (base)", description: "", version: "202610042111", minecraftVersion: "26.1.2", loader: "Vanilla", modCount: 0, size: 615e6, installedSize: 753e6, exportedAt: "2026-10-04T21:11:56-07:00", icon: null, source: "drive", location: "26.1.2", status: "instalado", instance: null, unsupported: null },
        { id: "minecraft-1.20.1", kind: "base", name: "Minecraft 1.20.1 (base)", description: "", version: "202610010000", minecraftVersion: "1.20.1", loader: "Vanilla", modCount: 0, size: 727e6, installedSize: 800e6, exportedAt: "2026-10-01T10:00:00-07:00", icon: null, source: "carpeta", location: "1.20.1", status: "nuevo", instance: null, unsupported: null },
      ],
    }, 400);
  if (command === "actualizacion_launcher") return later({ version: "0.7.1", notes: "Arreglos del catálogo de Drive.", size: 7.4e6 }, 200);
  if (command === "carpeta_exportar") return later("D:\\Drive\\Parabellum", 50);
  if (command === "instalar_de_catalogo") return later({ id: "Minecraft++", name: "Minecraft++", kind: "modpack", updated: true, downloadedFiles: 3, downloadedBytes: 8.4e6 }, 900);
  // Modpacks de ejemplo
  if (command === "contenido_exportable")
    return later([
      { name: "config", folder: true, size: 2.4e6, checked: true },
      { name: "mods", folder: true, size: 182e6, checked: true },
      { name: "resourcepacks", folder: true, size: 31e6, checked: true },
      { name: "saves", folder: true, size: 54e6, checked: false },
      { name: "screenshots", folder: true, size: 8e6, checked: false },
      { name: "options.txt", folder: false, size: 5200, checked: true },
      { name: "servers.dat", folder: false, size: 900, checked: true },
    ], 300);
  if (command === "exportar_modpack")
    return later(
      args.formato === "mrpack"
        ? { file: "C:\\Users\\Juan\\Desktop\\Minecraft++ 1.0.0.mrpack", downloads: 44, included: 37, warning: null }
        : { folder: "D:\\Drive\\Parabellum\\26.1.2", modpack: "D:\\Drive\\Parabellum\\26.1.2\\Minecraft++\\Minecraft++.zip", base: "D:\\Drive\\Parabellum\\26.1.2\\base-minecraft-26.1.2.zip", baseReused: false, size: 356e6 },
      900,
    );
  if (command === "importar_modpack") return later({ id: "Minecraft++", name: "Minecraft++", kind: "modpack", updated: false }, 1200);
  if (command === "instalar_modpack") return later({ id: "Minecraft++", name: "Fabulously Optimized", kind: "modpack", updated: false }, 1200);
  if (command === "instalar_datapack") return later("Se instaló Terralith 2.5.8 en el mundo «Mi mundo».", 600);
  if (command === "contenido") {
    const mod = (name, version, authors, enabled = true) => ({ file: `${name.toLowerCase().replace(/\W+/g, "-")}-${version}.jar${enabled ? "" : ".disabled"}`, name, version, authors, enabled, size: 1e6, modified: now - 86400e3, icon: null });
    return new Promise((r) => setTimeout(() => r({
      mods: [Object.assign(mod("Sodium", "0.6.0", "jellysquid3, IMS"), { file: "sodium-0.6.0.jar", projectId: "AANobbMI" }), mod("Fabric API", "0.102.0", "FabricMC"), mod("Iris Shaders", "1.8.0", "coderbot, IMS"), mod("Mod Menu", "11.0.1", "Prospector", false), mod("Lithium", "0.13.0", "jellysquid3")],
      resourcepacks: [{ file: "Faithful 32x.zip", name: "Faithful 32x", version: null, authors: null, enabled: true, size: 3e7, modified: now, icon: null }],
      shaderpacks: [],
      worlds: [{ file: "Mi mundo", name: "Mi mundo", enabled: true, size: 0, modified: now - 7200e3, icon: null }, { file: "Creativo", name: "Creativo", enabled: true, size: 0, modified: now - 86400e3 * 3, icon: null }],
    }), 500));
  }
  if (command === "registro")
    return Promise.resolve({ file: "latest.log", text: "[21:54:42] [Render thread/INFO]: Environment: Environment[sessionHost=https://sessionserver.mojang.com, name=PROD]\n[21:54:42] [Render thread/INFO]: Setting user: GxxseS\n[21:54:50] [Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play\n[21:55:00] [Render thread/INFO]: Connecting to mc.universocraft.com, 25565\n[21:55:02] [Render thread/INFO]: [CHAT] Bienvenido a UniversoCraft\n[21:56:10] [Render thread/ERROR]: Failed to load texture minecraft:textures/entity/foo.png\njava.io.FileNotFoundException: minecraft:textures/entity/foo.png\n\tat net.minecraft.client.renderer.texture.SimpleTexture.load(SimpleTexture.java:40)" });
  if (command === "editar_instancia" || command === "icono_instancia") {
    const instance = instances.find((i) => i.id === args.id);
    return Promise.resolve(Object.assign(instance, args.nombre ? { name: args.nombre, ramGb: args.ramGb, jvmArgs: args.jvmArgs } : { icon: args.png }));
  }
  if (["duplicar_instancia", "eliminar_instancia", "activar_contenido", "eliminar_contenido", "detener", "agregar_contenido", "elegir_contenido", "cambiar_loader"].includes(command))
    return Promise.reject("Modo demostración: esta acción solo funciona en Parabellum.exe.");
  if (command === "guardar_ajustes") return Promise.resolve(Object.assign(settings, args.ajustes));
  if (command === "jugar" || command === "crear_instancia") return Promise.reject("Modo demostración: abre Parabellum.exe para jugar.");
  return Promise.resolve(demo[command]);
}

init().catch((error) => toast(String(error), "error"));
