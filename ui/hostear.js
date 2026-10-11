"use strict";

// ======================================================================
// Hostear partida: servidores propios (consola, invitados, ajustes) y el túnel de playit.gg, que da
// una dirección pública fija al servidor encendido o al mundo abierto en LAN.
// Usa lo que define app.js: state, invoke, listen, t, $, $$, icon, toast, navigate, openModal…
// ======================================================================

const host = {
  view: null,
  /** Servidor abierto en la página de detalle. */
  id: null,
  tab: "console",
  console: [],
  history: [],
  historyAt: -1,
  connecting: false,
  creating: false,
  prepared: null,
  /** Al aceptar el EULA desde el aviso: el servidor que se enciende después. */
  eulaFor: null,
};
const EULA_URL = "https://aka.ms/MinecraftEULA";
const CONSOLE_MAX = 4000;
const hostServer = (id) => host.view?.servers.find((s) => s.id === id);

const gamemodeNames = () => ({ survival: t("Supervivencia", "Survival"), creative: t("Creativo", "Creative"), adventure: t("Aventura", "Adventure"), spectator: t("Espectador", "Spectator") });
const difficultyNames = () => ({ peaceful: t("Pacífico", "Peaceful"), easy: t("Fácil", "Easy"), normal: t("Normal", "Normal"), hard: t("Difícil", "Hard") });
const stateNames = () => ({ off: t("Apagado", "Off"), starting: t("Encendiendo…", "Starting…"), ready: t("Encendido", "On"), stopping: t("Apagando…", "Stopping…") });

function fillOptions(select, names, current) {
  select.innerHTML = Object.entries(names)
    .map(([value, label]) => `<option value="${value}" ${value === current ? "selected" : ""}>${escapeHtml(label)}</option>`)
    .join("");
}

function gb(mb) {
  return (mb / 1024).toFixed(mb < 10240 ? 1 : 0);
}

function uptime(since) {
  const minutes = Math.max(0, Math.floor((Date.now() - since) / 60000));
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${minutes % 60} min`;
}

// ---------- Memoria: ¿alcanza para jugar y tener el servidor a la vez? ----------
/** Barra con Windows, el juego, el servidor y lo que queda libre, y un consejo. */
function ramPlan(totalMb, serverGb, gameGb) {
  const total = totalMb / 1024;
  const windows = total <= 8.5 ? 2.5 : 3;
  // Java usa algo más que su máximo (código, hilos, bibliotecas nativas).
  const game = gameGb + 0.8;
  const server = serverGb + 0.5;
  const free = total - windows - game - server;
  const width = (value) => `${Math.max(0, (value / total) * 100).toFixed(1)}%`;
  const level = free < 0.4 ? "bad" : free < 1.2 ? "tight" : "good";
  const advice =
    level === "bad"
      ? t(
          `No alcanza: con ${Math.round(total)} GB, jugar y tener el servidor a la vez va a ir lento o cerrarse. Baja la memoria del servidor o, si juegas tú también, usa «Abrir mi mundo a amigos» (un solo Java en vez de dos).`,
          `Not enough: with ${Math.round(total)} GB, playing and running the server at the same time will be slow or crash. Lower the server memory or, if you play too, use «Open my world to friends» (one Java instead of two).`,
        )
      : level === "tight"
        ? t(
            `Justo: alcanza si cierras el navegador y otros programas mientras juegas. En un PC de ${Math.round(total)} GB, para modpacks grandes conviene «Abrir mi mundo a amigos».`,
            `Tight: it works if you close the browser and other programs while playing. On a ${Math.round(total)} GB PC, «Open my world to friends» is better for big modpacks.`,
          )
        : t("Bien: alcanza para jugar en este PC mientras el servidor está encendido.", "Good: there is enough to play on this PC while the server is on.");
  return `<div class="ram-bar" role="img" aria-label="${escapeHtml(advice)}">
      <span class="seg ram-windows" style="width:${width(windows)}" title="Windows"></span>
      <span class="seg ram-game" style="width:${width(game)}" title="${t("Tu juego", "Your game")}"></span>
      <span class="seg ram-server ${level}" style="width:${width(server)}" title="${t("Servidor", "Server")}"></span>
    </div>
    <div class="ram-legend">
      <span><i class="ram-windows"></i>Windows ${windows.toFixed(1)} GB</span>
      <span><i class="ram-game"></i>${t("Tu juego", "Your game")} ${game.toFixed(1)} GB</span>
      <span><i class="ram-server ${level}"></i>${t("Servidor", "Server")} ${server.toFixed(1)} GB</span>
      <span><i class="ram-free"></i>${t("Libre", "Free")} ${Math.max(0, free).toFixed(1)} GB</span>
    </div>
    <p class="ram-advice ${level}">${escapeHtml(advice)}</p>`;
}

// ---------- Estado general ----------
function applyHostView(view) {
  host.view = view;
  const running = view.servers.some((s) => s.state !== "off");
  $("#host-dot").classList.toggle("hidden", !running && !(view.playit.state === "on"));
  if (state.page === "host") renderHost();
  if (state.page === "hosted") renderHosted();
  if (state.page === "home") renderHomeServerTile();
}

async function loadHosting() {
  try {
    applyHostView(await invoke("hosting_estado"));
  } catch (error) {
    if (tauri) toast(String(error), "error");
  }
}

function enterHost() {
  if (!host.view) loadHosting();
  else renderHost();
  loadTunnelMods();
}

/** Instancias con e4mc: con playit.gg sobra (da otra dirección y en 26.2 cierra el mundo al entrar alguien). */
async function loadTunnelMods() {
  try {
    host.tunnelMods = await invoke("mods_de_tunel");
  } catch {
    host.tunnelMods = [];
  }
  renderTunnelMods();
}

function renderTunnelMods() {
  const box = $("#host-e4mc");
  const list = host.tunnelMods ?? [];
  const show = list.length && host.view?.shareWorld;
  box.classList.toggle("hidden", !show);
  if (!show) return;
  box.innerHTML = `${icon("alert")}<div class="grow">
      <b>${t("Tus instancias todavía tienen e4mc", "Your instances still have e4mc")}</b>
      <p>${t(
        "Con playit.gg ya no hace falta: e4mc abre otra dirección distinta y, en Minecraft 26.2, cierra tu mundo en cuanto entra un amigo. Desactívalo (puedes volver a activarlo en Contenido).",
        "With playit.gg it is no longer needed: e4mc opens a different address and, on Minecraft 26.2, closes your world as soon as a friend joins. Disable it (you can turn it back on in Content).",
      )}</p>
      <div class="e4mc-list">${list
        .map((m, i) => `<div class="e4mc-row"><span><b>${escapeHtml(m.name)}</b> · ${escapeHtml(m.file)}</span><button class="btn small" data-disable-tunnel="${i}">${t("Desactivar", "Disable")}</button></div>`)
        .join("")}</div>
    </div>`;
}

async function disableTunnelMod(index) {
  const item = host.tunnelMods?.[index];
  if (!item) return;
  if (state.running.has(item.id)) {
    toast(t("Cierra el juego antes de desactivar e4mc.", "Close the game before disabling e4mc."), "error");
    return;
  }
  try {
    await invoke("activar_contenido", { id: item.id, tipo: "mods", archivo: item.file, activo: false });
    toast(t(`e4mc desactivado en ${item.name}.`, `e4mc disabled in ${item.name}.`));
  } catch (error) {
    toast(String(error), "error");
  }
  loadTunnelMods();
}

// ---------- Tarjeta de playit.gg (compacta) ----------
function tunnelRow({ iconHtml, kind = "", title, address = null, detail = "", error = false, actions = "" }) {
  const pill = address
    ? `<span class="host-address-line"><span class="address-text">${escapeHtml(address)}</span><button class="circle-btn small" data-copy-address="${escapeHtml(address)}" title="${t("Copiar dirección", "Copy address")}">${icon("copy")}</button></span>`
    : "";
  return `<div class="tunnel-row">
      <div class="tunnel-icon ${kind}">${iconHtml}</div>
      <div class="grow">
        <div class="tunnel-title"><h3>${escapeHtml(title)}</h3>${pill}</div>
        ${detail ? `<p class="${error ? "tunnel-error" : "muted"}">${escapeHtml(detail)}</p>` : ""}
      </div>
      ${actions}
    </div>`;
}

function renderTunnel() {
  const p = host.view.playit;
  const box = $("#host-tunnel");
  const spinner = `<div class="spinner small"></div>`;
  const account =
    p.account === "guest"
      ? t(" Cuenta de invitado: crea tu cuenta gratis en playit.gg para no perder la dirección.", " Guest account: create your free account on playit.gg so you do not lose the address.")
      : p.account === "email-not-verified"
        ? t(" playit.gg pide verificar tu correo.", " playit.gg asks you to verify your email.")
        : "";
  if (p.claimUrl) {
    box.innerHTML = tunnelRow({
      iconHtml: spinner,
      kind: "busy",
      title: t("Acepta en tu navegador", "Accept in your browser"),
      detail: t("Inicia sesión en playit.gg (o entra como invitado), pulsa «Add agent» y vuelve aquí.", "Sign in to playit.gg (or continue as a guest), press «Add agent» and come back here."),
      actions: `<button class="btn small" id="tunnel-reopen">${icon("external")}${t("Abrir de nuevo", "Open again")}</button><button class="btn small" id="tunnel-cancel">${t("Cancelar", "Cancel")}</button>`,
    });
    return;
  }
  if (!p.connected) {
    const failed = p.state === "error" && p.message;
    box.innerHTML = tunnelRow({
      iconHtml: icon("globe"),
      title: t("playit.gg sin conectar", "playit.gg not connected"),
      detail: failed ? p.message : host.connecting ? (p.message ?? t("Preparando playit.gg…", "Preparing playit.gg…")) : t("Conéctalo una vez para que tus amigos entren desde cualquier red.", "Connect it once so your friends can join from any network."),
      error: !!failed,
      actions: `<button class="btn primary small" id="tunnel-connect" ${host.connecting ? "disabled" : ""}>${icon("globe")}${t("Conectar playit.gg", "Connect playit.gg")}</button>`,
    });
    return;
  }
  const target = p.target === "world" ? t("tu mundo", "your world") : hostServer(p.target)?.name ?? "";
  const menu = `<button class="circle-btn" id="tunnel-menu" title="${t("Más", "More")}">${icon("more")}</button>`;
  const states = {
    on: { kind: "on", iconHtml: icon("globe"), title: t(`playit.gg activo · ${target}`, `playit.gg active · ${target}`), detail: account.trim() },
    starting: { kind: "starting", iconHtml: spinner, title: t("Abriendo tu dirección…", "Opening your address…"), detail: p.message ?? "" },
    error: { kind: "error", iconHtml: icon("alert"), title: t("La dirección no se pudo abrir", "The address could not be opened"), detail: p.message ?? "", error: true },
    off: {
      kind: "",
      iconHtml: icon("globe"),
      title: t("playit.gg conectado", "playit.gg connected"),
      detail: (p.address ? t("Se activa al encender un servidor o abrir tu mundo.", "It turns on when you start a server or open your world.") : t("Tendrás una dirección fija al encender un servidor o abrir tu mundo.", "You will get a fixed address when you start a server or open your world.")) + account,
    },
  };
  const current = states[p.state] ?? states.off;
  box.innerHTML = tunnelRow({
    ...current,
    address: p.address,
    actions: (p.state === "error" ? `<button class="btn small" id="tunnel-retry">${icon("refresh")}${t("Reintentar", "Retry")}</button>` : "") + menu,
  });
}

async function connectPlayit() {
  const p = host.view.playit;
  if (!p.agentInstalled) {
    const ok = await confirmAction({
      title: t("Descargar el agente de playit.gg", "Download the playit.gg agent"),
      text: t(
        "Parabellum va a descargar el programa oficial de playit.gg (4,9 MB, firmado por playit.gg) desde su página de GitHub. Solo se usa mientras compartes un servidor o tu mundo, y se cierra con Parabellum.",
        "Parabellum will download the official playit.gg program (4.9 MB, signed by playit.gg) from its GitHub page. It is only used while you share a server or your world, and it closes with Parabellum.",
      ),
      ok: t("Descargar y conectar", "Download and connect"),
      primary: true,
    });
    if (!ok) return;
  }
  host.connecting = true;
  renderTunnel();
  try {
    applyHostView(await invoke("conectar_playit"));
    toast(t("playit.gg quedó conectado.", "playit.gg is connected."));
  } catch (error) {
    toast(String(error), "error");
  } finally {
    host.connecting = false;
    await loadHosting();
  }
}

function tunnelMenu(anchor) {
  const p = host.view.playit;
  const items = [];
  if (p.address) items.push({ icon: "copy", label: t("Copiar dirección", "Copy address"), action: () => copyText(p.address, t("Dirección copiada.", "Address copied.")) });
  items.push({ icon: "external", label: t("Abrir playit.gg", "Open playit.gg"), action: () => invoke("abrir_enlace", { url: "https://playit.gg/account" }) });
  items.push({
    icon: "trash",
    label: t("Desconectar playit.gg de este PC", "Disconnect playit.gg from this PC"),
    danger: true,
    separator: true,
    action: async () => {
      const ok = await confirmAction({
        title: t("Desconectar playit.gg", "Disconnect playit.gg"),
        text: t("Este PC olvida la vinculación. Tu dirección sigue en tu cuenta de playit.gg; si vuelves a conectar con la misma cuenta, se crea otra.", "This PC forgets the link. Your address stays in your playit.gg account; if you connect again with the same account, another one is created."),
        ok: t("Desconectar", "Disconnect"),
      });
      if (ok) applyHostView(await invoke("desconectar_playit"));
    },
  });
  contextMenu(anchor, items);
}

// ---------- Página principal ----------
function renderHost() {
  const view = host.view;
  if (!view) return;
  renderTunnel();
  $("#host-share-world").checked = view.shareWorld;
  renderTunnelMods();
  const world = view.world;
  const status = $("#host-world-status");
  if (world && view.shareWorld) {
    const instance = state.instances.find((i) => i.id === world.instance);
    status.innerHTML = world.address
      ? `${icon("check-circle")}<span class="grow">${t(`Tu mundo de ${escapeHtml(instance?.name ?? world.instance)} está abierto en <b>${escapeHtml(world.address)}</b>. Tus amigos pulsan «Unirse» en tu nombre.`, `Your ${escapeHtml(instance?.name ?? world.instance)} world is open at <b>${escapeHtml(world.address)}</b>. Your friends press «Join» on your name.`)}</span><button class="btn small" data-copy-address="${escapeHtml(world.address)}">${icon("copy")}${t("Copiar", "Copy")}</button>`
      : `${icon("info")}<span class="grow">${
          view.playit.connected
            ? t(`Tu mundo está abierto en LAN (puerto ${world.port}). Abriendo la dirección pública…`, `Your world is open to LAN (port ${world.port}). Opening the public address…`)
            : t(`Tu mundo está abierto en LAN (puerto ${world.port}). Conecta playit.gg para que entren tus amigos de otras redes.`, `Your world is open to LAN (port ${world.port}). Connect playit.gg so friends from other networks can join.`)
        }</span>`;
    status.classList.remove("hidden");
  } else {
    status.classList.add("hidden");
  }
  const names = stateNames();
  setHtml($("#host-servers"), view.servers
    .map((s, index) => {
      const sub = [s.instanceName, `${s.loader} ${s.minecraft}`];
      if (s.state === "ready") sub.push(t(`${s.players.length} de ${s.maxPlayers} jugadores`, `${s.players.length} of ${s.maxPlayers} players`));
      if (s.address) sub.push(s.address);
      const action =
        s.state === "off"
          ? `<button class="btn primary" data-host-start="${escapeHtml(s.id)}">${icon("play")}${t("Encender", "Start")}</button>`
          : `<button class="btn danger" data-host-stop="${escapeHtml(s.id)}" ${s.state === "stopping" ? "disabled" : ""}>${icon("stop")}${t("Apagar", "Stop")}</button>`;
      return `<div class="server host-card" data-hosted="${escapeHtml(s.id)}" style="animation-delay:${index * 40}ms" tabindex="0">
        <div class="server-icon">${icon("server")}</div>
        <div class="grow">
          <div class="title">${escapeHtml(s.name)} <span class="state-pill ${s.state}">${escapeHtml(names[s.state])}</span></div>
          <div class="sub">${sub.map(escapeHtml).join(" · ")}</div>
          ${s.error ? `<div class="sub host-error">${escapeHtml(s.error)}</div>` : ""}
        </div>
        ${action}
        <button class="circle-btn" data-hosted-open="${escapeHtml(s.id)}" title="${t("Abrir", "Open")}">${icon("right")}</button>
      </div>`;
    })
    .join(""));
  $("#host-empty").classList.toggle("hidden", view.servers.length > 0);
}

// ---------- Encender y apagar ----------
async function startHosted(id) {
  const server = hostServer(id);
  if (!server) return;
  if (!server.eula) {
    host.eulaFor = id;
    $("#he-accept").checked = false;
    $("#he-ok").disabled = true;
    openModal("#host-eula-dialog");
    return;
  }
  try {
    await invoke("iniciar_servidor", { id });
    if (state.page === "hosted" && host.id === id) {
      host.console = [];
      renderConsole(true);
    }
    toast(t(`Encendiendo ${server.name}…`, `Starting ${server.name}…`));
  } catch (error) {
    toast(String(error), "error");
  }
}

async function acceptEulaAndStart() {
  const id = host.eulaFor;
  closeModal("#host-eula-dialog");
  if (!id) return;
  try {
    const settings = await invoke("ajustes_servidor", { id });
    await invoke("guardar_servidor", { id, ajustes: { ...settings, eula: true } });
    await loadHosting();
    await startHosted(id);
  } catch (error) {
    toast(String(error), "error");
  }
}

async function stopHosted(id) {
  try {
    await invoke("detener_servidor", { id });
  } catch (error) {
    toast(String(error), "error");
  }
}

// ---------- Crear servidor ----------
async function openHostCreate(instanceId = null) {
  if (!state.instances.length) {
    toast(t("Primero crea una instancia: el servidor usa su versión y sus mods.", "Create an instance first: the server uses its version and mods."), "error");
    return;
  }
  const select = $("#hc-instance");
  select.innerHTML = state.instances.map((i) => `<option value="${escapeHtml(i.id)}">${escapeHtml(i.name)} · ${escapeHtml(subtitle(i))}</option>`).join("");
  const preferred = instanceId ?? state.instances.slice().sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))[0].id;
  select.value = preferred;
  fillOptions($("#hc-gamemode"), gamemodeNames(), "survival");
  fillOptions($("#hc-difficulty"), difficultyNames(), "normal");
  $("#hc-eula").checked = false;
  $("#hc-seed").value = "";
  $("#hc-jvm").value = "";
  $("#hc-progress").classList.add("hidden");
  $("#hc-create").disabled = false;
  openModal("#host-create-dialog");
  await prepareHostCreate();
}

async function prepareHostCreate() {
  const id = $("#hc-instance").value;
  const instance = state.instances.find((i) => i.id === id);
  $("#hc-name").value = instance?.name ?? "";
  $("#hc-info").textContent = t("Revisando la instancia…", "Checking the instance…");
  try {
    const info = await invoke("preparar_servidor", { instancia: id });
    host.prepared = info;
    $("#hc-world").innerHTML =
      `<option value="">${t("Mundo nuevo", "New world")}</option>` +
      info.worlds.map((w) => `<option value="${escapeHtml(w)}">${escapeHtml(t(`Copiar «${w}» de la instancia`, `Copy «${w}» from the instance`))}</option>`).join("");
    const ram = $("#hc-ram");
    ram.max = Math.max(2, Math.min(16, Math.floor(info.totalMb / 1024) - 2));
    ram.value = Math.min(info.recommendedRam, Number(ram.max));
    $("#hc-info").textContent = info.mods
      ? t(
          `${instance.loader} ${instance.minecraftVersion} con ${plural(info.mods, "mod", "mods")}. Los que solo son para el juego (gráficos, cámara, menús) se quedan fuera del servidor solos. Para ${plural(info.mods, "mod", "mods")} se recomiendan ${info.recommendedRam} GB.`,
          `${instance.loader} ${instance.minecraftVersion} with ${plural(info.mods, "mod", "mods")}. The client-only ones (graphics, camera, menus) are left out of the server automatically. For ${plural(info.mods, "mod", "mods")}, ${info.recommendedRam} GB is recommended.`,
        )
      : t(`${instance.loader} ${instance.minecraftVersion} sin mods: 2 GB alcanzan.`, `${instance.loader} ${instance.minecraftVersion} without mods: 2 GB is enough.`);
    updateHostCreateRam();
    toggleSeed();
  } catch (error) {
    $("#hc-info").textContent = String(error);
  }
}

function toggleSeed() {
  $("#hc-seed-box").classList.toggle("hidden", !!$("#hc-world").value);
}

function updateHostCreateRam() {
  const info = host.prepared;
  const value = Number($("#hc-ram").value);
  $("#hc-ram-value").textContent = `${value} GB`;
  if (info) $("#hc-plan").innerHTML = ramPlan(info.totalMb, value, info.gameRam);
}

async function createHosted() {
  if (host.creating) return;
  const options = {
    instance: $("#hc-instance").value,
    name: $("#hc-name").value.trim(),
    world: $("#hc-world").value || null,
    seed: $("#hc-seed").value.trim(),
    ramGb: Number($("#hc-ram").value),
    jvmArgs: $("#hc-jvm").value.split(/\s+/).filter(Boolean),
    eula: $("#hc-eula").checked,
    gamemode: $("#hc-gamemode").value,
    difficulty: $("#hc-difficulty").value,
    maxPlayers: Math.max(1, Math.min(100, Number($("#hc-max").value) || 10)),
    pvp: $("#hc-pvp").checked,
  };
  host.creating = true;
  $("#hc-create").disabled = true;
  $("#hc-progress").classList.remove("hidden");
  $("#hc-status").textContent = t("Preparando el servidor…", "Preparing the server…");
  try {
    const id = await invoke("crear_servidor", { opciones: options });
    closeModal("#host-create-dialog");
    await loadHosting();
    toast(t("Servidor creado.", "Server created."));
    navigate("hosted", id);
  } catch (error) {
    toast(String(error), "error");
    $("#hc-progress").classList.add("hidden");
  } finally {
    host.creating = false;
    $("#hc-create").disabled = false;
  }
}

// ---------- Página de un servidor ----------
async function enterHosted(id) {
  const changed = host.id !== id;
  host.id = id;
  if (!host.view) await loadHosting();
  if (changed) {
    host.history = [];
    selectHostedTab("console");
    try {
      host.console = await invoke("consola_servidor", { id });
    } catch {
      host.console = [];
    }
    renderConsole(true);
  }
  renderHosted();
}

function hostedTitle(id) {
  return hostServer(id)?.name ?? t("Servidor", "Server");
}

function renderHosted() {
  const s = hostServer(host.id);
  if (!s) return;
  const names = stateNames();
  $("#hosted-name").textContent = s.name;
  $("#crumb-text").textContent = s.name;
  const meta = [
    `<span class="state-pill ${s.state}">${escapeHtml(names[s.state])}</span>`,
    `<span>${icon("cubes")}${escapeHtml(s.instanceName)}</span>`,
    `<span>${escapeHtml(`${s.loader} ${s.minecraft}`)}</span>`,
    `<span>${icon("puzzle")}${escapeHtml(plural(s.mods, "mod", "mods"))}</span>`,
  ];
  if (s.startupSeconds) meta.push(`<span>${icon("clock")}${t(`Tarda ~${s.startupSeconds} s en encender`, `Takes ~${s.startupSeconds} s to start`)}</span>`);
  $("#hosted-meta").innerHTML = meta.join("");
  const on = s.state !== "off";
  $("#hosted-start").classList.toggle("hidden", on);
  $("#hosted-stop").classList.toggle("hidden", !on);
  $("#hosted-stop").disabled = s.state === "stopping";
  $("#hosted-command").disabled = !on;
  $("#hosted-send").disabled = !on;

  // Dirección
  const p = host.view.playit;
  let address;
  if (s.address) {
    address = `<div class="address-card on">${icon("globe")}<div class="grow"><span class="faint">${t("Dirección para tus amigos", "Address for your friends")}</span><b>${escapeHtml(s.address)}</b></div><button class="btn small" data-copy-address="${escapeHtml(s.address)}">${icon("copy")}${t("Copiar", "Copy")}</button></div>`;
  } else if (s.state === "ready" && p.target && p.target !== s.id && p.state === "on") {
    address = `<div class="address-card">${icon("info")}<div class="grow"><span class="faint">${t("Dirección pública", "Public address")}</span><b>${t("La está usando otro servidor encendido", "Another running server is using it")}</b></div></div>`;
  } else if (!p.connected) {
    address = `<div class="address-card">${icon("globe")}<div class="grow"><span class="faint">${t("Dirección pública", "Public address")}</span><b>${t("Conecta playit.gg en «Hostear partida»", "Connect playit.gg in «Host a game»")}</b></div><button class="btn small" id="hosted-go-connect">${t("Ir", "Go")}</button></div>`;
  } else if (on) {
    address = `<div class="address-card">${p.state === "error" ? icon("alert") : `<div class="spinner small"></div>`}<div class="grow"><span class="faint">${t("Dirección pública", "Public address")}</span><b>${escapeHtml(p.state === "error" ? (p.message ?? "") : t("Se abre cuando el servidor termine de encender…", "It opens when the server finishes starting…"))}</b></div></div>`;
  } else {
    address = `<div class="address-card">${icon("globe")}<div class="grow"><span class="faint">${t("Dirección pública", "Public address")}</span><b>${escapeHtml(p.address ?? t("Se crea al encender", "Created when you start it"))}</b></div></div>`;
  }
  if (s.lan) address += `<div class="address-card small">${icon("server")}<div class="grow"><span class="faint">${t("Misma red (LAN)", "Same network (LAN)")}</span><b>${escapeHtml(s.lan)}</b></div><button class="circle-btn small" data-copy-address="${escapeHtml(s.lan)}" title="${t("Copiar", "Copy")}">${icon("copy")}</button></div>`;
  $("#hosted-address").innerHTML = address;

  // Cifras
  const memoryLimit = s.ramGb * 1024;
  const memoryShare = on ? Math.min(100, (s.memoryMb / memoryLimit) * 100) : 0;
  $("#hosted-stats").innerHTML = `
    <div class="stat"><span class="faint">${t("Jugadores", "Players")}</span><b>${on ? `${s.players.length} / ${s.maxPlayers}` : "—"}</b></div>
    <div class="stat"><span class="faint">${t("Encendido hace", "On for")}</span><b>${s.started && on ? uptime(s.started) : "—"}</b></div>
    <div class="stat"><span class="faint">${t("Procesador", "CPU")}</span><b>${on ? `${Math.round(s.cpu)}%` : "—"}</b></div>
    <div class="stat wide"><span class="faint">${t("Memoria del servidor", "Server memory")}</span><b>${on ? `${gb(s.memoryMb)} / ${s.ramGb} GB` : `${s.ramGb} GB`}</b>
      <div class="meter"><div style="width:${memoryShare.toFixed(0)}%"></div></div>
      <span class="faint">${t(`Libre en el PC: ${gb(host.view.freeMb)} de ${gb(host.view.totalMb)} GB`, `Free on the PC: ${gb(host.view.freeMb)} of ${gb(host.view.totalMb)} GB`)}</span></div>`;

  // Problemas de la última vez (con sus arreglos)
  const problem = $("#hosted-problem");
  if (s.error || s.diagnosis?.findings?.length) {
    const findings = findingsHtml(s.diagnosis?.findings ?? [], { attr: "data-server-fix" });
    problem.innerHTML = `<div class="host-problem">${s.error ? `<div class="host-problem-head">${icon("alert")}<b>${escapeHtml(s.error)}</b></div>` : ""}<ol class="doc-findings">${findings}</ol></div>`;
  } else {
    problem.innerHTML = "";
  }
  if (host.tab === "players") renderGuests();
  $("#hs-restart").classList.toggle("hidden", !s.restartNeeded);
}

function selectHostedTab(tab) {
  host.tab = tab;
  for (const button of $$("#hosted-tabs .tab")) button.classList.toggle("active", button.dataset.hostedTab === tab);
  for (const name of ["console", "players", "settings"]) $(`#hosted-tab-${name}`).classList.toggle("hidden", name !== tab);
  if (tab === "players") renderGuests();
  if (tab === "settings") loadHostedSettings();
  if (tab === "console") renderConsole(true);
}

// ---------- Consola ----------
function consoleLine(line) {
  const css = /\/(ERROR|FATAL)\]|Exception|^\s+at /.test(line)
    ? "l-error"
    : /\/WARN\]/.test(line)
      ? "l-warn"
      : /^\[Parabellum\]/.test(line)
        ? "l-parabellum"
        : /^> /.test(line)
          ? "l-command"
          : /joined the game|left the game|<[A-Za-z0-9_]{1,16}>/.test(line)
            ? "l-chat"
            : /\/DEBUG\]/.test(line)
              ? "l-dim"
              : "";
  return css ? `<span class="${css}">${escapeHtml(line)}</span>` : escapeHtml(line);
}

function renderConsole(toEnd = false) {
  const box = $("#hosted-console");
  const s = hostServer(host.id);
  box.innerHTML = host.console.length
    ? host.console.map(consoleLine).join("\n")
    : `<span class="l-dim">${s?.state === "off" ? t("El servidor está apagado. Pulsa «Encender»: aquí verás todo lo que hace.", "The server is off. Press «Start»: you will see everything it does here.") : t("Esperando al servidor…", "Waiting for the server…")}</span>`;
  if (toEnd) box.scrollTop = box.scrollHeight;
}

function appendConsole(lines) {
  const box = $("#hosted-console");
  const atEnd = box.scrollHeight - box.scrollTop - box.clientHeight < 40;
  const wasEmpty = !host.console.length;
  host.console.push(...lines);
  if (host.console.length > CONSOLE_MAX) {
    host.console.splice(0, host.console.length - CONSOLE_MAX);
    if (state.page === "hosted" && host.tab === "console") renderConsole(atEnd);
    return;
  }
  if (state.page !== "hosted" || host.tab !== "console") return;
  if (wasEmpty) return renderConsole(true);
  box.insertAdjacentHTML("beforeend", "\n" + lines.map(consoleLine).join("\n"));
  if (atEnd) box.scrollTop = box.scrollHeight;
}

async function sendHostedCommand() {
  const input = $("#hosted-command");
  const text = input.value.trim();
  if (!text || !host.id) return;
  host.history.unshift(text);
  host.historyAt = -1;
  input.value = "";
  try {
    await invoke("comando_servidor", { id: host.id, texto: text });
  } catch (error) {
    toast(String(error), "error");
  }
}

// ---------- Invitados ----------
function guestRow(name, extra, button) {
  return `<div class="guest"><canvas width="8" height="8" class="head" data-head="${escapeHtml(name)}"></canvas><div class="grow"><b>${escapeHtml(name)}</b>${extra ? `<span>${escapeHtml(extra)}</span>` : ""}</div>${button}</div>`;
}

function renderGuests() {
  const s = hostServer(host.id);
  if (!s) return;
  const host_name = state.settings.username;
  $("#hosted-guests").innerHTML =
    guestRow(host_name, t("Tú · administrador", "You · admin"), "") +
    s.guests
      .map((g, i) =>
        guestRow(
          g.name,
          g.uid ? t("Amigo de Parabellum: ve la dirección solo", "Parabellum friend: sees the address on their own") : t("Pásale la dirección tú", "Send them the address yourself"),
          `<button class="circle-btn small" data-guest-remove="${i}" title="${t("Quitar invitación", "Remove invitation")}">${icon("close")}</button>`,
        ),
      )
      .join("");
  const friends = state.social?.friends?.friends ?? [];
  const invited = new Set(s.guests.map((g) => g.uid).filter(Boolean));
  const available = friends.filter((f) => !invited.has(f.uid) && f.name !== "…");
  $("#hosted-friends").innerHTML = state.social?.signedIn
    ? available.length
      ? `<span class="label">${t("Invitar a tus amigos de Parabellum", "Invite your Parabellum friends")}</span><div class="friend-chips">${available
          .map((f) => `<button class="chip-btn" data-invite-friend="${escapeHtml(f.uid)}"><canvas width="8" height="8" class="head" data-head="${escapeHtml(f.name)}"></canvas>${escapeHtml(f.name)}${icon("plus")}</button>`)
          .join("")}</div>`
      : friends.length
        ? ""
        : `<p class="muted small-text">${t("Cuando tengas amigos en Parabellum, aparecerán aquí para invitarlos con un clic.", "When you have friends on Parabellum, they will show up here to invite them with one click.")}</p>`
    : `<p class="muted small-text">${t("Inicia sesión con Google (panel de amigos) para invitar a tus amigos de Parabellum: verán la dirección aunque cambie.", "Sign in with Google (friends panel) to invite your Parabellum friends: they will see the address even if it changes.")}</p>`;
  $("#hosted-online").innerHTML = s.players.length
    ? s.players.map((name) => guestRow(name, "", `<button class="circle-btn small" data-player-menu="${escapeHtml(name)}" title="${t("Más", "More")}">${icon("more")}</button>`)).join("")
    : `<p class="muted small-text">${s.state === "ready" ? t("Nadie conectado todavía.", "Nobody connected yet.") : t("El servidor está apagado.", "The server is off.")}</p>`;
  for (const canvas of $$("#hosted-tab-players canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
}

async function saveGuests(guests) {
  try {
    const warning = await invoke("invitados_servidor", { id: host.id, invitados: guests });
    if (warning) toast(warning, "error");
    await loadHosting();
  } catch (error) {
    toast(String(error), "error");
  }
}

function addGuestByName() {
  const s = hostServer(host.id);
  const input = $("#hosted-guest-name");
  const name = input.value.trim();
  if (!s || !name) return;
  if (!/^[A-Za-z0-9_]{1,16}$/.test(name)) {
    toast(t("Un nombre de Minecraft lleva de 1 a 16 letras, números o _.", "A Minecraft name has 1 to 16 letters, numbers or _."), "error");
    return;
  }
  input.value = "";
  // Si es un amigo de Parabellum, se invita como amigo (verá la dirección solo).
  const friend = state.social?.friends?.friends.find((f) => f.name.toLowerCase() === name.toLowerCase());
  saveGuests([...s.guests, { name: friend?.name ?? name, uid: friend?.uid ?? null }]);
}

function playerMenu(anchor, name) {
  const send = (command) => invoke("comando_servidor", { id: host.id, texto: command }).catch((e) => toast(String(e), "error"));
  contextMenu(anchor, [
    { icon: "sparkle", label: t("Darle permisos de administrador", "Give admin permissions"), action: () => send(`op ${name}`) },
    { icon: "close", label: t("Quitarle los permisos", "Remove permissions"), action: () => send(`deop ${name}`) },
    { icon: "stop", label: t("Echarlo del servidor", "Kick from the server"), danger: true, separator: true, action: () => send(`kick ${name}`) },
  ]);
}

// ---------- Opciones del servidor (como en Aternos) ----------
async function loadHostedSettings() {
  const s = hostServer(host.id);
  if (!s) return;
  const id = host.id;
  try {
    const options = await invoke("opciones_servidor", { id });
    if (host.id !== id) return;
    host.options = options;
    renderHostedOptions();
  } catch (error) {
    toast(String(error), "error");
  }
  $("#hs-restart").classList.toggle("hidden", !s.restartNeeded);
  const left = s.leftOut ?? [];
  $("#hs-left-box").classList.toggle("hidden", !left.length);
  $("#hs-left-title").textContent = t(`Mods que no van en el servidor (${left.length})`, `Mods that do not go on the server (${left.length})`);
  $("#hs-left").innerHTML = left.map((l) => `<li><b>${escapeHtml(l.file)}</b><span>${escapeHtml(leftOutReason(l))}</span></li>`).join("");
}

/** Los colores de Minecraft (§0 … §f). */
const MC_COLORS = {
  0: "#000000", 1: "#0000AA", 2: "#00AA00", 3: "#00AAAA", 4: "#AA0000", 5: "#AA00AA", 6: "#FFAA00", 7: "#AAAAAA",
  8: "#555555", 9: "#5555FF", a: "#55FF55", b: "#55FFFF", c: "#FF5555", d: "#FF55FF", e: "#FFFF55", f: "#FFFFFF",
};
const MC_STYLES = { l: "mc-bold", o: "mc-italic", n: "mc-underline", m: "mc-strike" };

/** El MOTD con sus códigos de color y formato, como se ve en la lista de servidores. */
function motdHtml(text) {
  let color = null;
  let styles = [];
  let out = "";
  let run = "";
  const flush = () => {
    if (!run) return;
    out += `<span${color ? ` style="color:${color}"` : ""}${styles.length ? ` class="${styles.join(" ")}"` : ""}>${escapeHtml(run)}</span>`;
    run = "";
  };
  for (let i = 0; i < text.length; i++) {
    const code = text[i] === "§" ? (text[i + 1] ?? "").toLowerCase() : "";
    if (code && (code in MC_COLORS || code in MC_STYLES || code === "r" || code === "k")) {
      flush();
      if (code in MC_COLORS) {
        color = MC_COLORS[code];
        styles = [];
      } else if (code === "r") {
        color = null;
        styles = [];
      } else if (code in MC_STYLES && !styles.includes(MC_STYLES[code])) {
        styles.push(MC_STYLES[code]);
      }
      i++;
      continue;
    }
    run += text[i];
  }
  flush();
  return out;
}

function optionSource(source) {
  const at = source.startsWith("gamerule ") ? source.lastIndexOf(" ") : source.indexOf("=");
  if (at < 0) return `<code>${escapeHtml(source)}</code>`;
  return `<code>${escapeHtml(source.slice(0, at + 1))}<b>${escapeHtml(source.slice(at + 1))}</b></code>`;
}

function optionControl(o) {
  const label = escapeHtml(o.label);
  switch (o.kind) {
    case "bool":
      return `<label class="toggle big"><input type="checkbox" data-opt-input aria-label="${label}" ${o.value ? "checked" : ""}><span></span></label>`;
    case "number":
      return `<div class="stepper">
        <button data-opt-step="-1" aria-label="${t("Menos", "Less")}">${icon("minus")}</button>
        <input type="number" data-opt-input aria-label="${label}" min="${o.min}" max="${o.max}" value="${o.value}">
        <button data-opt-step="1" aria-label="${t("Más", "More")}">${icon("plus")}</button>
      </div>`;
    case "choice":
      return `<select data-opt-input aria-label="${label}">${o.choices
        .map(([value, name]) => `<option value="${escapeHtml(value)}" ${value === o.value ? "selected" : ""}>${escapeHtml(name)}</option>`)
        .join("")}</select>`;
    case "text":
      return `<input data-opt-input aria-label="${label}" maxlength="${o.max}" value="${escapeHtml(o.value)}" placeholder="${escapeHtml(o.id === "jvm" ? o.help : t("Vacío", "Empty"))}" autocomplete="off" spellcheck="false">`;
    case "ram":
      return `<span class="value-inline" data-ram-value>${o.value} GB</span>
        <div class="opt-ram"><input type="range" data-opt-input aria-label="${label}" min="${o.min}" max="${o.max}" step="1" value="${o.value}"><div class="ram-plan" data-ram-plan></div></div>`;
    default:
      return "";
  }
}

function optionHtml(o) {
  const wide = o.kind === "text" || o.kind === "ram";
  const help = o.help && o.id !== "jvm" ? ` title="${escapeHtml(o.help)}"` : "";
  return `<div class="opt ${o.kind} ${wide ? "wide" : ""}" data-opt="${escapeHtml(o.id)}"${help}>
    <div class="opt-main"><span class="opt-label">${escapeHtml(o.label)}</span>${optionControl(o)}</div>
    <div class="opt-key">${optionSource(o.source)}</div>
  </div>`;
}

function renderHostedOptions() {
  const view = host.options;
  if (!view) return;
  $("#hs-icon").innerHTML = view.icon ? `<img src="${view.icon}" alt="">` : icon("server");
  $("#hs-name-view").textContent = view.name;
  if (!host.editingMotd) showMotd(view.motd);
  $("#hs-options").innerHTML = view.groups
    .map((g) => `<section class="opt-group"><h3>${escapeHtml(g.title)}</h3><div class="opt-grid">${g.options.map(optionHtml).join("")}</div></section>`)
    .join("");
  updateOptionRam();
}

function showMotd(text) {
  const view = $("#hs-motd-view");
  view.dataset.empty = t("Sin mensaje", "No message");
  view.innerHTML = motdHtml(text ?? "");
}

function findOption(id) {
  for (const group of host.options?.groups ?? []) {
    const index = group.options.findIndex((o) => o.id === id);
    if (index >= 0) return { group, index, option: group.options[index] };
  }
  return null;
}

function updateOptionRam() {
  const card = $('#hs-options [data-opt="ram"]');
  if (!card) return;
  const value = Number(card.querySelector("[data-opt-input]").value);
  card.querySelector("[data-ram-value]").textContent = `${value} GB`;
  const s = hostServer(host.id);
  const instance = state.instances.find((i) => i.id === s?.instance);
  card.querySelector("[data-ram-plan]").innerHTML = ramPlan(host.view.totalMb, value, instance?.ramGb ?? state.settings.ramGb);
}

function optionValue(card) {
  const found = findOption(card.dataset.opt);
  const input = card.querySelector("[data-opt-input]");
  if (!found || !input) return null;
  switch (found.option.kind) {
    case "bool":
      return input.checked;
    case "number":
    case "ram": {
      const { min, max } = found.option;
      const number = Math.round(Number(input.value));
      return Number.isFinite(number) ? Math.max(min, Math.min(max, number)) : found.option.value;
    }
    default:
      return input.value;
  }
}

/** Guarda una opción y deja ver qué pasó (guardado, ya aplicado o al reiniciar). */
async function commitOption(id, value) {
  const serverId = host.id;
  try {
    const result = await invoke("cambiar_opcion_servidor", { id: serverId, opcion: id, valor: value });
    if (host.id !== serverId) return result;
    const card = $(`#hs-options [data-opt="${CSS.escape(id)}"]`);
    if (result.option) {
      const found = findOption(id);
      if (found) found.group.options[found.index] = result.option;
      if (card) {
        card.querySelector(".opt-key").innerHTML = optionSource(result.option.source);
        // El valor que quedó (p. ej. un número fuera de rango se ajusta).
        const input = card.querySelector("[data-opt-input]");
        if (input && result.option.kind === "number") input.value = result.option.value;
      }
    }
    if (card) {
      const note = result.applied ? t("Aplicado", "Applied") : result.needsRestart ? t("Al reiniciar", "On restart") : t("Guardado", "Saved");
      card.querySelector(".opt-key").insertAdjacentHTML("beforeend", `<span class="opt-note">${note}</span>`);
      card.classList.add("flash");
      clearTimeout(card.flashTimer);
      card.flashTimer = setTimeout(() => {
        card.classList.remove("flash");
        card.querySelector(".opt-note")?.remove();
      }, 1800);
    }
    return result;
  } catch (error) {
    toast(String(error), "error");
    loadHostedSettings();
    return null;
  }
}

function scheduleOption(card, delay) {
  clearTimeout(card.commitTimer);
  card.commitTimer = setTimeout(() => {
    const value = optionValue(card);
    const found = findOption(card.dataset.opt);
    if (value === null || !found || value === found.option.value) return;
    commitOption(card.dataset.opt, value);
  }, delay);
}

function editServerName() {
  const input = $("#hs-name-input");
  input.value = host.options?.name ?? "";
  $("#hs-name-view").classList.add("hidden");
  $("#hs-name-edit").classList.add("hidden");
  input.classList.remove("hidden");
  input.focus();
  input.select();
}

async function finishServerName(save) {
  const input = $("#hs-name-input");
  if (input.classList.contains("hidden")) return;
  input.classList.add("hidden");
  $("#hs-name-view").classList.remove("hidden");
  $("#hs-name-edit").classList.remove("hidden");
  const name = input.value.trim();
  if (!save || !name || name === host.options?.name) return;
  $("#hs-name-view").textContent = name;
  if (await commitOption("name", name)) host.options.name = name;
  else $("#hs-name-view").textContent = host.options?.name ?? "";
}

function motdTools() {
  const swatches = Object.entries(MC_COLORS)
    .map(([code, color]) => `<button class="swatch" data-code="${code}" style="background:${color}" title="§${code}"></button>`)
    .join("");
  const styles = [
    ["l", "<b>B</b>", t("Negrita", "Bold")],
    ["o", "<i>I</i>", t("Cursiva", "Italic")],
    ["n", "<u>U</u>", t("Subrayado", "Underline")],
    ["m", "<s>S</s>", t("Tachado", "Strikethrough")],
    ["r", "↺", t("Quitar formato", "Reset format")],
  ]
    .map(([code, label, title]) => `<button data-code="${code}" title="${escapeHtml(title)} (§${code})">${label}</button>`)
    .join("");
  return `${swatches}<span class="sep"></span>${styles}`;
}

function editMotd() {
  host.editingMotd = true;
  const input = $("#hs-motd-input");
  input.value = host.options?.motd ?? "";
  $("#hs-motd-tools").innerHTML = motdTools();
  $("#hs-motd-editor").classList.remove("hidden");
  input.focus();
}

function closeMotd() {
  host.editingMotd = false;
  $("#hs-motd-editor").classList.add("hidden");
  showMotd(host.options?.motd ?? "");
}

function motdInput() {
  const input = $("#hs-motd-input");
  // Dos renglones, como en la lista de servidores.
  const lines = input.value.split("\n");
  if (lines.length > 2) input.value = lines.slice(0, 2).join("\n");
  showMotd(input.value);
}

function insertMotdCode(code) {
  const input = $("#hs-motd-input");
  const [start, end] = [input.selectionStart, input.selectionEnd];
  const text = `§${code}`;
  input.value = input.value.slice(0, start) + text + input.value.slice(end);
  input.focus();
  input.selectionStart = input.selectionEnd = start + text.length;
  motdInput();
}

async function saveMotd() {
  const motd = $("#hs-motd-input").value;
  if (await commitOption("motd", motd)) {
    host.options.motd = motd.split("\n").slice(0, 2).join("\n");
    closeMotd();
    if (hostServer(host.id)?.state !== "off") toast(t("Mensaje guardado. Se ve al reiniciar el servidor.", "Message saved. It shows after restarting the server."));
  }
}

function serverIconClicked(anchor) {
  if (!host.options?.icon) return $("#hs-icon-file").click();
  contextMenu(anchor, [
    { icon: "image", label: t("Cambiar icono", "Change icon"), action: () => $("#hs-icon-file").click() },
    { icon: "trash", label: t("Quitar icono", "Remove icon"), danger: true, action: () => saveServerIcon(null) },
  ]);
}

/** La imagen elegida, recortada al centro y reducida a 64×64 (lo que pide Minecraft). */
async function serverIconFromFile(file) {
  const url = URL.createObjectURL(file);
  try {
    const image = await new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error(t("Esa imagen no se pudo abrir.", "That image could not be opened.")));
      img.src = url;
    });
    const side = Math.min(image.naturalWidth, image.naturalHeight);
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 64;
    const context = canvas.getContext("2d");
    // Las imágenes pequeñas (pixel art) se agrandan sin difuminar.
    context.imageSmoothingEnabled = side > 64;
    context.imageSmoothingQuality = "high";
    context.drawImage(image, (image.naturalWidth - side) / 2, (image.naturalHeight - side) / 2, side, side, 0, 0, 64, 64);
    await saveServerIcon(canvas.toDataURL("image/png"));
  } catch (error) {
    toast(String(error.message ?? error), "error");
  } finally {
    URL.revokeObjectURL(url);
  }
}

async function saveServerIcon(data) {
  try {
    await invoke("icono_servidor", { id: host.id, datos: data });
    host.options.icon = data;
    renderHostedOptions();
    const on = hostServer(host.id)?.state !== "off";
    toast(data ? (on ? t("Icono guardado. Se ve al reiniciar el servidor.", "Icon saved. It shows after restarting the server.") : t("Icono guardado.", "Icon saved.")) : t("Icono quitado.", "Icon removed."));
  } catch (error) {
    toast(String(error), "error");
  }
}

function bindHostedOptions() {
  const options = $("#hs-options");
  options.addEventListener("change", (event) => {
    const card = event.target.closest("[data-opt]");
    if (card && event.target.matches("[data-opt-input]")) scheduleOption(card, 0);
  });
  options.addEventListener("input", (event) => {
    if (event.target.closest('[data-opt="ram"]')) updateOptionRam();
  });
  options.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && event.target.matches("input[data-opt-input]")) event.target.blur();
  });
  options.addEventListener("click", (event) => {
    const step = event.target.closest("[data-opt-step]");
    if (!step) return;
    const card = step.closest("[data-opt]");
    const input = card.querySelector("[data-opt-input]");
    const found = findOption(card.dataset.opt);
    if (!found) return;
    input.value = Math.max(found.option.min, Math.min(found.option.max, (Number(input.value) || 0) + Number(step.dataset.optStep)));
    // Varios clics seguidos se guardan juntos.
    scheduleOption(card, 450);
  });
  $("#hs-name-edit").onclick = () => editServerName();
  $("#hs-name-input").addEventListener("keydown", (event) => {
    if (event.key === "Enter") finishServerName(true);
    if (event.key === "Escape") {
      event.stopPropagation();
      finishServerName(false);
    }
  });
  $("#hs-name-input").addEventListener("blur", () => finishServerName(true));
  $("#hs-motd-edit").onclick = () => (host.editingMotd ? closeMotd() : editMotd());
  $("#hs-motd-input").addEventListener("input", () => motdInput());
  $("#hs-motd-tools").addEventListener("click", (event) => {
    const button = event.target.closest("[data-code]");
    if (button) insertMotdCode(button.dataset.code);
  });
  $("#hs-motd-cancel").onclick = () => closeMotd();
  $("#hs-motd-save").onclick = () => saveMotd();
  $("#hs-icon").onclick = (event) => serverIconClicked(event.currentTarget);
  $("#hs-icon-file").onchange = (event) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (file) serverIconFromFile(file);
  };
  $("#hs-restart-btn").onclick = async () => {
    try {
      await invoke("reiniciar_servidor", { id: host.id });
      toast(t("Reiniciando el servidor…", "Restarting the server…"));
    } catch (error) {
      toast(String(error), "error");
    }
  };
}

/** Por qué un mod no va en el servidor (se guarda como código para mostrarlo en el idioma elegido). */
function leftOutReason(item) {
  const id = item.detail ?? "";
  switch (item.kind) {
    case "modrinth":
      return t("Modrinth dice que es solo para el juego (no funciona en servidores)", "Modrinth says it is client-only (does not work on servers)");
    case "client":
      return t(`${id} es solo para el juego`, `${id} is client-only`);
    case "dependency":
      return t(`Necesita ${id}, que es solo para el juego`, `It needs ${id}, which is client-only`);
    case "doctor":
      return t("Lo quitó el doctor: tumbaba el servidor", "Removed by the doctor: it crashed the server");
    case "tunnel":
      return t(`Abre su propia dirección (${id}); en el servidor la dirección la da playit.gg`, `It opens its own address (${id}); on the server the address comes from playit.gg`);
    default:
      return "";
  }
}

function hostedMenu(anchor) {
  const s = hostServer(host.id);
  if (!s) return;
  const items = [
    { icon: "folder", label: t("Abrir carpeta del servidor", "Open server folder"), action: () => invoke("carpeta_servidor", { id: s.id }).catch((e) => toast(String(e), "error")) },
    {
      icon: "refresh",
      label: t("Actualizar mods desde la instancia", "Update mods from the instance"),
      sub: t("Después de actualizar el modpack", "After updating the modpack"),
      action: async () => {
        if (s.state !== "off") return toast(t("Apaga el servidor antes de actualizar sus mods.", "Stop the server before updating its mods."), "error");
        toast(t("Copiando los mods de la instancia…", "Copying the instance's mods…"));
        try {
          const count = await invoke("sincronizar_servidor", { id: s.id });
          toast(t(`Listo: el servidor tiene ${plural(count, "mod", "mods")} de la instancia.`, `Done: the server has ${plural(count, "mod", "mods")} from the instance.`));
        } catch (error) {
          toast(String(error), "error");
        }
      },
    },
  ];
  if (s.address) items.push({ icon: "copy", label: t("Copiar dirección", "Copy address"), action: () => copyText(s.address, t("Dirección copiada.", "Address copied.")) });
  items.push({
    icon: "trash",
    label: t("Eliminar servidor", "Delete server"),
    danger: true,
    separator: true,
    action: async () => {
      if (s.state !== "off") return toast(t("Apaga el servidor antes de borrarlo.", "Stop the server before deleting it."), "error");
      const ok = await confirmAction({
        title: t(`Eliminar ${s.name}`, `Delete ${s.name}`),
        text: t("La carpeta del servidor (con su mundo) va a la papelera de reciclaje: puedes recuperarla desde ahí. Tus invitados dejarán de verlo.", "The server folder (with its world) goes to the Recycle Bin: you can restore it from there. Your guests will no longer see it."),
        ok: t("Eliminar", "Delete"),
      });
      if (!ok) return;
      try {
        await invoke("eliminar_servidor", { id: s.id });
        await loadHosting();
        navigate("host");
      } catch (error) {
        toast(String(error), "error");
      }
    },
  });
  contextMenu(anchor, items);
}

async function fixHostedProblem(findingIndex, optionIndex) {
  const s = hostServer(host.id);
  const option = s?.diagnosis?.findings[findingIndex]?.fixes[optionIndex];
  if (!option) return;
  try {
    toast(await invoke("arreglar_servidor", { id: s.id, arreglo: option.fix }));
    await loadHosting();
  } catch (error) {
    toast(String(error), "error");
  }
}

// ---------- Servidores de amigos (página Servidores) ----------
function renderFriendServers() {
  const box = $("#friend-servers");
  const s = state.social;
  if (!(s?.signedIn && s.connected)) {
    setHtml(box, `<p class="muted small-text friend-servers-empty">${t("Inicia sesión con Google para ver las partidas de tus amigos.", "Sign in with Google to see your friends' games.")}</p>`);
    return;
  }
  // Encendidos primero; los servidores apagados a los que te invitaron, al final.
  const items = friendActivity().sort((a, b) => Number(b.online) - Number(a.online));
  if (!items.length) {
    setHtml(box, `<p class="muted small-text friend-servers-empty">${t("Ninguno de tus amigos está jugando ahora.", "None of your friends is playing right now.")}</p>`);
    return;
  }
  const html = items
    .map((item) => {
      const pill = (on, label) => `<span class="state-pill ${on ? "ready" : "off"}">${escapeHtml(label)}</span>`;
      let iconName, title, pillHtml, sub, button;
      if (item.kind === "hosted") {
        const sv = item.server;
        iconName = "broadcast";
        title = sv.name;
        pillHtml = pill(sv.online, sv.online ? t("Encendido", "On") : t("Apagado", "Off"));
        const status = sv.online ? (sv.address ? t(`${sv.players} de ${sv.maxPlayers} jugadores`, `${sv.players} of ${sv.maxPlayers} players`) : t("Sin dirección pública todavía", "No public address yet")) : t("Te invitó", "Invited you");
        sub = [t(`Servidor de ${sv.hostName}`, `${sv.hostName}'s server`), sv.pack, sv.minecraft && `${sv.loader ?? ""} ${sv.minecraft}`.trim(), status];
        button = `<button class="btn primary play" data-friend-join="${escapeHtml(item.key)}" ${sv.online && sv.address ? "" : "disabled"}>${icon("play")}${t("Unirse", "Join")}</button>`;
      } else {
        const f = item.friend;
        const p = f.presence;
        if (item.kind === "world") {
          iconName = "globe";
          title = t(`Mundo de ${f.name}`, `${f.name}'s world`);
          pillHtml = pill(true, t("Abierto", "Open"));
          sub = [p.pack, p.minecraft && `Minecraft ${p.minecraft}`, p.address ? t("Por Internet", "Over the Internet") : t("Solo en tu red (LAN)", "Only on your network (LAN)")];
          button = `<button class="btn primary play" data-friend-join="${escapeHtml(item.key)}">${icon("play")}${t("Unirse", "Join")}</button>`;
        } else if (item.kind === "playing") {
          iconName = "server";
          title = p.server;
          pillHtml = pill(true, t(`${f.name} está aquí`, `${f.name} is here`));
          sub = [p.pack, p.minecraft && `Minecraft ${p.minecraft}`];
          button = `<button class="btn primary play" data-friend-join="${escapeHtml(item.key)}">${icon("play")}${t("Unirse", "Join")}</button>`;
        } else {
          iconName = "broadcast";
          title = p.hosting;
          pillHtml = pill(true, t("Encendido", "On"));
          sub = [t(`Servidor de ${f.name}`, `${f.name}'s server`), t("Pídele que te invite para ver la dirección", "Ask them to invite you to see the address")];
          button = `<button class="btn play" disabled>${icon("play")}${t("Unirse", "Join")}</button>`;
        }
      }
      return `<div class="server ${item.online ? "" : "dim"}">
        <div class="server-icon">${icon(iconName)}</div>
        <div class="grow">
          <div class="title">${escapeHtml(title)} ${pillHtml}</div>
          <div class="sub">${escapeHtml(sub.filter(Boolean).join(" · "))}</div>
        </div>
        ${button}
        <button class="circle-btn" data-friend-server-menu="${escapeHtml(item.key)}" title="${t("Más", "More")}">${icon("more")}</button>
      </div>`;
    })
    .join("");
  setHtml(box, html);
}

function findActivity(key) {
  return friendActivity().find((item) => item.key === key);
}

/** Entrar a la partida de un amigo: servidor hosteado, mundo abierto o el servidor donde juega. */
function joinActivity(item) {
  if (item.kind === "hosted") return joinFriendServer(item.server);
  const p = item.friend.presence;
  if (item.kind === "world") return joinFriend(item.friend, p.address ?? p.lan);
  if (item.kind === "playing") return joinFriend(item.friend, p.server);
}

async function joinFriendServer(server) {
  const instance =
    state.instances.find((i) => i.id.toLowerCase() === (server.packFolder ?? "").toLowerCase()) ??
    state.instances.find((i) => i.name.toLowerCase() === (server.pack ?? "").toLowerCase()) ??
    (server.project ? state.instances.find((i) => i.modpackProject === server.project) : null);
  if (!instance) {
    if (server.project) {
      toast(t(`Necesitas el modpack de ${server.hostName}: instálalo y vuelve a pulsar Unirse.`, `You need ${server.hostName}'s modpack: install it and press Join again.`));
      navigate("project", server.project);
    } else {
      toast(
        t(
          `${server.hostName} usa ${server.pack} (${server.loader} ${server.minecraft}) y tú no lo tienes. Pídele que te pase el modpack (Exportar) y vuelve a pulsar Unirse.`,
          `${server.hostName} uses ${server.pack} (${server.loader} ${server.minecraft}) and you do not have it. Ask them for the modpack (Export) and press Join again.`,
        ),
        "error",
      );
    }
    return;
  }
  if (state.running.size) {
    await copyText(server.address, t(`Ya tienes Minecraft abierto. Copié la dirección: en el juego ve a Multijugador › Conexión directa y pégala.`, `Minecraft is already open. I copied the address: in the game go to Multiplayer › Direct Connection and paste it.`));
    return;
  }
  play(instance.id, server.address);
}

function friendServerMenu(anchor, item) {
  const items = [];
  const address = item.kind === "hosted" ? item.server.address : item.kind === "world" ? (item.friend.presence.address ?? item.friend.presence.lan) : item.kind === "playing" ? item.friend.presence.server : null;
  if (address) items.push({ icon: "copy", label: t("Copiar dirección", "Copy address"), action: () => copyText(address, t("Dirección copiada.", "Address copied.")) });
  if (item.friend) items.push({ icon: "chat", label: t(`Mensaje a ${item.friend.name}`, `Message ${item.friend.name}`), action: () => messageFriend(item.friend.uid) });
  if (item.kind !== "hosted") {
    if (items.length) contextMenu(anchor, items);
    return;
  }
  const server = item.server;
  items.push({
    icon: "trash",
    label: t("Quitar de mi lista", "Remove from my list"),
    danger: true,
    separator: true,
    action: async () => {
      try {
        await invoke("salir_de_servidor", { anfitrion: server.host, clave: server.key });
      } catch (error) {
        toast(String(error), "error");
      }
    },
  });
  contextMenu(anchor, items);
}

// ---------- Eventos ----------
function wireHostEvents() {
  $("#host-new-server").onclick = () => openHostCreate();
  $("#host-share-world").onchange = (event) => invoke("compartir_mundo", { activo: event.target.checked });
  document.addEventListener("click", (event) => {
    const disable = event.target.closest("[data-disable-tunnel]");
    if (disable) return void disableTunnelMod(Number(disable.dataset.disableTunnel));
    const copy = event.target.closest("[data-copy-address]");
    if (copy) {
      copyText(copy.dataset.copyAddress, t("Dirección copiada.", "Address copied."));
      return;
    }
    if (event.target.closest("#tunnel-connect")) return void connectPlayit();
    if (event.target.closest("#tunnel-cancel")) return void invoke("cancelar_playit");
    if (event.target.closest("#tunnel-reopen")) {
      const url = host.view?.playit.claimUrl;
      if (url) invoke("abrir_enlace", { url });
      return;
    }
    if (event.target.closest("#tunnel-retry")) return void invoke("reintentar_playit").catch((e) => toast(String(e), "error"));
    const menu = event.target.closest("#tunnel-menu");
    if (menu) return void tunnelMenu(menu);
    if (event.target.closest("#hosted-go-connect")) return void navigate("host");
    const start = event.target.closest("[data-host-start]");
    if (start) return void startHosted(start.dataset.hostStart);
    const stop = event.target.closest("[data-host-stop]");
    if (stop) return void stopHosted(stop.dataset.hostStop);
    const card = event.target.closest("[data-hosted]");
    if (card) return void navigate("hosted", card.dataset.hosted);
    const fix = event.target.closest("[data-server-fix]");
    if (fix) {
      const [i, j] = fix.dataset.serverFix.split(":").map(Number);
      return void fixHostedProblem(i, j);
    }
    const remove = event.target.closest("[data-guest-remove]");
    if (remove) {
      const s = hostServer(host.id);
      if (s) saveGuests(s.guests.filter((_, i) => i !== Number(remove.dataset.guestRemove)));
      return;
    }
    const invite = event.target.closest("[data-invite-friend]");
    if (invite) {
      const s = hostServer(host.id);
      const friend = state.social?.friends?.friends.find((f) => f.uid === invite.dataset.inviteFriend);
      if (s && friend) saveGuests([...s.guests, { name: friend.name, uid: friend.uid }]);
      return;
    }
    const player = event.target.closest("[data-player-menu]");
    if (player) return void playerMenu(player, player.dataset.playerMenu);
    const join = event.target.closest("[data-friend-join]");
    if (join) {
      const item = findActivity(join.dataset.friendJoin);
      if (item) joinActivity(item);
      return;
    }
    const serverMenu = event.target.closest("[data-friend-server-menu]");
    if (serverMenu) {
      const item = findActivity(serverMenu.dataset.friendServerMenu);
      if (item) friendServerMenu(serverMenu, item);
      return;
    }
    const homeAction = event.target.closest("[data-home-action]");
    if (homeAction) {
      const action = homeAction.dataset.homeAction;
      if (action === "create-server") {
        navigate("host");
        openHostCreate();
      } else if (action === "host") navigate("host");
      else if (action === "servers") navigate("servers");
      else if (action === "sign-in") signInGoogle();
    }
  });
  $("#host-servers").addEventListener("keydown", (event) => {
    const card = event.target.closest("[data-hosted]");
    if (card && event.target === card && (event.key === "Enter" || event.key === " ")) {
      event.preventDefault();
      navigate("hosted", card.dataset.hosted);
    }
  });

  // Página del servidor
  $("#hosted-start").onclick = () => startHosted(host.id);
  $("#hosted-stop").onclick = () => stopHosted(host.id);
  $("#hosted-more").onclick = (event) => hostedMenu(event.currentTarget);
  for (const button of $$("#hosted-tabs .tab")) button.onclick = () => selectHostedTab(button.dataset.hostedTab);
  $("#hosted-send").onclick = () => sendHostedCommand();
  $("#hosted-command").onkeydown = (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      sendHostedCommand();
    } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      if (!host.history.length) return;
      event.preventDefault();
      host.historyAt = Math.max(-1, Math.min(host.history.length - 1, host.historyAt + (event.key === "ArrowUp" ? 1 : -1)));
      event.target.value = host.historyAt < 0 ? "" : host.history[host.historyAt];
    }
  };
  $("#hosted-guest-add").onclick = () => addGuestByName();
  $("#hosted-guest-name").onkeydown = (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      addGuestByName();
    }
  };
  bindHostedOptions();

  // Crear servidor
  $("#hc-close").onclick = () => closeModal("#host-create-dialog");
  $("#hc-cancel").onclick = () => closeModal("#host-create-dialog");
  $("#hc-instance").onchange = () => prepareHostCreate();
  $("#hc-world").onchange = () => toggleSeed();
  $("#hc-ram").oninput = () => updateHostCreateRam();
  $("#hc-create").onclick = () => createHosted();
  $("#hc-eula-link").onclick = (event) => {
    event.preventDefault();
    invoke("abrir_enlace", { url: EULA_URL });
  };

  // EULA al encender
  $("#he-close").onclick = () => closeModal("#host-eula-dialog");
  $("#he-cancel").onclick = () => closeModal("#host-eula-dialog");
  $("#he-link").onclick = () => invoke("abrir_enlace", { url: EULA_URL });
  $("#he-accept").onchange = (event) => {
    $("#he-ok").disabled = !event.target.checked;
  };
  $("#he-ok").onclick = () => acceptEulaAndStart();

  listen("hosting", ({ payload }) => applyHostView(payload));
  listen("servidor-consola", ({ payload }) => {
    if (payload.id === host.id) appendConsole(payload.lines);
  });
  // Cada minuto se repinta el "encendido hace".
  setInterval(() => {
    if (state.page === "hosted") renderHosted();
  }, 60000);
}

wireHostEvents();
loadHosting();
