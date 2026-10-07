"use strict";

// ======================================================================
// Puente con Rust (Tauri). Fuera de Tauri (navegador) se usan datos de ejemplo para revisar el diseño.
// ======================================================================
const tauri = window.__TAURI__;
const invoke = tauri ? tauri.core.invoke : demoInvoke;
const listen = tauri ? tauri.event.listen : () => Promise.resolve(() => {});
const appWindow = tauri ? tauri.window.getCurrentWindow() : null;

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => [...document.querySelectorAll(selector)];

const state = {
  settings: null,
  instances: [],
  running: new Set(),
  page: "home",
  previousPage: "home",
  current: null,
  systemRam: 8,
  versions: null,
  selectedVersion: null,
  nameEdited: false,
  creating: false,
};

// ======================================================================
// Utilidades
// ======================================================================
function hashOf(text) {
  let h = 2166136261;
  for (const c of text) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
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

function escapeHtml(text) {
  return String(text).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

function icon(name) {
  return `<svg><use href="#i-${name}"/></svg>`;
}

/** Icono de instancia: degradado morado con un tono distinto por nombre y su inicial. */
function instanceIcon(instance, extraClass = "") {
  const hue = 255 + (hashOf(instance.name) % 60) - 30;
  const style = `background: linear-gradient(135deg, hsl(${hue} 45% 30%), hsl(${hue + 10} 80% 62%));`;
  const letter = escapeHtml((instance.name.trim()[0] || "?").toUpperCase());
  return `<div class="instance-icon ${extraClass}" style="${style}">${letter}</div>`;
}

/** Cabeza de jugador pixelada (8×8) generada a partir del nombre. */
function drawHead(canvas, name) {
  const ctx = canvas.getContext("2d");
  const h = hashOf(name || "Jugador");
  const skins = ["#f2c7a5", "#e0ac83", "#c68a5e", "#a8714a", "#8d5a3b", "#5c3a26"];
  const hairs = ["#2b1d14", "#4a2f1e", "#7a4b2a", "#c79a4a", "#1d1a2b", "#6b3fa0", "#b5332e"];
  const skin = skins[h % skins.length];
  const hair = hairs[(h >> 3) % hairs.length];
  const eyes = ["#3b82f6", "#22a06b", "#7c4a24", "#8b5cf6"][(h >> 6) % 4];
  ctx.fillStyle = skin;
  ctx.fillRect(0, 0, 8, 8);
  ctx.fillStyle = hair;
  ctx.fillRect(0, 0, 8, 2);
  ctx.fillRect(0, 2, 1, (h >> 9) % 2 ? 3 : 1);
  ctx.fillRect(7, 2, 1, (h >> 10) % 2 ? 3 : 1);
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(1, 4, 2, 1);
  ctx.fillRect(5, 4, 2, 1);
  ctx.fillStyle = eyes;
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

// ======================================================================
// Ventana y navegación
// ======================================================================
const titles = { home: "Biblioteca", settings: "Ajustes", instance: "" };

function showPage(page) {
  if (page !== state.page && state.page !== "instance") state.previousPage = state.page;
  state.page = page;
  for (const section of $$(".page")) {
    const active = section.id === `page-${page}`;
    section.classList.toggle("hidden", !active);
    if (active) {
      section.classList.remove("enter");
      void section.offsetWidth; // reinicia la animación
      section.classList.add("enter");
      section.scrollTop = 0;
    }
  }
  for (const button of $$(".rail-btn[data-page]")) button.classList.toggle("active", button.dataset.page === page);
  $("#back").classList.toggle("hidden", page !== "instance");
  $("#crumb-text").textContent = page === "instance" ? state.current?.name ?? "" : titles[page];
}

function applyTheme(theme) {
  const resolved = theme === "sistema" ? (matchMedia("(prefers-color-scheme: light)").matches ? "claro" : "oscuro") : theme;
  document.documentElement.dataset.theme = resolved;
  for (const button of $$("#theme button")) button.classList.toggle("active", button.dataset.theme === theme);
}

// ======================================================================
// Biblioteca
// ======================================================================
function renderLibrary() {
  const query = $("#library-search").value.trim().toLowerCase();
  const visible = state.instances.filter((i) => !query || `${i.name} ${i.minecraftVersion} ${i.loader}`.toLowerCase().includes(query));
  $("#library").innerHTML = visible
    .map((instance, index) => {
      const running = state.running.has(instance.id);
      return `<div class="card" data-id="${escapeHtml(instance.id)}" style="animation-delay:${Math.min(index, 12) * 25}ms">
        ${instanceIcon(instance)}
        <div class="info">
          <div class="name">${escapeHtml(instance.name)}</div>
          <div class="sub">${escapeHtml(instance.loader)} ${escapeHtml(instance.minecraftVersion)}</div>
          <div class="when">${running ? "Jugando ahora" : ago(instance.lastPlayed)}</div>
        </div>
        <button class="play ${running ? "running" : ""}" data-play="${escapeHtml(instance.id)}" title="${running ? "En juego" : "Jugar"}">${icon(running ? "gamepad" : "play")}</button>
      </div>`;
    })
    .join("");
  const empty = state.instances.length === 0;
  $("#library-empty").classList.toggle("hidden", !empty);
  $("#library").classList.toggle("hidden", empty);
  $(".search").classList.toggle("hidden", empty);
  $("#home-sub").textContent = empty
    ? "Tus instancias de Minecraft"
    : `${state.instances.length} ${state.instances.length === 1 ? "instancia" : "instancias"}`;

  $("#rail-recent").innerHTML = state.instances
    .slice(0, 5)
    .map((i) => `<div title="${escapeHtml(i.name)}" data-open="${escapeHtml(i.id)}">${instanceIcon(i)}</div>`)
    .join("");
}

async function reloadInstances() {
  state.instances = await invoke("listar_instancias");
  renderLibrary();
}

// ======================================================================
// Detalle de instancia
// ======================================================================
function openInstance(id) {
  const instance = state.instances.find((i) => i.id === id);
  if (!instance) return;
  state.current = instance;
  $("#detail-icon").outerHTML = instanceIcon(instance, "xl").replace('class="instance-icon xl"', 'class="instance-icon xl" id="detail-icon"');
  $("#detail-name").textContent = instance.name;
  $("#detail-chips").innerHTML = `
    <span class="chip accent">${icon("cube")}${escapeHtml(instance.loader)}</span>
    <span class="chip">Minecraft ${escapeHtml(instance.minecraftVersion)}</span>
    <span class="chip">${escapeHtml(ago(instance.lastPlayed))}</span>`;
  $("#detail-rows").innerHTML = `
    <dt>Versión</dt><dd>${escapeHtml(instance.versionId)}</dd>
    <dt>Carpeta</dt><dd>instancias\\${escapeHtml(instance.id)}</dd>
    <dt>Memoria</dt><dd>${instance.ramGb ? `${instance.ramGb} GB (propia de esta instancia)` : `${state.settings.ramGb} GB (la general de Ajustes)`}</dd>`;
  updatePlayButton();
  showPage("instance");
}

function updatePlayButton() {
  if (!state.current) return;
  const running = state.running.has(state.current.id);
  $("#detail-play").disabled = running;
  $("#detail-play span").textContent = running ? "En juego" : "Jugar";
  $("#server-join").disabled = running;
}

async function play(id, server = null) {
  const instance = state.instances.find((i) => i.id === id);
  try {
    toast(server ? `Abriendo ${instance?.name} y entrando a ${server}…` : `Abriendo ${instance?.name}…`);
    await invoke("jugar", { id, servidor: server });
    state.running.add(id);
    refreshRunning();
    setTimeout(() => appWindow?.minimize(), 3500);
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
  $("#status").title = running ? `Ver ${running.name}` : "Ir a tu biblioteca";
  renderLibrary();
  updatePlayButton();
}

async function pollRunning() {
  try {
    const ids = await invoke("juegos_abiertos");
    const changed = ids.length !== state.running.size || ids.some((id) => !state.running.has(id));
    if (changed) {
      state.running = new Set(ids);
      refreshRunning();
    }
  } catch {
    /* se reintenta en la siguiente vuelta */
  }
}

// ======================================================================
// Ajustes
// ======================================================================
let saveTimer = null;

function renderSettings() {
  const s = state.settings;
  $("#username").value = s.username;
  $("#ram").max = Math.max(4, Math.min(32, state.systemRam - 2));
  $("#ram").value = s.ramGb;
  $("#ram-max").textContent = `${$("#ram").max} GB`;
  updateSettingsTexts();
  applyTheme(s.theme);
}

async function updateSettingsTexts() {
  const name = $("#username").value.trim();
  const valid = /^[A-Za-z0-9_]{1,16}$/.test(name);
  $("#username-error").textContent = "El nombre debe tener de 1 a 16 letras, números o guiones bajos (sin espacios ni acentos).";
  $("#username-error").classList.toggle("hidden", valid);
  $("#uuid-text").textContent = valid ? await invoke("uuid_offline", { nombre: name }) : "—";
  for (const canvas of [$("#settings-head"), $("#rail-head")]) drawHead(canvas, name);
  const ram = Number($("#ram").value);
  $("#ram-value").textContent = `${ram} GB`;
  let hint = `Memoria máxima para el juego. Tu PC tiene ${state.systemRam} GB. Vanilla funciona bien con 2–4 GB; los modpacks grandes piden de 6 a 10 GB.`;
  if (ram > state.systemRam - 3) hint += " Deja al menos 3 GB libres para Windows.";
  $("#ram-hint").textContent = hint;
}

function queueSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveSettings, 400);
}

async function saveSettings() {
  const name = $("#username").value.trim();
  if (!/^[A-Za-z0-9_]{1,16}$/.test(name)) return;
  try {
    state.settings = await invoke("guardar_ajustes", {
      ajustes: { ...state.settings, username: name, ramGb: Number($("#ram").value) },
    });
  } catch (error) {
    toast(String(error), "error");
  }
}

// ======================================================================
// Nueva instancia
// ======================================================================
async function openNewDialog() {
  state.selectedVersion = null;
  state.nameEdited = false;
  $("#new-name").value = "";
  $("#new-search").value = "";
  $("#new-status").textContent = "";
  $("#new-progress").classList.add("hidden");
  $("#new-create").disabled = true;
  $("#new-dialog").classList.remove("hidden");
  $("#new-versions").innerHTML = `<div class="version-empty">Cargando versiones…</div>`;
  $("#new-source").textContent = "Buscando versiones de Minecraft…";
  state.versions = await invoke("listar_versiones");
  $("#new-source").textContent = state.versions.mojang
    ? "Se descarga de los servidores oficiales de Mojang, con su Java incluido."
    : `${state.versions.message} Puedes usar las versiones ya descargadas en este PC.`;
  renderVersions();
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
          const kind = o.type === "release" ? "Versión estable" : "Snapshot";
          return `<div class="version ${o.id === state.selectedVersion ? "selected" : ""}" data-version="${escapeHtml(o.id)}">
            <div><div class="v-name">Minecraft ${escapeHtml(o.id)}</div><div class="v-sub">${kind}${date ? " · " + date : ""}</div></div>
            ${o.installed ? `<span class="badge">Descargada</span>` : ""}
          </div>`;
        })
        .join("")
    : `<div class="version-empty">${state.versions?.mojang === false && !state.versions.installed.length ? "No hay versiones descargadas en este PC." : "Ninguna versión coincide."}</div>`;
}

function selectVersion(id, rerender = true) {
  state.selectedVersion = id;
  if (!state.nameEdited) $("#new-name").value = `Minecraft ${id}`;
  const option = versionOptions().find((o) => o.id === id);
  $("#new-status").textContent = option?.installed ? "Ya está descargada: la instancia se crea al momento." : "Se descargará de Mojang.";
  $("#new-create").disabled = state.creating;
  if (rerender) renderVersions();
}

function closeNewDialog() {
  if (state.creating) return;
  const overlay = $("#new-dialog");
  overlay.style.animation = "fade-in .15s ease reverse both";
  setTimeout(() => {
    overlay.classList.add("hidden");
    overlay.style.animation = "";
  }, 140);
}

async function createInstance() {
  const name = $("#new-name").value.trim();
  if (!name || !state.selectedVersion) return;
  state.creating = true;
  $("#new-create").disabled = true;
  $("#new-cancel").disabled = true;
  $("#new-progress").classList.remove("hidden");
  $("#new-progress div").style.width = "2%";
  try {
    const instance = await invoke("crear_instancia", { nombre: name, version: state.selectedVersion });
    state.creating = false;
    closeNewDialog();
    await reloadInstances();
    toast(`${instance.name} está lista para jugar.`);
    openInstance(instance.id);
  } catch (error) {
    $("#new-status").textContent = String(error);
  } finally {
    state.creating = false;
    $("#new-create").disabled = false;
    $("#new-cancel").disabled = false;
    $("#new-progress").classList.add("hidden");
  }
}

// ======================================================================
// Eventos
// ======================================================================
function wireEvents() {
  $("#win-min").onclick = () => appWindow?.minimize();
  $("#win-max").onclick = () => appWindow?.toggleMaximize();
  $("#win-close").onclick = () => appWindow?.close();

  for (const button of $$(".rail-btn[data-page]")) button.onclick = () => showPage(button.dataset.page);
  $("#rail-avatar").onclick = () => showPage("settings");
  for (const button of [$("#rail-new"), $("#home-new"), $("#empty-new")]) button.onclick = openNewDialog;
  $("#back").onclick = () => showPage(state.previousPage);
  $("#status").onclick = () => {
    const running = state.instances.find((i) => state.running.has(i.id));
    if (running) openInstance(running.id);
    else showPage("home");
  };

  $("#library-search").oninput = renderLibrary;
  $("#library").onclick = (event) => {
    const playButton = event.target.closest("[data-play]");
    if (playButton) return play(playButton.dataset.play);
    const card = event.target.closest(".card");
    if (card) openInstance(card.dataset.id);
  };
  $("#rail-recent").onclick = (event) => {
    const item = event.target.closest("[data-open]");
    if (item) openInstance(item.dataset.open);
  };

  $("#detail-play").onclick = () => state.current && play(state.current.id);
  $("#detail-folder").onclick = () => state.current && invoke("abrir_carpeta", { id: state.current.id });
  $("#server-join").onclick = () => {
    const address = $("#server-address").value.trim();
    if (!address) return toast("Escribe la dirección del servidor.", "error");
    play(state.current.id, address);
  };
  $("#server-address").onkeydown = (event) => event.key === "Enter" && $("#server-join").click();

  $("#username").oninput = () => {
    updateSettingsTexts();
    queueSave();
  };
  $("#ram").oninput = () => {
    updateSettingsTexts();
    queueSave();
  };
  for (const button of $$("#theme button")) {
    button.onclick = async () => {
      applyTheme(button.dataset.theme);
      state.settings = await invoke("guardar_ajustes", { ajustes: { ...state.settings, theme: button.dataset.theme } });
    };
  }
  matchMedia("(prefers-color-scheme: light)").onchange = () => state.settings?.theme === "sistema" && applyTheme("sistema");
  $("#uuid-copy").onclick = async () => {
    await navigator.clipboard.writeText($("#uuid-text").textContent);
    toast("UUID copiado.");
  };
  $("#open-root").onclick = () => invoke("abrir_carpeta", { id: null });

  $("#new-close").onclick = closeNewDialog;
  $("#new-cancel").onclick = closeNewDialog;
  $("#new-dialog").onclick = (event) => event.target.id === "new-dialog" && closeNewDialog();
  $("#new-search").oninput = renderVersions;
  $("#new-snapshots").onchange = renderVersions;
  $("#new-versions").onclick = (event) => {
    const item = event.target.closest("[data-version]");
    if (item && !state.creating) selectVersion(item.dataset.version);
  };
  $("#new-name").oninput = () => (state.nameEdited = $("#new-name").value.length > 0);
  $("#new-create").onclick = createInstance;
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !$("#new-dialog").classList.contains("hidden")) closeNewDialog();
  });

  listen("progreso", ({ payload }) => {
    $("#new-status").textContent = payload.stage;
    $("#new-progress div").style.width = `${Math.max(2, payload.fraction * 100)}%`;
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
  renderSettings();
  renderLibrary();
  showPage("home");
  pollRunning();
  setInterval(pollRunning, 3000);
}

// ======================================================================
// Datos de ejemplo (solo al abrir index.html en un navegador, para revisar el diseño)
// ======================================================================
function demoInvoke(command, args = {}) {
  const now = Date.now();
  const demo = {
    estado_inicial: {
      settings: { username: "GxxseS", uuid: "", ramGb: 6, theme: "oscuro", windowWidth: 1280, windowHeight: 720 },
      offlineUuid: "8b67db88-c6fb-3d4d-bbee-71f84a0e752d",
      instances: [
        { id: "Vanilla 1.21.1", name: "Vanilla 1.21.1", versionId: "1.21.1", minecraftVersion: "1.21.1", loader: "Vanilla", ramGb: null, lastPlayed: now - 3600e3 },
        { id: "Better MC BMC4", name: "Better MC BMC4", versionId: "forge-47.4.20", minecraftVersion: "1.20.1", loader: "Forge", ramGb: 8, lastPlayed: now - 86400e3 * 2 },
        { id: "Minecraft++", name: "Minecraft++", versionId: "fabric-loader-0.19.5-26.1.2", minecraftVersion: "26.1.2", loader: "Fabric", ramGb: null, lastPlayed: null },
      ],
      version: "0.1.0 (demostración)",
      root: "C:\\Users\\Juan\\Desktop\\Parabellum",
      systemRamGb: 16,
    },
    uuid_offline: "8b67db88-c6fb-3d4d-bbee-71f84a0e752d",
    juegos_abiertos: [],
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
  if (command === "listar_instancias") return Promise.resolve(demo.estado_inicial.instances);
  if (command === "guardar_ajustes") return Promise.resolve(args.ajustes);
  if (command === "jugar" || command === "crear_instancia") return Promise.reject("Modo demostración: abre Parabellum.exe para jugar.");
  return Promise.resolve(demo[command]);
}

init().catch((error) => toast(String(error), "error"));
