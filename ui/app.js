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
};

const pages = {
  home: { title: "Inicio", icon: "home", sidebar: true },
  explore: { title: "Explorar", icon: "compass", sidebar: true },
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
function navigate(page, instanceId = null, record = true) {
  const target = { page, instanceId };
  const same = state.page === page && (state.current?.id ?? null) === instanceId;
  if (record && !same) {
    state.back.push({ page: state.page, instanceId: state.current?.id ?? null });
    state.forward = [];
  }
  if (page === "instance") {
    const instance = state.instances.find((i) => i.id === instanceId);
    if (!instance) return navigate("home", null, false);
    state.current = instance;
    renderInstance();
  } else {
    state.current = null;
  }
  state.page = target.page;
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
  for (const button of $$(".rail-btn[data-page]")) button.classList.toggle("active", button.dataset.page === page);
  $(".workspace").classList.toggle("no-sidebar", !pages[page].sidebar);
  $("#crumb-icon").innerHTML = `<use href="#i-${pages[page].icon}"/>`;
  $("#crumb-text").textContent = page === "instance" ? state.current.name : pages[page].title;
  $("#nav-back").disabled = state.back.length === 0;
  $("#nav-forward").disabled = state.forward.length === 0;
  renderRail();
}

function goBack() {
  const previous = state.back.pop();
  if (!previous) return;
  state.forward.push({ page: state.page, instanceId: state.current?.id ?? null });
  navigate(previous.page, previous.instanceId, false);
}

function goForward() {
  const next = state.forward.pop();
  if (!next) return;
  state.back.push({ page: state.page, instanceId: state.current?.id ?? null });
  navigate(next.page, next.instanceId, false);
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
    <dt>Loader</dt><dd>${escapeHtml(i.loader)}</dd>
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
  const query = $("#content-search").value.trim().toLowerCase();
  const items = content[state.kind].filter((i) => !query || `${i.name} ${i.file} ${i.authors ?? ""}`.toLowerCase().includes(query));
  if (!items.length) {
    const none = content[state.kind].length === 0;
    $("#content-list").innerHTML = `<div class="content-empty">${
      none ? `Esta instancia no tiene ${kindNames[state.kind]}. Puedes copiarlos con "Abrir carpeta".` : "Nada coincide con la búsqueda."
    }</div>`;
    return;
  }
  const world = state.kind === "worlds";
  $("#content-list").innerHTML = items
    .slice(0, 600)
    .map((item, index) => {
      const sub = world ? `Jugado ${ago(item.modified).toLowerCase()}` : item.authors ? `por ${item.authors}` : item.file;
      return `<div class="content-item ${item.enabled ? "" : "disabled"}" style="animation-delay:${Math.min(index, 20) * 15}ms">
        <div class="content-icon">${item.icon ? `<img src="${item.icon}" alt="">` : icon(kindIcons[state.kind])}</div>
        <div class="grow"><div class="c-name">${escapeHtml(item.name)}</div><div class="c-sub">${escapeHtml(sub)}</div></div>
        ${world ? "" : `<div class="c-version">${escapeHtml(item.version ?? "")}</div>
          <label class="toggle" title="${item.enabled ? "Desactivar" : "Activar"}"><input type="checkbox" data-toggle="${escapeHtml(item.file)}" ${item.enabled ? "checked" : ""}><span></span></label>`}
        <button class="circle-btn" data-remove="${escapeHtml(item.file)}" title="Mandar a la papelera">${icon("trash")}</button>
      </div>`;
    })
    .join("");
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
    state.back = state.back.filter((h) => h.instanceId !== id);
    state.forward = state.forward.filter((h) => h.instanceId !== id);
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
  updateEditTexts();
  openModal("#edit-dialog");
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
  try {
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
  }
}

/** Ventana de confirmación propia (en lugar de la del sistema). */
function confirmAction({ title, text, ok }) {
  return new Promise((resolve) => {
    $("#confirm-title").textContent = title;
    $("#confirm-text").textContent = text;
    $("#confirm-ok").textContent = ok;
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
  if (state.creating) return openModal("#create-dialog");
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

function versionOptions() {
  const v = state.versions;
  if (!v) return [];
  const installed = new Set(v.installed);
  if (!v.mojang) return v.installed.map((id) => ({ id, type: "release", releaseTime: null, installed: true }));
  return v.releases.map((r) => ({ ...r, installed: installed.has(r.id) }));
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
    : `<div class="version-empty">${state.versions?.mojang === false && !state.versions.installed.length ? "No hay versiones descargadas en este PC." : "Ninguna versión coincide."}</div>`;
}

function selectVersion(id, rerender = true) {
  state.selectedVersion = id;
  if (!state.nameEdited) {
    $("#new-name").value = `Minecraft ${id}`;
    updateCreateArt();
  }
  const option = versionOptions().find((o) => o.id === id);
  if (!state.creating) $("#new-status").textContent = option?.installed ? "Ya está descargada: la instancia se crea al momento." : "Se descargará de los servidores de Mojang, con su Java.";
  $("#new-create").disabled = state.creating;
  if (rerender) renderVersions();
}

async function createInstance() {
  const name = $("#new-name").value.trim();
  if (!name || !state.selectedVersion) return;
  state.creating = true;
  $("#new-create").disabled = true;
  $("#new-progress").classList.remove("hidden");
  $("#new-progress div").style.width = "2%";
  $("#download-indicator").classList.remove("hidden");
  try {
    const instance = await invoke("crear_instancia", { nombre: name, version: state.selectedVersion });
    state.creating = false;
    closeModal("#create-dialog");
    await reloadInstances();
    toast(`${instance.name} está lista para jugar.`);
    navigate("instance", instance.id);
  } catch (error) {
    $("#new-status").textContent = String(error);
  } finally {
    state.creating = false;
    $("#new-create").disabled = false;
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
    if (remove) removeContent(remove.dataset.remove);
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
  $("#create-back").onclick = () => !state.creating && openCreate();
  $("#create-dialog").onmousedown = (event) => event.target.id === "create-dialog" && closeModal("#create-dialog");
  $("[data-type='custom']").onclick = showCustomStep;
  $("#new-cancel").onclick = () => closeModal("#create-dialog");
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

  listen("juego-cerrado", ({ payload }) => onGameClosed(payload));
  listen("progreso", ({ payload }) => {
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
// Datos de ejemplo (solo al abrir index.html en un navegador, para revisar el diseño)
// ======================================================================
function demoInvoke(command, args = {}) {
  const now = Date.now();
  const instances = [
    { id: "Vanilla 1.21.1", name: "Vanilla 1.21.1", versionId: "1.21.1", minecraftVersion: "1.21.1", loader: "Vanilla", jvmArgs: [], icon: null, ramGb: null, lastPlayed: now - 3600e3, created: now - 86400e3 * 4 },
    { id: "Survival con amigos", name: "Survival con amigos", versionId: "26.2", minecraftVersion: "26.2", loader: "Vanilla", jvmArgs: [], icon: null, ramGb: null, lastPlayed: null, created: now - 3600e3 },
    { id: "Better MC BMC4", name: "Better MC BMC4", versionId: "forge-47.4.20", minecraftVersion: "1.20.1", loader: "Forge", jvmArgs: [], icon: null, ramGb: 8, lastPlayed: now - 86400e3 * 2, created: now - 86400e3 * 10 },
    { id: "Minecraft++", name: "Minecraft++", versionId: "fabric-loader-0.19.5-26.1.2", minecraftVersion: "26.1.2", loader: "Fabric", jvmArgs: [], icon: null, ramGb: null, lastPlayed: null, created: now - 86400e3 * 9 },
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
  if (command === "contenido") {
    const mod = (name, version, authors, enabled = true) => ({ file: `${name.toLowerCase().replace(/\W+/g, "-")}-${version}.jar${enabled ? "" : ".disabled"}`, name, version, authors, enabled, size: 1e6, modified: now - 86400e3, icon: null });
    return new Promise((r) => setTimeout(() => r({
      mods: [mod("Sodium", "0.6.0", "jellysquid3, IMS"), mod("Fabric API", "0.102.0", "FabricMC"), mod("Iris Shaders", "1.8.0", "coderbot, IMS"), mod("Mod Menu", "11.0.1", "Prospector", false), mod("Lithium", "0.13.0", "jellysquid3")],
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
  if (["duplicar_instancia", "eliminar_instancia", "activar_contenido", "eliminar_contenido", "detener"].includes(command))
    return Promise.reject("Modo demostración: esta acción solo funciona en Parabellum.exe.");
  if (command === "guardar_ajustes") return Promise.resolve(Object.assign(settings, args.ajustes));
  if (command === "jugar" || command === "crear_instancia") return Promise.reject("Modo demostración: abre Parabellum.exe para jugar.");
  return Promise.resolve(demo[command]);
}

init().catch((error) => toast(String(error), "error"));
