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
  launching: new Set(),
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
  home: { title: t("Inicio", "Home"), icon: "home", sidebar: true },
  explore: { title: t("Explorar", "Explore"), icon: "compass", sidebar: false },
  project: { title: t("Proyecto", "Project"), icon: "compass", sidebar: false },
  servers: { title: t("Servidores", "Servers"), icon: "server", sidebar: true },
  hosted: { title: "", icon: "server", sidebar: false },
  friends: { title: t("Amigos", "Friends"), icon: "users", sidebar: false },
  chat: { title: t("Mensajes", "Messages"), icon: "chat", sidebar: false },
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
  if (!ms) return t("Nunca jugada", "Never played");
  const minutes = (Date.now() - ms) / 60000;
  if (minutes < 1) return t("Hace un momento", "Just now");
  if (minutes < 60) return t(`Hace ${Math.floor(minutes)} min`, `${Math.floor(minutes)} min ago`);
  if (minutes < 1440) return t(`Hace ${Math.floor(minutes / 60)} h`, `${Math.floor(minutes / 60)} h ago`);
  if (minutes < 2880) return t("Ayer", "Yesterday");
  if (minutes < 43200) return t(`Hace ${Math.floor(minutes / 1440)} días`, `${Math.floor(minutes / 1440)} days ago`);
  return new Date(ms).toLocaleDateString(locale());
}

// ---------- Movimiento ----------
// Las mismas curvas que styles.css (--ease-out, --ease-in), para lo que se anima desde aquí.
const EASE_OUT = "cubic-bezier(0.16, 1, 0.3, 1)";
const EASE_IN = "cubic-bezier(0.5, 0, 0.75, 0)";
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
/** Sin animaciones si Windows pide reducir el movimiento o la ventana no se ve. */
const motionOk = () => !reducedMotion.matches && !document.hidden;

/** Lo que va al final de una animación, una sola vez: cuando termine o, si la ventana deja de
 * pintarse (minimizada), a los `ms` como mucho, para que nada se quede a medias. */
function whenDone(animation, done, ms) {
  let called = false;
  const once = () => {
    if (called) return;
    called = true;
    done();
  };
  animation.finished.then(once, once);
  setTimeout(once, ms);
}

/** Pone el HTML solo si cambió. Las listas se animan al aparecer la primera vez, no en cada
 * actualización (si no, parpadean cada vez que llega un dato nuevo). Si sus filas llevan data-key,
 * al cambiar se deslizan a su nuevo lugar, las nuevas aparecen y las que se van se desvanecen.
 * `scope`: si cambia (otra instancia, otro chat…) es otra lista y se anima como la primera vez.
 * `after`: lo que hay que hacer con el HTML nuevo antes de animar (por ejemplo, bajar el scroll). */
function setHtml(element, html, { scope = "", after = null } = {}) {
  if (!element) return false;
  const fresh = element.dataset.html === undefined || (element.dataset.scope ?? "") !== scope;
  if (!fresh && element.dataset.html === html) return false;
  const before = fresh ? null : keyedRects(element);
  element.classList.toggle("no-enter", !fresh);
  element.innerHTML = html;
  element.dataset.html = html;
  element.dataset.scope = scope;
  after?.();
  if (before) animateKeyed(element, before);
  return true;
}

/** Dónde estaba cada fila (data-key) antes de cambiar la lista. */
function keyedRects(element) {
  if (!motionOk() || !element.getClientRects().length) return null;
  const rects = new Map();
  for (const node of element.querySelectorAll("[data-key]")) rects.set(node.dataset.key, { node, rect: node.getBoundingClientRect() });
  return rects.size ? rects : null;
}

/** Cada fila va de donde estaba a donde quedó; solo las que se ven (una lista larga no cuesta). */
function animateKeyed(host, before) {
  const seen = (r) => r.width > 0 && r.bottom > -60 && r.top < innerHeight + 60;
  for (const node of host.querySelectorAll("[data-key]")) {
    const rect = node.getBoundingClientRect();
    const old = before.get(node.dataset.key);
    before.delete(node.dataset.key);
    if (old) {
      const dx = old.rect.left - rect.left;
      const dy = old.rect.top - rect.top;
      if ((Math.abs(dx) > 0.5 || Math.abs(dy) > 0.5) && (seen(rect) || seen(old.rect))) {
        node.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "none" }], { duration: 340, easing: EASE_OUT });
      }
    } else if (seen(rect)) {
      node.animate([{ opacity: 0, transform: "translateY(6px) scale(.98)" }, { opacity: 1, transform: "none" }], { duration: 260, easing: EASE_OUT, fill: "backwards" });
    }
  }
  // Lo que se fue se desvanece en su lugar mientras lo demás se acomoda (es la misma fila, ya fuera
  // de la lista: no se puede pulsar ni la lee el lector de pantalla).
  for (const { node, rect } of before.values()) {
    if (!seen(rect)) continue;
    node.removeAttribute("data-key");
    node.inert = true;
    node.setAttribute("aria-hidden", "true");
    Object.assign(node.style, { position: "absolute", left: "0px", top: "0px", width: `${rect.width}px`, height: `${rect.height}px`, margin: "0", pointerEvents: "none", zIndex: "1" });
    host.appendChild(node);
    const at = node.getBoundingClientRect();
    node.style.left = `${rect.left - at.left}px`;
    node.style.top = `${rect.top - at.top}px`;
    whenDone(node.animate([{ opacity: 1, transform: "none" }, { opacity: 0, transform: "scale(.96)" }], { duration: 200, easing: EASE_IN, fill: "forwards" }), () => node.remove(), 260);
  }
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
  // Los errores se anuncian de inmediato al lector de pantalla; lo demás, cuando termine de hablar.
  if (kind === "error") element.setAttribute("role", "alert");
  element.innerHTML = `${icon(kind === "error" ? "alert" : "check")}<div>${escapeHtml(message)}</div>`;
  $("#toasts").appendChild(element);
  setTimeout(() => {
    if (!motionOk()) return element.remove();
    // Se desvanece y después cierra su hueco: los otros avisos bajan suave en vez de saltar.
    const height = element.offsetHeight;
    const leave = element.animate(
      [
        { opacity: 1, transform: "none", height: `${height}px` },
        { opacity: 0, transform: "translateY(8px) scale(.98)", height: `${height}px`, offset: 0.55 },
        { opacity: 0, height: "0px", paddingTop: "0px", paddingBottom: "0px", marginTop: "-10px" },
      ],
      { duration: 300, easing: EASE_IN, fill: "forwards" },
    );
    whenDone(leave, () => element.remove(), 360);
  }, kind === "error" ? 7000 : 3500);
}

function contextMenu(anchor, items, { matchWidth = false, emojis = null } = {}) {
  $$(".menu").forEach((m) => m.remove());
  const menu = document.createElement("div");
  menu.className = "menu";
  // Con `emojis` ({ list, current, pick }): una fila para reaccionar encima de las entradas.
  const emojiRow = emojis
    ? `<div class="menu-emojis">${emojis.list.map((e) => `<button data-emoji="${escapeHtml(e)}" class="${e === emojis.current ? "current" : ""}" aria-label="${escapeHtml(e)}">${e}</button>`).join("")}</div>${items.length ? "<hr>" : ""}`
    : "";
  // Una entrada puede llevar la cabeza de un jugador en lugar de icono, una línea debajo y una marca.
  menu.innerHTML = items
    .map((item, i) => {
      const lead = item.head ? `<canvas width="8" height="8" class="head" data-head="${escapeHtml(item.head)}"></canvas>` : icon(item.icon);
      const text = item.sub ? `<span class="menu-text">${escapeHtml(item.label)}<small>${escapeHtml(item.sub)}</small></span>` : escapeHtml(item.label);
      const check = item.checked ? `<span class="menu-check">${icon("check")}</span>` : "";
      return `${item.separator ? "<hr>" : ""}<button data-i="${i}" class="${item.danger ? "danger" : ""}">${lead}${text}${check}</button>`;
    })
    .join("");
  menu.insertAdjacentHTML("afterbegin", emojiRow);
  if (matchWidth) menu.style.minWidth = `${Math.max(220, anchor.getBoundingClientRect().width)}px`;
  menu.style.maxHeight = `${innerHeight - 20}px`;
  document.body.appendChild(menu);
  for (const canvas of menu.querySelectorAll("canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
  // Siempre dentro de la ventana: bajo el botón y alineado a su derecha; si no cabe, encima; si el
  // botón está pegado a la izquierda (la barra lateral), a su lado y hacia arriba.
  const rect = anchor.getBoundingClientRect();
  const [width, height] = [menu.offsetWidth, menu.offsetHeight];
  let left = rect.right - width;
  let top = rect.bottom + 6;
  if (left < 10) {
    left = rect.right + 8;
    top = rect.bottom - height;
  } else if (top + height > innerHeight - 10) {
    top = rect.top - height - 6;
  }
  const clamp = (value, max) => Math.max(10, Math.min(value, max));
  menu.style.left = `${clamp(left, innerWidth - width - 10)}px`;
  menu.style.top = `${clamp(top, innerHeight - height - 10)}px`;
  menu.setAttribute("role", "menu");
  const buttons = [...menu.querySelectorAll("button")];
  for (const button of buttons) button.setAttribute("role", "menuitem");
  const close = (refocus) => {
    menu.remove();
    if (refocus && anchor.isConnected) anchor.focus();
  };
  menu.onclick = (event) => {
    const emoji = event.target.closest("[data-emoji]");
    const button = event.target.closest("[data-i]");
    close(!button && !emoji);
    if (emoji) emojis.pick(emoji.dataset.emoji);
    else if (button) items[button.dataset.i].action();
  };
  // Con el teclado: flechas para moverse, Enter para elegir, Escape o Tab para cerrar.
  menu.onkeydown = (event) => {
    const index = buttons.indexOf(document.activeElement);
    const go = (i) => {
      event.preventDefault();
      buttons[(i + buttons.length) % buttons.length]?.focus();
    };
    if (event.key === "ArrowDown") go(index + 1);
    else if (event.key === "ArrowUp") go(index - 1);
    else if (event.key === "Home") go(0);
    else if (event.key === "End") go(buttons.length - 1);
    else if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      event.stopPropagation();
      close(true);
    }
  };
  buttons[0]?.focus({ preventScroll: true });
  setTimeout(() => document.addEventListener("click", () => menu.remove(), { once: true }), 0);
}

// ======================================================================
// Navegación (con historial Atrás/Adelante)
// ======================================================================
/** Lo que identifica la página actual: la instancia o el proyecto abiertos. */
function currentParam() {
  if (state.page === "instance") return state.current?.id ?? null;
  if (state.page === "project") return state.projectId ?? null;
  if (state.page === "hosted") return host.id ?? null;
  return null;
}

function navigate(page, param = null, record = true) {
  // «Hostear partida» ahora es la sección «Tus servidores» de Servidores.
  if (page === "host") page = "servers";
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
  if (page === "hosted") {
    if (!param) return navigate("servers", null, false);
    enterHosted(param);
  }
  if (page === "servers") enterServers();
  if (page === "friends") renderFriendsPage();
  if (page === "chat") enterChat();
  else if (state.chatOpen) closeChat();
  // La página anterior se queda un momento debajo, desvaneciéndose, mientras entra la nueva.
  const leaving = motionOk() ? $$(".page").find((s) => !s.classList.contains("hidden") && !s.classList.contains("leaving") && s.id !== `page-${page}`) : null;
  for (const section of $$(".page")) {
    const active = section.id === `page-${page}`;
    section.classList.remove("leaving");
    section.classList.toggle("hidden", !active && section !== leaving);
    if (active) {
      section.classList.remove("enter");
      void section.offsetWidth;
      section.classList.add("enter");
      section.scrollTop = 0;
    }
  }
  if (leaving) {
    leaving.classList.add("leaving");
    const done = (event) => {
      if (event && event.target !== leaving) return;
      leaving.removeEventListener("animationend", done);
      if (!leaving.classList.contains("leaving")) return;
      leaving.classList.remove("leaving");
      leaving.classList.add("hidden");
    };
    leaving.addEventListener("animationend", done);
    setTimeout(done, 400);
  }
  if (page === "instance") morphArt(param, "#detail-art", "#page-instance");
  if (page === "hosted") morphArt(param, "#hosted-icon", "#page-hosted");
  const railPage = page === "project" ? "explore" : page === "hosted" ? "servers" : page;
  for (const button of $$(".rail-btn[data-page]")) button.classList.toggle("active", button.dataset.page === railPage);
  updateSidebar();
  $("#crumb-icon").innerHTML = `<use href="#i-${pages[page].icon}"/>`;
  $("#crumb-text").textContent =
    page === "instance" ? state.current.name : page === "project" ? (state.projectData?.title ?? pages.project.title) : page === "hosted" ? hostedTitle(param) : pages[page].title;
  $("#nav-back").disabled = state.back.length === 0;
  $("#nav-forward").disabled = state.forward.length === 0;
  renderRail();
}

/** Al abrir una instancia o un servidor desde su tarjeta, la portada viaja de la tarjeta a la cabecera. */
function morphArt(id, targetSelector, pageSelector) {
  const from = state.morphFrom;
  state.morphFrom = null;
  if (!from || from.id !== id || performance.now() - from.at > 800 || !motionOk()) return;
  const target = $(targetSelector);
  const page = $(pageSelector);
  if (!target || !page) return;
  const to = target.getBoundingClientRect();
  if (!to.width || !from.rect.width) return;
  // La página todavía está entrando (un poco más abajo): la portada va a donde va a quedar.
  const top = to.top - (new DOMMatrixReadOnly(getComputedStyle(page).transform).m42 || 0);
  const ghost = target.cloneNode(true);
  ghost.removeAttribute("id");
  ghost.setAttribute("aria-hidden", "true");
  Object.assign(ghost.style, { position: "fixed", left: `${to.left}px`, top: `${top}px`, width: `${to.width}px`, height: `${to.height}px`, margin: "0", zIndex: "50", pointerEvents: "none", transformOrigin: "top left" });
  document.body.appendChild(ghost);
  target.style.visibility = "hidden";
  const scale = from.rect.width / to.width;
  const flight = ghost.animate([{ transform: `translate(${from.rect.left - to.left}px, ${from.rect.top - top}px) scale(${scale})` }, { transform: "none" }], { duration: 420, easing: EASE_OUT });
  whenDone(
    flight,
    () => {
      target.style.visibility = "";
      ghost.remove();
    },
    500,
  );
}

/** El panel de la derecha: lo usan algunas páginas y el jugador lo puede ocultar (se recuerda). */
function updateSidebar() {
  const allowed = !!pages[state.page]?.sidebar;
  const hidden = !!state.settings?.sidebarHidden;
  $(".workspace").classList.toggle("no-sidebar", !allowed || hidden);
  const button = $("#sidebar-toggle");
  button.classList.toggle("hidden", !allowed);
  button.classList.toggle("active", allowed && !hidden);
  const label = hidden ? t("Mostrar el panel (Ctrl+B)", "Show the panel (Ctrl+B)") : t("Ocultar el panel (Ctrl+B)", "Hide the panel (Ctrl+B)");
  button.title = label;
  button.setAttribute("aria-label", label);
  button.setAttribute("aria-pressed", String(!hidden));
}

async function toggleSidebar() {
  if (!pages[state.page]?.sidebar) return;
  state.settings.sidebarHidden = !state.settings.sidebarHidden;
  updateSidebar();
  if (!state.settings.sidebarHidden) {
    const sidebar = $("#sidebar");
    sidebar.classList.remove("enter");
    void sidebar.offsetWidth;
    sidebar.classList.add("enter");
  }
  try {
    await invoke("panel_lateral", { oculto: state.settings.sidebarHidden });
  } catch (error) {
    toast(String(error), "error");
  }
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
  renderHomeTiles();
  renderLibrary();
}

/** Partidas abiertas de tus amigos: servidores a los que te invitaron, mundos abiertos, servidores
 * donde juegan y servidores que hostean (aunque no te hayan invitado). */
function friendActivity() {
  const view = state.social?.friends;
  if (!view) return [];
  const items = [];
  const invitedHosts = new Set();
  for (const server of view.servers ?? []) {
    if (server.online) invitedHosts.add(server.host);
    items.push({ kind: "hosted", key: `hosted:${server.host}:${server.key}`, server, online: server.online });
  }
  for (const friend of view.friends ?? []) {
    const p = friend.presence;
    if (!friend.online || !p) continue;
    if (friend.worldOpen) items.push({ kind: "world", key: `world:${friend.uid}`, friend, online: true });
    if (p.state === "playing" && p.world === "server" && p.server) items.push({ kind: "playing", key: `playing:${friend.uid}`, friend, online: true });
    if (p.hosting && !invitedHosts.has(friend.uid)) items.push({ kind: "uninvited", key: `uninvited:${friend.uid}`, friend, online: true });
  }
  return items;
}

function renderHomeTiles() {
  renderSteps();
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
  const html = [...groups]
    .map(([title, list]) => {
      const tiles = list
        .map((i) => {
          const running = state.running.has(i.id);
          const launching = !running && state.launching.has(i.id);
          return `<div class="tile" data-open="${escapeHtml(i.id)}" data-key="i:${escapeHtml(i.id)}" style="animation-delay:${Math.min(index++, 14) * 30}ms">
            <div style="position:relative">${art(i)}
              <button class="tile-play ${running ? "running" : ""} ${launching ? "launching" : ""}" data-play="${escapeHtml(i.id)}" title="${running ? t("En juego", "In game") : t("Jugar", "Play")}">${icon("play")}</button>
            </div>
            <div class="name">${escapeHtml(i.name)}</div>
            <div class="sub">${escapeHtml(subtitle(i))}</div>
          </div>`;
        })
        .join("");
      return `${title ? `<div class="group-title" data-key="g:${escapeHtml(title)}">${escapeHtml(title)}<span>${list.length}</span></div>` : ""}<div class="library-grid">${tiles}</div>`;
    })
    .join("");
  // Al buscar o al cambiar algo, las tarjetas se acomodan; al cambiar el orden o la agrupación, entran de nuevo.
  setHtml($("#library"), html, { scope: `${state.sort}|${state.group}` });
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
  // Con icono propio el elemento es "art has-icon xl": el id se pone al principio para no perderlo.
  $("#detail-art").outerHTML = art(i, "xl").replace('<div class="art', '<div id="detail-art" class="art');
  $("#detail-name").textContent = i.name;
  $("#detail-meta").innerHTML = `
    <span>${icon("cubes")}${escapeHtml(subtitle(i))}</span>
    <span>${icon("play-outline")}${escapeHtml(state.running.has(i.id) ? t("Jugando ahora", "Playing now") : ago(i.lastPlayed))}</span>`;
  $("#detail-rows").innerHTML = `
    <dt>${t("Versión de Minecraft", "Minecraft version")}</dt><dd>${escapeHtml(i.minecraftVersion)}</dd>
    ${i.modpack ? `<dt>Modpack</dt><dd>${escapeHtml(i.modpack)}</dd>` : ""}
    <dt>Loader</dt><dd>${escapeHtml(i.loader)}${i.loaderVersion ? " " + escapeHtml(i.loaderVersion) : ""}</dd>
    <dt>${t("Versión del perfil", "Profile version")}</dt><dd>${escapeHtml(i.versionId)}</dd>
    <dt>${t("Carpeta", "Folder")}</dt><dd>instancias\\${escapeHtml(i.id)}</dd>
    <dt>${t("Memoria", "Memory")}</dt><dd>${i.ramGb ? t(`${i.ramGb} GB (propia de esta instancia)`, `${i.ramGb} GB (this instance's own)`) : t(`${state.settings.ramGb} GB (la general de Ajustes)`, `${state.settings.ramGb} GB (the general one from Settings)`)}</dd>
    <dt>${t("Argumentos de Java", "Java arguments")}</dt><dd>${i.jvmArgs.length ? escapeHtml(i.jvmArgs.join(" ")) : "—"}</dd>
    <dt>${t("Creada", "Created")}</dt><dd>${i.created ? new Date(i.created).toLocaleString(locale()) : "—"}</dd>`;
  const running = state.running.has(i.id);
  const stop = $("#detail-stop");
  // Al empezar a jugar, «Detener» aparece con un pequeño rebote en lugar de «Jugar».
  if (running && stop.classList.contains("hidden") && motionOk()) stop.animate([{ transform: "scale(.9)", opacity: 0 }, { transform: "none", opacity: 1 }], { duration: 300, easing: "cubic-bezier(0.34, 1.45, 0.64, 1)" });
  $("#detail-play").classList.toggle("hidden", running);
  stop.classList.toggle("hidden", !running);
  showLaunching();
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
const kindNames = { mods: "mods", resourcepacks: "resource packs", shaderpacks: "shaders", worlds: t("mundos", "worlds") };
const kindIcons = { mods: "puzzle", resourcepacks: "image", shaderpacks: "sun-sparkle", worlds: "globe" };

async function loadContent() {
  const id = state.current?.id;
  if (!id) return;
  setHtml($("#content-list"), `<div class="content-loading"><div class="spinner"></div></div>`, { scope: "cargando" });
  try {
    const content = await invoke("contenido", { id });
    if (state.current?.id !== id) return;
    state.content = content;
    renderContent();
    // Buscar actualizaciones en segundo plano (como mucho cada 10 minutos por instancia).
    if (state.updates.for !== id || Date.now() - state.updates.at > 600e3) checkUpdates(id);
  } catch (error) {
    setHtml($("#content-list"), `<div class="content-empty">${escapeHtml(error)}</div>`, { scope: "error" });
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
  $("#content-favorites").classList.toggle("hidden", !favoriteKinds[state.kind]);
  $("#content-update-all").classList.toggle("hidden", !updates.length);
  $("#content-update-all span").textContent = t(`Actualizar todo (${updates.length})`, `Update all (${updates.length})`);
  const query = $("#content-search").value.trim().toLowerCase();
  const items = content[state.kind].filter((i) => !query || `${i.name} ${i.file} ${i.authors ?? ""}`.toLowerCase().includes(query));
  if (!items.length) {
    const none = content[state.kind].length === 0;
    const vanillaMods = state.kind === "mods" && state.current?.loader === "Vanilla";
    setHtml($("#content-list"), `<div class="content-empty">${
      !none
        ? t("Nada coincide con la búsqueda.", "Nothing matches the search.")
        : vanillaMods
          ? t("Las instancias vanilla no cargan mods. Crea una instancia con un loader (Fabric, Quilt, Forge o NeoForge) para usarlos.", "Vanilla instances do not load mods. Create an instance with a loader (Fabric, Quilt, Forge or NeoForge) to use them.")
          : t(`Esta instancia no tiene ${kindNames[state.kind]}. Arrástralos a la ventana o usa "Agregar contenido".`, `This instance has no ${kindNames[state.kind]}. Drag them onto the window or use "Add content".`)
    }</div>`, { scope: contentScope() });
    return;
  }
  const world = state.kind === "worlds";
  const html = items
    .slice(0, 600)
    .map((item, index) => {
      const sub = world ? `${t("Jugado", "Played")} ${ago(item.modified).toLowerCase()}${item.version ? ` · ${item.version}` : ""}` : item.authors ? t(`por ${item.authors}`, `by ${item.authors}`) : item.file;
      return `<div class="content-item ${item.enabled ? "" : "disabled"}" data-key="${escapeHtml(item.file)}" style="animation-delay:${Math.min(index, 20) * 15}ms">
        <div class="content-icon">${item.icon ? `<img src="${item.icon}" alt="" loading="lazy" decoding="async">` : icon(kindIcons[state.kind])}</div>
        <div class="grow">${
          item.projectId
            ? `<button class="c-name link" data-project-open="${escapeHtml(item.projectId)}" title="${t("Ver su página", "See its page")}">${escapeHtml(item.name)}</button>`
            : `<div class="c-name">${escapeHtml(item.name)}</div>`
        }<div class="c-sub">${escapeHtml(sub)}</div></div>
        ${updateChip(item.file)}
        ${world ? "" : `<div class="c-version">${escapeHtml(item.version ?? "")}</div>
          <label class="toggle" title="${item.enabled ? t("Desactivar", "Disable") : t("Activar", "Enable")}"><input type="checkbox" data-toggle="${escapeHtml(item.file)}" ${item.enabled ? "checked" : ""}><span></span></label>`}
        ${
          item.projectId && favoriteKinds[state.kind]
            ? `<button class="circle-btn fav-toggle ${isFavoriteMod(item.projectId) ? "starred" : ""}" data-fav="${escapeHtml(item.file)}" title="${isFavoriteMod(item.projectId) ? t("Quitar de favoritos", "Remove from favorites") : t("Agregar a favoritos", "Add to favorites")}">${icon("star")}</button>`
            : ""
        }
        <button class="circle-btn" data-remove="${escapeHtml(item.file)}" title="${t("Mandar a la papelera", "Move to the Recycle Bin")}">${icon("trash")}</button>
      </div>`;
    })
    .join("");
  // Al buscar, quitar o actualizar, las filas se acomodan; otra instancia u otro tipo entra de nuevo.
  setHtml($("#content-list"), html, { scope: contentScope() });
}

const contentScope = () => `${state.current?.id}|${state.kind}`;

/** Botón de actualizar (o aviso de que la nueva versión rompería algo) junto a un elemento. */
function updateChip(file) {
  if (state.updates.for !== state.current?.id) return "";
  const update = state.updates.list.find((u) => u.replaces === file);
  if (!update) return "";
  const reason = state.heldBack.get(update.versionId);
  return reason
    ? `<span class="update-chip held" title="${escapeHtml(t(`${update.versionNumber} no se instaló: ${reason}`, `${update.versionNumber} was not installed: ${reason}`))}">${icon("alert")}${t("No compatible", "Not compatible")}</span>`
    : `<button class="update-chip" data-update="${escapeHtml(file)}" title="${t("Actualizar a", "Update to")} ${escapeHtml(update.versionNumber)}">${icon("update")}${escapeHtml(update.versionNumber)}</button>`;
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
    title: state.kind === "worlds" ? t("¿Eliminar este mundo?", "Delete this world?") : t("¿Eliminar este archivo?", "Delete this file?"),
    text: t(`"${item?.name ?? file}" se moverá a la papelera de reciclaje; desde ahí lo puedes recuperar.`, `"${item?.name ?? file}" will be moved to the Recycle Bin; you can restore it from there.`),
    ok: t("Mandar a la papelera", "Move to the Recycle Bin"),
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
  const counts = { mods: ["mod", "mods"], resourcepacks: ["resource pack", "resource packs"], shaderpacks: ["shader", "shaders"], worlds: [t("mundo", "world"), t("mundos", "worlds")] };
  const parts = Object.entries(counts).filter(([kind]) => added[kind]).map(([kind, [one, many]]) => plural(added[kind], one, many));
  const total = Object.keys(counts).reduce((sum, kind) => sum + added[kind], 0);
  const joined = parts.length > 1 ? `${parts.slice(0, -1).join(", ")} ${t("y", "and")} ${parts.at(-1)}` : parts[0];
  if (parts.length) toast(t(`Se ${total === 1 ? "agregó" : "agregaron"} ${joined}.`, `Added ${joined}.`));
  if (added.skipped.length) {
    const more = added.skipped.length > 3 ? t(` (y ${added.skipped.length - 3} más)`, ` (and ${added.skipped.length - 3} more)`) : "";
    toast(t(`No se ${added.skipped.length === 1 ? "agregó" : "agregaron"}: `, "Not added: ") + `${added.skipped.slice(0, 3).join(" · ")}${more}`, "error");
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
    : `<span class="l-dim">${query ? t("Ninguna línea coincide.", "No line matches.") : t("Esta instancia todavía no tiene registro: se crea al abrir el juego.", "This instance has no log yet: it is created when the game opens.")}</span>`;
  if (scrollToEnd || $("#log-follow").checked) box.scrollTop = box.scrollHeight;
}

// ---------- Acciones sobre instancias ----------
function instanceMenuItems(id) {
  return [
    { icon: "info", label: t("Ver instancia", "View instance"), action: () => navigate("instance", id) },
    { icon: "settings", label: t("Ajustes de la instancia", "Instance settings"), action: () => openEdit(id) },
    { icon: "folder", label: t("Abrir carpeta", "Open folder"), action: () => invoke("abrir_carpeta", { id }) },
    { icon: "wrench", label: t("Diagnosticar y arreglar", "Diagnose and fix"), action: () => openDoctor(id, "manual") },
    { icon: "check-circle", label: t("Verificar y reparar archivos", "Check and repair files"), action: () => verifyInstance(id) },
    { icon: "duplicate", label: t("Duplicar", "Duplicate"), action: () => duplicateInstance(id) },
    { icon: "chat", label: t("Compartir con un amigo", "Share with a friend"), action: () => shareWithMenu(pointerAnchor(), id) },
    { icon: "upload", label: t("Exportar modpack", "Export modpack"), action: () => openExport(id) },
    { icon: "flag", label: t("Reportar problema", "Report a problem"), action: () => openReport(id) },
    { icon: "trash", label: t("Eliminar", "Delete"), danger: true, action: () => deleteInstance(id) },
  ];
}

/** Comprueba la huella de cada archivo del modpack y repone lo que falte o esté dañado. */
async function verifyInstance(id) {
  const instance = state.instances.find((i) => i.id === id);
  if (!instance) return;
  if (state.verifying) return toast(t("Espera a que termine la verificación que está en curso.", "Wait for the current check to finish."), "error");
  state.verifying = id;
  toast(t(`Verificando los archivos de ${instance.name}…`, `Checking the files of ${instance.name}…`));
  $("#download-indicator").classList.remove("hidden");
  $("#download-percent").textContent = "…";
  try {
    const report = await invoke("verificar_instancia", { id });
    const names = `${report.repaired.slice(0, 3).join(", ")}${report.repaired.length > 3 ? t(` y ${report.repaired.length - 3} más`, ` and ${report.repaired.length - 3} more`) : ""}`;
    const repaired = report.repaired.length ? t(`se ${report.repaired.length === 1 ? "repuso" : "repusieron"}: ${names}`, `restored: ${names}`) : "";
    if (report.failed.length) {
      const list = report.failed.slice(0, 4).join("\n") + (report.failed.length > 4 ? t(`\n…y ${report.failed.length - 4} más`, `\n…and ${report.failed.length - 4} more`) : "");
      const missing = t(`faltan ${plural(report.failed.length, "archivo", "archivos")}`, `${plural(report.failed.length, "file is", "files are")} missing`);
      toast(`${instance.name}: ${repaired ? `${repaired}. ${t("Pero", "But")} ` : ""}${missing}:\n${list}`, "error");
    } else if (repaired) {
      toast(t(`${instance.name} quedó completo: ${repaired}.`, `${instance.name} is complete: ${repaired}.`));
    } else if (report.checked) {
      toast(t(`${instance.name} está completo: ${plural(report.checked, "archivo revisado", "archivos revisados")}, todos bien.`, `${instance.name} is complete: ${plural(report.checked, "file checked", "files checked")}, all fine.`));
    } else {
      toast(t(`${instance.name} no viene de un modpack, así que no hay con qué compararla. Lo que pide su configuración está completo.`, `${instance.name} does not come from a modpack, so there is nothing to compare it with. What its configuration asks for is complete.`));
    }
    if (state.contentFor === id) {
      state.content = null;
      if (state.page === "instance" && state.current?.id === id) renderInstance();
    }
  } catch (error) {
    toast(String(error), "error");
  } finally {
    state.verifying = null;
    $("#download-indicator").classList.add("hidden");
  }
}

// ---------- Accesos directos (Escritorio y menú Inicio) ----------
const shortcutHints = {
  desktop: t("Para abrir Parabellum sin buscar su carpeta. Sigue siendo portable: no se instala nada.", "To open Parabellum without looking for its folder. It is still portable: nothing is installed."),
  startMenu: t("Aparece al buscar «Parabellum» en Inicio.", "It shows up when you search «Parabellum» in Start."),
  other: t("Hay uno que abre Parabellum desde otra carpeta. Actívalo para que abra este.", "There is one that opens Parabellum from another folder. Turn it on so it opens this one."),
};

function renderShortcuts(status) {
  for (const [key, id] of [["desktop", "shortcut-desktop"], ["startMenu", "shortcut-start"]]) {
    $(`#${id}`).checked = status[key] === "si";
    $(`#${id}-hint`).textContent = status[key] === "otro" ? shortcutHints.other : shortcutHints[key];
  }
}

async function loadShortcuts() {
  try {
    renderShortcuts(await invoke("accesos_directos"));
  } catch (error) {
    toast(String(error), "error");
  }
}

async function toggleShortcut(place, input) {
  input.disabled = true;
  try {
    renderShortcuts(await invoke("acceso_directo", { lugar: place, crear: input.checked }));
  } catch (error) {
    input.checked = !input.checked;
    toast(String(error), "error");
  } finally {
    input.disabled = false;
  }
}

// ---------- Doctor: qué falla y cómo arreglarlo ----------
/** mode: "crash" (se cerró con error), "prelaunch" (la revisión encontró algo antes de abrir) o "manual". */
async function openDoctor(id, mode, { diagnosis = null, reason = null, server = null } = {}) {
  const instance = state.instances.find((i) => i.id === id);
  if (!instance) return;
  state.doctor = { id, mode, server, diagnosis, applied: [], busy: false, reason, steps: [], result: null, loading: !diagnosis, error: null };
  $("#doctor-art").innerHTML = art(instance, "doc");
  $("#doctor-title").textContent = instance.name;
  state.doctor.meta = [instance.loader === "Vanilla" ? null : `${instance.loader} ${instance.loaderVersion ?? ""}`.trim(), `Minecraft ${instance.minecraftVersion}`].filter(Boolean);
  renderDoctor();
  openModal("#doctor-dialog");
  if (!diagnosis) {
    try {
      state.doctor.diagnosis = await invoke("diagnosticar", { id });
    } catch (error) {
      state.doctor.error = String(error);
    }
    state.doctor.loading = false;
    renderDoctor();
  }
}

const severityNames = () => ({ error: t("Impide abrir", "Blocks opening"), aviso: t("Posible causa", "Possible cause"), info: t("Para saber", "Good to know") });
const severityIcons = { error: "octagon", aviso: "triangle", info: "info", ok: "check" };

/** La prueba de un hallazgo: archivos (como fichas) o la línea del registro. */
function evidenceHtml(text) {
  if (!text) return "";
  const parts = text.split(/\s+(→|✕|·)\s+/);
  const isFile = (p) => /\.(jar|disabled|zip|json|json5|toml|cfg|txt|dat|mrpack)$/i.test(p.trim()) || p.trim() === "world";
  if (parts.filter((_, i) => i % 2 === 0).every(isFile)) {
    const sep = { "→": icon("right"), "✕": icon("close"), "·": "" };
    return `<div class="finding-files">${parts
      .map((p, i) => (i % 2 ? `<span class="finding-sep">${sep[p] ?? ""}</span>` : `<span class="file-chip">${icon(/\.jar(\.disabled)?$/i.test(p.trim()) ? "puzzle" : "file")}<span>${escapeHtml(p)}</span></span>`))
      .join("")}</div>`;
  }
  return `<pre class="finding-log">${escapeHtml(text)}</pre>`;
}

/** Lista de hallazgos con sus arreglos (la usan el doctor y la página de un servidor). */
function findingsHtml(findings, { applied = [], busy = false, attr = "data-doctor-fix" } = {}) {
  const names = severityNames();
  return findings
    .map(
      (f, i) => `<li class="finding ${f.severity}">
        <span class="finding-mark" title="${escapeHtml(names[f.severity] ?? "")}">${icon(severityIcons[f.severity] ?? "info")}</span>
        <div class="finding-main">
          <h3>${escapeHtml(f.title)}</h3>
          <p>${escapeHtml(f.detail)}</p>
          ${evidenceHtml(f.evidence)}
          ${
            f.fixes.length
              ? `<div class="finding-actions">${f.fixes
                  .map((o, j) =>
                    applied.some((a) => JSON.stringify(a) === JSON.stringify(o.fix))
                      ? `<span class="fix-done">${icon("check")}${escapeHtml(o.label)}</span>`
                      : `<button class="btn ${o.recommended ? "suggested" : "ghost"}" ${attr}="${i}:${j}" ${busy ? "disabled" : ""}>${escapeHtml(o.label)}</button>`,
                  )
                  .join("")}</div>`
              : ""
          }
        </div>
      </li>`,
    )
    .join("");
}

/** Una fila de estado (cargando, todo bien, ya abre…) con el mismo aspecto que un hallazgo. */
function noteRow(kind, title, detail = "", extra = "") {
  const mark = kind === "loading" ? `<span class="spinner tiny"></span>` : icon(severityIcons[kind] ?? "info");
  return `<li class="finding ${kind}"><span class="finding-mark">${mark}</span><div class="finding-main"><h3>${escapeHtml(title)}</h3>${detail ? `<p>${escapeHtml(detail)}</p>` : ""}${extra}</div></li>`;
}

/** [tono, texto] de la línea de estado de arriba. */
function doctorState(doctor, findings) {
  const blocking = findings.filter((f) => f.severity === "error").length;
  if (doctor.busy) return ["work", doctor.auto ? t("Probando el juego…", "Testing the game…") : t("Arreglando…", "Fixing…")];
  if (doctor.loading) return ["work", t("Revisando…", "Checking…")];
  if (doctor.result?.ok) return ["ok", t("Ya abre", "It opens now")];
  if (doctor.result)
    return [
      "bad",
      doctor.result.attempts
        ? t(`Lo abrí ${plural(doctor.result.attempts, "vez", "veces")} y todavía no llega al menú`, `I opened it ${plural(doctor.result.attempts, "time", "times")} and it still does not reach the menu`)
        : t("Todavía no abre", "It still does not open"),
    ];
  if (doctor.mode === "crash") return ["bad", t("Se cerró con un error", "It closed with an error")];
  if (!findings.length) return ["ok", t("Todo en orden", "All good")];
  if (blocking) {
    return doctor.mode === "prelaunch"
      ? ["bad", t(`${plural(blocking, "problema impide", "problemas impiden")} abrirlo`, `${plural(blocking, "problem keeps", "problems keep")} it from opening`)]
      : ["bad", t(`${plural(blocking, "problema", "problemas")} por arreglar`, `${plural(blocking, "problem", "problems")} to fix`)];
  }
  return ["warn", t(`${plural(findings.length, "cosa", "cosas")} por revisar`, `${plural(findings.length, "thing", "things")} to check`)];
}

function renderDoctor() {
  const doctor = state.doctor;
  if (!doctor) return;
  const findings = doctor.diagnosis?.findings ?? [];
  const [tone, label] = doctorState(doctor, findings);
  const status = $("#doctor-state");
  status.className = `doc-state ${tone}`;
  status.innerHTML = `${tone === "work" ? `<span class="spinner tiny"></span>` : "<i></i>"}${escapeHtml(label)}`;
  const checked = doctor.diagnosis?.checkedMods;
  $("#doctor-meta").textContent = [...doctor.meta, checked ? t(`${plural(checked, "mod revisado", "mods revisados")}`, `${plural(checked, "mod checked", "mods checked")}`) : null].filter(Boolean).join(" · ");

  let rows;
  if (doctor.error) rows = noteRow("error", t("No se pudo revisar", "Could not check"), doctor.error);
  else if (doctor.loading) rows = noteRow("loading", t("Leyendo los mods y el último registro…", "Reading the mods and the last log…"));
  else if (doctor.result?.ok) {
    const applied = doctor.result.applied.map((a) => `<span class="fix-done">${icon("check")}${escapeHtml(a)}</span>`).join("");
    rows = noteRow(
      "ok",
      t("Llega al menú principal", "It reaches the main menu"),
      doctor.result.applied.length ? t("Esto es lo que arreglé:", "This is what I fixed:") : t("No hizo falta arreglar nada.", "Nothing needed fixing."),
      applied ? `<div class="finding-actions">${applied}</div>` : "",
    );
    // Avisos que el juego muestra al abrir y no se pudieron quitar solos.
    rows += findingsHtml(doctor.result.warnings?.findings ?? [], { applied: doctor.applied, busy: doctor.busy, attr: "data-doctor-warning" });
  } else if (doctor.busy && doctor.auto) rows = ""; // mientras prueba, los pasos dicen lo que hace
  else if (findings.length) rows = findingsHtml(findings, { applied: doctor.applied, busy: doctor.busy });
  else if (doctor.busy) rows = "";
  else if (doctor.mode === "crash" || doctor.result)
    rows = noteRow(
      "info",
      t("No reconozco la causa", "I do not recognize the cause"),
      t("El registro no trae un error que conozca. «Probar y arreglar solo» abre el juego, mira qué pasa y lo arregla.", "The log has no error I know. «Test and fix» opens the game, sees what happens and fixes it."),
    );
  else
    rows = noteRow(
      "ok",
      t("No encontré nada roto", "Nothing broken found"),
      t("Si aun así falla, «Probar y arreglar solo» abre el juego, mira qué pasa y lo arregla.", "If it still fails, «Test and fix» opens the game, sees what happens and fixes it."),
    );
  $("#doctor-findings").innerHTML = rows;
  $("#doctor-findings").classList.toggle("hidden", !rows);
  renderDoctorSteps();
  const raw = !!doctor.reason && !findings.length && !doctor.loading && !doctor.result?.ok;
  $("#doctor-raw").classList.toggle("hidden", !raw);
  if (raw) $("#doctor-reason").textContent = doctor.reason;

  // Abajo, una sola acción principal: la que toca ahora.
  const recommended = findings.flatMap((f) => f.fixes.filter((o) => o.recommended)).filter((o) => !doctor.applied.some((a) => JSON.stringify(a) === JSON.stringify(o.fix)));
  const done = !!doctor.result?.ok;
  const fixAll = $("#doctor-fix-all");
  fixAll.classList.toggle("hidden", !recommended.length || done);
  fixAll.querySelector("span").textContent = doctor.mode === "prelaunch" ? t("Arreglar y jugar", "Fix and play") : t("Arreglar lo recomendado", "Fix the recommended");
  const anyway = $("#doctor-anyway");
  anyway.classList.toggle("hidden", doctor.mode !== "prelaunch" && !done);
  anyway.classList.toggle("primary", done);
  anyway.classList.toggle("ghost", !done);
  anyway.innerHTML = done ? `${icon("play")}${t("Jugar", "Play")}` : t("Jugar de todos modos", "Play anyway");
  const auto = $("#doctor-auto");
  auto.classList.toggle("hidden", done);
  auto.classList.toggle("primary", !recommended.length);
  $("#doctor-logs").classList.toggle("hidden", doctor.mode === "prelaunch" && !doctor.result);
  for (const button of [fixAll, auto, anyway]) button.disabled = doctor.busy || doctor.loading;
}

/** Los pasos de «Probar y arreglar solo» (y de cada arreglo), como una línea de tiempo. */
function renderDoctorSteps() {
  const doctor = state.doctor;
  const list = $("#doctor-steps");
  // Si al final abre, lo que importa (qué se arregló) ya está en la fila de abajo.
  const shown = doctor.result?.ok ? [] : doctor.steps;
  list.classList.toggle("hidden", !shown.length);
  list.innerHTML = shown
    .map((step, i) => {
      const last = i === shown.length - 1;
      const status = last && doctor.busy ? "run" : last && doctor.result && !doctor.result.ok ? "fail" : "done";
      const mark = status === "run" ? `<span class="spinner tiny"></span>` : icon(status === "fail" ? "close" : "check");
      const text = status === "run" ? step.text : step.text.replace(/…$/, "");
      return `<li class="doc-step ${status}"><span class="doc-step-mark">${mark}</span><span>${escapeHtml(text)}${step.detail && status === "run" ? `<small>${escapeHtml(step.detail)}</small>` : ""}</span></li>`;
    })
    .join("");
}

/** Un aviso de progreso del doctor: los pasos conocidos son filas; lo demás (descargas…), su detalle. */
function doctorStep(text) {
  const doctor = state.doctor;
  if (!doctor?.busy || !text) return;
  const last = doctor.steps.at(-1);
  if (last?.text === text) return;
  if (/^(Revisando los mods|Arreglando|Abriendo el juego|Llega al menú|Checking the mods|Fixing|Opening the game|It reaches)/.test(text) || !last) doctor.steps.push({ text });
  else last.detail = text;
  renderDoctorSteps();
}

function doctorBusy(busy, status = "") {
  state.doctor.busy = busy;
  if (busy && status) doctorStep(status);
  renderDoctor();
}

/** Aplica un arreglo y vuelve a revisar los mods. */
async function applyDoctorFix(fix, label) {
  const doctor = state.doctor;
  doctorBusy(true, t(`Arreglando: ${label}…`, `Fixing: ${label}…`));
  try {
    const done = await invoke("aplicar_arreglo", { id: doctor.id, arreglo: fix });
    doctor.applied.push(fix);
    toast(done);
    return true;
  } catch (error) {
    toast(String(error), "error");
    return false;
  } finally {
    doctorBusy(false);
  }
}

async function doctorFixAll() {
  const doctor = state.doctor;
  const options = (doctor.diagnosis?.findings ?? []).flatMap((f) => f.fixes.filter((o) => o.recommended));
  let ok = true;
  for (const option of options) {
    if (doctor.applied.some((a) => JSON.stringify(a) === JSON.stringify(option.fix))) continue;
    ok = (await applyDoctorFix(option.fix, option.label)) && ok;
  }
  await reloadInstances();
  // Se vuelve a revisar: si ya no queda nada que impida abrir y era antes de jugar, se abre.
  try {
    doctor.diagnosis = await invoke("revisar_instancia", { id: doctor.id });
    doctor.applied = [];
  } catch {}
  renderDoctor();
  if (doctor.mode === "prelaunch" && ok && !doctor.diagnosis.findings.some((f) => f.severity === "error")) {
    closeModal("#doctor-dialog");
    play(doctor.id, doctor.server, { skipCheck: true });
  }
}

/** Abre el juego hasta el menú principal; si no llega, aplica los arreglos recomendados y repite. */
async function doctorAuto() {
  const doctor = state.doctor;
  doctor.steps = [];
  doctor.result = null;
  doctor.auto = true;
  doctorBusy(true, t("Revisando los mods…", "Checking the mods…"));
  try {
    const result = await invoke("probar_y_arreglar", { id: doctor.id });
    await reloadInstances();
    doctor.result = result;
    if (result.ok) {
      doctor.diagnosis = { findings: [], checkedMods: doctor.diagnosis?.checkedMods ?? 0 };
      toast(t("El juego llega al menú principal.", "The game reaches the main menu."));
    } else {
      doctor.diagnosis = result.diagnosis ?? doctor.diagnosis;
    }
  } catch (error) {
    toast(String(error), "error");
  } finally {
    doctor.auto = false;
    doctorBusy(false);
  }
}

async function duplicateInstance(id) {
  const instance = state.instances.find((i) => i.id === id);
  toast(t(`Duplicando ${instance?.name}… (se copian también sus mundos y mods)`, `Duplicating ${instance?.name}… (its worlds and mods are copied too)`));
  try {
    const copy = await invoke("duplicar_instancia", { id });
    await reloadInstances();
    toast(t(`Se creó ${copy.name}.`, `${copy.name} was created.`));
  } catch (error) {
    toast(String(error), "error");
  }
}

async function deleteInstance(id) {
  const instance = state.instances.find((i) => i.id === id);
  const ok = await confirmAction({
    title: t(`¿Eliminar ${instance?.name}?`, `Delete ${instance?.name}?`),
    text: t("La instancia, con sus mundos y mods, se moverá a la papelera de reciclaje. Desde ahí la puedes recuperar.", "The instance, with its worlds and mods, will be moved to the Recycle Bin. You can restore it from there."),
    ok: t("Mandar a la papelera", "Move to the Recycle Bin"),
  });
  if (!ok) return;
  try {
    await invoke("eliminar_instancia", { id });
    if (state.current?.id === id) navigate("home");
    state.back = state.back.filter((h) => !(h.page === "instance" && h.param === id));
    state.forward = state.forward.filter((h) => !(h.page === "instance" && h.param === id));
    await reloadInstances();
    toast(t(`${instance?.name} se movió a la papelera.`, `${instance?.name} was moved to the Recycle Bin.`));
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
  const usesLoader = ["Fabric", "Quilt", "Forge", "NeoForge"].includes(instance.loader);
  $("#edit-loader-box").classList.toggle("hidden", !usesLoader);
  if (usesLoader) loadEditLoaderVersions(instance);
  updateEditTexts();
  openModal("#edit-dialog");
}

async function loadEditLoaderVersions(instance) {
  const select = $("#edit-loader");
  const loader = instance.loader.toLowerCase();
  const current = instance.loaderVersion ?? "";
  $("#edit-loader-label").textContent = t(`Versión de ${instance.loader}`, `${instance.loader} version`);
  $("#edit-loader-hint").textContent = t("Algunos mods piden una versión más nueva del loader.", "Some mods ask for a newer loader version.");
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
  if (!info.online) $("#edit-loader-hint").textContent = `${info.message} ${t("Solo se muestran las versiones ya descargadas.", "Only already downloaded versions are shown.")}`;
}

function updateEditTexts() {
  const name = $("#edit-name").value.trim() || "Instancia";
  $("#edit-art").outerHTML = art({ name, icon: state.editing?.icon }, "large").replace('class="art', 'id="edit-art" class="art');
  $("#edit-icon-remove").classList.toggle("hidden", !state.editing?.icon);
  $("#edit-ram-box").classList.toggle("hidden", !$("#edit-ram-own").checked);
  $("#edit-ram-value").textContent = `${$("#edit-ram").value} GB`;
}

/** El archivo como data URL. */
function fileDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error(t("No se pudo leer ese archivo.", "That file could not be read.")));
    reader.readAsDataURL(file);
  });
}

function loadImage(src) {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => (image.naturalWidth && image.naturalHeight ? resolve(image) : reject(new Error("vacía")));
    image.onerror = () => reject(new Error("no se pudo abrir"));
    image.src = src;
  });
}

/** Cualquier imagen elegida o pegada, lista para dibujar. La ventana abre PNG, JPG, GIF, WEBP, BMP y
 *  AVIF; con lo demás (fotos HEIC del celular, TIFF, ICO…) decía «The source image cannot be decoded»:
 *  esas las convierte Windows. */
async function loadImageFile(file) {
  const data = await fileDataUrl(file);
  try {
    return await loadImage(data);
  } catch {
    // Sigue abajo: la convierte Windows.
  }
  if (!tauri) throw new Error(t("Esa imagen no se pudo abrir.", "That image could not be opened."));
  const converted = await invoke("convertir_imagen", { datos: data.slice(data.indexOf(",") + 1) }).catch((error) => {
    throw new Error(String(error));
  });
  return loadImage(converted);
}

/** Recorta la imagen elegida a un cuadrado de 256 px y la convierte a PNG. */
async function imageToPng(file) {
  const image = await loadImageFile(file).catch((error) => {
    throw error.message ?? t("No se pudo leer esa imagen.", "That image could not be read.");
  });
  const size = Math.min(image.naturalWidth, image.naturalHeight);
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 256;
  const ctx = canvas.getContext("2d");
  ctx.imageSmoothingEnabled = size > 64; // los iconos pixelados se mantienen nítidos
  ctx.drawImage(image, (image.naturalWidth - size) / 2, (image.naturalHeight - size) / 2, size, size, 0, 0, 256, 256);
  return canvas.toDataURL("image/png");
}

async function saveEdit() {
  const { id, icon: newIcon, iconChanged } = state.editing;
  const instance = state.instances.find((i) => i.id === id);
  const wantedLoader = $("#edit-loader-box").classList.contains("hidden") ? "" : $("#edit-loader").value;
  $("#edit-save").disabled = true;
  try {
    if (wantedLoader && wantedLoader !== instance?.loaderVersion) {
      $("#edit-loader-hint").textContent = t(`Preparando ${instance.loader} ${wantedLoader}…`, `Preparing ${instance.loader} ${wantedLoader}…`);
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
    toast(t("Cambios guardados.", "Changes saved."));
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
function onGameClosed({ id, code, reason, diagnosis }) {
  state.running.delete(id);
  refreshRunning();
  if (state.page === "instance" && state.current?.id === id && state.tab === "logs") loadLog(false);
  if (code === null || code === undefined || code === 0) return;
  if (state.doctor?.busy) return;
  appWindow?.unminimize?.();
  openDoctor(id, "crash", {
    diagnosis: diagnosis ?? null,
    reason: reason ?? t(`Java terminó con el código ${code}.`, `Java ended with code ${code}.`),
  });
}

/** «Jugar» mientras se prepara el juego: se ve trabajando (y llenándose si falta bajar algo). */
function showLaunching() {
  const button = $("#detail-play");
  const launching = !!state.current && state.launching.has(state.current.id) && !state.running.has(state.current.id);
  button.classList.toggle("launching", launching);
  $("span", button).textContent = launching ? t("Abriendo…", "Opening…") : t("Jugar", "Play");
  if (!launching) button.style.removeProperty("--p");
  for (const tile of $$(".tile-play[data-play]")) tile.classList.toggle("launching", state.launching.has(tile.dataset.play) && !state.running.has(tile.dataset.play));
}

async function play(id, server = null, { skipCheck = false } = {}) {
  const instance = state.instances.find((i) => i.id === id);
  if (!instance || state.launching.has(id)) return;
  state.launching.add(id);
  showLaunching();
  try {
    await launch(instance, id, server, skipCheck);
  } finally {
    state.launching.delete(id);
    showLaunching();
  }
}

async function launch(instance, id, server, skipCheck) {
  // También en Vanilla: en Mac con chip Apple, 1.17 y anteriores necesitan Rosetta (lo ofrece el doctor).
  if (!skipCheck) {
    try {
      const diagnosis = await invoke("revisar_instancia", { id });
      if (diagnosis.findings.some((f) => f.severity === "error")) return openDoctor(id, "prelaunch", { diagnosis, server });
    } catch {
      // Si la revisión falla, se abre igual: el juego dirá lo que pase.
    }
  }
  try {
    toast(server ? t(`Abriendo ${instance.name} y entrando a ${server}…`, `Opening ${instance.name} and joining ${server}…`) : t(`Abriendo ${instance.name}…`, `Opening ${instance.name}…`));
    await invoke("jugar", { id, servidor: server });
    state.running.add(id);
    if (state.settings.minimizeOnLaunch) setTimeout(() => appWindow?.minimize(), 3500);
  } catch (error) {
    toast(String(error), "error");
  } finally {
    // Si antes de abrir se bajó algo que le faltaba al modpack, se quita el aviso de descarga.
    if (state.completingShown) {
      state.completingShown = false;
      $("#download-indicator").classList.add("hidden");
    }
    refreshRunning();
  }
}

// ======================================================================
// Juego abierto
// ======================================================================
function refreshRunning() {
  const running = state.instances.find((i) => state.running.has(i.id));
  $("#status").classList.toggle("running", !!running);
  $("#status-text").textContent = running ? t(`Jugando ${running.name}`, `Playing ${running.name}`) : t("Ningún juego abierto", "No game open");
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
  drawHead($("#rail-head"), state.settings.username);
  $("#rail-avatar").title = t(`Jugando como ${state.settings.username}`, `Playing as ${state.settings.username}`);
}

// ---------- Cuentas ----------
/** Menú de "Jugando como": cambiar de cuenta, añadir otra o administrarlas. */
function openAccountMenu(anchor) {
  const s = state.settings;
  contextMenu(anchor, [
    { head: s.username, label: s.username, sub: s.uuid ? t("En uso · UUID fijo", "In use · fixed UUID") : t("En uso", "In use"), checked: true, action: () => {} },
    ...s.accounts.map((a, i) => ({ head: a.username, label: a.username, sub: a.uuid ? "UUID fijo" : "", action: () => switchAccount(i) })),
    { icon: "brush", label: t("Cambiar nombre", "Change name"), separator: true, action: () => openAccountDialog("rename") },
    { icon: "user-plus", label: t("Añadir cuenta", "Add account"), action: () => openAccountDialog("add") },
    { icon: "settings", label: t("Administrar cuentas", "Manage accounts"), action: () => openSettings("profile") },
  ], { matchWidth: true });
}

async function saveAccounts(changes) {
  clearTimeout(saveTimer);
  state.settings = await invoke("guardar_ajustes", { ajustes: { ...state.settings, ...changes } });
  renderSidebar();
  renderSettings();
  renderSteps();
}

async function switchAccount(index) {
  const s = state.settings;
  const target = s.accounts[index];
  if (!target) return;
  const accounts = [{ username: s.username, uuid: s.uuid }, ...s.accounts.filter((_, i) => i !== index)];
  try {
    await saveAccounts({ username: target.username, uuid: target.uuid, accounts });
    toast(t(`Ahora juegas como ${target.username}.`, `You now play as ${target.username}.`));
  } catch (error) {
    toast(String(error), "error");
  }
}

async function removeAccount(index) {
  const target = state.settings.accounts[index];
  if (!target) return;
  const ok = await confirmAction({
    title: t(`Quitar a ${target.username}`, `Remove ${target.username}`),
    text: t("Solo se quita de la lista para cambiar de cuenta. Sus mundos y partidas no se tocan.", "It is only removed from the account list. Their worlds and games are not touched."),
    ok: t("Quitar", "Remove"),
  });
  if (!ok) return;
  try {
    await saveAccounts({ accounts: state.settings.accounts.filter((_, i) => i !== index) });
  } catch (error) {
    toast(String(error), "error");
  }
}

/** "add": otra cuenta (y pasar a usarla); "rename": cambiar el nombre de la cuenta en uso. */
function openAccountDialog(mode = "add") {
  state.accountDialog = mode;
  const rename = mode === "rename";
  $("#account-title").textContent = rename ? t("Cambiar nombre de jugador", "Change player name") : t("Añadir cuenta", "Add account");
  $("#account-ok").textContent = rename ? t("Guardar", "Save") : t("Añadir y usar", "Add and use");
  $("#account-uuid-box").classList.toggle("hidden", rename);
  $("#account-rename-hint").classList.toggle("hidden", !rename);
  $("#account-name").value = rename ? state.settings.username : "";
  $("#account-uuid").value = "";
  $("#account-error").classList.add("hidden");
  openModal("#account-dialog");
  setTimeout(() => {
    $("#account-name").focus();
    $("#account-name").select();
  }, 60);
}

/** Otra cuenta guardada con ese nombre (sin contar la que está en uso). */
const otherAccountNamed = (name) => (state.settings.accounts ?? []).some((a) => a.username.toLowerCase() === name.toLowerCase());

const validUuid = (text) => /^[0-9a-f]{32}$/i.test(text.replaceAll("-", ""));

async function addAccount() {
  const name = $("#account-name").value.trim();
  const uuid = $("#account-uuid").value.trim();
  const s = state.settings;
  const fail = (message) => {
    $("#account-error").textContent = message;
    $("#account-error").classList.remove("hidden");
  };
  if (!/^[A-Za-z0-9_]{1,16}$/.test(name)) return fail(t("El nombre debe tener de 1 a 16 letras, números o guiones bajos (sin espacios ni acentos).", "The name must have 1 to 16 letters, numbers or underscores (no spaces or accents)."));
  if (state.accountDialog === "rename") {
    if (name === s.username) return closeModal("#account-dialog");
    if (otherAccountNamed(name)) return fail(t("Ya tienes otra cuenta con ese nombre: cámbiate a ella desde «Jugando como».", "You already have another account with that name: switch to it from «Playing as»."));
    try {
      await saveAccounts({ username: name });
      closeModal("#account-dialog");
      toast(t(`Ahora te llamas ${name}.`, `Your name is now ${name}.`));
    } catch (error) {
      fail(String(error));
    }
    return;
  }
  if (uuid && !validUuid(uuid)) return fail(t("El UUID tiene 32 letras y números (0–9, a–f), con o sin guiones. Déjalo vacío si no lo necesitas.", "The UUID has 32 letters and numbers (0–9, a–f), with or without dashes. Leave it empty if you do not need it."));
  if ([s.username, ...s.accounts.map((a) => a.username)].some((n) => n.toLowerCase() === name.toLowerCase())) return fail(t("Ya tienes una cuenta con ese nombre.", "You already have an account with that name."));
  try {
    await saveAccounts({ username: name, uuid, accounts: [{ username: s.username, uuid: s.uuid }, ...s.accounts] });
    closeModal("#account-dialog");
    toast(t(`Cuenta añadida: ahora juegas como ${name}.`, `Account added: you now play as ${name}.`));
  } catch (error) {
    fail(String(error));
  }
}

function renderAccounts() {
  const accounts = state.settings.accounts ?? [];
  $("#account-list").innerHTML = accounts.length
    ? accounts
        .map(
          (a, i) => `<div class="account-row">
            <canvas width="8" height="8" class="head" data-head="${escapeHtml(a.username)}"></canvas>
            <div class="grow"><b>${escapeHtml(a.username)}</b><span>${a.uuid ? `${t("UUID fijo", "Fixed UUID")} ${escapeHtml(a.uuid)}` : t("UUID de su nombre", "UUID from their name")}</span></div>
            <button class="btn small" data-use-account="${i}">${t("Usar", "Use")}</button>
            <button class="circle-btn small" data-remove-account="${i}" title="${t("Quitar de la lista", "Remove from the list")}">${icon("trash")}</button>
          </div>`,
        )
        .join("")
    : `<p class="muted small-text">${t("Añade otra cuenta para cambiar de jugador con un clic desde «Jugando como».", "Add another account to switch players with one click from «Playing as».")}</p>`;
  for (const canvas of $$("#account-list canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
}

function renderSteps() {
  const steps = [
    { done: state.instances.length > 0, label: t("Crea tu primera instancia", "Create your first instance"), action: openCreate },
    { done: state.settings.username !== "Jugador", label: t("Elige tu nombre de jugador", "Choose your player name"), action: () => openAccountDialog("rename"), always: true },
    { done: !!state.social?.signedIn, label: t("Inicia sesión con Google", "Sign in with Google"), action: signInGoogle, disabled: state.social?.available === false },
  ];
  $("#steps-list").innerHTML = `<div>${steps
    .map(
      (s, i) => `<button class="step ${s.done ? "done" : ""}" data-step="${i}" ${(s.done && !s.always) || s.disabled ? "disabled" : ""}>
        ${icon(s.done ? "check-circle" : "circle")}<span class="grow">${s.label}</span>${s.soon ? `<span class="soon">${t("Pronto", "Soon")}</span>` : ""}</button>`,
    )
    .join("")}</div>`;
  $$("#steps-list .step").forEach((button) => (button.onclick = () => steps[button.dataset.step].action?.()));
  $("#tile-steps")?.classList.toggle("hidden", steps.every((s) => s.done));
}

// ======================================================================
// Amigos (sesión de Google)
// ======================================================================
function friendStatus(f) {
  if (!f.online) return f.lastSeen ? `${t("Desconectado", "Offline")} · ${ago(f.lastSeen).toLowerCase()}` : t("Desconectado", "Offline");
  const p = f.presence;
  if (p?.state !== "playing") return p?.hosting ? t(`En línea · hosteando ${p.hosting}`, `Online · hosting ${p.hosting}`) : t("En línea", "Online");
  if (f.worldOpen) return t(`Jugando ${p.pack} · mundo abierto`, `Playing ${p.pack} · world open`);
  if (p.world === "server") return t(`Jugando ${p.pack} · en ${p.server}`, `Playing ${p.pack} · on ${p.server}`);
  if (p.world === "single") return t(`Jugando ${p.pack} · en su mundo`, `Playing ${p.pack} · in their world`);
  return t(`Jugando ${p.pack}`, `Playing ${p.pack}`);
}

function renderFriends() {
  const s = state.social;
  const connected = !!(s?.signedIn && s.connected);
  $("#friends-signed-out").classList.toggle("hidden", connected);
  $("#friends-signed-in").classList.toggle("hidden", !connected);
  renderSteps();
  renderGoogleBox();
  if (!connected) {
    $("#friends-count").textContent = "";
    let note = t("Inicia sesión con Google para agregar amigos, ver a qué están jugando y entrar a sus mundos con un clic.", "Sign in with Google to add friends, see what they are playing and join their worlds with one click.");
    let label = t("Iniciar sesión con Google", "Sign in with Google");
    let show = true;
    if (!s || s.available === false) {
      note = t("Los amigos no están disponibles en esta copia de Parabellum. Descarga la versión más reciente.", "Friends are not available in this copy of Parabellum. Download the latest version.");
      show = false;
    } else if (s.signingIn) {
      note = t("Se abrió tu navegador. Elige tu cuenta de Google y vuelve aquí…", "Your browser opened. Choose your Google account and come back here…");
      show = false;
    } else if (s.connecting) {
      note = t("Conectando con tus amigos…", "Connecting with your friends…");
      show = false;
    } else if (s.signedIn) {
      note = t("No se pudo conectar con el servidor de amigos", "Could not connect to the friends server") + (s.error ? `: ${s.error}` : ".");
      label = t("Reintentar", "Retry");
    } else if (s.error) {
      note = t(`No se pudo iniciar sesión: ${s.error}`, `Could not sign in: ${s.error}`);
    }
    $("#friends-note").textContent = note;
    $("#friends-sign-in span").textContent = label;
    $("#friends-sign-in").classList.toggle("hidden", !show);
    updateFriendsBadge();
    renderFriendsPage();
    return;
  }

  // Tu estado
  const me = s.me ?? { state: "online" };
  const open = me.address || me.lan;
  const myText =
    me.state !== "playing"
      ? t("En línea", "Online")
      : open
        ? t(`Jugando ${me.pack} · mundo abierto en ${me.address ?? me.lan}`, `Playing ${me.pack} · world open at ${me.address ?? me.lan}`)
        : t(`Jugando ${me.pack}`, `Playing ${me.pack}`) + (me.world === "single" ? t(" · en tu mundo", " · in your world") : me.world === "server" ? t(` · en ${me.server}`, ` · on ${me.server}`) : "");
  $("#my-status").innerHTML = `<span class="presence-dot ${me.state === "playing" ? "playing" : "online"}"></span>
    <span class="grow"><b>${escapeHtml(s.friends?.name ?? state.settings.username)}</b> · ${escapeHtml(myText)}</span>
    ${open ? `<button class="circle-btn small" id="copy-my-address" title="${t("Copiar la dirección de tu mundo", "Copy your world's address")}">${icon("copy")}</button>` : ""}`;
  const copyButton = $("#copy-my-address");
  if (copyButton) copyButton.onclick = () => copyText(me.address ?? me.lan, t("Dirección copiada.", "Address copied."));

  // Avisos: nombre ocupado, problemas de conexión y consejos para abrir tu mundo
  let notice = null;
  let kind = "";
  if (s.nameConflict) {
    notice = t(`El nombre "${s.nameConflict}" ya lo usa otro jugador de Parabellum. Cámbialo con «Jugando como» › Cambiar nombre: tus amigos no podrían encontrarte.`, `The name "${s.nameConflict}" is already used by another Parabellum player. Change it with «Playing as» › Change name: your friends would not be able to find you.`);
    kind = "danger";
  } else if (s.warning) {
    notice = s.warning;
    kind = "warn";
  } else if (open && me.world === "single") {
    notice = t("Tus amigos ya pueden pulsar Unirse. Deben abrir el juego desde Parabellum.", "Your friends can now press Join. They must open the game from Parabellum.");
  } else if (s.notice) {
    notice = s.notice;
  }
  $("#friends-notice").textContent = notice ?? "";
  $("#friends-notice").className = `side-note friends-notice ${kind} ${notice ? "" : "hidden"}`;

  // Solicitudes y amigos
  const view = s.friends ?? { friends: [], requests: [] };
  setHtml($("#friend-requests"), view.requests
    .map(
      (r) => `<div class="friend request" data-key="r:${escapeHtml(r.uid)}">
        <canvas width="8" height="8" class="head" data-head="${escapeHtml(r.name)}"></canvas>
        <div class="grow"><b>${escapeHtml(r.name)}</b><span>${t("Quiere ser tu amigo", "Wants to be your friend")}</span></div>
        <button class="btn primary small" data-accept="${escapeHtml(r.uid)}">${t("Aceptar", "Accept")}</button>
        <button class="circle-btn small" data-decline="${escapeHtml(r.uid)}" title="${t("Rechazar", "Decline")}">${icon("close")}</button>
      </div>`,
    )
    .join(""));
  const starred = favoriteFriendUids();
  const row = (f) => `<div class="friend ${f.online ? "online" : ""}" data-uid="${escapeHtml(f.uid)}" data-key="f:${escapeHtml(f.uid)}" title="${t(`Escribirle a ${escapeHtml(f.name)}`, `Message ${escapeHtml(f.name)}`)}">
      <div class="friend-head"><canvas width="8" height="8" class="head" data-head="${escapeHtml(f.name)}"></canvas><span class="presence-dot ${f.online ? (f.presence?.state === "playing" ? "playing" : "online") : ""}"></span></div>
      <div class="grow"><b>${escapeHtml(f.name)}${starred.has(f.uid) ? favMark() : ""}</b><span title="${escapeHtml(friendStatus(f))}">${escapeHtml(friendStatus(f))}</span></div>
      ${f.worldOpen ? `<button class="btn primary small" data-join="${escapeHtml(f.uid)}">${t("Unirse", "Join")}</button>` : ""}
      <button class="circle-btn small friend-chat" data-friend-chat="${escapeHtml(f.uid)}" title="${t("Enviar mensaje", "Send message")}">${icon("chat")}</button>
      <button class="circle-btn small" data-friend-menu="${escapeHtml(f.uid)}" title="${t("Más", "More")}">${icon("more")}</button>
    </div>`;
  const rest = view.friends.filter((f) => !starred.has(f.uid));
  const sections = [
    [t("Favoritos", "Favorites"), view.friends.filter((f) => starred.has(f.uid))],
    [t("Jugando", "Playing"), rest.filter((f) => f.online && f.presence?.state === "playing")],
    [t("En línea", "Online"), rest.filter((f) => f.online && f.presence?.state !== "playing")],
    [t("Desconectados", "Offline"), rest.filter((f) => !f.online)],
  ].filter(([, list]) => list.length);
  const friendsHtml = view.friends.length
    ? sections.map(([title, list]) => `<div class="friend-section" data-key="s:${escapeHtml(title)}">${escapeHtml(title)} <span>${list.length}</span></div>${list.map(row).join("")}`).join("")
    : `<p class="side-note">${t("Todavía no tienes amigos en Parabellum. Escribe el nombre de jugador de un amigo y pulsa +.", "You have no friends on Parabellum yet. Type a friend's player name and press +.")}</p>`;
  setHtml($("#friend-list"), friendsHtml);
  for (const canvas of $$("#friends-block canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
  const online = view.friends.filter((f) => f.online).length;
  $("#friends-count").textContent = view.friends.length ? t(`${online} en línea`, `${online} online`) : "";
  renderGroups();
  updateFriendsBadge();
  renderFriendsPage();
}

/** Solicitudes de amistad sin contestar, en el botón de Amigos de la barra. */
function updateFriendsBadge() {
  const count = state.social?.connected ? state.social?.friends?.requests?.length ?? 0 : 0;
  $("#friends-badge").textContent = count > 9 ? "9+" : String(count);
  $("#friends-badge").classList.toggle("hidden", count === 0);
}

/** El avatar de un grupo: su foto o el icono. */
function groupAvatar(chat) {
  return chat.photo ? `<img class="group-avatar" src="${escapeHtml(chat.photo)}" alt="">` : `<span class="group-avatar">${icon("users")}</span>`;
}

/** Los grupos de chat: en el panel de la derecha y en la página de Amigos. */
function renderGroups() {
  const groups = (state.chats ?? []).filter((c) => c.kind === "grupo").sort((a, b) => isFavoriteChat(b.id) - isFavoriteChat(a.id));
  const html = groups.length
    ? groups
        .map(
          (g) => `<button class="group-row ${g.unread ? "unread" : ""}" data-open-group="${escapeHtml(g.id)}" data-key="g:${escapeHtml(g.id)}">
            ${groupAvatar(g)}
            <span class="grow"><b>${escapeHtml(g.name)}${isFavoriteChat(g.id) ? favMark() : ""}</b><span>${escapeHtml(g.lastText ? `${g.lastName}: ${chatPreview(g.lastText)}` : t(`${g.members.length} miembros`, `${g.members.length} members`))}</span></span>
            ${g.unread ? `<i class="unread-dot"></i>` : ""}
          </button>`,
        )
        .join("")
    : `<p class="side-note no-top">${t("Crea un grupo para hablar con varios amigos a la vez.", "Create a group to talk with several friends at once.")}</p>`;
  for (const box of [$("#group-list"), $("#fp-groups")]) if (box) setHtml(box, html);
}

// ---------- Página de Amigos ----------
/** Tienes algo a lo que invitar: un servidor encendido o tu mundo abierto. */
function canInviteNow() {
  const servers = typeof host !== "undefined" ? host.view?.servers ?? [] : [];
  const me = state.social?.me;
  return servers.some((s) => s.state === "ready") || (me?.state === "playing" && !!(me.lan || me.address));
}

function renderFriendsPage() {
  if (!$("#page-friends")) return;
  const s = state.social;
  const connected = !!(s?.signedIn && s.connected);
  $("#fp-signed-out").classList.toggle("hidden", connected);
  $("#fp-signed-in").classList.toggle("hidden", !connected);
  if (!connected) {
    $("#fp-count").textContent = "";
    const withButton = !$("#friends-sign-in").classList.contains("hidden");
    $("#fp-signed-out").innerHTML = `<span class="chat-empty-icon">${icon("users")}</span><p>${escapeHtml($("#friends-note").textContent)}</p>${
      withButton ? `<button class="btn primary" id="fp-sign-in">${icon("user")}<span>${escapeHtml($("#friends-sign-in span").textContent)}</span></button>` : ""
    }`;
    const button = $("#fp-sign-in");
    if (button) button.onclick = signInGoogle;
    return;
  }
  const view = s.friends ?? { friends: [], requests: [] };
  const online = view.friends.filter((f) => f.online);
  const playing = online.filter((f) => f.presence?.state === "playing");
  $("#fp-count").textContent = view.friends.length ? t(`${online.length} de ${view.friends.length} en línea`, `${online.length} of ${view.friends.length} online`) : "";
  setHtml(
    $("#fp-requests"),
    view.requests
      .map(
        (r) => `<div class="fp-request" data-key="r:${escapeHtml(r.uid)}">
          <canvas width="8" height="8" class="head" data-head="${escapeHtml(r.name)}"></canvas>
          <div class="grow"><b>${escapeHtml(r.name)}</b><span>${t("Quiere ser tu amigo", "Wants to be your friend")}</span></div>
          <button class="btn primary small" data-accept="${escapeHtml(r.uid)}">${t("Aceptar", "Accept")}</button>
          <button class="btn small" data-decline="${escapeHtml(r.uid)}">${t("Rechazar", "Decline")}</button>
        </div>`,
      )
      .join(""),
  );
  const filter = state.friendsFilter ?? "all";
  const counts = { all: view.friends.length, online: online.length, playing: playing.length };
  for (const button of $$("#fp-filters [data-filter]")) {
    button.classList.toggle("active", button.dataset.filter === filter);
    button.querySelector("span").textContent = counts[button.dataset.filter] || "";
  }
  const query = $("#fp-search").value.trim().toLowerCase();
  const starred = favoriteFriendUids();
  const rank = (f) => (starred.has(f.uid) ? 0 : !f.online ? 3 : f.presence?.state === "playing" ? 1 : 2);
  const list = view.friends
    .filter((f) => filter === "all" || (f.online && (filter === "online" || f.presence?.state === "playing")))
    .filter((f) => !query || f.name.toLowerCase().includes(query))
    .sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name, "es", { sensitivity: "base" }));
  const invite = canInviteNow();
  const card = (f) => {
    const join = joinTarget(f);
    return `<div class="fp-friend ${f.online ? "online" : ""}" data-uid="${escapeHtml(f.uid)}" data-key="f:${escapeHtml(f.uid)}" title="${t(`Escribirle a ${escapeHtml(f.name)}`, `Message ${escapeHtml(f.name)}`)}">
      <div class="friend-head"><canvas width="8" height="8" class="head" data-head="${escapeHtml(f.name)}"></canvas><span class="presence-dot ${f.online ? (f.presence?.state === "playing" ? "playing" : "online") : ""}"></span></div>
      <div class="grow"><b>${escapeHtml(f.name)}${starred.has(f.uid) ? favMark() : ""}</b><span>${escapeHtml(friendStatus(f))}</span></div>
      ${join ? `<button class="btn primary small" data-fp-join="${escapeHtml(f.uid)}" title="${escapeHtml(join.title)}">${icon("play")}${t("Unirse", "Join")}</button>` : ""}
      ${invite && f.online && !join ? `<button class="btn small" data-fp-invite="${escapeHtml(f.uid)}" title="${t("Invitarlo a tu partida", "Invite them to your game")}">${icon("send")}${t("Invitar", "Invite")}</button>` : ""}
      <button class="circle-btn small" data-fp-chat="${escapeHtml(f.uid)}" title="${t("Enviar mensaje", "Send message")}">${icon("chat")}</button>
      <button class="circle-btn small" data-fp-menu="${escapeHtml(f.uid)}" title="${t("Más", "More")}">${icon("more")}</button>
    </div>`;
  };
  const empty = !view.friends.length
    ? t("Todavía no tienes amigos en Parabellum. Escribe arriba el nombre de jugador de un amigo.", "You have no friends on Parabellum yet. Type a friend's player name above.")
    : query
      ? t("Nadie coincide con la búsqueda.", "No one matches the search.")
      : filter === "playing"
        ? t("Nadie está jugando ahora.", "No one is playing right now.")
        : t("Nadie está en línea ahora.", "No one is online right now.");
  setHtml($("#fp-list"), list.length ? list.map(card).join("") : `<p class="fp-none" data-key="none">${escapeHtml(empty)}</p>`);
  for (const canvas of $$("#page-friends canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
}

async function inviteFriend(uid) {
  try {
    toast(await invoke("invitar_amigo", { uid }));
  } catch (error) {
    toast(String(error), "error");
  }
}

/** Aceptar o rechazar una solicitud (panel de la derecha y página de Amigos). */
async function answerRequest(event) {
  const accept = event.target.closest("[data-accept]");
  const decline = event.target.closest("[data-decline]");
  try {
    if (accept) await invoke("aceptar_solicitud", { uid: accept.dataset.accept });
    if (decline) await invoke("rechazar_solicitud", { uid: decline.dataset.decline });
  } catch (error) {
    toast(String(error), "error");
  }
}

function openAddFriendDialog() {
  $("#dialog-friend-name").value = "";
  $("#dialog-friend-message").className = "side-note hidden";
  openModal("#add-friend-dialog");
  setTimeout(() => $("#dialog-friend-name").focus(), 60);
}

function renderGoogleBox() {
  const s = state.social;
  const box = $("#google-box");
  if (!box) return;
  if (!s || s.available === false) {
    box.innerHTML = `<p class="muted small-text">${t("Esta copia de Parabellum no trae el inicio de sesión con Google.", "This copy of Parabellum does not include Google sign-in.")}</p>`;
    return;
  }
  if (s.signedIn) {
    box.innerHTML = `<div class="account-row">
        <span class="google-badge">${icon("user")}</span>
        <div class="grow"><b>${escapeHtml(s.displayName || s.email)}</b><span>${escapeHtml(s.email)} · ${t("tu perfil se guarda en una carpeta oculta de tu Google Drive", "your profile is saved in a hidden folder of your Google Drive")}</span></div>
        <button class="btn small" id="google-sign-out">${t("Cerrar sesión", "Sign out")}</button>
      </div>`;
    $("#google-sign-out").onclick = signOutGoogle;
    return;
  }
  box.innerHTML = `<p class="muted small-text">${t("Para tener amigos, ver a qué juegan y que tu nombre sea solo tuyo. Parabellum solo ve tu correo y una carpeta oculta de tu Drive para guardar tu perfil.", "To have friends, see what they play and keep your name yours. Parabellum only sees your email and a hidden folder of your Drive to save your profile.")}</p>
    <div class="google-actions">
      <label class="check"><input type="checkbox" id="google-remember" checked> ${t("Recordar en este PC (desmárcalo en una PC compartida)", "Remember on this PC (uncheck it on a shared PC)")}</label>
      <button class="btn primary small" id="google-sign-in" ${s.signingIn ? "disabled" : ""}>${s.signingIn ? t("Esperando al navegador…", "Waiting for the browser…") : t("Iniciar sesión con Google", "Sign in with Google")}</button>
    </div>`;
  $("#google-sign-in").onclick = signInGoogle;
}

async function signInGoogle() {
  const s = state.social;
  if (s?.signedIn && !s.connected) {
    state.social = await invoke("reconectar_amigos").catch(() => s);
    return renderFriends();
  }
  if (s?.signingIn) return;
  const remember = $("#google-remember")?.checked ?? true;
  try {
    state.social = await invoke("iniciar_sesion_google", { recordar: remember });
    renderFriends();
    if (state.social.notice) toast(state.social.notice);
    else toast(t(`Sesión iniciada como ${state.social.email}.`, `Signed in as ${state.social.email}.`));
  } catch (error) {
    toast(String(error), "error");
  }
}

async function signOutGoogle() {
  const ok = await confirmAction({
    title: t("Cerrar la sesión de Google", "Sign out of Google"),
    text: t("Tus amigos te verán desconectado y no podrás entrar a sus mundos con un clic hasta que vuelvas a iniciarla. Tus instancias no cambian.", "Your friends will see you offline and you will not be able to join their worlds with one click until you sign in again. Your instances do not change."),
    ok: t("Cerrar sesión", "Sign out"),
  });
  if (!ok) return;
  state.social = await invoke("cerrar_sesion_google");
  renderFriends();
}

async function copyText(text, message) {
  try {
    await navigator.clipboard.writeText(text);
    toast(message);
  } catch {
    toast(text);
  }
}

/** Envía una solicitud con el nombre escrito: en el panel de la derecha, en Amigos o en Mensajes. */
async function addFriend(input = "#add-friend", messageBox = "#add-friend-message", button = "#add-friend-button") {
  const name = $(input).value.trim();
  const message = $(messageBox);
  const show = (text, kind = "") => {
    message.textContent = text;
    message.className = `side-note ${kind}`;
  };
  if (!/^[A-Za-z0-9_]{1,16}$/.test(name)) return show(t("Escribe su nombre de jugador (1 a 16 letras, números o guiones bajos).", "Type their player name (1 to 16 letters, numbers or underscores)."), "danger");
  $(button).disabled = true;
  try {
    show(await invoke("agregar_amigo", { nombre: name }), "accent");
    $(input).value = "";
  } catch (error) {
    show(String(error), "warn");
  } finally {
    $(button).disabled = false;
  }
}

function findFriend(uid) {
  return state.social?.friends?.friends.find((f) => f.uid === uid);
}

/** Abre el mismo modpack que tu amigo y entra directo a su mundo. */
async function joinFriend(friend, address) {
  const p = friend.presence ?? {};
  const instance =
    state.instances.find((i) => i.id.toLowerCase() === (p.packFolder ?? "").toLowerCase()) ??
    state.instances.find((i) => i.name.toLowerCase() === (p.pack ?? "").toLowerCase());
  if (!instance) {
    toast(t(`${friend.name} está jugando ${p.pack} (Minecraft ${p.minecraft}) y tú no lo tienes. Búscalo en Explorar y vuelve a pulsar Unirse.`, `${friend.name} is playing ${p.pack} (Minecraft ${p.minecraft}) and you do not have it. Look for it in Explore and press Join again.`), "error");
    return;
  }
  if (state.running.size) {
    await copyText(address, t(`Ya tienes Minecraft abierto. Copié la dirección de ${friend.name}: en el juego ve a Multijugador › Conexión directa y pégala.`, `Minecraft is already open. I copied ${friend.name}'s address: in the game go to Multiplayer › Direct Connection and paste it.`));
    return;
  }
  play(instance.id, address);
}

function friendMenu(anchor, friend) {
  const p = friend.presence ?? {};
  const starred = favoriteFriendUids().has(friend.uid);
  const items = [
    { icon: "chat", label: t("Enviar mensaje", "Send message"), action: () => messageFriend(friend.uid) },
    { icon: "star", label: starred ? t("Quitar de favoritos", "Remove from favorites") : t("Agregar a favoritos", "Add to favorites"), action: () => toggleFavoriteFriend(friend) },
  ];
  if (friend.online && canInviteNow()) items.push({ icon: "send", label: t("Invitar a tu partida", "Invite to your game"), action: () => inviteFriend(friend.uid) });
  if (friend.worldOpen) {
    if (p.address) items.push({ icon: "globe", label: t("Unirse por Internet", "Join over the Internet"), action: () => joinFriend(friend, p.address) });
    if (p.lan) items.push({ icon: "server", label: t("Unirse por red local", "Join over the local network"), action: () => joinFriend(friend, p.lan) });
    items.push({ icon: "copy", label: t("Copiar dirección", "Copy address"), action: () => copyText(p.address ?? p.lan, t("Dirección copiada.", "Address copied.")) });
  }
  items.push({
    icon: "trash",
    label: t("Eliminar amigo", "Remove friend"),
    danger: true,
    separator: true,
    action: async () => {
      const ok = await confirmAction({ title: t(`Eliminar a ${friend.name}`, `Remove ${friend.name}`), text: t("Dejarán de verse en sus listas de amigos.", "You will no longer see each other in your friend lists."), ok: t("Eliminar", "Remove") });
      if (!ok) return;
      try {
        await invoke("eliminar_amigo", { uid: friend.uid });
      } catch (error) {
        toast(String(error), "error");
      }
    },
  });
  contextMenu(anchor, items);
}

function wireFriendsEvents() {
  $("#friends-sign-in").onclick = signInGoogle;
  $("#add-friend-button").onclick = addFriend;
  $("#add-friend").onkeydown = (event) => {
    if (event.key === "Enter") addFriend();
  };
  $("#friend-requests").onclick = answerRequest;
  $("#side-new-group").onclick = () => openGroupDialog("create");
  $("#group-list").onclick = (event) => {
    const group = event.target.closest("[data-open-group]");
    if (group) openChat(group.dataset.openGroup);
  };
  // Página de Amigos
  $("#fp-add-button").onclick = () => addFriend("#fp-add-name", "#fp-add-message", "#fp-add-button");
  $("#fp-add-name").onkeydown = (event) => {
    if (event.key === "Enter") addFriend("#fp-add-name", "#fp-add-message", "#fp-add-button");
  };
  $("#fp-requests").onclick = answerRequest;
  $("#fp-filters").onclick = (event) => {
    const button = event.target.closest("[data-filter]");
    if (!button) return;
    state.friendsFilter = button.dataset.filter;
    renderFriendsPage();
  };
  $("#fp-search").oninput = () => renderFriendsPage();
  $("#fp-new-group").onclick = () => openGroupDialog("create");
  $("#fp-groups").onclick = (event) => {
    const group = event.target.closest("[data-open-group]");
    if (group) openChat(group.dataset.openGroup);
  };
  $("#fp-list").onclick = (event) => {
    const by = (attribute) => event.target.closest(`[${attribute}]`)?.getAttribute(attribute);
    const join = by("data-fp-join");
    if (join) {
      const target = findFriend(join) && joinTarget(findFriend(join));
      if (target) target.world ? joinFriendWorld(target.friend) : joinFriend(target.friend, target.address);
      return;
    }
    const invite = by("data-fp-invite");
    if (invite) return void inviteFriend(invite);
    const chat = by("data-fp-chat");
    if (chat) return void messageFriend(chat);
    const menu = event.target.closest("[data-fp-menu]");
    if (menu) {
      const friend = findFriend(menu.dataset.fpMenu);
      if (friend) friendMenu(menu, friend);
      return;
    }
    const row = event.target.closest(".fp-friend[data-uid]");
    if (row) messageFriend(row.dataset.uid);
  };
  // Agregar amigo desde Mensajes
  $("#chat-add-friend").onclick = openAddFriendDialog;
  const sendDialogRequest = () => addFriend("#dialog-friend-name", "#dialog-friend-message", "#dialog-friend-send");
  $("#dialog-friend-send").onclick = sendDialogRequest;
  $("#dialog-friend-name").onkeydown = (event) => {
    if (event.key === "Enter") sendDialogRequest();
  };
  $("#dialog-friend-cancel").onclick = () => closeModal("#add-friend-dialog");
  $("#friend-list").onclick = (event) => {
    const chat = event.target.closest("[data-friend-chat]");
    if (chat) {
      messageFriend(chat.dataset.friendChat);
      return;
    }
    const join = event.target.closest("[data-join]");
    if (join) {
      const friend = findFriend(join.dataset.join);
      if (friend) joinFriendWorld(friend);
      return;
    }
    const more = event.target.closest("[data-friend-menu]");
    if (more) {
      const friend = findFriend(more.dataset.friendMenu);
      if (friend) friendMenu(more, friend);
      return;
    }
    // Clic en el amigo: su conversación, con lo que está jugando y «Unirse» arriba.
    const row = event.target.closest(".friend[data-uid]");
    if (row) messageFriend(row.dataset.uid);
  };
  listen("social", ({ payload }) => {
    state.social = payload;
    renderFriends();
    if (state.page === "chat" && state.chatOpen) renderChatHeader();
    renderFriendServers();
    if (state.page === "chat") renderChatList();
    if (state.page === "hosted" && host.tab === "players") renderGuests();
  });
  listen("ajustes", ({ payload }) => {
    state.settings = payload;
    renderSidebar();
    renderSettings();
    renderSteps();
  });
}

// ======================================================================
// Mensajes (chat con amigos y grupos)
// ======================================================================
function chatTime(ms) {
  if (!ms) return "";
  const date = new Date(ms);
  const today = new Date();
  if (date.toDateString() === today.toDateString()) return date.toLocaleTimeString(locale(), { hour: "numeric", minute: "2-digit" });
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) return t("Ayer", "Yesterday");
  return date.toLocaleDateString(locale(), { day: "numeric", month: "short" });
}

/** La vista previa de una imagen se guarda en español en la base de datos (la ven todos). */
function chatPreview(text) {
  return text === "📷 Imagen" ? t("📷 Imagen", "📷 Image") : text;
}

function chatAvatar(chat) {
  if (chat.kind === "grupo") return groupAvatar(chat);
  return `<canvas width="8" height="8" class="head" data-head="${escapeHtml(chat.name)}"></canvas>`;
}

/** El amigo de una conversación directa (para su punto de en línea). */
function chatFriend(chat) {
  if (chat?.kind === "grupo") return null;
  const other = chat?.members?.find((m) => !m.you) ?? chat?.members?.[0];
  return other ? findFriend(other.uid) : null;
}

/** Cabeza con el punto verde (en línea) o morado (jugando), como en el panel de la derecha. */
function presenceHead(name, friend) {
  const dot = friend?.online ? (friend.presence?.state === "playing" ? "playing" : "online") : "";
  return `<div class="friend-head"><canvas width="8" height="8" class="head" data-head="${escapeHtml(name)}"></canvas><span class="presence-dot ${dot}"></span></div>`;
}

function updateChatBadge() {
  const unread = (state.chats ?? []).filter((c) => c.unread).length;
  $("#chat-badge").textContent = unread > 9 ? "9+" : String(unread);
  $("#chat-badge").classList.toggle("hidden", unread === 0);
}

function enterChat() {
  renderChatList();
  if (state.social?.connected) invoke("chats").then((list) => {
    state.chats = list;
    renderChatList();
  });
}

function renderChatList() {
  updateChatBadge();
  const connected = !!state.social?.connected;
  $("#chat-new-group").classList.toggle("hidden", !connected);
  $("#chat-add-friend").classList.toggle("hidden", !connected);
  if (!connected) {
    setHtml($("#chat-list"), "", { scope: "fuera" });
    showChatEmpty(
      state.social?.signedIn
        ? t("Conectando con tus amigos…", "Connecting with your friends…")
        : t("Inicia sesión con Google para chatear con tus amigos y sus grupos.", "Sign in with Google to chat with your friends and their groups."),
      !state.social?.signedIn,
    );
    return;
  }
  const chats = allChats();
  const friends = state.social?.friends?.friends ?? [];
  // Amigos con los que todavía no hay conversación: para empezar una.
  const talking = new Set(chats.filter((c) => c.kind === "directo").flatMap((c) => c.members.map((m) => m.uid)));
  const others = friends.filter((f) => !talking.has(f.uid));
  const favorites = chats.filter((c) => isFavoriteChat(c.id));
  const recent = chats.filter((c) => !isFavoriteChat(c.id));
  const chatRow = (c) => `<button class="chat-item ${c.id === state.chatOpen ? "active" : ""} ${c.unread ? "unread" : ""}" data-chat="${escapeHtml(c.id)}" data-key="c:${escapeHtml(c.id)}">
          ${c.kind === "grupo" ? chatAvatar(c) : presenceHead(c.name, chatFriend(c))}
          <span class="grow">
            <span class="chat-item-top"><b>${escapeHtml(c.name)}${isFavoriteChat(c.id) ? favMark() : ""}</b><small>${escapeHtml(chatTime(c.lastAt))}</small></span>
            <span class="chat-item-last">${escapeHtml(c.lastText ? (c.kind === "grupo" && c.lastName ? `${c.lastName}: ${chatPreview(c.lastText)}` : chatPreview(c.lastText)) : c.kind === "grupo" ? t("Grupo nuevo", "New group") : t("Sin mensajes todavía", "No messages yet"))}</span>
          </span>
          ${c.unread ? `<span class="unread-dot"></span>` : ""}
        </button>`;
  const listHtml =
    (favorites.length
      ? `<p class="chat-list-label" data-key="l:fav">${t("Favoritos", "Favorites")}</p>${favorites.map(chatRow).join("")}${recent.length ? `<p class="chat-list-label" data-key="l:recent">${t("Recientes", "Recent")}</p>` : ""}`
      : "") +
    recent.map(chatRow).join("") +
    (others.length
      ? `<p class="chat-list-label" data-key="l:others">${t("Escribir a un amigo", "Write to a friend")}</p>` +
        others
          .map(
            (f) => `<button class="chat-item" data-chat-friend="${escapeHtml(f.uid)}" data-key="f:${escapeHtml(f.uid)}">
              ${presenceHead(f.name, f)}
              <span class="grow"><span class="chat-item-top"><b>${escapeHtml(f.name)}</b></span><span class="chat-item-last">${escapeHtml(friendStatus(f))}</span></span>
            </button>`,
          )
          .join("")
      : "") ||
    `<p class="side-note">${t("Todavía no tienes conversaciones. Agrega amigos con el botón de arriba para escribirles.", "You have no conversations yet. Add friends with the button above to write to them.")}</p>`;
  // La conversación con un mensaje nuevo sube a su lugar en vez de saltar.
  if (setHtml($("#chat-list"), listHtml)) for (const canvas of $$("#chat-list canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
  if (!state.chatOpen) showChatEmpty(chats.length ? t("Elige una conversación.", "Choose a conversation.") : t("Escríbele a un amigo para empezar.", "Write to a friend to start."), false);
  else renderChatHeader();
}

function showChatEmpty(text, withSignIn) {
  $("#chat-conversation").classList.add("hidden");
  $("#chat-empty").classList.remove("hidden");
  $("#chat-empty").innerHTML = `<span class="chat-empty-icon">${icon("chat")}</span><p>${escapeHtml(text)}</p>${
    withSignIn ? `<button class="btn primary" id="chat-sign-in">${t("Iniciar sesión con Google", "Sign in with Google")}</button>` : ""
  }`;
  const button = $("#chat-sign-in");
  if (button) button.onclick = signInGoogle;
}

function currentChat() {
  return (state.chats ?? []).find((c) => c.id === state.chatOpen) ?? (state.pendingChat?.id === state.chatOpen ? state.pendingChat : undefined);
}

// ---------- Favoritos: conversaciones y grupos (salen primero) ----------
function isFavoriteChat(id) {
  return (state.settings.favoriteChats ?? []).includes(id);
}

const favMark = () => `<span class="fav-mark" title="${t("Favorito", "Favorite")}">${icon("star")}</span>`;

async function toggleFavoriteChat(id) {
  try {
    state.settings.favoriteChats = await invoke("favorito_chat", { chat: id, activo: !isFavoriteChat(id) });
    renderGroups();
    renderFriends();
    if (state.page === "chat") renderChatList();
  } catch (error) {
    toast(String(error), "error");
  }
}

/** La conversación directa con un amigo (se crea si no existe) para marcarla como favorita. */
async function toggleFavoriteFriend(friend) {
  let chat = allChats().find((c) => c.kind === "directo" && c.members.some((m) => m.uid === friend.uid));
  try {
    const id = chat?.id ?? (await invoke("chat_con_amigo", { uid: friend.uid }));
    if (!chat && !(state.chats ?? []).some((c) => c.id === id)) {
      state.pendingChat = { id, kind: "directo", name: friend.name, members: [{ uid: friend.uid, name: friend.name }], lastText: "", lastName: "", lastAt: 0, unread: false };
    }
    await toggleFavoriteChat(id);
  } catch (error) {
    toast(String(error), "error");
  }
}

/** Amigos con su conversación en favoritos. */
function favoriteFriendUids() {
  return new Set(
    allChats()
      .filter((c) => c.kind === "directo" && isFavoriteChat(c.id))
      .flatMap((c) => c.members.map((m) => m.uid)),
  );
}

/** Las conversaciones, con la que se acaba de crear aunque Firebase todavía no la mande. */
function allChats() {
  const chats = state.chats ?? [];
  const pending = state.pendingChat && !chats.some((c) => c.id === state.pendingChat.id) ? [state.pendingChat] : [];
  return [...pending, ...chats];
}

function renderChatHeader() {
  const chat = currentChat();
  if (!chat) return;
  $("#chat-avatar").innerHTML = chat.kind === "grupo" ? chatAvatar(chat) : presenceHead(chat.name, chatFriend(chat));
  const canvas = $("#chat-avatar canvas");
  if (canvas) drawHead(canvas, canvas.dataset.head);
  $("#chat-title").textContent = chat.name;
  let join = null;
  $("#chat-avatar").classList.toggle("clickable", chat.kind === "grupo");
  $("#chat-avatar").title = chat.kind === "grupo" ? t("Miembros del grupo", "Group members") : "";
  if (chat.kind === "grupo") {
    $("#chat-subtitle").textContent = chat.members.map((m) => m.name).join(", ");
  } else {
    const friend = (state.social?.friends?.friends ?? []).find((f) => chat.members.some((m) => m.uid === f.uid));
    $("#chat-subtitle").textContent = friend ? friendStatus(friend) : "";
    join = friend && joinTarget(friend);
  }
  state.chatJoin = join;
  const favorite = isFavoriteChat(chat.id);
  $("#chat-fav").classList.toggle("starred", favorite);
  $("#chat-fav").title = favorite ? t("Quitar de favoritos", "Remove from favorites") : t("Agregar a favoritos", "Add to favorites");
  $("#chat-join").classList.toggle("hidden", !join);
  if (join) $("#chat-join").title = join.title;
}

/** A dónde se puede entrar con un amigo: su mundo abierto o el servidor donde está jugando. */
function joinTarget(friend) {
  const p = friend.presence ?? {};
  if (!friend.online || p.state !== "playing") return null;
  if (friend.worldOpen && (p.address || p.lan)) return { friend, world: true, address: p.address ?? p.lan, title: t(`Entrar al mundo de ${friend.name}`, `Join ${friend.name}'s world`) };
  if (p.world === "server" && p.server) return { friend, address: p.server, title: t(`Entrar a ${p.server} con ${friend.name}`, `Join ${p.server} with ${friend.name}`) };
  return null;
}

async function openChat(id) {
  if (state.page !== "chat") navigate("chat");
  if (state.chatOpen !== id) state.chatReply = null;
  renderReplyBar();
  state.chatOpen = id;
  state.chatMessages = null;
  $("#chat-empty").classList.add("hidden");
  $("#chat-conversation").classList.remove("hidden");
  state.chatShown = null;
  setHtml($("#chat-messages"), `<div class="content-loading"><div class="spinner"></div></div>`, { scope: "cargando" });
  showNewBelow(false);
  renderChatList();
  renderChatHeader();
  try {
    await invoke("abrir_chat", { chat: id });
    invoke("marcar_leido", { chat: id }).catch(() => {});
  } catch (error) {
    toast(String(error), "error");
  }
  setTimeout(() => $("#chat-input").focus(), 50);
}

function closeChat() {
  state.chatOpen = null;
  state.chatMessages = null;
  invoke("cerrar_chat").catch(() => {});
}

function renderMessages() {
  const box = $("#chat-messages");
  const messages = state.chatMessages ?? [];
  const nearBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 120;
  // Mismo chat que la última vez: lo nuevo entra y lo demás se acomoda. Otro chat: se pinta de golpe.
  const same = state.chatShown === state.chatOpen;
  const known = same ? state.chatKnown : null;
  state.chatShown = state.chatOpen;
  state.chatKnown = new Set(messages.map((m) => m.id));
  if (!messages.length) {
    setHtml(box, `<p class="chat-hint">${t("Todavía no hay mensajes. ¡Saluda!", "No messages yet. Say hi!")}</p>`, { scope: state.chatOpen ?? "" });
    showNewBelow(false);
    return;
  }
  const group = currentChat()?.kind === "grupo";
  let html = "";
  let previous = null;
  for (const m of messages) {
    const day = new Date(m.at).toDateString();
    if (!previous || new Date(previous.at).toDateString() !== day) {
      html += `<div class="chat-day" data-key="d:${escapeHtml(day)}">${escapeHtml(new Date(m.at).toLocaleDateString("es-MX", { weekday: "long", day: "numeric", month: "long" }))}</div>`;
    }
    const continued = previous && previous.from === m.from && m.at - previous.at < 5 * 60000 && new Date(previous.at).toDateString() === day;
    const author = group && !m.mine && !continued ? `<span class="bubble-author">${escapeHtml(m.name)}</span>` : "";
    const share = !m.image && parseShare(m.text);
    const invite = !m.image && !share && parseInvite(m.text);
    const editing = state.chatEditing?.id === m.id;
    const body = editing
      ? `<textarea class="bubble-edit" rows="1" maxlength="2000" aria-label="${t("Editar mensaje", "Edit message")}">${escapeHtml(state.chatEditing.draft)}</textarea><span class="bubble-edit-hint">${t("Enter: guardar · Esc: cancelar", "Enter: save · Esc: cancel")}</span>`
      : m.image
      ? `<button class="bubble-image" data-image="${escapeHtml(m.id)}" style="aspect-ratio:${m.width || 4}/${m.height || 3}">${
          state.chatImages?.[m.id] ? `<img src="${state.chatImages[m.id]}" alt="">` : `<span class="spinner tiny"></span>`
        }</button>`
      : share
        ? shareCardHtml(share, m)
        : invite
          ? inviteCardHtml(invite, m)
          : `<span class="bubble-text">${escapeHtml(m.text ?? "")}</span>`;
    // Al pasar el ratón: reaccionar y responder; en los tuyos también editar (solo texto) y borrar.
    // Clic derecho para el menú.
    const tool = (attribute, name, title) => `<button class="bubble-tool" ${attribute}="${escapeHtml(m.id)}" title="${title}">${icon(name)}</button>`;
    const tools = editing
      ? ""
      : `<div class="bubble-tools">${tool("data-msg-react", "smile", t("Reaccionar", "React"))}${tool("data-msg-reply", "reply", t("Responder", "Reply"))}${
          m.mine ? `${canEditMessage(m) ? tool("data-msg-edit", "pencil", t("Editar", "Edit")) : ""}${tool("data-msg-delete", "trash", t("Borrar", "Delete"))}` : ""
        }</div>`;
    const quote = m.reply && !editing ? replyQuoteHtml(m.reply, messages) : "";
    const reactions = m.reactions?.length
      ? `<div class="bubble-reactions">${m.reactions
          .map(
            (r) =>
              `<button class="reaction ${r.mine ? "mine" : ""}" data-reaction="${escapeHtml(m.id)}" data-emoji="${escapeHtml(r.emoji)}" title="${escapeHtml(r.names.join(", "))}">${escapeHtml(r.emoji)}${r.count > 1 ? `<span>${r.count}</span>` : ""}</button>`,
          )
          .join("")}</div>`
      : "";
    const time = new Date(m.at).toLocaleTimeString(locale(), { hour: "numeric", minute: "2-digit" });
    const stack = `<div class="bubble-stack"><div class="bubble ${editing ? "editing" : ""}">${author}${quote}${body}<span class="bubble-time">${m.edited ? `<span class="bubble-edited">${t("editado", "edited")}</span> · ` : ""}${escapeHtml(time)}</span></div>${reactions}</div>`;
    html += `<div class="bubble-row ${m.mine ? "mine" : ""} ${continued ? "continued" : ""}" data-msg="${escapeHtml(m.id)}" data-key="m:${escapeHtml(m.id)}">
      ${m.mine ? tools + stack : stack + tools}
    </div>`;
    previous = m;
  }
  // Abajo si ya estabas abajo, si acabas de mandar algo o si es la primera vez que se ve este chat.
  const stick = nearBottom || !same || state.chatStick;
  state.chatStick = false;
  const changed = setHtml(box, html, { scope: state.chatOpen ?? "", after: () => stick && (box.scrollTop = box.scrollHeight) });
  // Si estás leyendo más arriba y llega algo de otro, un botón te lleva abajo.
  const arrived = known && messages.some((m) => !m.mine && !known.has(m.id));
  if (stick) showNewBelow(false);
  else if (arrived) showNewBelow(true);
  if (!changed) return void loadChatImages();
  // Si llega algo mientras editas, sigues escribiendo donde ibas.
  const area = box.querySelector(".bubble-edit");
  if (area) {
    autoGrow(area);
    area.focus();
    const at = state.chatEditing.caret ?? area.value.length;
    area.setSelectionRange(at, at);
  }
  loadChatImages();
}

// ---------- Editar y borrar tus mensajes ----------
/** El texto se edita; las imágenes y las instancias compartidas solo se borran. */
function canEditMessage(m) {
  return m.mine && !m.image && !parseShare(m.text) && !parseInvite(m.text);
}

// ---------- Responder y reaccionar ----------
/** Lo que se ve de un mensaje al citarlo o en la lista. */
function messagePreview(m) {
  if (m.image) return t("📷 Imagen", "📷 Image");
  const share = parseShare(m.text);
  if (share) return `📦 ${share.n}`;
  const invite = parseInvite(m.text);
  if (invite) return `🎮 ${t("Invitación", "Invite")}`;
  return (m.text ?? "").replace(/\s+/g, " ").trim();
}

function replyQuoteHtml(reply, messages) {
  const original = messages.find((x) => x.id === reply.id);
  // Si estaba entre los mensajes cargados (los ids van por hora) y ya no está, lo borraron.
  const gone = !original && messages.length > 0 && reply.id >= messages[0].id;
  const text = gone ? t("Mensaje borrado", "Deleted message") : original ? messagePreview(original) : chatPreview(reply.text);
  return `<button class="bubble-quote ${gone ? "gone" : ""}" data-quote="${escapeHtml(reply.id)}" title="${t("Ir al mensaje", "Go to the message")}"><b>${escapeHtml(original?.name ?? reply.name)}</b><span>${escapeHtml(text)}</span></button>`;
}

function startReply(id) {
  const m = state.chatMessages?.find((x) => x.id === id);
  if (!m) return;
  if (state.chatEditing) cancelEditMessage();
  state.chatReply = { id, name: m.name, mine: m.mine, text: messagePreview(m) };
  renderReplyBar();
  $("#chat-input").focus();
}

function cancelReply() {
  state.chatReply = null;
  renderReplyBar();
  $("#chat-input").focus();
}

/** La barra «Respondiendo a…» encima de la caja de escribir. */
function renderReplyBar() {
  const bar = $("#chat-reply");
  const reply = state.chatReply;
  bar.classList.toggle("hidden", !reply);
  if (!reply) return void (bar.innerHTML = "");
  const who = reply.mine ? t("Respondiendo a tu mensaje", "Replying to your message") : t(`Respondiendo a ${reply.name}`, `Replying to ${reply.name}`);
  bar.innerHTML = `${icon("reply")}<div class="grow"><b>${escapeHtml(who)}</b><span>${escapeHtml(reply.text)}</span></div>
    <button class="circle-btn small" id="chat-reply-cancel" title="${t("Cancelar (Esc)", "Cancel (Esc)")}">${icon("close")}</button>`;
  $("#chat-reply-cancel").onclick = cancelReply;
}

/** Lleva al mensaje citado y lo resalta un momento. */
function showQuoted(id) {
  const row = $(`#chat-messages [data-msg="${CSS.escape(id)}"]`);
  if (!row) return toast(t("Ese mensaje es más viejo que los que se ven aquí.", "That message is older than the ones shown here."));
  row.scrollIntoView({ block: "center", behavior: motionOk() ? "smooth" : "auto" });
  row.classList.remove("flash");
  void row.offsetWidth;
  row.classList.add("flash");
  setTimeout(() => row.classList.remove("flash"), 1300);
}

const REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🔥"];

/** Pone tu reacción (o la quita si ya era esa). Se ve al momento; Firebase la confirma después. */
async function react(id, emoji) {
  const m = state.chatMessages?.find((x) => x.id === id);
  if (!m || !state.chatOpen) return;
  const current = m.reactions?.find((r) => r.mine)?.emoji ?? null;
  const next = current === emoji ? null : emoji;
  const me = state.social?.friends?.name ?? state.settings.username;
  const list = (m.reactions ?? []).map((r) => ({ ...r, names: [...r.names] }));
  const mine = list.find((r) => r.mine);
  if (mine) Object.assign(mine, { count: mine.count - 1, mine: false, names: mine.names.filter((n) => n !== me) });
  if (next) {
    let target = list.find((r) => r.emoji === next);
    if (!target) list.push((target = { emoji: next, count: 0, mine: false, names: [] }));
    Object.assign(target, { count: target.count + 1, mine: true, names: [...target.names, me] });
  }
  m.reactions = list.filter((r) => r.count > 0);
  renderMessages();
  try {
    await invoke("reaccionar", { chat: state.chatOpen, mensaje: id, emoji: next });
  } catch (error) {
    toast(String(error), "error");
  }
}

/** Las reacciones para elegir, en un menú junto al mensaje. */
function reactionPicker(anchor, id, extra = []) {
  const m = state.chatMessages?.find((x) => x.id === id);
  if (!m) return;
  const current = m.reactions?.find((r) => r.mine)?.emoji;
  contextMenu(anchor, extra, { emojis: { list: REACTIONS, current, pick: (emoji) => react(id, emoji) } });
}

// ---------- Reportar problema ----------
/** Arma el resumen (versión, PC, instancia, error, doctor y P2P) y lo manda por chat a quien elijas
 * (sale elegido quien da soporte, si es tu amigo). Sin sesión de Google, se puede copiar. */
async function openReport(id = null) {
  state.report = { id, text: null, to: null };
  $("#report-note").value = "";
  $("#report-preview").innerHTML = `<span class="spinner tiny"></span>${t("Revisando la instancia…", "Checking the instance…")}`;
  $("#report-send").disabled = true;
  $("#report-copy").disabled = true;
  renderReportTargets();
  openModal("#report-dialog");
  setTimeout(() => $("#report-note").focus(), 60);
  try {
    const report = await invoke("reporte_problema", { id });
    if (state.report?.id !== id) return;
    Object.assign(state.report, report);
    $("#report-preview").textContent = report.text;
    $("#report-copy").disabled = false;
    renderReportTargets();
  } catch (error) {
    $("#report-preview").textContent = String(error);
  }
}

/** A quién se puede mandar: amigos (quien da soporte primero) y grupos. */
function renderReportTargets() {
  const connected = !!state.social?.connected;
  const friends = [...(state.social?.friends?.friends ?? [])];
  const support = state.report?.to?.toLowerCase();
  friends.sort((a, b) => (b.name.toLowerCase() === support) - (a.name.toLowerCase() === support) || a.name.localeCompare(b.name, "es", { sensitivity: "base" }));
  const groups = (state.chats ?? []).filter((c) => c.kind === "grupo");
  const options = [...friends.map((f) => `<option value="f:${escapeHtml(f.uid)}">${escapeHtml(f.name)}</option>`), ...groups.map((g) => `<option value="g:${escapeHtml(g.id)}">${escapeHtml(g.name)} (${t("grupo", "group")})</option>`)];
  // Lo que eligió el jugador se respeta; si no, va a quien da soporte (sale primero).
  const picked = state.report?.picked;
  $("#report-to").innerHTML = options.join("");
  if (picked && $(`#report-to option[value="${CSS.escape(picked)}"]`)) $("#report-to").value = picked;
  $("#report-to-box").classList.toggle("hidden", !connected || !options.length);
  const hint = !connected
    ? t("Inicia sesión con Google para mandarlo por chat. Mientras, puedes copiarlo y pegarlo donde quieras.", "Sign in with Google to send it by chat. Meanwhile, you can copy it and paste it anywhere.")
    : !options.length
      ? t("Agrega un amigo para mandárselo. Mientras, puedes copiarlo.", "Add a friend to send it to them. Meanwhile, you can copy it.")
      : state.report?.to && !friends.some((f) => f.name.toLowerCase() === support)
        ? t(`Los reportes los revisa ${state.report.to}: agrégalo en Amigos para mandárselo a él.`, `Reports are checked by ${state.report.to}: add them in Friends to send it to them.`)
        : "";
  $("#report-hint").textContent = hint;
  $("#report-hint").classList.toggle("hidden", !hint);
  $("#report-send").disabled = !connected || !options.length || !state.report?.text;
}

function reportMessage() {
  const note = $("#report-note").value.trim();
  return note ? `${note}\n\n${state.report.text}` : state.report.text;
}

async function sendReport() {
  const target = $("#report-to").value;
  if (!target || !state.report?.text) return;
  $("#report-send").disabled = true;
  try {
    const chat = target.startsWith("f:") ? await invoke("chat_con_amigo", { uid: target.slice(2) }) : target.slice(2);
    await invoke("enviar_mensaje", { chat, texto: reportMessage(), respuesta: null });
    closeModal("#report-dialog");
    toast(t("Reporte enviado. Gracias.", "Report sent. Thanks."));
  } catch (error) {
    toast(String(error), "error");
    $("#report-send").disabled = false;
  }
}

function wireReportEvents() {
  $("#report-close").onclick = () => closeModal("#report-dialog");
  $("#report-send").onclick = sendReport;
  $("#report-to").onchange = () => state.report && (state.report.picked = $("#report-to").value);
  $("#report-copy").onclick = () => state.report?.text && copyText(reportMessage(), t("Reporte copiado.", "Report copied."));
  $("#about-report").onclick = () => {
    closeModal("#settings-dialog");
    openReport(null);
  };
  $("#doctor-report").onclick = () => {
    const id = state.doctor?.id;
    closeModal("#doctor-dialog");
    openReport(id ?? null);
  };
}

// ---------- Invitaciones que se mandan desde el juego ----------
const INVITE_PREFIX = "parabellum:invitacion:";

function parseInvite(text) {
  if (!text?.startsWith(INVITE_PREFIX)) return null;
  try {
    const invite = JSON.parse(text.slice(INVITE_PREFIX.length));
    return invite?.t ? invite : null;
  } catch {
    return null;
  }
}

function inviteCardHtml(invite, m) {
  const server = invite.t === "servidor";
  const title = m.mine
    ? server ? t(`Invitaste a ${invite.n}`, `You invited them to ${invite.n}`) : t("Invitaste a tu mundo", "You invited them to your world")
    : server ? t(`Te invitó a ${invite.n}`, `Invited you to ${invite.n}`) : t("Te invitó a su mundo", "Invited you to their world");
  return `<div class="invite-card"><span class="invite-icon">🎮</span><div class="grow"><b>${escapeHtml(title)}</b><span>${escapeHtml(invite.p ?? "")}</span></div>${
    m.mine ? "" : `<button class="btn small" data-invite-join="${escapeHtml(m.id)}">${t("Unirse", "Join")}</button>`
  }</div>`;
}

function joinInvite(id) {
  const m = state.chatMessages?.find((x) => x.id === id);
  const invite = m && parseInvite(m.text);
  if (!invite) return;
  if (invite.t === "servidor") {
    const item = findActivity(`hosted:${m.from}:${invite.k}`);
    if (item?.online) return joinActivity(item);
    return toast(t(`${invite.n} no está encendido ahora.`, `${invite.n} is not on right now.`), "error");
  }
  const friend = findFriend(m.from);
  if (friend?.worldOpen) return joinFriendWorld(friend);
  toast(t(`${m.name} ya no tiene su mundo abierto.`, `${m.name} no longer has their world open.`), "error");
}

function startEditMessage(id) {
  const m = state.chatMessages?.find((x) => x.id === id);
  if (!m || !canEditMessage(m)) return;
  state.chatEditing = { id, draft: m.text ?? "" };
  renderMessages();
}

function cancelEditMessage() {
  state.chatEditing = null;
  renderMessages();
  $("#chat-input").focus();
}

async function saveEditMessage() {
  const editing = state.chatEditing;
  if (!editing) return;
  const text = editing.draft.trim();
  // Vacío: es borrarlo (y eso se confirma).
  if (!text) return void deleteMessage(editing.id);
  const m = state.chatMessages?.find((x) => x.id === editing.id);
  state.chatEditing = null;
  renderMessages();
  $("#chat-input").focus();
  if (!m || m.text === text) return;
  try {
    await invoke("editar_mensaje", { chat: state.chatOpen, mensaje: editing.id, texto: text });
  } catch (error) {
    toast(String(error), "error");
  }
}

async function deleteMessage(id) {
  const m = state.chatMessages?.find((x) => x.id === id);
  if (!m?.mine) return;
  const ok = await confirmAction({
    title: t("Borrar mensaje", "Delete message"),
    text: t("Se borra para todos en esta conversación.", "It is deleted for everyone in this conversation."),
    ok: t("Borrar", "Delete"),
  });
  if (!ok) return;
  if (state.chatEditing?.id === id) state.chatEditing = null;
  try {
    await invoke("borrar_mensaje", { chat: state.chatOpen, mensaje: id });
  } catch (error) {
    toast(String(error), "error");
  }
}

function messageMenu(anchor, id) {
  const m = state.chatMessages?.find((x) => x.id === id);
  if (!m) return;
  const items = [{ icon: "reply", label: t("Responder", "Reply"), action: () => startReply(id) }];
  if (m.text && !parseShare(m.text) && !parseInvite(m.text)) items.push({ icon: "copy", label: t("Copiar texto", "Copy text"), action: () => copyText(m.text, t("Texto copiado.", "Text copied.")) });
  if (canEditMessage(m)) items.push({ icon: "pencil", label: t("Editar", "Edit"), action: () => startEditMessage(id) });
  if (m.mine) items.push({ icon: "trash", label: t("Borrar", "Delete"), danger: true, separator: true, action: () => deleteMessage(id) });
  reactionPicker(anchor, id, items);
}

async function loadChatImages() {
  state.chatImages ??= {};
  for (const button of $$("#chat-messages [data-image]")) {
    const id = button.dataset.image;
    if (state.chatImages[id] || state.chatImageLoading?.has(id)) continue;
    (state.chatImageLoading ??= new Set()).add(id);
    const chat = state.chatOpen;
    invoke("imagen_chat", { chat, mensaje: id })
      .then((data) => {
        state.chatImages[id] = data;
        const target = $(`#chat-messages [data-image="${CSS.escape(id)}"]`);
        if (target) target.innerHTML = `<img src="${data}" alt="">`;
      })
      .catch(() => {
        const target = $(`#chat-messages [data-image="${CSS.escape(id)}"]`);
        if (target) target.innerHTML = `<span class="muted small-text">${t("No se pudo cargar", "Could not load")}</span>`;
      })
      .finally(() => state.chatImageLoading.delete(id));
  }
}

/** El botón «Mensajes nuevos» sobre la caja de escribir. */
function showNewBelow(show) {
  const button = $("#chat-new-below");
  if (!button || button.classList.contains("hidden") === !show) return;
  button.innerHTML = `${icon("down")}<span>${t("Mensajes nuevos", "New messages")}</span>`;
  button.style.bottom = `${($(".chat-composer")?.offsetHeight ?? 72) + 12}px`;
  button.classList.toggle("hidden", !show);
}

async function sendChatText() {
  const input = $("#chat-input");
  const text = input.value.trim();
  if (!text || !state.chatOpen) return;
  state.chatStick = true;
  input.value = "";
  autoGrow(input);
  const reply = state.chatReply;
  state.chatReply = null;
  renderReplyBar();
  try {
    await invoke("enviar_mensaje", { chat: state.chatOpen, texto: text, respuesta: reply?.id ?? null });
  } catch (error) {
    input.value = text;
    state.chatReply = reply;
    renderReplyBar();
    toast(String(error), "error");
  }
}

function autoGrow(textarea) {
  textarea.style.height = "auto";
  textarea.style.height = `${Math.min(textarea.scrollHeight, 160)}px`;
}

/** Reduce una imagen (máx. 1280 px y ~330 KB en JPEG) para enviarla por el chat. */
async function reduceImage(source) {
  // Un archivo (elegido o pegado) o un data URL (soltado en la ventana, ya leído por Parabellum).
  const image = typeof source === "string" ? await loadImage(source) : await loadImageFile(source);
  let scale = Math.min(1, 1280 / Math.max(image.naturalWidth, image.naturalHeight));
  for (let attempt = 0; attempt < 8; attempt++) {
    const width = Math.max(1, Math.round(image.naturalWidth * scale));
    const height = Math.max(1, Math.round(image.naturalHeight * scale));
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext("2d");
    ctx.fillStyle = "#ffffff"; // las transparencias quedan en blanco (JPEG no tiene)
    ctx.fillRect(0, 0, width, height);
    ctx.drawImage(image, 0, 0, width, height);
    for (const quality of [0.82, 0.72, 0.6]) {
      const data = canvas.toDataURL("image/jpeg", quality);
      if (data.length <= 440000) return { data, width, height };
    }
    scale *= 0.75;
  }
  throw new Error(t("No se pudo reducir la imagen lo suficiente.", "The image could not be shrunk enough."));
}

/** Elegida, pegada o soltada: primero se ve tal como se va a mandar (ya reducida) y se confirma. */
async function sendChatImage(source) {
  if (!state.chatOpen) return toast(t("Primero abre una conversación.", "Open a conversation first."), "error");
  const chat = state.chatOpen;
  let image;
  try {
    image = await reduceImage(source);
  } catch (error) {
    return toast(String(error?.message ?? error), "error");
  }
  const name = currentChat()?.name;
  state.imageToSend = { chat, ...image };
  $("#image-title").textContent = name ? t(`Enviar imagen a ${name}`, `Send image to ${name}`) : t("Enviar imagen", "Send image");
  $("#image-preview").innerHTML = `<img src="${image.data}" alt="">`;
  const kb = Math.round((image.data.length * 3) / 4 / 1024);
  $("#image-meta").textContent = `${image.width} × ${image.height} · ${kb} KB`;
  $("#image-send").disabled = false;
  openModal("#image-dialog");
  setTimeout(() => $("#image-send").focus(), 60);
}

async function confirmChatImage() {
  const image = state.imageToSend;
  if (!image) return;
  $("#image-send").disabled = true;
  try {
    await invoke("enviar_imagen", { chat: image.chat, datos: image.data, ancho: image.width, alto: image.height });
    state.imageToSend = null;
    closeModal("#image-dialog");
    if (state.chatOpen === image.chat) state.chatStick = true;
  } catch (error) {
    $("#image-send").disabled = false;
    toast(String(error?.message ?? error), "error");
  }
}

function cancelChatImage() {
  state.imageToSend = null;
  closeModal("#image-dialog");
  $("#chat-input")?.focus();
}

/** Ancla en el último clic: para abrir un menú que sale de otro menú (que ya se cerró). */
function pointerAnchor() {
  const { x = innerWidth / 2, y = innerHeight / 2 } = state.pointer ?? {};
  return { getBoundingClientRect: () => new DOMRect(x, y, 0, 0), isConnected: false, focus() {} };
}
document.addEventListener("pointerdown", (event) => (state.pointer = { x: event.clientX, y: event.clientY }), true);

// ---------- Favoritos: mods, resource packs y shaders de Modrinth ----------
const favoriteKinds = { mods: "mod", resourcepacks: "resourcepack", shaderpacks: "shader" };

function isFavoriteMod(projectId) {
  return (state.settings.favoriteMods ?? []).some((m) => m.projectId === projectId);
}

async function toggleFavoriteMod(favorite) {
  const on = !isFavoriteMod(favorite.projectId);
  try {
    state.settings.favoriteMods = await invoke("favorito_mod", { proyecto: favorite, activo: on });
    toast(on ? t(`${favorite.title} está en tus favoritos: instálalo en cualquier instancia con «Favoritos».`, `${favorite.title} is in your favorites: install it in any instance with «Favorites».`) : t(`${favorite.title} ya no está en tus favoritos.`, `${favorite.title} is no longer in your favorites.`));
  } catch (error) {
    toast(String(error), "error");
  }
  if (state.page === "instance") renderContent();
  if (state.page === "project" && state.projectData) renderProjectInstall();
  if (!$("#fav-dialog").classList.contains("hidden")) renderFavoritesDialog();
}

/** Instala favoritos en una instancia (los que sean compatibles). Devuelve un resumen. */
async function installFavorites(instanceId, favorites, onStep = () => {}) {
  const files = new Map();
  const installed = [];
  const skipped = [];
  for (const [index, favorite] of favorites.entries()) {
    onStep(t(`Revisando ${favorite.title} (${index + 1}/${favorites.length})…`, `Checking ${favorite.title} (${index + 1}/${favorites.length})…`));
    try {
      const plan = await invoke("preparar_instalacion", { id: instanceId, proyecto: favorite.projectId, version: null });
      if (plan.installed) continue;
      if (plan.problem) {
        skipped.push(`${favorite.title}: ${plan.problem}`);
        continue;
      }
      for (const file of plan.files) if (!files.has(file.projectId)) files.set(file.projectId, file);
      installed.push(favorite.title);
    } catch (error) {
      skipped.push(`${favorite.title}: ${error}`);
    }
  }
  if (files.size) {
    onStep(t(`Descargando ${plural(files.size, "archivo", "archivos")}…`, `Downloading ${plural(files.size, "file", "files")}…`));
    await invoke("instalar_contenido", { id: instanceId, archivos: [...files.values()] });
  }
  if (state.contentFor === instanceId) {
    state.content = null;
    state.updates = { for: null, list: [] };
  }
  return { installed, skipped };
}

function favoritesSummary(name, { installed, skipped }) {
  const done = installed.length ? t(`En ${name}: ${installed.join(", ")}.`, `In ${name}: ${installed.join(", ")}.`) : "";
  const not = skipped.length ? t(` No compatibles: ${skipped.join(" · ")}`, ` Not compatible: ${skipped.join(" · ")}`) : "";
  return (done + not).trim() || t("Ya tenía todos tus favoritos.", "It already had all your favorites.");
}

/** Diálogo de la pestaña Contenido: tus favoritos de esta categoría, para instalarlos aquí. */
function openFavoritesDialog() {
  if (!state.current) return;
  state.favDialog = { kind: state.kind, selected: null, busy: false };
  $("#fav-status").textContent = "";
  $("#fav-status").classList.remove("error");
  renderFavoritesDialog();
  openModal("#fav-dialog");
}

function renderFavoritesDialog() {
  const dialog = state.favDialog;
  if (!dialog) return;
  const kind = favoriteKinds[dialog.kind] ?? "mod";
  const list = (state.settings.favoriteMods ?? []).filter((m) => m.kind === kind);
  const have = new Set((state.content?.[dialog.kind] ?? []).map((i) => i.projectId).filter(Boolean));
  dialog.selected ??= new Set(list.filter((m) => !have.has(m.projectId)).map((m) => m.projectId));
  $("#fav-title").textContent = t(`Tus ${kindNames[dialog.kind]} favoritos`, `Your favorite ${kindNames[dialog.kind]}`);
  $("#fav-list").innerHTML = list.length
    ? list
        .map((m) => {
          const owned = have.has(m.projectId);
          return `<div class="fav-row">
            <div class="content-icon">${m.iconUrl ? `<img src="${escapeHtml(m.iconUrl)}" alt="">` : icon(kindIcons[dialog.kind])}</div>
            <div class="grow"><b>${escapeHtml(m.title)}</b></div>
            ${owned ? `<span class="tag">${t("Ya está", "Already here")}</span>` : `<label class="toggle"><input type="checkbox" data-fav-pick="${escapeHtml(m.projectId)}" ${dialog.selected.has(m.projectId) ? "checked" : ""} ${dialog.busy ? "disabled" : ""}><span></span></label>`}
            <button class="circle-btn small starred" data-fav-remove="${escapeHtml(m.projectId)}" title="${t("Quitar de favoritos", "Remove from favorites")}">${icon("star")}</button>
          </div>`;
        })
        .join("")
    : `<p class="side-note">${t("Todavía no tienes favoritos aquí. Márcalos con la estrella en su página de Explorar o en la lista de Contenido de una instancia.", "You have no favorites here yet. Mark them with the star on their Explore page or in an instance's Content list.")}</p>`;
  const count = list.filter((m) => !have.has(m.projectId) && dialog.selected.has(m.projectId)).length;
  $("#fav-install").disabled = dialog.busy || !count;
  $("#fav-install span").textContent = dialog.busy ? t("Instalando…", "Installing…") : t(`Instalar (${count})`, `Install (${count})`);
}

async function installFavoritesHere() {
  const dialog = state.favDialog;
  const instance = state.current;
  if (!dialog || !instance || dialog.busy) return;
  const kind = favoriteKinds[dialog.kind];
  const picked = (state.settings.favoriteMods ?? []).filter((m) => m.kind === kind && dialog.selected.has(m.projectId));
  dialog.busy = true;
  renderFavoritesDialog();
  $("#download-indicator").classList.remove("hidden");
  try {
    const result = await installFavorites(instance.id, picked, (step) => ($("#fav-status").textContent = step));
    closeModal("#fav-dialog");
    toast(favoritesSummary(instance.name, result), result.skipped.length ? "error" : undefined);
    if (state.page === "instance" && state.current?.id === instance.id) renderInstance();
  } catch (error) {
    $("#fav-status").textContent = String(error);
    $("#fav-status").classList.add("error");
  } finally {
    dialog.busy = false;
    $("#download-indicator").classList.add("hidden");
    renderFavoritesDialog();
  }
}

// ---------- Instancias compartidas (ver compartir.rs: viaja una receta, no los archivos) ----------
const SHARE_PREFIX = "parabellum:instancia:";

function parseShare(text) {
  if (!text?.startsWith(SHARE_PREFIX)) return null;
  try {
    const recipe = JSON.parse(text.slice(SHARE_PREFIX.length));
    return recipe?.n && recipe?.mc ? recipe : null;
  } catch {
    return null;
  }
}

/** La instancia que ya tiene este PC (el mismo modpack de Modrinth, o el mismo nombre). */
function sharedOwned(recipe) {
  if (recipe.s === "modrinth" && recipe.p) return state.instances.find((i) => i.modpackProject === recipe.p);
  return state.instances.find((i) => i.name.toLowerCase() === recipe.n.toLowerCase());
}

function shareCardHtml(recipe, message) {
  const loader = recipe.l && !["vanilla", "none"].includes(recipe.l.toLowerCase()) ? ` · ${recipe.l}` : "";
  const mods = recipe.k ? ` · ${plural(recipe.k, "mod", "mods")}` : "";
  const source =
    recipe.s === "modrinth"
      ? t("Modpack de Modrinth", "Modrinth modpack")
      : recipe.s === "catalogo"
        ? t("Del catálogo de Drive", "From the Drive catalog")
        : t("Mods de Modrinth", "Mods from Modrinth") + (recipe.x ? t(` · ${recipe.x} no viajan (no son de Modrinth)`, ` · ${recipe.x} do not travel (not from Modrinth)`) : "");
  const owned = sharedOwned(recipe);
  const installing = state.shareInstalling === message.id;
  let action = "";
  if (owned) action = `<button class="btn small" data-share-open="${escapeHtml(owned.id)}">${icon("play")}${t("Abrir", "Open")}</button>`;
  else if (!message.mine)
    action = `<button class="btn primary small" data-share-install="${escapeHtml(message.id)}" ${state.shareInstalling ? "disabled" : ""}>${
      installing ? `<span class="spinner tiny"></span>${escapeHtml(state.shareProgress ?? t("Instalando…", "Installing…"))}` : `${icon("download")}${t("Instalar", "Install")}`
    }</button>`;
  return `<span class="share-kicker">${t("Instancia compartida", "Shared instance")}</span>
    <div class="share-card">${art(owned ?? { name: recipe.n })}<div class="grow"><b>${escapeHtml(recipe.n)}</b><span>${escapeHtml(`Minecraft ${recipe.mc}${loader}${mods}`)}</span><small>${escapeHtml(source)}</small></div>${action}</div>`;
}

async function shareInstance(chat, id) {
  const instance = state.instances.find((i) => i.id === id);
  if (!chat || !instance) return;
  toast(t(`Compartiendo ${instance.name}…`, `Sharing ${instance.name}…`));
  try {
    await invoke("compartir_instancia", { chat, id });
  } catch (error) {
    toast(String(error), "error");
  }
}

/** Elegir qué instancia compartir en la conversación abierta. */
function shareMenu(anchor) {
  if (!state.chatOpen) return;
  if (!state.instances.length) return toast(t("Todavía no tienes instancias para compartir.", "You have no instances to share yet."), "error");
  const items = [...state.instances]
    .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))
    .map((i) => ({ icon: "cubes", label: i.name, sub: `Minecraft ${i.minecraftVersion}${i.loader && i.loader !== "Vanilla" ? ` · ${i.loader}` : ""}`, action: () => shareInstance(state.chatOpen, i.id) }));
  contextMenu(anchor, items);
}

/** Desde el menú de una instancia: elegir con quién compartirla. */
function shareWithMenu(anchor, id) {
  if (!state.social?.connected) return toast(t("Inicia sesión con Google para compartir con tus amigos.", "Sign in with Google to share with your friends."), "error");
  const chats = allChats();
  const talking = new Set(chats.filter((c) => c.kind === "directo").flatMap((c) => c.members.map((m) => m.uid)));
  const items = [
    ...chats.map((c) => ({ ...(c.kind === "grupo" ? { icon: "chat" } : { head: c.name }), label: c.name, action: () => shareInstance(c.id, id) })),
    ...(state.social?.friends?.friends ?? [])
      .filter((f) => !talking.has(f.uid))
      .map((f) => ({
        head: f.name,
        label: f.name,
        action: async () => {
          try {
            shareInstance(await invoke("chat_con_amigo", { uid: f.uid }), id);
          } catch (error) {
            toast(String(error), "error");
          }
        },
      })),
  ];
  if (!items.length) return toast(t("Agrega amigos para compartirles instancias.", "Add friends to share instances with them."), "error");
  contextMenu(anchor, items);
}

/** Arma en este PC la instancia que compartió un amigo. */
async function installShared(messageId) {
  const message = (state.chatMessages ?? []).find((m) => m.id === messageId);
  const recipe = message && parseShare(message.text);
  if (!recipe || state.shareInstalling) return;
  state.shareInstalling = messageId;
  state.shareProgress = null;
  renderMessages();
  $("#download-indicator").classList.remove("hidden");
  $("#download-percent").textContent = "…";
  try {
    const id = await invoke("instalar_compartida", { texto: message.text });
    await reloadInstances();
    toast(t(`Lista: ${recipe.n}. Ya puedes jugarla.`, `Ready: ${recipe.n}. You can play it now.`));
    navigate("instance", id);
  } catch (error) {
    toast(String(error), "error");
  } finally {
    state.shareInstalling = null;
    $("#download-indicator").classList.add("hidden");
    if (state.page === "chat") renderMessages();
  }
}

async function messageFriend(uid) {
  const friend = findFriend(uid);
  try {
    const id = await invoke("chat_con_amigo", { uid });
    // La primera vez, Firebase tarda un momento en mandar la conversación: mientras, se abre con lo que ya
    // se sabe del amigo (antes se abría vacía y había que volver a pulsar).
    if (!(state.chats ?? []).some((c) => c.id === id)) {
      const name = friend?.name ?? "…";
      state.pendingChat = { id, kind: "directo", name, members: [{ uid, name }], lastText: "", lastName: "", lastAt: 0, unread: false };
    }
    openChat(id);
  } catch (error) {
    toast(String(error), "error");
  }
}

function openGroupDialog(mode) {
  const chat = currentChat();
  state.groupDialog = mode;
  state.groupPhoto = { data: mode === "edit" ? chat?.photo ?? null : null, changed: false };
  const inGroup = new Set(mode === "add" ? chat?.members.map((m) => m.uid) : []);
  const friends = (state.social?.friends?.friends ?? []).filter((f) => !inGroup.has(f.uid));
  const [title, ok] = {
    add: [t("Agregar amigos al grupo", "Add friends to the group"), t("Agregar", "Add")],
    edit: [t("Editar grupo", "Edit group"), t("Guardar", "Save")],
    members: [t("Miembros del grupo", "Group members"), t("Listo", "Done")],
    create: [t("Nuevo grupo", "New group"), t("Crear grupo", "Create group")],
  }[mode];
  $("#group-title").textContent = title;
  $("#group-ok").textContent = ok;
  $("#group-cancel").classList.toggle("hidden", mode === "members");
  $("#group-name-box").classList.toggle("hidden", mode === "add" || mode === "members");
  $("#group-friends").classList.toggle("hidden", mode === "edit");
  $("#group-friends-label").classList.toggle("hidden", mode === "edit");
  $("#group-friends-label").textContent = mode === "members" ? t("Miembros", "Members") : t("Amigos", "Friends");
  $("#group-name").value = mode === "edit" ? chat?.name ?? "" : "";
  $("#group-error").classList.add("hidden");
  renderGroupPhoto();
  if (mode === "members") {
    renderGroupMembers();
    openModal("#group-dialog");
    return;
  }
  $("#group-friends").innerHTML = friends.length
    ? friends
        .map(
          (f) => `<label class="group-friend"><input type="checkbox" value="${escapeHtml(f.uid)}"><canvas width="8" height="8" class="head" data-head="${escapeHtml(f.name)}"></canvas><span>${escapeHtml(f.name)}</span></label>`,
        )
        .join("")
    : `<p class="muted small-text">${mode === "add" ? t("Todos tus amigos ya están en el grupo.", "All your friends are already in the group.") : t("Primero agrega amigos.", "Add friends first.")}</p>`;
  for (const canvas of $$("#group-friends canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
  openModal("#group-dialog");
  setTimeout(() => (mode === "add" ? null : $("#group-name").focus()), 60);
}

function renderGroupPhoto() {
  const data = state.groupPhoto?.data;
  const pick = $("#group-photo");
  pick.classList.toggle("has-photo", !!data);
  pick.style.backgroundImage = data ? `url("${data}")` : "";
  pick.innerHTML = icon("camera");
  $("#group-photo-remove").classList.toggle("hidden", !data);
}

/** La foto de un grupo: un cuadrado de 128 px del centro de la imagen, en JPEG (unos 10 KB). */
async function reduceGroupPhoto(file) {
  const image = await loadImageFile(file);
  const size = 128;
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext("2d");
  const side = Math.min(image.naturalWidth, image.naturalHeight);
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, size, size);
  ctx.drawImage(image, (image.naturalWidth - side) / 2, (image.naturalHeight - side) / 2, side, side, 0, 0, size, size);
  for (const quality of [0.85, 0.72, 0.6, 0.45]) {
    const data = canvas.toDataURL("image/jpeg", quality);
    if (data.length <= 38000) return data;
  }
  throw new Error(t("No se pudo reducir la foto lo suficiente.", "The photo could not be shrunk enough."));
}

/** Los miembros, con el dueño primero. El dueño puede sacar a alguien o pasarle el grupo. */
function renderGroupMembers() {
  const chat = currentChat();
  if (!chat) return;
  const members = [...chat.members].sort((a, b) => (b.uid === chat.owner) - (a.uid === chat.owner) || b.you - a.you || a.name.localeCompare(b.name, "es", { sensitivity: "base" }));
  const ownerName = chat.members.find((m) => m.uid === chat.owner)?.name;
  $("#group-friends").innerHTML =
    members
      .map((m) => {
        const owner = m.uid === chat.owner;
        const tag = owner ? `<span class="owner-badge">${icon("crown")}${t("Creó el grupo", "Created the group")}</span>` : m.you ? `<span>${t("Tú", "You")}</span>` : "";
        const actions =
          chat.canEdit && !m.you
            ? `<button class="circle-btn small" data-member-owner="${escapeHtml(m.uid)}" title="${t("Pasarle el grupo", "Hand the group over")}">${icon("crown")}</button>
               <button class="circle-btn small" data-member-remove="${escapeHtml(m.uid)}" title="${t("Sacar del grupo", "Remove from the group")}">${icon("close")}</button>`
            : "";
        return `<div class="member-row"><canvas width="8" height="8" class="head" data-head="${escapeHtml(m.name)}"></canvas><div class="grow"><b>${escapeHtml(m.name)}${m.you && owner ? ` · ${t("tú", "you")}` : ""}</b>${tag}</div>${actions}</div>`;
      })
      .join("") +
    (chat.canEdit
      ? ""
      : `<p class="muted small-text">${escapeHtml(ownerName ? t(`Solo ${ownerName} puede cambiar el nombre, la foto y los miembros.`, `Only ${ownerName} can change the name, the photo and the members.`) : "")}</p>`);
  for (const canvas of $$("#group-friends canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
}

async function groupMemberAction(event) {
  if (state.groupDialog !== "members") return;
  const chat = currentChat();
  const remove = event.target.closest("[data-member-remove]");
  const owner = event.target.closest("[data-member-owner]");
  const uid = remove?.dataset.memberRemove ?? owner?.dataset.memberOwner;
  const member = chat?.members.find((m) => m.uid === uid);
  if (!member) return;
  const ok = await confirmAction(
    remove
      ? { title: t(`Sacar a ${member.name}`, `Remove ${member.name}`), text: t("Dejará de ver los mensajes del grupo. Puedes volver a agregarlo.", "They will stop seeing the group's messages. You can add them again."), ok: t("Sacar", "Remove") }
      : { title: t(`Pasarle el grupo a ${member.name}`, `Hand the group over to ${member.name}`), text: t("Podrá cambiar el nombre, la foto y los miembros. Tú ya no.", "They will be able to change the name, the photo and the members. You will not."), ok: t("Pasárselo", "Hand it over") },
  );
  if (!ok) return;
  try {
    await invoke(remove ? "sacar_del_grupo" : "pasar_grupo", { chat: chat.id, uid });
  } catch (error) {
    toast(String(error), "error");
  }
}

async function confirmGroupDialog() {
  const mode = state.groupDialog;
  if (mode === "members") return closeModal("#group-dialog");
  const name = $("#group-name").value.trim();
  const picked = $$("#group-friends input:checked").map((i) => i.value);
  const photo = state.groupPhoto ?? { data: null, changed: false };
  const fail = (text) => {
    $("#group-error").textContent = text;
    $("#group-error").classList.remove("hidden");
  };
  try {
    if (mode === "create") {
      if (!name) return fail(t("Ponle un nombre al grupo.", "Give the group a name."));
      if (!picked.length) return fail(t("Elige al menos un amigo.", "Choose at least one friend."));
      const id = await invoke("crear_grupo", { nombre: name, amigos: picked, foto: photo.data });
      closeModal("#group-dialog");
      openChat(id);
    } else if (mode === "add") {
      if (!picked.length) return fail(t("Elige al menos un amigo.", "Choose at least one friend."));
      await invoke("agregar_al_grupo", { chat: state.chatOpen, amigos: picked });
      closeModal("#group-dialog");
    } else {
      if (!name) return fail(t("Ponle un nombre al grupo.", "Give the group a name."));
      const changed = name !== currentChat()?.name;
      if (changed || photo.changed) {
        await invoke("editar_grupo", { chat: state.chatOpen, nombre: changed ? name : null, foto: photo.changed ? photo.data : null, quitarFoto: photo.changed && !photo.data });
      }
      closeModal("#group-dialog");
    }
  } catch (error) {
    fail(String(error));
  }
}

function chatMenu(anchor) {
  const chat = currentChat();
  if (!chat) return;
  let items;
  if (chat.kind === "grupo") {
    const owner = chat.owner && chat.members.find((m) => m.uid === chat.owner && m.you);
    items = [
      ...(chat.canEdit
        ? [
            { icon: "pencil", label: t("Editar grupo", "Edit group"), action: () => openGroupDialog("edit") },
            { icon: "user-plus", label: t("Agregar amigos", "Add friends"), action: () => openGroupDialog("add") },
          ]
        : []),
      { icon: "users", label: t("Miembros", "Members"), action: () => openGroupDialog("members") },
      {
        icon: "close",
        label: t("Salir del grupo", "Leave the group"),
        danger: true,
        separator: true,
        action: () =>
          leaveChat(
            chat,
            t("Salir del grupo", "Leave the group"),
            owner
              ? t("Dejarás de ver sus mensajes y el grupo pasará a otro miembro.", "You will stop seeing its messages and the group will pass to another member.")
              : t("Dejarás de ver sus mensajes. Quien creó el grupo puede volver a agregarte.", "You will stop seeing its messages. Whoever created the group can add you again."),
          ),
      },
    ];
  } else {
    items = [{ icon: "close", label: t("Ocultar conversación", "Hide conversation"), action: () => leaveChat(chat, t("Ocultar conversación", "Hide conversation"), t("Desaparece de tu lista; vuelve si te escribe de nuevo.", "It disappears from your list; it comes back if they write to you again.")) }];
  }
  contextMenu(anchor, items);
}

async function leaveChat(chat, title, text) {
  const ok = await confirmAction({ title, text, ok: title.split(" ")[0] });
  if (!ok) return;
  try {
    await invoke("salir_del_chat", { chat: chat.id });
    closeChat();
    renderChatList();
  } catch (error) {
    toast(String(error), "error");
  }
}

function wireChatEvents() {
  $("#chat-new-below").onclick = () => {
    const box = $("#chat-messages");
    box.scrollTo({ top: box.scrollHeight, behavior: motionOk() ? "smooth" : "auto" });
    showNewBelow(false);
  };
  $("#chat-messages").addEventListener("scroll", (event) => {
    const box = event.currentTarget;
    if (box.scrollHeight - box.scrollTop - box.clientHeight < 60) showNewBelow(false);
  });
  $("#chat-list").onclick = (event) => {
    const chat = event.target.closest("[data-chat]");
    if (chat) return openChat(chat.dataset.chat);
    const friend = event.target.closest("[data-chat-friend]");
    if (friend) messageFriend(friend.dataset.chatFriend);
  };
  $("#chat-send").onclick = sendChatText;
  $("#chat-input").addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      sendChatText();
    } else if (event.key === "Escape" && state.chatReply) {
      event.preventDefault();
      event.stopPropagation();
      cancelReply();
    } else if (event.key === "ArrowUp" && !event.target.value) {
      // Como en Discord: flecha arriba con la caja vacía edita tu último mensaje.
      const last = [...(state.chatMessages ?? [])].reverse().find((m) => m.mine);
      if (last && canEditMessage(last)) {
        event.preventDefault();
        startEditMessage(last.id);
      }
    }
  });
  $("#chat-input").addEventListener("input", (event) => autoGrow(event.target));
  // Pegar una imagen (por ejemplo una captura) la envía.
  $("#chat-input").addEventListener("paste", (event) => {
    const file = [...(event.clipboardData?.items ?? [])].find((i) => i.type.startsWith("image/"))?.getAsFile();
    if (!file) return;
    event.preventDefault();
    sendChatImage(file);
  });
  $("#chat-attach").onclick = () => $("#chat-file").click();
  $("#image-send").onclick = confirmChatImage;
  $("#image-cancel").onclick = cancelChatImage;
  $("#image-close").onclick = cancelChatImage;
  $("#image-dialog").addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      confirmChatImage();
    }
  });
  $("#chat-file").onchange = () => {
    const file = $("#chat-file").files[0];
    $("#chat-file").value = "";
    if (file) sendChatImage(file);
  };
  $("#chat-share").onclick = (event) => shareMenu(event.currentTarget);
  $("#chat-fav").onclick = () => state.chatOpen && toggleFavoriteChat(state.chatOpen).then(renderChatHeader);
  // Clic derecho en una conversación: favorito.
  $("#chat-list").addEventListener("contextmenu", (event) => {
    const item = event.target.closest("[data-chat]");
    if (!item) return;
    event.preventDefault();
    const id = item.dataset.chat;
    contextMenu(item, [
      { icon: "star", label: isFavoriteChat(id) ? t("Quitar de favoritos", "Remove from favorites") : t("Agregar a favoritos", "Add to favorites"), action: () => toggleFavoriteChat(id) },
      { icon: "chat", label: t("Abrir", "Open"), action: () => openChat(id) },
    ]);
  });
  $("#chat-join").onclick = () => {
    const join = state.chatJoin;
    if (join) join.world ? joinFriendWorld(join.friend) : joinFriend(join.friend, join.address);
  };
  $("#chat-messages").addEventListener("keydown", (event) => {
    if (!event.target.matches(".bubble-edit")) return;
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      saveEditMessage();
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      cancelEditMessage();
    }
  });
  $("#chat-messages").addEventListener("input", (event) => {
    if (!event.target.matches(".bubble-edit") || !state.chatEditing) return;
    state.chatEditing.draft = event.target.value;
    state.chatEditing.caret = event.target.selectionStart;
    autoGrow(event.target);
  });
  $("#chat-messages").addEventListener("contextmenu", (event) => {
    const row = event.target.closest("[data-msg]");
    if (!row || event.target.closest(".bubble-edit")) return;
    event.preventDefault();
    messageMenu(row.querySelector(".bubble"), row.dataset.msg);
  });
  $("#chat-messages").onclick = (event) => {
    const reaction = event.target.closest("[data-reaction]");
    if (reaction) return void react(reaction.dataset.reaction, reaction.dataset.emoji);
    const reactTool = event.target.closest("[data-msg-react]");
    if (reactTool) return void reactionPicker(reactTool, reactTool.dataset.msgReact);
    const reply = event.target.closest("[data-msg-reply]");
    if (reply) return void startReply(reply.dataset.msgReply);
    const quote = event.target.closest("[data-quote]");
    if (quote) return void showQuoted(quote.dataset.quote);
    const inviteJoin = event.target.closest("[data-invite-join]");
    if (inviteJoin) return void joinInvite(inviteJoin.dataset.inviteJoin);
    const edit = event.target.closest("[data-msg-edit]");
    if (edit) return void startEditMessage(edit.dataset.msgEdit);
    const remove = event.target.closest("[data-msg-delete]");
    if (remove) return void deleteMessage(remove.dataset.msgDelete);
    const install = event.target.closest("[data-share-install]");
    if (install) return void installShared(install.dataset.shareInstall);
    const open = event.target.closest("[data-share-open]");
    if (open) return void navigate("instance", open.dataset.shareOpen);
    const image = event.target.closest("[data-image] img");
    if (!image) return;
    $("#lightbox-image").src = image.src;
    $("#lightbox-caption").textContent = "";
    $("#lightbox").classList.remove("hidden");
  };
  $("#chat-more").onclick = (event) => chatMenu(event.currentTarget);
  $("#chat-avatar").onclick = () => currentChat()?.kind === "grupo" && openGroupDialog("members");
  $("#chat-new-group").onclick = () => openGroupDialog("create");
  $("#group-ok").onclick = confirmGroupDialog;
  $("#group-friends").addEventListener("click", groupMemberAction);
  $("#group-photo").onclick = () => $("#group-photo-file").click();
  $("#group-photo-file").onchange = async () => {
    const file = $("#group-photo-file").files[0];
    $("#group-photo-file").value = "";
    if (!file) return;
    try {
      state.groupPhoto = { data: await reduceGroupPhoto(file), changed: true };
      renderGroupPhoto();
    } catch (error) {
      $("#group-error").textContent = String(error?.message ?? error);
      $("#group-error").classList.remove("hidden");
    }
  };
  $("#group-photo-remove").onclick = () => {
    state.groupPhoto = { data: null, changed: true };
    renderGroupPhoto();
  };
  $("#group-cancel").onclick = () => closeModal("#group-dialog");
  $("#group-name").addEventListener("keydown", (event) => {
    if (event.key === "Enter") confirmGroupDialog();
  });
  listen("chats", ({ payload }) => {
    state.chats = payload;
    if (state.pendingChat && payload.some((c) => c.id === state.pendingChat.id)) state.pendingChat = null;
    renderGroups();
    if (state.chatOpen && !payload.some((c) => c.id === state.chatOpen) && state.pendingChat?.id !== state.chatOpen) closeChat();
    if (state.page === "chat") renderChatList();
    else updateChatBadge();
    // Con la lista de miembros abierta: alguien entró, salió o cambió el dueño.
    if (state.groupDialog === "members" && !$("#group-dialog").classList.contains("hidden")) renderGroupMembers();
  });
  // Favoritos que llegan de la cuenta de Google (cambiados en otro PC).
  listen("favoritos", ({ payload }) => {
    state.settings.favoriteChats = payload.chats;
    state.settings.favoriteMods = payload.mods;
    renderGroups();
    renderFriends();
    if (state.page === "chat") renderChatList();
    if (state.page === "instance") renderContent();
    if (state.page === "project" && state.projectData) renderProjectInstall();
    if (!$("#fav-dialog").classList.contains("hidden")) renderFavoritesDialog();
  });
  listen("chat-mensajes", ({ payload }) => {
    if (payload.chat !== state.chatOpen) return;
    state.chatMessages = payload.messages;
    renderMessages();
    // Leído si la conversación está a la vista.
    if (state.page === "chat" && document.hasFocus()) invoke("marcar_leido", { chat: payload.chat }).catch(() => {});
  });
  listen("abrir-chat", ({ payload }) => openChat(payload));
  if (!tauri) {
    window.addEventListener("demo-mensajes", ({ detail }) => {
      if (detail.chat !== state.chatOpen) return;
      state.chatMessages = detail.messages;
      renderMessages();
    });
  }
  window.addEventListener("focus", () => {
    if (state.page === "chat" && state.chatOpen && currentChat()?.unread) invoke("marcar_leido", { chat: state.chatOpen }).catch(() => {});
  });
}

// ======================================================================
// Servidores
// ======================================================================
function renderServers() {
  const select = $("#server-instance");
  const previous = select.value;
  select.innerHTML = state.instances.length
    ? state.instances.map((i) => `<option value="${escapeHtml(i.id)}">${escapeHtml(i.name)} · ${escapeHtml(subtitle(i))}</option>`).join("")
    : `<option value="">${t("Primero crea una instancia", "Create an instance first")}</option>`;
  if (previous) select.value = previous;
  const servers = state.settings.servers ?? [];
  // Filas como las de tus amigos: icono y estado del propio servidor; clic para el detalle.
  setHtml(
    $("#server-list"),
    servers.length
      ? servers
          .map((s, index) => {
            const info = savedInfo(s);
            const button = `<button class="btn small" data-server-play="${index}" ${info.canJoin ? "" : "disabled"}>${icon("play")}${t("Jugar", "Play")}</button>`;
            return serverCardHtml({ key: `saved:${index}`, info, button, dim: info.status && !info.status.online });
          })
          .join("")
      : `<p class="srv-empty" data-key="vacio">${t("Guarda aquí los servidores públicos a los que entras, con la instancia con la que juegas en cada uno.", "Save the public servers you join here, with the instance you play each one with.")}</p>`,
  );
  $("#servers-count").textContent = servers.length || "";
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
const FOCUSABLE = 'button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [href], [tabindex]:not([tabindex="-1"])';

/** Lo que se puede enfocar y se ve (los pasos ocultos de un diálogo no cuentan). */
function focusables(root) {
  return [...root.querySelectorAll(FOCUSABLE)].filter((e) => e.getClientRects().length && !e.closest(".hidden"));
}

/** El diálogo abierto que está encima. */
function topModal() {
  return [...$$(".overlay")].filter((o) => !o.classList.contains("hidden") && !o.classList.contains("closing")).pop() ?? null;
}

function openModal(id) {
  const overlay = $(id);
  const wasHidden = overlay.classList.contains("hidden") || overlay.classList.contains("closing");
  clearTimeout(overlay.closeTimer);
  overlay.classList.remove("hidden", "closing");
  if (!wasHidden) return;
  if (!overlay.contains(document.activeElement)) overlay.returnFocus = document.activeElement;
  // El foco entra al diálogo (a su primer campo, si tiene) para poder usarlo con el teclado.
  requestAnimationFrame(() => {
    if (overlay.contains(document.activeElement)) return;
    const modal = $(".modal", overlay) ?? overlay;
    const field = focusables(modal).find((e) => e.matches("input:not([type=checkbox]):not([type=range]), textarea"));
    (field ?? modal).focus({ preventScroll: true });
  });
}

function closeModal(id) {
  const overlay = $(id);
  if (overlay.classList.contains("hidden")) return;
  overlay.classList.add("closing");
  overlay.closeTimer = setTimeout(() => overlay.classList.add("hidden"), 150);
  // El foco vuelve a donde estaba (el botón que abrió el diálogo).
  const back = overlay.returnFocus;
  overlay.returnFocus = null;
  if (back?.isConnected && overlay.contains(document.activeElement)) back.focus({ preventScroll: true });
}

/** Marca cada diálogo para el lector de pantalla y hace que las tarjetas se puedan usar con el teclado. */
function setupAccessibility() {
  for (const overlay of $$(".overlay")) {
    const modal = $(".modal", overlay);
    if (!modal) continue;
    modal.setAttribute("role", "dialog");
    modal.setAttribute("aria-modal", "true");
    modal.tabIndex = -1;
    const title = $("h2", modal);
    if (title) {
      title.id ||= `${overlay.id}-titulo`;
      modal.setAttribute("aria-labelledby", title.id);
    }
  }
  // Tarjetas y filas que se pulsan pero no son botones: reciben foco y responden a Enter y Espacio.
  const clickable = "[data-open], [data-project], [data-version], [data-chat], [data-chat-friend], [data-image], [data-external], [data-drive], [data-loader], [data-value], [data-category], [data-format], [data-use-account]";
  const mark = (root) => {
    for (const element of root.querySelectorAll?.(clickable) ?? []) {
      if (element.matches("button, a, input, label, select, textarea") || element.hasAttribute("tabindex")) continue;
      element.tabIndex = 0;
      element.setAttribute("role", "button");
    }
  };
  mark(document);
  new MutationObserver((changes) => {
    for (const change of changes) for (const node of change.addedNodes) if (node.nodeType === 1) mark(node.parentElement ?? node);
  }).observe(document.body, { childList: true, subtree: true });
  document.addEventListener("keydown", (event) => {
    const target = event.target;
    if ((event.key === "Enter" || event.key === " ") && target.getAttribute?.("role") === "button" && !target.matches("button")) {
      event.preventDefault();
      target.click();
    }
    // Tab no sale del diálogo abierto.
    if (event.key === "Tab") {
      const overlay = topModal();
      if (!overlay) return;
      const items = focusables($(".modal", overlay) ?? overlay);
      if (!items.length) return;
      const [first, last] = [items[0], items[items.length - 1]];
      if (!overlay.contains(document.activeElement)) {
        event.preventDefault();
        first.focus();
      } else if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
  });
}

// ---------- Tamaño de la interfaz ----------
const SCALES = [0.9, 1, 1.1, 1.25, 1.5];

function renderLanguage() {
  for (const button of $$("#language button")) {
    const active = button.dataset.language === (state.settings.language === "en" ? "en" : "es");
    button.classList.toggle("active", active);
    button.setAttribute("aria-checked", String(active));
  }
}

/** Cambiar de idioma: se guarda en Ajustes y la ventana se vuelve a abrir en el nuevo. */
async function setLanguage(code) {
  if (code === state.settings.language) return;
  state.settings.language = code;
  renderLanguage();
  await saveSettings({ language: code });
  if (adoptLanguage(code)) location.reload();
}

function renderScale() {
  const current = state.settings.uiScale ?? 1;
  for (const button of $$("#ui-scale button")) {
    const active = Math.abs(Number(button.dataset.scale) - current) < 0.001;
    button.classList.toggle("active", active);
    button.setAttribute("aria-checked", String(active));
  }
}

function setScale(scale) {
  state.settings.uiScale = scale;
  renderScale();
  saveSettings({ uiScale: scale });
}

/** Ctrl + / Ctrl − / Ctrl 0 */
function stepScale(direction) {
  const current = state.settings.uiScale ?? 1;
  if (direction === 0) return setScale(1);
  const index = SCALES.findIndex((s) => Math.abs(s - current) < 0.001);
  const next = SCALES[Math.min(SCALES.length - 1, Math.max(0, (index < 0 ? 1 : index) + direction))];
  if (next !== current) setScale(next);
}

// ---------- Crear instancia ----------
function openCreate() {
  if (state.creating || state.importing) return openModal("#create-dialog");
  $("#create-step-import").classList.add("hidden");
  $("#create-step-external").classList.add("hidden");
  $("#create-step-type").classList.remove("hidden");
  $("#create-step-custom").classList.add("hidden");
  $("#create-back").classList.add("hidden");
  $("#create-title").textContent = t("Crear instancia", "Create instance");
  openModal("#create-dialog");
}

async function showCustomStep() {
  $("#create-step-type").classList.add("hidden");
  $("#create-step-custom").classList.remove("hidden");
  $("#create-back").classList.remove("hidden");
  $("#create-title").textContent = t("Configuración personalizada", "Custom setup");
  state.selectedVersion = null;
  state.nameEdited = false;
  state.loader = "vanilla";
  state.loaderVersions = {};
  renderCreateFavorites();
  for (const button of $$("#loader-choice [data-loader]")) button.classList.toggle("active", button.dataset.loader === "vanilla");
  $("#loader-version-box").classList.add("hidden");
  $("#create-step-custom").classList.remove("with-loader");
  $("#new-name").value = "";
  $("#new-search").value = "";
  $("#new-status").textContent = "";
  $("#new-create").disabled = true;
  updateCreateArt();
  $("#new-versions").innerHTML = `<div class="version-empty">${t("Cargando versiones…", "Loading versions…")}</div>`;
  state.versions = await invoke("listar_versiones");
  if (!state.versions.mojang) $("#new-status").textContent = `${state.versions.message} ${t("Puedes usar las versiones ya descargadas en este PC.", "You can use the versions already downloaded on this PC.")}`;
  renderVersions();
}

function updateCreateArt() {
  $("#create-art").outerHTML = art({ name: $("#new-name").value.trim() || "Nueva" }, "large").replace('<div class="art', '<div id="create-art" class="art');
}

const loaderNames = { vanilla: "Vanilla", fabric: "Fabric", quilt: "Quilt", forge: "Forge", neoforge: "NeoForge" };

async function selectLoader(loader) {
  state.loader = loader;
  renderCreateFavorites();
  for (const button of $$("#loader-choice [data-loader]")) button.classList.toggle("active", button.dataset.loader === loader);
  $("#loader-version-box").classList.toggle("hidden", loader === "vanilla");
  $("#create-step-custom").classList.toggle("with-loader", loader !== "vanilla");
  $("#loader-version-label").textContent = t(`Versión de ${loaderNames[loader]}`, `${loaderNames[loader]} version`);
  if (loader !== "vanilla" && !state.loaderGames[loader]?.online) {
    $("#new-versions").innerHTML = `<div class="version-empty">${t(`Buscando las versiones compatibles con ${loaderNames[loader]}…`, `Looking for the versions compatible with ${loaderNames[loader]}…`)}</div>`;
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
  if (state.loader !== "vanilla" && games && !games.online) return `${games.message} ${t(`No hay ${loaderNames[state.loader]} descargado en este PC.`, `There is no ${loaderNames[state.loader]} downloaded on this PC.`)}`;
  if (state.versions?.mojang === false && !state.versions.installed.length) return t("No hay versiones descargadas en este PC.", "There are no versions downloaded on this PC.");
  return t("Ninguna versión coincide.", "No version matches.");
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
            <div><div class="v-name">Minecraft ${escapeHtml(o.id)}</div><div class="v-sub">${o.type === "release" ? t("Versión estable", "Stable version") : "Snapshot"}${date ? " · " + date : ""}</div></div>
            ${o.installed ? `<span class="badge">${t("Descargada", "Downloaded")}</span>` : ""}
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
    $("#new-status").textContent = option.installed ? t("Ya está descargada: la instancia se crea al momento.", "It is already downloaded: the instance is created right away.") : t("Se descargará de los servidores de Mojang, con su Java.", "It will be downloaded from Mojang's servers, with its Java.");
    return;
  }
  const name = loaderNames[state.loader];
  const chosen = $("#loader-version").value;
  if (!chosen) return;
  const ready = state.loaderVersions[`${state.loader}:${option.id}`]?.installed.includes(chosen);
  $("#new-status").textContent =
    option.installed && ready
      ? t("Ya está descargada: la instancia se crea al momento.", "It is already downloaded: the instance is created right away.")
      : option.installed
        ? t(`Se descargará ${name} ${chosen} (tarda unos segundos).`, `${name} ${chosen} will be downloaded (it takes a few seconds).`)
        : t(`Se descargará Minecraft ${option.id} con su Java y ${name} ${chosen}.`, `Minecraft ${option.id} will be downloaded with its Java and ${name} ${chosen}.`);
}

async function loadLoaderVersions(minecraft) {
  const loader = state.loader;
  const key = `${loader}:${minecraft}`;
  const select = $("#loader-version");
  if (!state.loaderVersions[key]) {
    select.innerHTML = `<option value="">${t("Cargando versiones…", "Loading versions…")}</option>`;
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
  if (!info.versions.length) $("#new-status").textContent = info.message ?? t(`${loaderNames[loader]} todavía no está disponible para Minecraft ${minecraft}.`, `${loaderNames[loader]} is not available for Minecraft ${minecraft} yet.`);
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
    : `<option value="">${t("No disponible", "Not available")}</option>`;
}

/** Los favoritos que tienen sentido en una instancia nueva (vanilla: solo resource packs). */
function createFavorites(loader) {
  const vanilla = !loader || loader === "vanilla";
  return (state.settings.favoriteMods ?? []).filter((m) => !vanilla || m.kind === "resourcepack");
}

function renderCreateFavorites() {
  const list = createFavorites(state.loader);
  $("#new-favorites-box").classList.toggle("hidden", !list.length);
  $("#new-favorites-text").textContent = t(`Con mis favoritos (${list.length})`, `With my favorites (${list.length})`);
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
    // Con tus favoritos (los que sean compatibles con esta versión y loader).
    const favorites = createFavorites(loader);
    let summary = null;
    if (favorites.length && $("#new-favorites").checked) {
      summary = await installFavorites(instance.id, favorites, (step) => ($("#new-status").textContent = step));
    }
    state.creating = false;
    closeModal("#create-dialog");
    await reloadInstances();
    toast(summary ? `${t(`${instance.name} está lista para jugar.`, `${instance.name} is ready to play.`)} ${favoritesSummary(instance.name, summary)}` : t(`${instance.name} está lista para jugar.`, `${instance.name} is ready to play.`));
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
  if (section === "behavior") loadShortcuts();
  for (const button of $$(".settings-nav button")) button.classList.toggle("active", button.dataset.section === section);
  for (const panel of $$(".settings-content section")) panel.classList.toggle("hidden", panel.dataset.section !== section);
}

function renderSettings() {
  const s = state.settings;
  $("#username").value = s.username;
  $("#uuid-fixed").checked = !!s.uuid;
  $("#uuid-input").value = s.uuid ?? "";
  $("#uuid-input").classList.toggle("hidden", !s.uuid);
  renderAccounts();
  renderGoogleBox();
  $("#minimize-on-launch").checked = s.minimizeOnLaunch;
  $("#close-to-tray").checked = s.closeToTray ?? true;
  $("#notifications").checked = s.notifications ?? true;
  $("#message-sound").checked = s.messageSound ?? true;
  $("#game-panel").checked = s.gamePanel ?? true;
  $("#ram").max = Math.max(4, Math.min(32, state.systemRam - 2));
  $("#ram").value = s.ramGb;
  $("#ram-max").textContent = `${$("#ram").max} GB`;
  $("#win-width").value = s.windowWidth;
  $("#win-height").value = s.windowHeight;
  renderScale();
  renderLanguage();
  updateSettingsTexts();
  applyTheme(s.theme);
}

async function updateSettingsTexts() {
  const name = $("#username").value.trim();
  const valid = /^[A-Za-z0-9_]{1,16}$/.test(name);
  $("#username-error").textContent = t("El nombre debe tener de 1 a 16 letras, números o guiones bajos (sin espacios ni acentos).", "The name must have 1 to 16 letters, numbers or underscores (no spaces or accents).");
  $("#username-error").classList.toggle("hidden", valid);
  const fixed = $("#uuid-fixed").checked;
  const typed = $("#uuid-input").value.trim();
  $("#uuid-error").classList.toggle("hidden", !fixed || !typed || validUuid(typed));
  if (fixed && validUuid(typed)) {
    const hex = typed.replaceAll("-", "").toLowerCase();
    $("#uuid-text").textContent = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  } else {
    $("#uuid-text").textContent = valid ? await invoke("uuid_offline", { nombre: name }) : "—";
  }
  drawHead($("#settings-head"), name);
  const ram = Number($("#ram").value);
  $("#ram-value").textContent = `${ram} GB`;
  let hint = t(`Tu PC tiene ${state.systemRam} GB. Vanilla funciona bien con 2–4 GB; los modpacks grandes piden de 6 a 10 GB.`, `Your PC has ${state.systemRam} GB. Vanilla works well with 2–4 GB; big modpacks ask for 6 to 10 GB.`);
  if (ram > state.systemRam - 3) hint += t(" Deja al menos 3 GB libres para Windows.", " Leave at least 3 GB free for Windows.");
  $("#ram-hint").textContent = hint;
}

function queueSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveSettings, 400);
}

async function saveSettings(extra = {}) {
  const name = $("#username").value.trim();
  if (!/^[A-Za-z0-9_]{1,16}$/.test(name)) return;
  if (name !== state.settings.username && otherAccountNamed(name)) {
    $("#username-error").textContent = t("Ya tienes otra cuenta con ese nombre: cámbiate a ella desde «Jugando como».", "You already have another account with that name: switch to it from «Playing as».");
    $("#username-error").classList.remove("hidden");
    return;
  }
  const renamed = name !== state.settings.username;
  // Con "UUID fijo" a medio escribir se conserva el que había.
  const typed = $("#uuid-input").value.trim();
  const uuid = !$("#uuid-fixed").checked ? "" : validUuid(typed) ? typed : state.settings.uuid;
  try {
    state.settings = await invoke("guardar_ajustes", {
      ajustes: {
        ...state.settings,
        username: name,
        uuid,
        ramGb: Number($("#ram").value),
        windowWidth: Number($("#win-width").value) || state.settings.windowWidth,
        windowHeight: Number($("#win-height").value) || state.settings.windowHeight,
        minimizeOnLaunch: $("#minimize-on-launch").checked,
        closeToTray: $("#close-to-tray").checked,
        notifications: $("#notifications").checked,
        messageSound: $("#message-sound").checked,
        gamePanel: $("#game-panel").checked,
        ...extra,
      },
    });
    renderSidebar();
    renderSteps();
    if (renamed) {
      // Se guarda solo al escribir: se avisa para que se note.
      $("#username-saved").classList.remove("hidden");
      clearTimeout(state.savedHintTimer);
      state.savedHintTimer = setTimeout(() => $("#username-saved").classList.add("hidden"), 2200);
    }
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
    : `<p class="muted">${t("Todavía no hay ningún Java: se descargará al crear tu primera instancia.", "There is no Java yet: it will be downloaded when you create your first instance.")}</p>`;
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
  $("#win-close").onclick = async () => {
    // La primera vez que la X lo deja junto al reloj, se explica cómo cerrarlo del todo.
    let explained = true;
    try {
      explained = localStorage.getItem("bandeja-explicada") === "1";
    } catch {}
    // Con un servidor encendido, cerrar la ventana no lo apaga: se avisa cada vez.
    const hosting = host.view?.servers.filter((s) => s.state !== "off") ?? [];
    if (hosting.length) {
      const ok = await confirmAction({
        title: t("Tu servidor sigue encendido", "Your server is still on"),
        text: t(
          `${hosting.map((s) => s.name).join(", ")} sigue encendido y Parabellum se queda junto al reloj. Para apagar todo: clic derecho en su icono › Salir (guarda el mundo antes de cerrar).`,
          `${hosting.map((s) => s.name).join(", ")} is still on and Parabellum stays next to the clock. To turn everything off: right-click its icon › Quit (it saves the world before closing).`,
        ),
        ok: t("Dejarlo encendido", "Keep it on"),
        primary: true,
      });
      if (ok) appWindow?.close();
      return;
    }
    if (state.social?.signedIn && state.settings.closeToTray && !explained) {
      await confirmAction({
        title: t("Parabellum sigue abierto", "Parabellum is still open"),
        text: t("Se queda junto al reloj para que tus amigos te vean y puedan entrar a tu mundo. Para cerrarlo del todo: clic derecho en su icono › Salir. Puedes cambiarlo en Ajustes › Comportamiento.", "It stays next to the clock so your friends can see you and join your world. To close it completely: right-click its icon › Quit. You can change this in Settings › Behavior."),
        ok: t("Entendido", "Got it"),
        primary: true,
      });
      try {
        localStorage.setItem("bandeja-explicada", "1");
      } catch {}
    }
    appWindow?.close();
  };
  $("#sidebar-toggle").onclick = () => toggleSidebar();
  $("#nav-back").onclick = goBack;
  $("#nav-forward").onclick = goForward;

  for (const button of $$(".rail-btn[data-page]")) button.onclick = () => navigate(button.dataset.page);
  for (const button of [$("#rail-new"), $("#home-new"), $("#empty-new")]) button.onclick = openCreate;
  $("#rail-settings").onclick = () => openSettings("appearance");
  $("#rail-avatar").onclick = () => openAccountMenu($("#rail-avatar"));
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
  $("#content-favorites").onclick = () => openFavoritesDialog();
  $("#content-list").onclick = (event) => {
    const favorite = event.target.closest("[data-fav]");
    if (favorite) {
      const item = state.content?.[state.kind]?.find((i) => i.file === favorite.dataset.fav);
      const iconUrl = item?.icon && /^(https:|http:\/\/icono\.localhost\/)/.test(item.icon) ? item.icon : null;
      if (item?.projectId) toggleFavoriteMod({ projectId: item.projectId, title: item.name, iconUrl, kind: favoriteKinds[state.kind] });
      return;
    }
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
    toast(t("Registro copiado: pégalo donde quieras pedir ayuda.", "Log copied: paste it wherever you want to ask for help."));
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
  $("#edit-dialog").onmousedown = (event) => {
    if (event.target.id === "edit-dialog") closeModal("#edit-dialog");
  };
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
  $("#edit-name").onkeydown = (event) => {
    if (event.key === "Enter") saveEdit();
  };

  // Confirmación y error del juego
  $("#confirm-dialog").onmousedown = (event) => {
    if (event.target.id === "confirm-dialog") state.confirmCancel?.();
  };
  $("#doctor-close").onclick = () => {
    if (!state.doctor?.busy) closeModal("#doctor-dialog");
  };
  $("#doctor-findings").onclick = (event) => {
    const button = event.target.closest("[data-doctor-fix], [data-doctor-warning]");
    if (!button || state.doctor?.busy) return;
    const warning = !!button.dataset.doctorWarning;
    const [i, j] = (button.dataset.doctorFix ?? button.dataset.doctorWarning).split(":").map(Number);
    const option = (warning ? state.doctor.result.warnings : state.doctor.diagnosis).findings[i].fixes[j];
    applyDoctorFix(option.fix, option.label).then(async (ok) => {
      if (!ok) return;
      await reloadInstances();
      renderDoctor();
    });
  };
  $("#doctor-fix-all").onclick = () => doctorFixAll();
  $("#doctor-auto").onclick = () => doctorAuto();
  $("#doctor-anyway").onclick = () => {
    const { id, server } = state.doctor;
    closeModal("#doctor-dialog");
    play(id, server, { skipCheck: true });
  };
  $("#doctor-logs").onclick = () => {
    const id = state.doctor.id;
    closeModal("#doctor-dialog");
    navigate("instance", id);
    selectTab("logs");
  };
  $("#detail-join").onclick = () => {
    const address = $("#detail-server").value.trim();
    if (!address) return toast(t("Escribe la dirección del servidor.", "Type the server address."), "error");
    play(state.current.id, address);
  };
  $("#detail-server").onkeydown = (event) => {
    if (event.key === "Enter") $("#detail-join").click();
  };

  // Servidores: «Agregar servidor» abre un diálogo con nombre, dirección e instancia.
  $("#server-new").onclick = () => {
    $("#server-name").value = "";
    $("#server-address").value = "";
    renderServers();
    openModal("#server-dialog");
  };
  for (const id of ["#server-dialog-close", "#server-cancel"]) $(id).onclick = () => closeModal("#server-dialog");
  $("#server-add").onclick = async () => {
    const address = $("#server-address").value.trim();
    const instance = $("#server-instance").value;
    if (!address) {
      $("#server-address").focus();
      return toast(t("Escribe la dirección del servidor.", "Type the server address."), "error");
    }
    if (!instance) return toast(t("Primero crea una instancia para entrar al servidor.", "Create an instance first to join the server."), "error");
    await saveServers([...(state.settings.servers ?? []), { name: $("#server-name").value.trim(), address, instance }]);
    closeModal("#server-dialog");
  };
  for (const id of ["#server-name", "#server-address"]) {
    $(id).onkeydown = (event) => {
      if (event.key === "Enter") $("#server-add").click();
    };
  }
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
  $("#create-dialog").onmousedown = (event) => {
    if (event.target.id === "create-dialog") closeModal("#create-dialog");
  };
  $("[data-type='custom']").onclick = showCustomStep;
  $("[data-type='import']").onclick = () => importModpack();
  $("[data-type='external']").onclick = showExternalStep;
  $("#external-list").onclick = (event) => {
    const button = event.target.closest("[data-external]");
    if (button) importExternal(state.externals[Number(button.dataset.external)]);
  };
  $("#external-pick").onclick = async () => {
    try {
      const picked = await invoke("elegir_instancia_externa");
      if (picked) importExternal(picked);
    } catch (error) {
      toast(String(error), "error");
    }
  };
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
  $("#settings-dialog").onmousedown = (event) => {
    if (event.target.id === "settings-dialog") closeModal("#settings-dialog");
  };
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
  $("#close-to-tray").onchange = () => saveSettings();
  $("#notifications").onchange = () => saveSettings();
  $("#game-panel").onchange = () => saveSettings();
  $("#message-sound").onchange = () => {
    saveSettings();
    // Al activarlo se oye cómo suena.
    if ($("#message-sound").checked) invoke("sonido_mensaje").catch(() => {});
  };
  $("#shortcut-desktop").onchange = () => toggleShortcut("escritorio", $("#shortcut-desktop"));
  $("#shortcut-start").onchange = () => toggleShortcut("inicio", $("#shortcut-start"));
  for (const button of $$("#theme button")) {
    button.onclick = () => {
      applyTheme(button.dataset.theme);
      saveSettings({ theme: button.dataset.theme });
    };
  }
  matchMedia("(prefers-color-scheme: light)").onchange = () => state.settings?.theme === "sistema" && applyTheme("sistema");
  $("#uuid-fixed").onchange = () => {
    const fixed = $("#uuid-fixed").checked;
    $("#uuid-input").classList.toggle("hidden", !fixed);
    if (fixed) $("#uuid-input").focus();
    updateSettingsTexts();
    queueSave();
  };
  $("#uuid-input").oninput = () => {
    updateSettingsTexts();
    queueSave();
  };
  $("#account-add").onclick = () => openAccountDialog("add");
  $("#account-list").onclick = (event) => {
    const use = event.target.closest("[data-use-account]");
    if (use) return switchAccount(Number(use.dataset.useAccount));
    const remove = event.target.closest("[data-remove-account]");
    if (remove) removeAccount(Number(remove.dataset.removeAccount));
  };
  $("#account-ok").onclick = addAccount;
  $("#account-cancel").onclick = () => closeModal("#account-dialog");
  for (const input of [$("#account-name"), $("#account-uuid")]) {
    input.onkeydown = (event) => {
      if (event.key === "Enter") addAccount();
    };
  }
  $("#uuid-copy").onclick = async () => {
    await navigator.clipboard.writeText($("#uuid-text").textContent);
    toast(t("UUID copiado.", "UUID copied."));
  };
  $("#open-root").onclick = () => invoke("abrir_carpeta", { id: null });

  // Colapsables y teclado
  for (const collapsible of $$(".collapsible")) $(".collapsible-head", collapsible).onclick = () => collapsible.classList.toggle("open");
  for (const button of $$("#ui-scale button")) button.onclick = () => setScale(Number(button.dataset.scale));
  for (const button of $$("#language button")) button.onclick = () => setLanguage(button.dataset.language);
  document.addEventListener(
    "keydown",
    (event) => {
      if ($("#update-dialog").classList.contains("hidden")) return;
      if (event.key === "Tab") {
        const buttons = [$("#update-quit"), $("#update-now")].filter((b) => !b.disabled);
        const at = buttons.indexOf(document.activeElement);
        buttons[(at + (event.shiftKey ? -1 : 1) + buttons.length) % buttons.length]?.focus();
      }
      if (event.key !== "Enter" && event.key !== " ") {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
    },
    true,
  );
  document.addEventListener("keydown", (event) => {
    if (event.ctrlKey && !event.altKey && !event.shiftKey && (event.key === "b" || event.key === "B") && !isTyping()) {
      event.preventDefault();
      toggleSidebar();
      return;
    }
    if (event.ctrlKey && !event.altKey && ["+", "=", "-", "0"].includes(event.key)) {
      event.preventDefault();
      stepScale(event.key === "0" ? 0 : event.key === "-" ? -1 : 1);
      return;
    }
    if (event.key === "Escape") {
      closeModal("#create-dialog");
      closeModal("#settings-dialog");
      closeModal("#edit-dialog");
      if (!state.doctor?.busy) closeModal("#doctor-dialog");
      closeModal("#install-dialog");
      closeModal("#group-dialog");
      closeModal("#server-dialog");
      closeModal("#icon-dialog");
      closeModal("#srv-dialog");
      if (!state.favDialog?.busy) closeModal("#fav-dialog");
      closeModal("#account-dialog");
      closeModal("#add-friend-dialog");
      closeModal("#report-dialog");
      if (!$("#image-dialog").classList.contains("hidden")) cancelChatImage();
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

  // Para que la portada viaje de la tarjeta (o del icono de la barra) a la página de la instancia o del servidor.
  document.addEventListener(
    "click",
    (event) => {
      const opener = event.target.closest?.("[data-open], [data-hosted]");
      const cover = opener && (opener.matches(".art, .server-icon, .srv-row-icon") ? opener : $(".art, .server-icon, .srv-row-icon", opener));
      state.morphFrom = cover ? { id: opener.dataset.open ?? opener.dataset.hosted, rect: cover.getBoundingClientRect(), at: performance.now() } : null;
    },
    true,
  );
  wireExploreEvents();
  wireModpackEvents();
  wireDriveEvents();
  wireFriendsEvents();
  wireChatEvents();
  wireReportEvents();
  listen("juego-cerrado", ({ payload }) => onGameClosed(payload));
  // Arrastrar archivos a la ventana: se agregan a la instancia abierta.
  // Fuera de una instancia (o con un .mrpack) lo que se suelta se instala como modpack.
  const dropsModpack = (paths) => !(state.page === "instance" && state.current) || (paths ?? []).some((p) => /\.mrpack$/i.test(p));
  tauri?.webview?.getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (state.page === "chat") {
      if (payload.type === "drop" && payload.paths?.length && state.chatOpen) {
        invoke("leer_imagen", { ruta: payload.paths[0] }).then(sendChatImage, (error) => toast(String(error), "error"));
      }
      return;
    }
    if (payload.type === "enter") {
      const modpack = dropsModpack(payload.paths);
      $("#drop-title").textContent = modpack ? t("Suelta para instalar el modpack", "Drop to install the modpack") : t(`Suelta para agregar a ${state.current.name}`, `Drop to add to ${state.current.name}`);
      $("#drop-text").textContent = modpack
        ? t("Un .mrpack o un paquete .zip de Parabellum (de Drive o una USB).", "A .mrpack or a Parabellum .zip package (from Drive or a USB drive).")
        : t("Mods, resource packs, shaders y mundos: cada uno va a su lugar.", "Mods, resource packs, shaders and worlds: each one goes to its place.");
      $("#drop-overlay").classList.remove("hidden");
    } else if (payload.type === "drop" || payload.type === "leave") {
      $("#drop-overlay").classList.add("hidden");
      if (payload.type !== "drop" || !payload.paths?.length) return;
      if (dropsModpack(payload.paths)) importModpack(payload.paths[0]);
      else addContentFiles(payload.paths);
    }
  });

  listen("progreso", ({ payload }) => {
    // El porcentaje de arriba también dice cuánto falta, cuando ya se puede calcular.
    const left = timeLeft(payload.task, payload.fraction);
    const percentText = (fraction) => `${Math.round(fraction * 100)}%${left ? ` · ${left}` : ""}`;
    if (payload.task === "catalogo") {
      const line = state.drive.installing && $(`[data-progress-for="${CSS.escape(state.drive.installing)}"]`);
      if (line) line.textContent = payload.stage;
      $("#download-percent").textContent = percentText(payload.fraction);
      return;
    }
    if (payload.task === "actualizacion") {
      const percent = Math.round(payload.fraction * 100);
      $("#update-text").textContent = t(`Descargando… ${percent}%`, `Downloading… ${percent}%`) + (left ? ` · ${left}` : "");
      $("#update-progress div").style.width = `${Math.max(2, percent)}%`;
      $("#update-status").textContent = payload.stage || t(`Descargando… ${percent}%`, `Downloading… ${percent}%`);
      return;
    }
    if (payload.task === "drive") {
      showDriveProgress(payload.stage, payload.fraction);
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
      $("#download-percent").textContent = percentText(payload.fraction);
      return;
    }
    if (payload.task === "compartida" || (state.shareInstalling && (payload.task === "importar" || payload.task === "catalogo"))) {
      const percent = Math.round(payload.fraction * 100);
      state.shareProgress = `${percent}%`;
      $("#download-percent").textContent = `${percent}%`;
      const button = $(`[data-share-install="${CSS.escape(state.shareInstalling ?? "")}"]`);
      if (button) button.innerHTML = `<span class="spinner tiny"></span>${percent}%`;
      if (payload.task !== "compartida") return void (payload.task === "importar" && ($("#import-status").textContent = payload.stage));
      return;
    }
    if (payload.task === "instalar") {
      $("#download-percent").textContent = percentText(payload.fraction);
      return;
    }
    if (payload.task === "loader") {
      $("#edit-loader-hint").textContent = payload.stage;
      return;
    }
    if (payload.task === "jugar") {
      // Antes de abrir el juego se baja lo que le falte al modpack.
      if (payload.fraction >= 1) return;
      state.completingShown = true;
      $("#download-indicator").classList.remove("hidden");
      $("#download-percent").textContent = percentText(payload.fraction);
      $("#status-text").textContent = payload.stage;
      // «Abriendo…» se va llenando con lo que se baja.
      if (state.current && state.launching.has(state.current.id)) $("#detail-play").style.setProperty("--p", payload.fraction);
      return;
    }
    if (payload.task === "doctor") {
      doctorStep(payload.stage);
      return;
    }
    if (payload.task === "servidor") {
      $("#hc-status").textContent = payload.stage;
      return;
    }
    if (payload.task === "playit") {
      if (host.view) host.view.playit.message = payload.stage;
      if (state.page === "hosted") renderHosted();
      if (state.page === "servers") renderHost();
      return;
    }
    if (payload.task === "verificar") {
      $("#download-percent").textContent = percentText(payload.fraction);
      $("#download-indicator").title = payload.stage;
      return;
    }
    $("#new-status").textContent = payload.stage;
    const percent = Math.max(2, payload.fraction * 100);
    $("#new-progress div").style.width = `${percent}%`;
    $("#download-percent").textContent = percentText(percent / 100);
  });
}

/** Cuánto falta para terminar, con lo que se ha avanzado en los últimos segundos (null mientras no se
 * sepa bien o si falta muy poco). Si la tarea vuelve a empezar, se vuelve a medir. */
const progressClock = {};
function timeLeft(task, fraction) {
  const now = performance.now();
  let clock = progressClock[task];
  if (!clock || fraction < clock.last - 0.001 || now - clock.seen > 15000) clock = progressClock[task] = { start: now, from: fraction, last: fraction, seen: now };
  clock.last = fraction;
  clock.seen = now;
  const done = fraction - clock.from;
  const elapsed = (now - clock.start) / 1000;
  if (elapsed < 3 || done < 0.02 || fraction >= 1) return null;
  const left = (elapsed * (1 - fraction)) / done;
  if (left < 5) return null;
  return left < 60 ? `${Math.ceil(left / 5) * 5} s` : `${Math.round(left / 60)} min`;
}

async function init() {
  translateStatic();
  wireEvents();
  setupAccessibility();
  const initial = await invoke("estado_inicial");
  // El idioma lo deciden los Ajustes; si la ventana se pintó en el otro, se recarga una vez.
  if (adoptLanguage(initial.settings.language)) return location.reload();
  state.settings = initial.settings;
  state.instances = initial.instances;
  state.systemRam = initial.systemRamGb;
  state.version = initial.version;
  state.driveUrl = initial.driveUrl;
  setCreator(initial.creator);
  // Lo que depende del sistema (Windows o macOS): por ejemplo, en Mac no hay accesos directos.
  document.body.dataset.platform = initial.platform ?? "Windows";
  document.body.classList.toggle("no-shortcuts", initial.shortcuts === false);
  // Al abrir se busca una versión nueva (y cada media hora, por si se queda junto al reloj): si la
  // hay, hay que actualizar para seguir.
  setTimeout(() => checkLauncherUpdate(false, true), 1200);
  setInterval(() => checkLauncherUpdate(true, true), 30 * 60e3);
  $("#root-text").textContent = initial.root;
  $("#version-text").textContent = `Parabellum ${initial.version}`;
  applyTheme(state.settings.theme);
  renderSidebar();
  renderSteps();
  renderHome();
  renderServers();
  state.social = await invoke("estado_social").catch(() => null);
  renderFriends();
  renderFriendServers();
  state.chats = await invoke("chats").catch(() => []);
  updateChatBadge();
  navigate("home", null, false);
  // Al volver a la ventana (o desde la bandeja) se piden los datos al momento, sin esperar avisos.
  let lastSync = Date.now();
  const resync = async () => {
    if (Date.now() - lastSync < 4000) return;
    lastSync = Date.now();
    try {
      state.social = await invoke("estado_social");
      renderFriends();
      renderFriendServers();
      state.chats = await invoke("chats");
      renderGroups();
      if (state.page === "chat") renderChatList();
      else updateChatBadge();
      if (state.chatOpen) invoke("abrir_chat", { chat: state.chatOpen }).catch(() => {});
    } catch {}
  };
  window.addEventListener("focus", resync);
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) resync();
  });
  // Los «hace X min» de los amigos se ponen al día solos.
  setInterval(() => {
    if (state.social?.signedIn) renderFriends();
  }, 60000);
  pollRunning();
  setInterval(pollRunning, 3000);
}

// ======================================================================
// Explorar: mods, resource packs y shaders de Modrinth
// ======================================================================
const exploreKinds = {
  mod: { placeholder: t("Buscar mods…", "Search mods…"), contentKind: "mods", noun: "mods" },
  resourcepack: { placeholder: t("Buscar resource packs…", "Search resource packs…"), contentKind: "resourcepacks", noun: "resource packs" },
  shader: { placeholder: t("Buscar shaders…", "Search shaders…"), contentKind: "shaderpacks", noun: "shaders" },
  datapack: { placeholder: t("Buscar data packs…", "Search data packs…"), contentKind: "worlds", noun: "data packs" },
  modpack: { placeholder: t("Buscar modpacks…", "Search modpacks…"), contentKind: null, noun: "modpacks" },
  drive: { placeholder: t("Buscar en tus paquetes…", "Search your packages…"), contentKind: null, noun: t("paquetes", "packages") },
};
const contentToExplore = { mods: "mod", resourcepacks: "resourcepack", shaderpacks: "shader", worlds: "datapack" };
/** Qué es el proyecto abierto: lo dice la pestaña desde la que se abrió (los data packs en Modrinth son "mod"). */
const projectKind = () => state.projectKind ?? state.projectData?.projectType;

const categoryNamesEs = {
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
// En inglés, las categorías de Modrinth ya vienen con su nombre: solo se ajustan algunas.
const categoryNamesEn = {
  "game-mechanics": "Game mechanics", worldgen: "World generation", "core-shaders": "Core shaders", "vanilla-like": "Vanilla-like",
  "colored-lighting": "Colored lighting", "path-tracing": "Path tracing", "semi-realistic": "Semi-realistic", pbr: "PBR", gui: "GUI",
  datapack: "Data pack", "8x-": "8x or less", "512x+": "512x or more", optifine: "OptiFine", neoforge: "NeoForge",
};
const categoryNames = LANG === "en" ? categoryNamesEn : categoryNamesEs;
const categoryHeaders = { categories: t("Categorías", "Categories"), features: t("Características", "Features"), resolutions: t("Resolución", "Resolution"), "performance impact": t("Rendimiento", "Performance impact") };
const loaderCategories = new Set(["fabric", "quilt", "forge", "neoforge", "iris", "optifine", "canvas", "vanilla", "minecraft", "datapack", "liteloader", "modloader", "rift", "bukkit", "paper", "spigot", "velocity"]);
const categoryName = (name) => categoryNames[name] ?? name.charAt(0).toUpperCase() + name.slice(1);
const compact = new Intl.NumberFormat(locale(), { notation: "compact", maximumFractionDigits: 1 });

function relativeDate(iso) {
  const ms = Date.parse(iso);
  if (!ms) return "";
  const days = (Date.now() - ms) / 86400e3;
  if (days < 1) return t("hoy", "today");
  if (days < 2) return t("ayer", "yesterday");
  const months = Math.floor(days / 30);
  const years = Math.floor(days / 365);
  if (days < 31) return t(`hace ${Math.floor(days)} días`, `${Math.floor(days)} days ago`);
  if (days < 365) return t(`hace ${months} ${months === 1 ? "mes" : "meses"}`, `${months} ${months === 1 ? "month" : "months"} ago`);
  return t(`hace ${years} ${years === 1 ? "año" : "años"}`, `${years} ${years === 1 ? "year" : "years"} ago`);
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
    `<option value="">${t("Ninguna (solo ver)", "None (just browse)")}</option>` +
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
  if (!instance) note.textContent = t("Elige una instancia para ver solo lo compatible e instalar con un clic.", "Choose an instance to see only what is compatible and install with one click.");
  else if (e.kind === "mod" && instance.loader === "Vanilla") note.textContent = t("Es vanilla: no carga mods. Elige una con un loader.", "It is vanilla: it does not load mods. Choose one with a loader.");
  else if (e.kind === "datapack") note.textContent = t(`Se muestra solo lo compatible con Minecraft ${instance.minecraftVersion}. Al instalar eliges el mundo.`, `Only what is compatible with Minecraft ${instance.minecraftVersion} is shown. You choose the world when installing.`);
  else note.textContent = t("Se muestra solo lo compatible con", "Only what is compatible is shown:") + ` ${e.kind === "mod" ? instance.loader + " " : "Minecraft "}${instance.minecraftVersion}.`;
  const showNote = instance && e.kind === "mod" && instance.loader === "Vanilla";
  $("#explore-note").classList.toggle("hidden", !showNote);
  $("#explore-note").textContent = showNote ? t(`${instance.name} es vanilla y no puede cargar mods. Crea una instancia con un loader, o explora resource packs y shaders.`, `${instance.name} is vanilla and cannot load mods. Create an instance with a loader, or explore resource packs and shaders.`) : "";
}

async function loadExploreVersions() {
  if (state.explore.versionsLoaded) return;
  state.explore.versionsLoaded = true;
  try {
    const versions = state.versions ?? (await invoke("listar_versiones"));
    state.versions ??= versions;
    const releases = versions.mojang ? versions.releases.filter((r) => r.type === "release").map((r) => r.id) : versions.installed;
    $("#explore-version").innerHTML = `<option value="">${t("Todas", "All")}</option>` + releases.map((v) => `<option>${escapeHtml(v)}</option>`).join("");
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
    if (/No se puede conectar con Modrinth|Cannot connect to Modrinth/.test(String(error)) && !e.autoDrive) {
      e.autoDrive = true;
      toast(t("Modrinth está bloqueado en esta red: aquí tienes los modpacks del catálogo de Drive.", "Modrinth is blocked on this network: here are the modpacks from the Drive catalog."));
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
  if (e.installing.has(projectId)) return `<button class="btn ${extraClass}" disabled><span class="spinner tiny"></span>${t("Instalando…", "Installing…")}</button>`;
  const installed = e.kind === "modpack" ? modpackInstalled(projectId) : e.kind !== "datapack" && e.installed.has(projectId);
  if (installed) return `<button class="btn installed ${extraClass}" disabled><svg><use href="#i-check"/></svg>${t("Instalado", "Installed")}</button>`;
  return `<button class="btn primary ${extraClass}" data-install="${escapeHtml(projectId)}"><svg><use href="#i-download"/></svg>${t("Instalar", "Install")}</button>`;
}

function renderExploreResults(from = 0) {
  const e = state.explore;
  const list = $("#explore-results");
  if (!e.hits.length) {
    list.innerHTML = `<div class="content-empty">${t("No se encontró nada", "Nothing was found")}${e.query ? ` ${t("para", "for")} "${escapeHtml(e.query)}"` : ""}. ${t("Prueba con otras palabras o quita filtros.", "Try other words or remove filters.")}</div>`;
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
          <div class="r-title"><b>${escapeHtml(hit.title)}</b><span class="muted">${t("por", "by")} ${escapeHtml(hit.author)}</span></div>
          <div class="r-desc">${escapeHtml(hit.description)}</div>
          <div class="r-tags">${tags.map((t) => `<span class="tag">${escapeHtml(categoryName(t))}</span>`).join("")}</div>
        </div>
        <div class="r-side">
          <div class="r-stats">
            <span title="${t("Descargas", "Downloads")}">${icon("download")}${compact.format(hit.downloads)}</span>
            <span title="${t("Seguidores", "Followers")}">${icon("heart")}${compact.format(hit.follows)}</span>
          </div>
          <div class="faint r-updated">${icon("clock")}${t("Actualizado", "Updated")} ${escapeHtml(relativeDate(hit.updated))}</div>
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
    toast(t('Elige en qué instancia instalarlo en "Instalar en".', 'Choose which instance to install it to in "Install to".'), "error");
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
      toast(t(`${plan.title} ${plan.versionNumber} ya está instalado en ${instance.name}.`, `${plan.title} ${plan.versionNumber} is already installed in ${instance.name}.`));
    } else if (plan.problem) {
      showInstallProblem(plan, projectId);
    } else {
      await invoke("instalar_contenido", { id: instance.id, archivos: plan.files });
      const dependencies = plan.files.filter((f) => f.dependency).map((f) => f.title);
      toast(t(`Se instaló ${plan.title} ${plan.versionNumber} en ${instance.name}${dependencies.length ? `, con ${dependencies.join(", ")}` : ""}.`, `${plan.title} ${plan.versionNumber} was installed in ${instance.name}${dependencies.length ? `, with ${dependencies.join(", ")}` : ""}.`));
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
    title: t(`No se puede instalar ${plan.title}`, `${plan.title} cannot be installed`),
    text: plan.problem,
    items: [],
    ok: state.page === "project" ? "" : t("Ver versiones", "See versions"),
    secondary: t("Entendido", "Got it"),
    onOk: () => {
      navigate("project", projectId);
      selectProjectTab("versions");
    },
  });
}

function offerOptional(plan) {
  openInstallDialog({
    alert: false,
    title: t(`${plan.title} está instalado`, `${plan.title} is installed`),
    text: t("También funciona con estos mods opcionales. ¿Quieres alguno?", "It also works with these optional mods. Do you want any?"),
    items: plan.optional,
    ok: t("Instalar los marcados", "Install the checked ones"),
    secondary: t("No, gracias", "No, thanks"),
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
  $("#project-title").textContent = t("Cargando…", "Loading…");
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
    $("#project-title").textContent = t("No se pudo abrir", "Could not open");
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
    <span>${icon("clock")}${t("Actualizado", "Updated")} ${escapeHtml(relativeDate(p.updated))}</span>`;
  renderProjectInstall();
  const body = $("#project-body");
  body.replaceChildren(Markdown.toFragment(p.body || p.description));
  const releases = p.gameVersions.filter((v) => /^\d+(\.\d+)+$/.test(v));
  const sides = { required: t("Necesario", "Required"), optional: t("Opcional", "Optional"), unsupported: t("No se usa", "Not used"), unknown: t("Sin datos", "Unknown") };
  $("#project-aside").innerHTML = `
    <div class="card-panel"><h4>${t("Compatibilidad", "Compatibility")}</h4>
      <dl class="mini-details">
        <dt>Minecraft</dt><dd>${releases.length ? `${escapeHtml(releases[0])} – ${escapeHtml(releases.at(-1))}` : escapeHtml(p.gameVersions.at(-1) ?? "—")}</dd>
        <dt>${t("Plataformas", "Platforms")}</dt><dd>${escapeHtml(p.loaders.map(categoryName).join(", ") || "—")}</dd>
        <dt>${t("En el cliente", "On the client")}</dt><dd>${sides[p.clientSide] ?? "—"}</dd>
        <dt>${t("En el servidor", "On the server")}</dt><dd>${sides[p.serverSide] ?? "—"}</dd>
      </dl></div>
    <div class="card-panel"><h4>${t("Detalles", "Details")}</h4>
      <dl class="mini-details">
        <dt>${t("Autor", "Author")}</dt><dd>${escapeHtml(p.author || "—")}</dd>
        <dt>${t("Licencia", "License")}</dt><dd>${escapeHtml(p.license || "—")}</dd>
        <dt>${t("Publicado", "Published")}</dt><dd>${p.published ? new Date(p.published).toLocaleDateString(locale()) : "—"}</dd>
      </dl>
      <div class="r-tags">${p.categories.filter((c) => !loaderCategories.has(c)).map((c) => `<span class="tag">${escapeHtml(categoryName(c))}</span>`).join("")}</div>
    </div>
    <div class="card-panel"><h4>${t("Enlaces", "Links")}</h4>
      <div class="link-list">
        <a href="https://modrinth.com/${escapeHtml(p.projectType)}/${escapeHtml(p.slug)}">${icon("external")}${t("Ver en Modrinth", "See on Modrinth")}</a>
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
    : `<div class="content-empty">${t("Este proyecto no tiene imágenes.", "This project has no images.")}</div>`;
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
    ? t("Instalando…", "Installing…")
    : done
      ? t("Instalado", "Installed")
      : modpack
        ? t("Instalar modpack", "Install modpack")
        : instance
          ? t(`Instalar en ${instance.name}`, `Install to ${instance.name}`)
          : t("Instalar", "Install");
  $("use", button).setAttribute("href", done ? "#i-check" : "#i-download");
  const favorable = ["mod", "resourcepack", "shader"].includes(kind);
  $("#project-fav").classList.toggle("hidden", !favorable);
  $("#project-fav").classList.toggle("starred", isFavoriteMod(p.id));
  $("#project-fav").title = isFavoriteMod(p.id) ? t("Quitar de favoritos", "Remove from favorites") : t("Agregar a favoritos", "Add to favorites");
  $("#project-target-box").classList.toggle("hidden", modpack);
  $("#project-install-note").classList.toggle("hidden", !modpack);
  $("#project-install-note").textContent = t("Se instala como una instancia nueva, con su versión de Minecraft y su loader.", "It is installed as a new instance, with its Minecraft version and its loader.");
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
      ? t("Cada versión se instala como una instancia nueva, con su loader.", "Each version is installed as a new instance, with its loader.")
      : instance
        ? t(`Versiones compatibles con ${instance.name}`, `Versions compatible with ${instance.name}`) + ` (${kind === "mod" ? instance.loader + " " : "Minecraft "}${instance.minecraftVersion}).`
        : t("Todas las versiones. Elige una instancia en \"Instalar en\" para ver solo las compatibles.", "All versions. Choose an instance in \"Install to\" to see only the compatible ones.");
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
              ? `<button class="btn installed small" disabled><svg><use href="#i-check"/></svg>${t("Instalada", "Installed")}</button>`
              : `<button class="btn small" data-install-version="${escapeHtml(v.id)}" ${state.explore.installing.has(state.projectId) ? "disabled" : ""}><svg><use href="#i-download"/></svg>${t("Instalar", "Install")}</button>`;
          return `<div class="content-item">
            <span class="version-type ${typeClass}">${typeName}</span>
            <div class="grow"><div class="c-name">${escapeHtml(v.name || v.versionNumber)}</div>
              <div class="c-sub">${escapeHtml(v.loaders.map(categoryName).join(", "))} · Minecraft ${escapeHtml(games)}</div></div>
            <div class="c-version">${new Date(v.published).toLocaleDateString(locale())} · ${compact.format(v.downloads)}</div>
            ${action}
          </div>`;
        })
        .join("")
    : `<div class="content-empty">${t("No hay versiones", "There are no versions")}${instance ? ` ${t("para", "for")} ${escapeHtml(instance.name)}` : ""}.</div>`;
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
    if (done) toast(t(`Se ${done === 1 ? "actualizó" : "actualizaron"} ${plural(done, "elemento", "elementos")}.`, `${plural(done, "item was", "items were")} updated.`));
    if (heldBack.length) {
      const one = heldBack.length === 1;
      toast(t(`No se ${one ? "actualizó" : "actualizaron"} porque ${one ? "rompería" : "romperían"} algo: `, `Not updated because ${one ? "it" : "they"} would break something: `) + heldBack.map((h) => `${h.title} (${h.reason})`).join(" · "), "error");
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
    { label: t("Buscar en Modrinth", "Search on Modrinth"), icon: "compass", action: () => openExploreFor(id, state.kind) },
    { label: t("Desde un archivo de la PC", "From a file on the PC"), icon: "upload", action: pickContentFiles },
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
  $("#project-fav").onclick = () => {
    const p = state.projectData;
    if (p) toggleFavoriteMod({ projectId: p.id, title: p.title, iconUrl: p.iconUrl ?? null, kind: projectKind() });
  };
  // Diálogo de favoritos (pestaña Contenido).
  $("#fav-close").onclick = () => closeModal("#fav-dialog");
  $("#fav-dialog").onmousedown = (event) => {
    if (event.target.id === "fav-dialog" && !state.favDialog?.busy) closeModal("#fav-dialog");
  };
  $("#fav-install").onclick = () => installFavoritesHere();
  $("#fav-explore").onclick = () => {
    closeModal("#fav-dialog");
    if (state.current) openExploreFor(state.current.id, state.favDialog?.kind ?? state.kind);
  };
  $("#fav-list").onchange = (event) => {
    const pick = event.target.closest("[data-fav-pick]");
    if (!pick || !state.favDialog) return;
    if (pick.checked) state.favDialog.selected.add(pick.dataset.favPick);
    else state.favDialog.selected.delete(pick.dataset.favPick);
    renderFavoritesDialog();
  };
  $("#fav-list").onclick = (event) => {
    const remove = event.target.closest("[data-fav-remove]");
    const favorite = remove && (state.settings.favoriteMods ?? []).find((m) => m.projectId === remove.dataset.favRemove);
    if (favorite) toggleFavoriteMod(favorite);
  };
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
  $("#install-dialog").onmousedown = (event) => {
    if (event.target.id === "install-dialog") closeModal("#install-dialog");
  };

  // Contenido: agregar, actualizar y abrir el proyecto de un mod
  $("#content-add").onclick = (event) => {
    event.stopPropagation();
    contentAddMenu();
  };
  $("#content-update-all").onclick = async () => {
    // Un modpack trae versiones que su autor probó juntas; actualizarlas todas de golpe puede romperlo.
    if (state.current?.modpack) {
      const ok = await confirmAction({
        title: t("¿Actualizar todos los mods del modpack?", "Update all the modpack's mods?"),
        text: t(`${state.current.name} trae versiones que su autor probó juntas. Actualizarlas todas puede hacer que el juego no abra o que cambie cómo funciona. Si quieres el modpack al día, es mejor instalar su versión nueva.`, `${state.current.name} includes versions its author tested together. Updating them all can stop the game from opening or change how it works. If you want the modpack up to date, it is better to install its new version.`),
        ok: t("Actualizar de todos modos", "Update anyway"),
      });
      if (!ok) return;
    }
    applyUpdates(availableUpdates());
  };

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
    toast(t(`${result.name} está lista para jugar. La encuentras en tu biblioteca.`, `${result.name} is ready to play. You will find it in your library.`));
    if (result.warning) toast(result.warning, "error");
    if (result.notes?.length) toast(result.notes.join(" "));
  } catch (error) {
    toast(String(error), "error");
  } finally {
    setBusy(projectId, false);
  }
}

async function installDatapack(projectId, versionId = null) {
  const instance = exploreTarget();
  if (!instance) {
    toast(t('Elige en qué instancia instalarlo en "Instalar en".', 'Choose which instance to install it to in "Install to".'), "error");
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
      title: t("Todavía no hay mundos", "There are no worlds yet"),
      text: t(`Los data packs van dentro de un mundo y ${instance.name} aún no tiene ninguno. Abre el juego, crea un mundo y vuelve.`, `Data packs go inside a world and ${instance.name} has none yet. Open the game, create a world and come back.`),
      items: [],
      ok: "",
      secondary: t("Entendido", "Got it"),
    });
  }
  openInstallDialog({
    alert: false,
    title: t("¿En qué mundo?", "In which world?"),
    text: t("Los data packs se instalan dentro de un mundo. Si el mundo ya existe, se activan la próxima vez que lo abras.", "Data packs are installed inside a world. If the world already exists, they turn on the next time you open it."),
    items: worlds.map((w) => ({ id: w.file, title: w.name, iconUrl: w.icon })),
    ok: t("Instalar", "Install"),
    secondary: t("Cancelar", "Cancel"),
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

/** Importar de otro launcher: lista lo que hay de CurseForge, Prism y MultiMC en este PC. */
async function showExternalStep() {
  $("#create-step-type").classList.add("hidden");
  $("#create-step-external").classList.remove("hidden");
  $("#create-back").classList.remove("hidden");
  $("#create-title").textContent = t("Importar de otro launcher", "Import from another launcher");
  $("#external-note").textContent = t("Buscando instancias de CurseForge, Prism y MultiMC…", "Looking for CurseForge, Prism and MultiMC instances…");
  $("#external-list").innerHTML = `<div class="content-loading"><div class="spinner"></div></div>`;
  const found = await invoke("instancias_externas").catch(() => []);
  $("#external-note").textContent = found.length
    ? t("Se copian con sus mods, mundos y opciones; el original no se toca. El loader lo instala Parabellum.", "They are copied with their mods, worlds and options; the original is not touched. Parabellum installs the loader.")
    : t("No se encontraron instancias en las carpetas de CurseForge, Prism ni MultiMC. Si están en otro lugar, elige su carpeta.", "No instances were found in the CurseForge, Prism or MultiMC folders. If they are somewhere else, choose their folder.");
  $("#external-list").innerHTML = found.map(externalItem).join("");
  state.externals = found;
}

function externalItem(e, index) {
  const loader = e.loader === "Vanilla" ? "Vanilla" : `${e.loader} ${e.loaderVersion ?? ""}`.trim();
  const details = [`Minecraft ${e.minecraft}`, loader];
  if (e.mods) details.push(`${e.mods} mods`);
  if (e.worlds) details.push(e.worlds === 1 ? t("1 mundo", "1 world") : t(`${e.worlds} mundos`, `${e.worlds} worlds`));
  const taken = state.instances.some((i) => i.name.toLowerCase() === e.name.toLowerCase());
  return `<div class="external-item">
    <div class="grow">
      <b>${escapeHtml(e.name)}</b>
      <span class="muted">${escapeHtml(details.join(" · "))}${taken ? t(" · ya tienes una con ese nombre", " · you already have one with that name") : ""}</span>
    </div>
    <span class="tag">${escapeHtml(e.launcher)}</span>
    <button class="btn primary" data-external="${index}">${t("Importar", "Import")}</button>
  </div>`;
}

async function importExternal(external) {
  if (state.importing) return toast(t("Ya se está instalando algo; espera a que termine.", "Something is already being installed; wait for it to finish."), "error");
  state.importing = true;
  $("#create-step-external").classList.add("hidden");
  $("#create-step-import").classList.remove("hidden");
  $("#create-back").classList.add("hidden");
  $("#create-title").textContent = t("Importar de otro launcher", "Import from another launcher");
  $("#import-title").textContent = external.name;
  $("#import-status").textContent = t("Preparando…", "Preparing…");
  $("#import-progress div").style.width = "2%";
  $("#download-indicator").classList.remove("hidden");
  $("#download-percent").textContent = "…";
  try {
    const result = await invoke("importar_externa", { carpeta: external.folder });
    state.importing = false;
    closeModal("#create-dialog");
    await reloadInstances();
    toast(t(`${result.name} está lista para jugar.`, `${result.name} is ready to play.`));
    if (result.warning) toast(result.warning, "error");
    if (result.notes?.length) toast(result.notes.join(" "));
    navigate("instance", result.id);
  } catch (error) {
    $("#import-title").textContent = t("No se pudo importar", "Could not import");
    $("#import-status").textContent = String(error);
    $("#create-back").classList.remove("hidden");
  } finally {
    state.importing = false;
    $("#download-indicator").classList.add("hidden");
  }
}

/** Subir un modpack: .mrpack o paquete de Parabellum. Sin ruta, abre la ventana para elegirlo. */
async function importModpack(path = null) {
  if (state.importing) return toast(t("Ya se está instalando un modpack; espera a que termine.", "A modpack is already being installed; wait for it to finish."), "error");
  state.importing = true;
  $("#create-step-type").classList.add("hidden");
  $("#create-step-custom").classList.add("hidden");
  $("#create-step-import").classList.remove("hidden");
  $("#create-back").classList.add("hidden");
  $("#create-title").textContent = t("Subir un modpack", "Upload a modpack");
  $("#import-title").textContent = path ? path.split(/[\\/]/).pop() : t("Elige el archivo del modpack…", "Choose the modpack file…");
  $("#import-status").textContent = path ? t("Preparando…", "Preparing…") : t("Se abrió una ventana para elegirlo.", "A window opened to choose it.");
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
      toast(t(`Se instaló ${result.name}. Ya puedes instalar los modpacks de esa versión.`, `${result.name} was installed. You can now install the modpacks of that version.`));
    } else {
      toast(result.updated ? t(`${result.name} se actualizó: tus mundos y opciones se conservaron.`, `${result.name} was updated: your worlds and options were kept.`) : t(`${result.name} está lista para jugar.`, `${result.name} is ready to play.`));
      if (result.warning) toast(result.warning, "error");
    if (result.notes?.length) toast(result.notes.join(" "));
      navigate("instance", result.id);
    }
  } catch (error) {
    $("#import-title").textContent = t("No se pudo instalar", "Could not install");
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
  config: t("Configuración de los mods", "Mod settings"),
  resourcepacks: "Resource packs",
  shaderpacks: "Shaders",
  saves: t("Mundos (tus partidas)", "Worlds (your saves)"),
  "options.txt": t("Opciones del juego (teclas, video, sonido)", "Game options (keys, video, sound)"),
  "servers.dat": t("Lista de servidores", "Server list"),
  screenshots: t("Capturas de pantalla", "Screenshots"),
  defaultconfigs: t("Configuración inicial", "Default settings"),
  kubejs: t("Scripts de KubeJS", "KubeJS scripts"),
};

function formatSize(bytes) {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(bytes < 10 * 1024 ** 2 ? 1 : 0)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

async function openExport(id) {
  const instance = state.instances.find((i) => i.id === id);
  if (!instance) return;
  state.exporting = { id, format: "drive", entries: [], busy: false, file: null, result: null, folder: await invoke("carpeta_exportar").catch(() => null), choose: false };
  $("#export-drive").classList.add("hidden");
  $("#export-upload").classList.add("hidden");
  $("#export-title").textContent = t(`Exportar ${instance.name}`, `Export ${instance.name}`);
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
  $("#export-ok span").textContent = t("Exportar…", "Export…");
  $("#export-ok use").setAttribute("href", "#i-download");
  state.exporting.file = null;
}

function selectExportFormat(format) {
  if (state.exporting.busy) return;
  state.exporting.format = format;
  for (const option of $$("#export-formats [data-format]")) option.classList.toggle("active", option.dataset.format === format);
  $("#export-mrpack-fields").classList.toggle("hidden", format !== "mrpack");
  $("#export-drive").classList.add("hidden");
  $("#export-upload").classList.add("hidden");
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
  $("#export-size").textContent = t(`${formatSize(total)} de la instancia`, `${formatSize(total)} of the instance`);
}

async function runExport() {
  const exporting = state.exporting;
  if (exporting.file) return invoke("mostrar_archivo", { ruta: exporting.file }).catch((error) => toast(String(error), "error"));
  if (exporting.busy) return;
  const include = $$("#export-list [data-entry]:checked").map((i) => i.dataset.entry);
  if (!include.length) return toast(t("Marca al menos una cosa para incluir.", "Check at least one thing to include."), "error");
  exporting.busy = true;
  $("#export-ok").disabled = true;
  $("#export-status").textContent = exporting.format === "mrpack" ? t("Elige dónde guardar el .mrpack…", "Choose where to save the .mrpack…") : t("Elige la carpeta donde guardar el paquete…", "Choose the folder to save the package in…");
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
      $("#export-status").textContent = t(`Listo: ${plural(result.downloads, "archivo se descargará", "archivos se descargarán")} de Modrinth y ${plural(result.included, "va", "van")} dentro del .mrpack.`, `Done: ${plural(result.downloads, "file will be", "files will be")} downloaded from Modrinth and ${plural(result.included, "goes", "go")} inside the .mrpack.`) + (result.warning ? ` ${result.warning}` : "");
    } else {
      exporting.file = result.modpack;
      exporting.result = result;
      exporting.folder = result.folder.replace(/[\\/][^\\/]+$/, "");
      exporting.choose = false;
      updateExportDestination();
      const publisher = state.creator && !!state.driveUrl;
      $("#export-drive").classList.toggle("hidden", !publisher);
      $("#export-upload").classList.toggle("hidden", !publisher);
      const folder = result.folder.split(/[\\/]/).pop();
      $("#export-status").textContent =
        (publisher
          ? t(`Listo (${formatSize(result.size)}). Súbelo al catálogo con «Subir a Drive», o copia la carpeta «${folder}» a una USB.`, `Done (${formatSize(result.size)}). Upload it to the catalog with «Upload to Drive», or copy the «${folder}» folder to a USB drive.`)
          : t(`Listo (${formatSize(result.size)}). Copia la carpeta «${folder}» a una USB para instalarlo en otro PC: dentro van el modpack y Minecraft ${folder} con su Java.`, `Done (${formatSize(result.size)}). Copy the «${folder}» folder to a USB drive to install it on another PC: it contains the modpack and Minecraft ${folder} with its Java.`)) +
        (result.baseReused ? t(" Minecraft ya estaba en esa carpeta y no se volvió a empaquetar.", " Minecraft was already in that folder and was not packed again.") : "");
    }
    $("#export-ok span").textContent = t("Mostrar en la carpeta", "Show in folder");
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
  $("#export-dialog").onmousedown = (event) => {
    if (event.target.id === "export-dialog" && !state.exporting?.busy) closeModal("#export-dialog");
  };
}

// ======================================================================
// Drive: modpacks y versiones de Minecraft (también en la escuela), publicar y actualizar Parabellum
// ======================================================================

async function loadDriveCatalog(reload = false) {
  const drive = state.drive;
  if (drive.loading) return;
  drive.loading = true;
  if (!drive.data || reload) {
    $("#explore-results").innerHTML = `<div class="content-loading column"><div class="spinner"></div><span class="muted">${t("Leyendo los paquetes de Drive…", "Reading the Drive packages…")}</span></div>`;
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
  if (drive.installing === entry.id) return `<button class="btn" disabled><span class="spinner tiny"></span>${t("Instalando…", "Installing…")}</button>`;
  const busy = drive.installing ? "disabled" : "";
  if (entry.kind === "base") {
    return entry.status === "instalado"
      ? `<button class="btn installed" disabled><svg><use href="#i-check"/></svg>${t("Instalada", "Installed")}</button>`
      : `<button class="btn primary" data-drive-install="${escapeHtml(entry.id)}" ${busy}><svg><use href="#i-download"/></svg>${t("Instalar", "Install")}</button>`;
  }
  if (entry.status === "actualizar") {
    return `<button class="btn primary" data-drive-install="${escapeHtml(entry.id)}" ${busy}><svg><use href="#i-update"/></svg>${t("Actualizar", "Update")}</button>`;
  }
  if (entry.status === "instalado") {
    const repair = entry.source === "drive"
      ? `<button class="circle-btn" data-drive-repair="${escapeHtml(entry.id)}" ${busy} title="${t("Reparar: vuelve a bajar solo lo que falte o esté dañado", "Repair: downloads again only what is missing or damaged")}">${icon("refresh")}</button>`
      : "";
    return `${repair}<button class="btn" data-drive-open="${escapeHtml(entry.instance ?? "")}"><svg><use href="#i-play"/></svg>${t("Abrir", "Open")}</button>`;
  }
  return `<button class="btn primary" data-drive-install="${escapeHtml(entry.id)}" ${busy}><svg><use href="#i-download"/></svg>${t("Instalar", "Install")}</button>`;
}

function driveCard(entry, index) {
  const base = entry.kind === "base";
  const subtitle = base
    ? `Minecraft ${entry.minecraftVersion}`
    : `${entry.loader} · Minecraft ${entry.minecraftVersion}${entry.modCount ? ` · ${plural(entry.modCount, "mod", "mods")}` : ""}`;
  const description = entry.description || (base ? t("Minecraft con sus sonidos, idiomas y Java. Sirve para crear instancias aunque Mojang esté bloqueado.", "Minecraft with its sounds, languages and Java. It lets you create instances even if Mojang is blocked.") : "");
  const date = entry.exportedAt ? relativeDate(entry.exportedAt) : "";
  return `<div class="result drive-item" data-drive="${escapeHtml(entry.id)}" style="animation-delay:${Math.min(index, 12) * 25}ms">
    <div class="result-icon">${entry.icon ? `<img src="${entry.icon}" alt="">` : icon(base ? "cubes" : "puzzle")}</div>
    <div class="grow result-body">
      <div class="r-title"><b>${escapeHtml(entry.name)}</b><span class="muted">${escapeHtml(subtitle)}</span></div>
      ${description ? `<div class="r-desc">${escapeHtml(description)}</div>` : ""}
      <div class="r-tags">
        <span class="tag">${entry.source === "drive" ? "Google Drive" : t("Carpeta paquetes", "Packages folder")}</span>
        <span class="tag">${formatSize(entry.size)}</span>
        ${date ? `<span class="tag">${t("Subido", "Uploaded")} ${escapeHtml(date)}</span>` : ""}
        ${entry.status === "actualizar" ? `<span class="tag accent">${t("Versión nueva", "New version")}</span>` : ""}
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
      ? `<a href="${escapeHtml(data.folderUrl)}">${icon("external")}${t("Carpeta de Google Drive", "Google Drive folder")}</a>`
      : `<span class="muted">${t("No hay carpeta de Drive configurada.", "No Drive folder is configured.")}</span>`,
  );
  sources.push(`<button class="link-btn" data-open-packages>${icon("folder")}${t("Carpeta «paquetes» de este PC", "This PC's «paquetes» folder")}</button>`);
  if (data.warnings.length) {
    sources.push(`<details class="drive-warnings"><summary>${plural(data.warnings.length, t("aviso", "warning"), t("avisos", "warnings"))}</summary>${data.warnings.map((w) => `<p>${escapeHtml(w)}</p>`).join("")}</details>`);
  }
  $("#drive-sources").innerHTML = sources.join("");

  const query = state.explore.query.toLowerCase();
  const matches = (e) => !query || `${e.name} ${e.description} ${e.minecraftVersion} ${e.loader}`.toLowerCase().includes(query);
  const modpacks = data.entries.filter((e) => e.kind === "modpack" && matches(e));
  const bases = data.entries.filter((e) => e.kind === "base" && matches(e));
  // Primero lo que tiene versión nueva, luego lo no instalado.
  const order = { actualizar: 0, nuevo: 1, instalado: 2 };
  modpacks.sort((a, b) => order[a.status] - order[b.status] || a.name.localeCompare(b.name, "es"));
  bases.sort((a, b) => b.minecraftVersion.localeCompare(a.minecraftVersion, "es", { numeric: true }));
  if (!modpacks.length && !bases.length) {
    $("#explore-results").innerHTML = `<div class="content-empty">${
      query
        ? t("Nada coincide con la búsqueda.", "Nothing matches the search.")
        : data.entries.length
          ? t("No hay modpacks.", "There are no modpacks.")
          : state.creator
            ? t("Todavía no hay paquetes. Exporta un modpack «Para Drive o USB» y súbelo a la carpeta de Drive, o copia los zips a la carpeta «paquetes» junto a Parabellum.", "There are no packages yet. Export a modpack «For Drive or USB» and upload it to the Drive folder, or copy the zips to the «paquetes» folder next to Parabellum.")
            : t("Todavía no hay modpacks en el catálogo.", "There are no modpacks in the catalog yet.")
    }</div>`;
    return;
  }
  let html = "";
  if (modpacks.length) html += `<h3 class="results-title">Modpacks</h3>${modpacks.map(driveCard).join("")}`;
  if (bases.length) {
    html += `<h3 class="results-title">${t("Versiones de Minecraft", "Minecraft versions")}</h3>
      <p class="muted small-text results-note">${t("Los modpacks instalan la suya solos. Instálalas aparte para crear instancias vanilla aunque Mojang esté bloqueado.", "Modpacks install their own. Install these separately to create vanilla instances even if Mojang is blocked.")}</p>
      ${bases.map(driveCard).join("")}`;
  }
  $("#explore-results").innerHTML = html;
  $("#explore-more").classList.add("hidden");
}

async function installFromDrive(id, repair = false) {
  const drive = state.drive;
  if (drive.installing) return toast(t("Espera a que termine la instalación que está en curso.", "Wait for the current installation to finish."), "error");
  drive.installing = id;
  renderDrive();
  $("#download-indicator").classList.remove("hidden");
  $("#download-percent").textContent = "…";
  try {
    const result = await invoke("instalar_de_catalogo", { id, reparar: repair });
    await reloadInstances();
    if (result.kind === "base") {
      toast(t(`${result.name} quedó instalado: ya puedes crear instancias de esa versión.`, `${result.name} is installed: you can now create instances of that version.`));
    } else if (result.downloadedFiles != null) {
      const what = result.downloadedFiles
        ? t(`se bajaron ${plural(result.downloadedFiles, "archivo", "archivos")} (${formatSize(result.downloadedBytes)})`, `${plural(result.downloadedFiles, "file was", "files were")} downloaded (${formatSize(result.downloadedBytes)})`)
        : t("todo estaba bien, no hizo falta bajar nada", "everything was fine, nothing needed downloading");
      toast(repair ? t(`${result.name} revisado: ${what}.`, `${result.name} checked: ${what}.`) : t(`${result.name} actualizado: solo ${what}. Tus mundos se conservaron.`, `${result.name} updated: only ${what}. Your worlds were kept.`));
    } else {
      toast(result.updated ? t(`${result.name} se actualizó; tus mundos y opciones se conservaron.`, `${result.name} was updated; your worlds and options were kept.`) : t(`${result.name} está lista para jugar.`, `${result.name} is ready to play.`));
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
    if (update) $("#update-text").textContent = t(`Actualizar a ${update.version}`, `Update to ${update.version}`);
    $("#about-update").textContent = update
      ? t(`Hay una versión nueva: ${update.version}`, `There is a new version: ${update.version}`) + `${update.notes ? ` — ${update.notes.replace(/\.$/, "")}` : ""}.`
      : t("Tienes la versión más nueva.", "You have the newest version.");
    if (!update && !quiet) toast(t(`Ya tienes la versión más nueva (${state.version}).`, `You already have the newest version (${state.version}).`));
    // Hay que actualizar para seguir usando Parabellum (fuera de Tauri, el modo de muestra no se bloquea).
    if (update && (tauri || !quiet)) showUpdateDialog();
  } catch (error) {
    if (!quiet) toast(String(error), "error");
  }
}

/** Con un servidor encendido no se actualiza (se apagaría al reiniciar Parabellum): se espera. */
function updateWaitsForServers() {
  return (host?.view?.servers ?? []).some((s) => s.state !== "off");
}

/** La ventana de la versión nueva: no se puede cerrar; solo actualizar (o cerrar Parabellum). */
function showUpdateDialog() {
  const update = state.launcherUpdate;
  if (!update || !$("#update-dialog").classList.contains("hidden")) return;
  const waits = updateWaitsForServers();
  $("#update-from").textContent = state.version;
  $("#update-to").textContent = update.version;
  $("#update-notes").textContent = update.notes ?? "";
  $("#update-size").textContent = t(
    `Para seguir usando Parabellum hay que actualizarlo. Se descarga (${formatSize(update.size)}) y se reinicia solo; los juegos abiertos no se cierran.`,
    `To keep using Parabellum you must update it. It downloads (${formatSize(update.size)}) and restarts on its own; open games are not closed.`,
  );
  if (!state.updating) {
    $("#update-progress").classList.add("hidden");
    $("#update-status").classList.toggle("hidden", !waits);
    $("#update-status").classList.remove("error");
    $("#update-status").textContent = waits ? t("Primero apaga tus servidores: se cerrarían al reiniciar Parabellum.", "First stop your servers: they would close when Parabellum restarts.") : "";
    $("#update-now").disabled = waits;
    $("#update-quit").disabled = false;
    $("#update-now span").textContent = t("Actualizar ahora", "Update now");
  }
  openModal("#update-dialog");
  requestAnimationFrame(() => (waits ? $("#update-quit") : $("#update-now")).focus());
}

/** Al apagarse los servidores, ya se puede actualizar. */
function updateServersChanged() {
  if ($("#update-dialog").classList.contains("hidden") || state.updating) return;
  const waits = updateWaitsForServers();
  $("#update-now").disabled = waits;
  $("#update-status").classList.toggle("hidden", !waits);
}

async function applyLauncherUpdate() {
  const update = state.launcherUpdate;
  if (!update || state.updating) return;
  if (updateWaitsForServers()) return showUpdateDialog();
  state.updating = true;
  $("#update-now").disabled = true;
  $("#update-quit").disabled = true;
  $("#update-progress").classList.remove("hidden");
  $("#update-progress div").style.width = "2%";
  $("#update-status").classList.remove("hidden", "error");
  $("#update-status").textContent = t("Descargando…", "Downloading…");
  $("#update-pill").disabled = true;
  $("#update-text").textContent = t("Descargando…", "Downloading…");
  try {
    // Al terminar, Parabellum se cierra y se abre la versión nueva.
    await invoke("aplicar_actualizacion");
    $("#update-status").textContent = t("Reiniciando…", "Restarting…");
  } catch (error) {
    state.updating = false;
    $("#update-status").textContent = String(error);
    $("#update-status").classList.add("error");
    $("#update-progress").classList.add("hidden");
    $("#update-now").disabled = false;
    $("#update-quit").disabled = false;
    $("#update-now span").textContent = t("Reintentar", "Retry");
    $("#update-pill").disabled = false;
    $("#update-text").textContent = t(`Actualizar a ${update.version}`, `Update to ${update.version}`);
  }
}

/** Las herramientas de publicar solo se ven en el PC de quien mantiene el catálogo. */
function setCreator(on) {
  state.creator = !!on;
  document.body.classList.toggle("creator", state.creator);
}

/** Siete clics seguidos en la versión (en Acerca de) activan o quitan las herramientas de publicar. */
async function versionClicked() {
  const now = Date.now();
  state.versionClicks = (state.versionClicks ?? []).filter((t) => now - t < 4000).concat(now);
  if (state.versionClicks.length < 7) return;
  state.versionClicks = [];
  try {
    setCreator(await invoke("herramientas_creador", { activar: !state.creator }));
    toast(state.creator ? t("Herramientas de publicar activadas en este PC.", "Publishing tools turned on on this PC.") : t("Herramientas de publicar ocultas en este PC.", "Publishing tools hidden on this PC."));
    refreshAbout();
  } catch (error) {
    toast(String(error), "error");
  }
}

async function refreshAbout() {
  $("#about-version").textContent = `Parabellum ${state.version}`;
  if (!state.creator) return;
  const folder = await invoke("carpeta_exportar").catch(() => null);
  $("#about-folder").textContent = folder ?? t("Todavía no eliges una (se pide al publicar o exportar).", "You have not chosen one yet (it is asked when publishing or exporting).");
  $("#about-open-drive").classList.toggle("hidden", !state.driveUrl);
  await refreshDrivePublisher();
}

/** Con quién se publica en Drive (o que todavía no se conectó). */
async function refreshDrivePublisher() {
  try {
    state.drivePublisher = await invoke("drive_publicar");
  } catch (error) {
    state.drivePublisher = null;
    $("#about-drive-state").textContent = String(error);
  }
  const email = state.drivePublisher;
  if (email || !$("#about-drive-state").textContent) {
    $("#about-drive-state").textContent = email
      ? t(`Con ${email}: lo que publiques o exportes se sube solo al catálogo.`, `With ${email}: what you publish or export is uploaded to the catalog on its own.`)
      : t("Sin conectar: lo subes tú a mano.", "Not connected: you upload it by hand.");
  }
  $("#about-drive-connect span").textContent = email ? t("Desconectar", "Disconnect") : t("Conectar Drive", "Connect Drive");
  $("#about-drive-connect").classList.toggle("hidden", !state.driveUrl);
  $("#about-publish span").textContent = email ? t("Publicar en Drive", "Publish to Drive") : t("Publicar en la carpeta", "Publish to the folder");
}

/** Pide en el navegador el permiso de escribir en el Drive del catálogo. Devuelve si quedó conectado. */
async function connectDrive() {
  const ok = await confirmAction({
    title: t("Conectar Drive para publicar", "Connect Drive to publish"),
    text: t(
      "Se abrirá Google para que permitas a Parabellum escribir en tu Drive. Elige la cuenta que puede editar la carpeta del catálogo. Solo lo necesita quien publica: los demás no ven este permiso.",
      "Google will open so you allow Parabellum to write to your Drive. Choose the account that can edit the catalog folder. Only whoever publishes needs it: the others never see this permission.",
    ),
    ok: t("Abrir Google", "Open Google"),
    primary: true,
  });
  if (!ok) return false;
  toast(t("Termina en el navegador y vuelve aquí.", "Finish in the browser and come back here."));
  try {
    const email = await invoke("conectar_drive");
    toast(t(`Drive conectado con ${email}.`, `Drive connected with ${email}.`));
    await refreshDrivePublisher();
    return true;
  } catch (error) {
    toast(String(error), "error");
    return false;
  }
}

async function toggleDrivePublisher() {
  if (!state.drivePublisher) return connectDrive();
  await invoke("desconectar_drive").catch((error) => toast(String(error), "error"));
  await refreshDrivePublisher();
}

/** Muestra el avance de una subida a Drive (en la exportación o en Acerca de). */
function showDriveProgress(stage, fraction) {
  const where = state.driveUpload === "export" ? "export" : "about";
  $(`#${where}-progress`).classList.remove("hidden");
  $(`#${where}-progress > div`).style.width = `${Math.round(fraction * 100)}%`;
  const status = $(`#${where}-status`);
  status.classList.remove("hidden");
  status.textContent = stage;
}

async function publishLauncher() {
  const button = $("#about-publish");
  button.disabled = true;
  try {
    const folder = await invoke("publicar_launcher", { notas: $("#about-notes").value, elegir: false });
    if (!folder) return;
    refreshAbout();
    if (!state.drivePublisher) {
      toast(t(`Listo: sube la carpeta «_parabellum» (está en ${folder.replace(/[\\/]_parabellum$/, "")}) a tu Drive. Quien abra Parabellum se actualizará.`, `Done: upload the «_parabellum» folder (it is in ${folder.replace(/[\\/]_parabellum$/, "")}) to your Drive. Whoever opens Parabellum will update.`));
      return;
    }
    state.driveUpload = "about";
    const message = await invoke("subir_launcher_drive", { carpeta: folder });
    $("#about-progress").classList.add("hidden");
    $("#about-status").textContent = message;
    toast(message);
  } catch (error) {
    $("#about-progress").classList.add("hidden");
    $("#about-status").classList.add("hidden");
    toast(String(error), "error");
  } finally {
    state.driveUpload = null;
    button.disabled = false;
  }
}

/** Sube a Drive el paquete que se acaba de exportar (pide conectar Drive la primera vez). */
async function uploadExport() {
  const exporting = state.exporting;
  if (!exporting?.result || exporting.busy) return;
  if (!state.drivePublisher) {
    await refreshDrivePublisher();
    if (!state.drivePublisher && !(await connectDrive())) return;
  }
  exporting.busy = true;
  state.driveUpload = "export";
  $("#export-upload").disabled = true;
  try {
    const message = await invoke("subir_paquete_drive", { modpack: exporting.result.modpack, base: exporting.result.base });
    $("#export-progress").classList.add("hidden");
    $("#export-status").textContent = message;
    $("#export-upload").classList.add("hidden");
    toast(message);
  } catch (error) {
    $("#export-progress").classList.add("hidden");
    $("#export-status").textContent = String(error);
  } finally {
    exporting.busy = false;
    state.driveUpload = null;
    $("#export-upload").disabled = false;
  }
}

function wireDriveEvents() {
  $("#drive-reload").onclick = () => loadDriveCatalog(true);
  $("#drive-sources").onclick = (event) => {
    if (event.target.closest("[data-open-packages]")) invoke("abrir_carpeta_paquetes").catch((error) => toast(String(error), "error"));
  };
  $("#update-pill").onclick = () => showUpdateDialog();
  $("#update-now").onclick = () => applyLauncherUpdate();
  $("#update-quit").onclick = () => invoke("cerrar_parabellum").catch(() => window.close());
  $("#about-check").onclick = () => checkLauncherUpdate(true, false);
  $("#about-folder-change").onclick = async () => {
    try {
      if (await invoke("elegir_carpeta_exportar")) refreshAbout();
    } catch (error) {
      toast(String(error), "error");
    }
  };
  $("#about-publish").onclick = publishLauncher;
  $("#about-drive-connect").onclick = toggleDrivePublisher;
  $("#about-version").onclick = versionClicked;
  $("#export-upload").onclick = uploadExport;
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
    exporting.choose || !exporting.folder ? (state.creator ? t("Elegirás la carpeta al exportar (la de tus paquetes para Drive, o una USB).", "You will choose the folder when exporting (your Drive packages folder, or a USB drive).") : t("Elegirás la carpeta al exportar (por ejemplo, una USB).", "You will choose the folder when exporting (for example, a USB drive).")) : t(`Se guardará en ${exporting.folder}`, `It will be saved in ${exporting.folder}`);
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
    username: "GxxseS", uuid: "", accounts: [{ username: "Hawntd", uuid: "" }], ramGb: 6, theme: "oscuro", windowWidth: 1280, windowHeight: 720, minimizeOnLaunch: true,
    servers: [{ name: "UniversoCraft", address: "mc.universocraft.com", instance: "Vanilla 1.21.1" }],
  });
  const demo = {
    hosting_estado: {
      playit: { connected: true, agentInstalled: true, claimUrl: null, state: "on", address: "brave-creeper.gl.joinmc.link", target: "Survival con amigos", account: "verified", message: null },
      worldPlayit: false,
      world: null,
      totalMb: 8100,
      freeMb: 2300,
      servers: [
        { id: "Survival con amigos", name: "Survival con amigos", key: "s0123456789ab", instance: "Better MC BMC4", instanceName: "Better MC BMC4", minecraft: "1.20.1", loader: "Forge", ramGb: 4, jvmArgs: [], javaMajor: 17, eula: true, port: 25565, guests: [{ name: "Hawntd", uid: "f1" }, { name: "Pepe_07", uid: null }], created: now - 86400e3, lastStarted: now - 1500e3, leftOut: [{ file: "oculus-mc1.20.1-1.8.0.jar", kind: "modrinth", detail: null }], startupSeconds: 74, state: "ready", players: ["Hawntd"], started: now - 1500e3, cpu: 23, memoryMb: 3120, mods: 56, maxPlayers: 10, address: "brave-creeper.gl.joinmc.link", lan: "192.168.1.20:25565", error: null, diagnosis: null },
        { id: "Vanilla 1.21.1", name: "Vanilla 1.21.1", key: "s0123456789ac", instance: "Vanilla 1.21.1", instanceName: "Vanilla 1.21.1", minecraft: "1.21.1", loader: "Vanilla", ramGb: 2, jvmArgs: [], javaMajor: 21, eula: false, port: 25566, guests: [], created: now - 86400e3 * 3, lastStarted: null, leftOut: [], startupSeconds: null, state: "off", players: [], started: null, cpu: 0, memoryMb: 0, mods: 0, maxPlayers: 10, address: null, lan: null, error: null, diagnosis: null },
      ],
    },
    consola_servidor: ["[12:00:01] [Server thread/INFO]: Starting minecraft server version 1.20.1", "[12:01:15] [Server thread/INFO]: Done (74.012s)! For help, type \"help\"", "[12:03:40] [Server thread/INFO]: Hawntd joined the game", "[12:04:02] [Server thread/INFO]: <Hawntd> ¡Hola!"],
    preparar_servidor: { worlds: ["Mi mundo"], mods: 99, recommendedRam: 4, gameRam: 6, totalMb: 8100, freeMb: 2300 },
    ajustes_servidor: { name: "Survival con amigos", ramGb: 4, jvmArgs: [], eula: true, motd: "Survival con amigos", maxPlayers: 10, gamemode: "survival", difficulty: "normal", pvp: true, viewDistance: 8, simulationDistance: 6, allowFlight: true },
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
  const soon = (value) => new Promise((resolve) => setTimeout(() => resolve(value), 300));
  if (command === "estado_social" || command === "iniciar_sesion_google" || command === "reconectar_amigos") {
    const now = Date.now();
    return soon({
      available: true, signedIn: true, email: "gxxses@uabc.edu.mx", displayName: "Juan", signingIn: false, connecting: false, connected: true,
      error: null, warning: null, nameConflict: null, notice: null,
      me: { state: "playing", pack: "Better MC BMC4", world: "single", address: "silly-cat.sa.e4mc.link", lan: "192.168.1.20:51234" },
      friends: {
        name: "GxxseS",
        requests: [{ uid: "r1", name: "Steve_22" }],
        friends: [
          { uid: "f1", name: "Hawntd", online: true, worldOpen: true, lastSeen: now, presence: { state: "playing", pack: "Minecraft++", packFolder: "Minecraft++", minecraft: "26.1.2", world: "single", address: "happy-fox.sa.e4mc.link" } },
          { uid: "f2", name: "Alex_mx", online: true, worldOpen: false, lastSeen: now, presence: { state: "playing", pack: "Fabulously Optimized", minecraft: "26.2", world: "server", server: "mc.universocraft.com" } },
          { uid: "f3", name: "Dani", online: true, worldOpen: false, lastSeen: now, presence: { state: "online" } },
          { uid: "f4", name: "Pepe_07", online: false, worldOpen: false, lastSeen: now - 86400e3 },
        ],
      },
    });
  }
  if (command === "cerrar_sesion_google") return soon({ available: true, signedIn: false, connected: false });
  if (command === "chats") {
    const now = Date.now();
    return soon([
      { id: "d_a_f1", kind: "directo", name: "Hawntd", members: [{ uid: "me", name: "GxxseS", you: true }, { uid: "f1", name: "Hawntd", you: false }], lastText: "¿Abres tu mundo? Ya tengo el modpack", lastName: "Hawntd", lastAt: now - 120e3, unread: true, owner: null, photo: null, canEdit: false },
      { id: "g_demo", kind: "grupo", name: "Los del salón", members: [{ uid: "me", name: "GxxseS", you: true }, { uid: "f2", name: "Alex_mx", you: false }, { uid: "f3", name: "Dani", you: false }], lastText: "📷 Imagen", lastName: "Dani", lastAt: now - 3600e3, unread: false, owner: "me", photo: null, canEdit: true },
      { id: "g_demo2", kind: "grupo", name: "Los panas", members: [{ uid: "me", name: "GxxseS", you: true }, { uid: "f1", name: "Hawntd", you: false }], lastText: "", lastName: "", lastAt: now - 86400e3, unread: false, owner: "f1", photo: `data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="#2f9e6e"/><circle cx="32" cy="28" r="14" fill="#fff"/></svg>')}`, canEdit: false },
    ]);
  }
  if (command === "abrir_chat") {
    const now = Date.now();
    const messages = args.chat === "g_demo"
      ? [
          { id: "1", from: "f2", name: "Alex_mx", text: "¿Quién juega hoy en la tarde?", at: now - 7200e3, mine: false, image: false, reactions: [{ emoji: "👍", count: 2, mine: true, names: ["GxxseS", "Dani"] }] },
          { id: "2", from: "me", name: "GxxseS", text: "Yo, a las 5 abro mi mundo de Better MC", at: now - 7000e3, mine: true, image: false, reply: { id: "1", name: "Alex_mx", text: "¿Quién juega hoy en la tarde?" }, reactions: [{ emoji: "🔥", count: 1, mine: false, names: ["Alex_mx"] }, { emoji: "😂", count: 1, mine: false, names: ["Dani"] }] },
          { id: "3", from: "f3", name: "Dani", text: null, image: true, width: 640, height: 360, at: now - 3600e3, mine: false, reactions: [] },
          { id: "4", from: "f2", name: "Alex_mx", text: 'parabellum:invitacion:{"t":"servidor","n":"SESE","p":"Better MC [FORGE] - BMC4","k":"s0123456789ab"}', at: now - 1800e3, mine: false, image: false, reactions: [] },
        ]
      : [
          { id: "1", from: "f1", name: "Hawntd", text: "Oye, ¿ya instalaste Minecraft++?", at: now - 600e3, mine: false, image: false },
          { id: "2", from: "me", name: "GxxseS", text: "Sí, ya lo tengo", at: now - 500e3, mine: true, image: false },
          { id: "3", from: "f1", name: "Hawntd", text: "¿Abres tu mundo? Ya tengo el modpack", at: now - 120e3, mine: false, image: false },
        ];
    setTimeout(() => window.dispatchEvent(new CustomEvent("demo-mensajes", { detail: { chat: args.chat, messages } })), 300);
    return soon(null);
  }
  if (command === "imagen_chat") return soon(`data:image/svg+xml,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360"><rect width="640" height="360" fill="#6d4bd8"/><text x="320" y="190" font-size="40" text-anchor="middle" fill="white">Captura</text></svg>')}`);
  if (["cerrar_chat", "marcar_leido", "enviar_mensaje", "enviar_imagen", "salir_del_chat", "agregar_al_grupo", "renombrar_grupo", "reaccionar", "editar_grupo", "sacar_del_grupo", "pasar_grupo"].includes(command)) return soon(null);
  if (command === "invitar_amigo") return soon("Invitaste a tu amigo.");
  if (command === "reporte_problema") {
    const text = ["🛠 Parabellum 0.27.0 · Windows (x86_64) · 16 GB de RAM", "Instancia: Better MC BMC4 · Minecraft 1.20.1 · Forge 47.4.20 · 6 GB", "Se abrió hace 5 min", "Error:", "Rendering overlay", "java.lang.NullPointerException", "P2P:", "19:38:00 David se conectó: directo (36 ms)"];
    return soon({ text: text.join("\n"), to: "Hawntd" });
  }
  if (command === "chat_con_amigo") return soon("d_a_f1");
  if (command === "crear_grupo") return soon("g_demo");
  if (command === "agregar_amigo") return soon(`Solicitud enviada a ${args.nombre}. Aparecerá en tu lista cuando la acepte.`);
  if (["aceptar_solicitud", "rechazar_solicitud", "eliminar_amigo"].includes(command)) return soon(null);
  if (command === "instancias_externas")
    return new Promise((r) => setTimeout(() => r([
      { launcher: "CurseForge", name: "Better MC [BMC6]", folder: "C:\curseforge\Better MC [BMC6]", minecraft: "26.1.2", loader: "NeoForge", loaderVersion: "26.1.2.97", ramGb: null, mods: 228, worlds: 1 },
      { launcher: "Prism", name: "Create Above and Beyond", folder: "C:\Prism\instances\CAB", minecraft: "1.16.5", loader: "Forge", loaderVersion: "36.2.39", ramGb: 6, mods: 187, worlds: 0 },
    ]), 400));
  if (command === "importar_externa") return new Promise((r) => setTimeout(() => r({ id: "Better MC [BMC6]", name: "Better MC [BMC6]", kind: "instancia", updated: false }), 1500));
  if (command === "versiones_compatibles")
    return new Promise((r) => setTimeout(() => r({ online: true, games: ["26.3", "26.3-pre2", "26.2", "26.1.2", "1.21.11", "1.21.1", "1.20.1"], message: null }), 300));
  if (command === "versiones_loader")
    return new Promise((r) =>
      setTimeout(() => r({
        online: true,
        versions: {
          quilt: [{ version: "0.31.0-beta.4", stable: false }, { version: "0.30.1", stable: true }, { version: "0.29.2", stable: true }],
          forge: [{ version: "47.4.20", stable: true }, { version: "47.4.10", stable: true }, { version: "47.3.0", stable: true }],
          neoforge: [{ version: "26.3.0.58-beta", stable: false }, { version: "21.1.77", stable: true }],
        }[args.loader] ?? [{ version: "0.19.5", stable: true }, { version: "0.19.4", stable: true }, { version: "0.18.2", stable: true }, { version: "0.16.14", stable: true }],
        recommended: { quilt: "0.30.1", forge: "47.4.20", neoforge: "21.1.77" }[args.loader] ?? "0.19.5",
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
  if (command === "accesos_directos" || command === "acceso_directo")
    return later({ desktop: command === "acceso_directo" && args.lugar === "escritorio" && args.crear ? "si" : "no", startMenu: "otro" }, 200);
  if (command === "revisar_instancia") return later({ findings: [], checkedMods: 12 }, 200);
  if (command === "diagnosticar")
    return later({
      checkedMods: 100,
      findings: [
        { severity: "error", title: "CustomSkinLoader choca con otro mod al arrancar", detail: "Java encontró el mismo paquete (net.minecraft.client.renderer) en dos módulos y Forge no puede arrancar así. Desactivar CustomSkinLoader lo evita.", evidence: "java.lang.module.ResolutionException: Modules customskinloader and minecraft export package net.minecraft.client.renderer", fixes: [{ label: "Desactivar CustomSkinLoader", recommended: true, fix: { type: "disableMod", file: "CustomSkinLoader.jar" } }] },
        { severity: "error", title: "Hace falta otra versión de Forge", detail: "ParCool! pide Forge 47.4.23 o más nueva y la instancia tiene 47.4.20.", evidence: "ParCool-1.20.1-4.0.1.2.jar", fixes: [{ label: "Cambiar Forge a una versión compatible", recommended: true, fix: { type: "setLoader", requirements: ["[47.4.23,)"] } }, { label: "Desactivar ParCool!", recommended: false, fix: { type: "disableMod", file: "ParCool.jar" } }] },
      ],
    }, 600);
  if (command === "aplicar_arreglo") return later("Listo.", 500);
  if (command === "probar_y_arreglar") return later({ ok: true, attempts: 2, applied: ["Se desactivó CustomSkinLoader_Universal-15.0.jar. Lo puedes volver a activar en Contenido."], diagnosis: null }, 2500);
  if (command === "verificar_instancia") return later({ checked: 394, repaired: ["balm-fabric-1.20.1-7.3.38.jar", "voidz-1.0.11.jar"], failed: [] }, 1500);
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
