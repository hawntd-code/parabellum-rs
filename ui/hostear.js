"use strict";

// ======================================================================
// Hostear partida: servidores propios (consola, invitados, ajustes, conexiones). Tus amigos entran
// con «Unirse» por P2P o red local; playit.gg (opcional, por servidor) da una dirección pública fija.
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
          `No alcanza: con ${Math.round(total)} GB, jugar y tener el servidor a la vez va a ir lento o cerrarse. Baja la memoria del servidor o, si juegas tú también, abre tu mundo en LAN (Pausa › Abrir en LAN: un solo Java en vez de dos).`,
          `Not enough: with ${Math.round(total)} GB, playing and running the server at the same time will be slow or crash. Lower the server memory or, if you play too, open your world to LAN (Pause › Open to LAN: one Java instead of two).`,
        )
      : level === "tight"
        ? t(
            `Justo: alcanza si cierras el navegador y otros programas mientras juegas. En un PC de ${Math.round(total)} GB, para modpacks grandes conviene abrir tu mundo en LAN.`,
            `Tight: it works if you close the browser and other programs while playing. On a ${Math.round(total)} GB PC, opening your world to LAN is better for big modpacks.`,
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
  $("#world-playit").checked = !!view.worldPlayit;
  if (state.page === "host") renderHost();
  if (state.page === "hosted") renderHosted();
  updateServersChanged();
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
  const show = list.length > 0;
  box.classList.toggle("hidden", !show);
  if (!show) return;
  box.innerHTML = `${icon("alert")}<div class="grow">
      <b>${t("Tus instancias todavía tienen e4mc", "Your instances still have e4mc")}</b>
      <p>${t(
        "Parabellum ya comparte tu mundo con tus amigos: e4mc sobra y, en Minecraft 26.2, cierra tu mundo en cuanto entra un amigo. Desactívalo (puedes volver a activarlo en Contenido).",
        "Parabellum already shares your world with your friends: e4mc is not needed and, on Minecraft 26.2, closes your world as soon as a friend joins. Disable it (you can turn it back on in Content).",
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

// ---------- Conexiones de un servidor: P2P, red local y playit.gg ----------
function connectionsOf(s) {
  return { p2p: true, lan: true, playit: false, ...(s.connections ?? {}) };
}

function connCard(key, iconName, title, on, body, extra = "") {
  return `<div class="conn-card ${on ? "on" : ""}">
      <div class="conn-head">
        <span class="conn-icon">${icon(iconName)}</span>
        <b class="grow">${escapeHtml(title)}</b>
        ${extra}
        <label class="toggle" title="${on ? t("Desactivar", "Turn off") : t("Activar", "Turn on")}"><input type="checkbox" data-connection="${key}" ${on ? "checked" : ""}><span></span></label>
      </div>
      <div class="conn-body">${body}</div>
    </div>`;
}

const connNote = (text, css = "muted") => `<span class="${css}">${escapeHtml(text)}</span>`;

function connectionsHtml(s) {
  const c = connectionsOf(s);
  const off = connNote(t("Desactivada", "Off"));
  const p2p = !c.p2p
    ? off
    : !state.social?.signedIn
      ? connNote(t("Inicia sesión con Google para que tus amigos entren directo.", "Sign in with Google so your friends can join directly."))
      : !host.view.p2p
        ? connNote(t("Esperando internet…", "Waiting for the internet…"))
        : s.p2pGuests?.length
          ? p2pGuestsHtml(s.p2pGuests)
          : connNote(t("Tus amigos de Parabellum entran directo con «Unirse».", "Your Parabellum friends join directly with «Join»."));
  const lan = !c.lan
    ? off
    : host.firewall === false
      ? `${connNote(t("Windows no deja entrar a quien está en tu red.", "Windows does not let people on your network in."))}<button class="btn small primary" id="hosted-firewall">${t("Permitir", "Allow")}</button>`
      : s.lan
        ? `<span class="conn-address">${escapeHtml(s.lan)}</span>`
        : connNote(t("Para quien está en tu casa o en tu escuela.", "For whoever is at your home or school."));
  const menu = host.view.playit.connected ? `<button class="circle-btn small" id="tunnel-menu" title="${t("Más", "More")}">${icon("more")}</button>` : "";
  return (
    connCard("p2p", "broadcast", "P2P", c.p2p, p2p) +
    connCard("lan", "server", t("Red local", "Local network"), c.lan, lan) +
    connCard("playit", "globe", "playit.gg", c.playit, c.playit ? playitBody(s) : connNote(t("Dirección pública para quien no usa Parabellum.", "Public address for people without Parabellum.")), menu)
  );
}

/** Debajo del TPS: cuánto tarda cada tick y, si en el último minuto hubo un trabón, de cuánto fue. */
function tickLine(s, on) {
  if (!on || s.mspt == null) return "";
  const hitch = s.hitchMs != null ? ` · <span class="tps-warn">${t(`trabón de ${Math.round(s.hitchMs)} ms`, `${Math.round(s.hitchMs)} ms hitch`)}</span>` : "";
  return `<span class="faint">${t(`${s.mspt.toFixed(1)} ms por tick`, `${s.mspt.toFixed(1)} ms per tick`)}${hitch}</span>`;
}

/** Quién está jugando por P2P ahora: directo entre las dos PCs o por el relevo, y su ping. */
function p2pGuestsHtml(guests) {
  return `<div class="p2p-guests">${guests
    .map((g) => {
      const path = g.direct ? t("directo", "direct") : t("por relevo", "via relay");
      const ping = g.rttMs != null ? ` · ${g.rttMs} ms` : "";
      // Lo que se nota como saltos: picos de ping o paquetes perdidos en el último minuto.
      const peak = g.peakMs != null && g.rttMs != null && g.peakMs > Math.max(g.rttMs * 2, g.rttMs + 40) ? ` · ${t(`picos de ${g.peakMs} ms`, `spikes of ${g.peakMs} ms`)}` : "";
      const loss = g.lossPct >= 1 ? ` · ${t(`pierde ${g.lossPct.toFixed(0)} %`, `${g.lossPct.toFixed(0)} % loss`)}` : "";
      const shaky = peak || loss ? " shaky" : "";
      return `<span class="p2p-guest ${g.direct ? "direct" : "relay"}${shaky}"><b>${escapeHtml(g.name)}</b> · ${path}${ping}${peak}${loss}</span>`;
    })
    .join("")}</div>`;
}

/** playit.gg activado en este servidor: vincular, abrir la dirección o lo que falte. */
function playitBody(s) {
  const p = host.view.playit;
  const spinner = `<div class="spinner small"></div>`;
  if (p.claimUrl) {
    // Los pasos los dice playit.gg: si alguien inicia sesión y su página lo deja en otra parte, aquí se
    // ve que falta volver a la página de vincular.
    const steps = {
      visit: t("Abre la página de playit.gg. Si te pidió iniciar sesión, al terminar pulsa «Abrir de nuevo».", "Open the playit.gg page. If it asked you to sign in, when you finish press «Open again»."),
      accept: t("Pulsa «Add agent» (o «Accept») en la página de playit.gg.", "Press «Add agent» (or «Accept») on the playit.gg page."),
      finishing: p.message ?? t("Terminando de conectar…", "Finishing the connection…"),
    };
    const buttons =
      p.claimStep === "finishing"
        ? ""
        : `<button class="btn small" id="tunnel-reopen">${icon("external")}${t("Abrir de nuevo", "Open again")}</button><button class="btn small" id="tunnel-cancel">${t("Cancelar", "Cancel")}</button>`;
    return `${spinner}${connNote(steps[p.claimStep] ?? steps.visit)}${buttons}`;
  }
  if (!p.connected) {
    if (host.connecting) return `${spinner}${connNote(p.message ?? t("Preparando playit.gg…", "Preparing playit.gg…"))}`;
    const failed = p.state === "error" && p.message;
    return `${failed ? connNote(p.message, "tunnel-error") : connNote(t("Vincúlalo una vez con tu cuenta de playit.gg.", "Link it once with your playit.gg account."))}<button class="btn small primary" id="tunnel-connect">${t("Vincular", "Link")}</button>`;
  }
  if (s.address) return `<span class="conn-address">${escapeHtml(s.address)}</span><button class="circle-btn small" data-copy-address="${escapeHtml(s.address)}" title="${t("Copiar dirección", "Copy address")}">${icon("copy")}</button>`;
  const on = s.state !== "off";
  if (s.state === "ready" && p.target && p.target !== s.id && p.state === "on") return connNote(t("La está usando otro servidor encendido.", "Another running server is using it."));
  if (on && p.state === "verify") return `${connNote(t("Verifica tu correo en playit.gg.", "Verify your email on playit.gg."), "tunnel-error")}<button class="btn small primary" data-playit-retry>${t("Ya lo verifiqué", "I verified it")}</button>`;
  if (on && p.state === "error") return `${connNote(p.message ?? "", "tunnel-error")}<button class="btn small" data-playit-retry>${t("Reintentar", "Retry")}</button>`;
  if (on) return `${spinner}${connNote(t("Se abre cuando el servidor termine de encender…", "It opens when the server finishes starting…"))}`;
  return connNote(p.address ? t(`Se abre al encender: ${p.address}`, `Opens when started: ${p.address}`) : t("Se crea al encender.", "Created when you start it."));
}

async function setConnection(key, on) {
  const s = hostServer(host.id);
  if (!s) return;
  const conexiones = { ...connectionsOf(s), [key]: on };
  try {
    applyHostView(await invoke("conexiones_servidor", { id: s.id, conexiones }));
  } catch (error) {
    toast(String(error), "error");
    renderHosted();
    return;
  }
  // playit.gg sin vincular: se vincula al activarlo.
  if (key === "playit" && on && !host.view.playit.connected && !host.view.playit.claimUrl) connectPlayit();
}

/** «Reintentar» o «Ya lo verifiqué»: vuelve a dejar lista la dirección con la cuenta ya vinculada. */
async function retryPlayit() {
  try {
    await invoke("reintentar_playit");
    const address = host.view?.playit.address;
    if (address) toast(t(`Listo. Tu dirección: ${address}`, `Done. Your address: ${address}`));
  } catch (error) {
    toast(String(error), "error");
  }
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
  if (state.page === "hosted") renderHosted();
  if (state.page === "host") renderHost();
  try {
    applyHostView(await invoke("conectar_playit"));
    const address = host.view.playit.address;
    toast(address ? t(`playit.gg quedó conectado. Tu dirección: ${address}`, `playit.gg is connected. Your address: ${address}`) : t("playit.gg quedó conectado.", "playit.gg is connected."));
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
  renderTunnelMods();
  // playit.gg solo aparece aquí mientras se vincula o si el mundo se comparte por él y algo falta.
  const p = view.playit;
  const showPlayit = !!p.claimUrl || host.connecting || (view.worldPlayit && (!p.connected || p.state === "error" || p.state === "verify"));
  $("#host-playit").classList.toggle("hidden", !showPlayit);
  if (showPlayit) {
    const world = { id: "world", state: view.world ? "ready" : "off", address: view.world?.address };
    $("#host-playit").innerHTML = `<div class="conn-card on"><div class="conn-head"><span class="conn-icon">${icon("globe")}</span><b class="grow">playit.gg</b></div><div class="conn-body">${playitBody(world)}</div></div>`;
  }
  // Tu mundo abierto en LAN: tus amigos entran con «Unirse» (P2P o red local), sin nada que activar.
  const world = view.world;
  const status = $("#host-world-status");
  if (world) {
    const name = escapeHtml(state.instances.find((i) => i.id === world.instance)?.name ?? world.instance);
    const address = world.address ? `<button class="btn small" data-copy-address="${escapeHtml(world.address)}" title="playit.gg">${icon("copy")}${escapeHtml(world.address)}</button>` : "";
    // Si alguien ya entró por P2P, se ve quién y por dónde (en vez de la explicación).
    const guests = world.p2pGuests?.length ? `<span class="grow">${p2pGuestsHtml(world.p2pGuests)}</span>` : "";
    status.innerHTML = `${icon("check-circle")}${guests || `<span class="grow">${t(`Tu mundo de ${name} está abierto: tus amigos pulsan «Unirse» en tu nombre.`, `Your ${name} world is open: your friends press «Join» on your name.`)}</span>`}${address}`;
    status.classList.remove("hidden");
  } else {
    status.classList.add("hidden");
  }
  const names = stateNames();
  setHtml($("#host-servers"), view.servers
    .map((s, index) => {
      const sub = [s.instanceName, `${s.loader} ${s.minecraft}`];
      if (s.state === "ready") sub.push(t(`${s.players.length} de ${s.maxPlayers} jugadores`, `${s.players.length} of ${s.maxPlayers} players`));

      const action =
        s.state === "off"
          ? `<button class="btn primary" data-host-start="${escapeHtml(s.id)}">${icon("play")}${t("Encender", "Start")}</button>`
          : `<button class="btn danger" data-host-stop="${escapeHtml(s.id)}" ${s.state === "stopping" ? "disabled" : ""}>${icon("stop")}${t("Apagar", "Stop")}</button>`;
      return `<div class="server host-card" data-hosted="${escapeHtml(s.id)}" data-key="h:${escapeHtml(s.id)}" style="animation-delay:${index * 40}ms" tabindex="0">
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
async function refreshFirewall() {
  if (!tauri) return;
  host.firewall = await invoke("firewall_servidores").catch(() => null);
  if (state.page === "hosted") renderHosted();
}

async function openFirewall() {
  try {
    await invoke("abrir_firewall_servidores");
    host.firewall = true;
    toast(t("Listo: quien esté en tu red ya puede entrar.", "Done: people on your network can now join."));
  } catch (error) {
    toast(String(error), "error");
  }
  if (state.page === "hosted") renderHosted();
}

async function enterHosted(id) {
  const changed = host.id !== id;
  host.id = id;
  if (host.firewall == null) refreshFirewall();
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
  $("#hosted-play").classList.toggle("hidden", s.state !== "ready");
  $("#hosted-stop").disabled = s.state === "stopping";
  $("#hosted-command").disabled = !on;
  $("#hosted-send").disabled = !on;

  // Conexiones: por dónde entran tus amigos.
  setHtml($("#hosted-connections"), connectionsHtml(s));

  // Cifras
  const memoryLimit = s.ramGb * 1024;
  const memoryShare = on ? Math.min(100, (s.memoryMb / memoryLimit) * 100) : 0;
  // TPS: 20 es lo normal; si baja, el lag es del servidor (no de la conexión).
  const paused = s.state === "ready" && s.tps != null && s.tps < 0.5 && !s.players.length;
  const tpsClass = s.tps == null ? "" : s.tps >= 18.5 ? "tps-good" : s.tps >= 14 ? "tps-warn" : "tps-bad";
  const tpsText = s.state !== "ready" || s.tps == null ? "—" : paused ? t("En pausa", "Paused") : `${s.tps.toFixed(1)}`;
  const tpsTitle = t("Ticks por segundo: 20 es lo normal. Si baja, el lag es del servidor; si está en 20 y hay lag, es la conexión.", "Ticks per second: 20 is normal. If it drops, the lag comes from the server; if it is at 20 and there is lag, it is the connection.");
  $("#hosted-stats").innerHTML = `
    <div class="stat"><span class="faint">${t("Jugadores", "Players")}</span><b>${on ? `${s.players.length} / ${s.maxPlayers}` : "—"}</b></div>
    <div class="stat" title="${escapeHtml(tpsTitle)}"><span class="faint">TPS${s.behind ? ` · ${t("se atrasó", "fell behind")}` : ""}</span><b class="${paused ? "" : tpsClass}">${escapeHtml(tpsText)}</b>${tickLine(s, on)}</div>
    <div class="stat"><span class="faint">${t("Encendido hace", "On for")}</span><b>${s.started && on ? uptime(s.started) : "—"}</b></div>
    <div class="stat"><span class="faint">${t("Procesador", "CPU")}</span><b>${on ? `${Math.round(s.cpu)}%` : "—"}</b></div>
    <div class="stat"><span class="faint">${t("Memoria", "Memory")}</span><b>${on ? `${gb(s.memoryMb)} / ${s.ramGb} GB` : `${s.ramGb} GB`}</b>
      <div class="meter"><div style="width:${memoryShare.toFixed(0)}%"></div></div>
      <span class="faint">${t(`Libre en el PC: ${gb(host.view.freeMb)} GB`, `Free on the PC: ${gb(host.view.freeMb)} GB`)}</span></div>`;

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
          g.uid ? t("Amigo de Parabellum: entra con «Unirse»", "Parabellum friend: joins with «Join»") : t("Sin Parabellum: necesita playit.gg (Conexiones)", "Without Parabellum: needs playit.gg (Connections)"),
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

// ---------- Editor del icono: cualquier imagen → 64×64 PNG (lo que pide Minecraft) ----------
const iconEditor = { image: null, zoom: 1, cx: 0, cy: 0, pixel: false, drag: null };

/** La imagen elegida (si la ventana no sabe abrirla, la convierte Windows: ver loadImageFile). */
async function readImageFile(file) {
  try {
    return await loadImageFile(file);
  } catch (error) {
    throw new Error(error?.message && error.message !== "no se pudo abrir" ? error.message : t("Esa imagen no se pudo abrir. Prueba con un PNG, JPG, GIF o WEBP.", "That image could not be opened. Try a PNG, JPG, GIF or WEBP."));
  }
}

async function serverIconFromFile(file) {
  try {
    iconEditor.image = await readImageFile(file);
  } catch (error) {
    return toast(error.message, "error");
  }
  const { naturalWidth: width, naturalHeight: height } = iconEditor.image;
  iconEditor.zoom = 1;
  iconEditor.cx = width / 2;
  iconEditor.cy = height / 2;
  // Las imágenes pequeñas suelen ser pixel art: se agrandan sin difuminar.
  iconEditor.pixel = Math.min(width, height) <= 128;
  $("#icon-zoom").value = "1";
  $("#icon-pixel").checked = iconEditor.pixel;
  $("#icon-sample-name").textContent = host.options?.name ?? "";
  $("#icon-sample-motd").innerHTML = motdHtml(host.options?.motd ?? "");
  drawIconEditor();
  openModal("#icon-dialog");
}

/** Cuántos píxeles de la imagen caben de lado en el recorte. */
function iconSpan() {
  const { naturalWidth: width, naturalHeight: height } = iconEditor.image;
  return Math.min(width, height) / iconEditor.zoom;
}

function drawIconEditor() {
  const image = iconEditor.image;
  if (!image) return;
  const span = iconSpan();
  const half = span / 2;
  iconEditor.cx = Math.min(Math.max(iconEditor.cx, half), image.naturalWidth - half);
  iconEditor.cy = Math.min(Math.max(iconEditor.cy, half), image.naturalHeight - half);
  const view = $("#icon-canvas");
  const context = view.getContext("2d");
  context.clearRect(0, 0, view.width, view.height);
  context.imageSmoothingEnabled = !iconEditor.pixel;
  context.imageSmoothingQuality = "high";
  context.drawImage(image, iconEditor.cx - half, iconEditor.cy - half, span, span, 0, 0, view.width, view.height);
  // El de 64×64 sale del de 256 (en dos pasos se ve mejor que de golpe) o, pixelado, directo.
  const preview = $("#icon-preview");
  const small = preview.getContext("2d");
  small.clearRect(0, 0, 64, 64);
  small.imageSmoothingEnabled = !iconEditor.pixel;
  small.imageSmoothingQuality = "high";
  if (iconEditor.pixel) small.drawImage(image, iconEditor.cx - half, iconEditor.cy - half, span, span, 0, 0, 64, 64);
  else small.drawImage(view, 0, 0, view.width, view.height, 0, 0, 64, 64);
}

function zoomIcon(factor) {
  iconEditor.zoom = Math.min(5, Math.max(1, iconEditor.zoom * factor));
  $("#icon-zoom").value = String(iconEditor.zoom);
  drawIconEditor();
}

async function saveEditedIcon() {
  const button = $("#icon-save");
  button.disabled = true;
  await saveServerIcon($("#icon-preview").toDataURL("image/png"));
  button.disabled = false;
  closeModal("#icon-dialog");
}

function bindIconEditor() {
  const view = $("#icon-canvas");
  view.addEventListener("pointerdown", (event) => {
    view.setPointerCapture(event.pointerId);
    iconEditor.drag = { x: event.clientX, y: event.clientY };
  });
  view.addEventListener("pointermove", (event) => {
    if (!iconEditor.drag || !iconEditor.image) return;
    // Lo que se arrastra en pantalla, en píxeles de la imagen.
    const ratio = iconSpan() / view.getBoundingClientRect().width;
    iconEditor.cx -= (event.clientX - iconEditor.drag.x) * ratio;
    iconEditor.cy -= (event.clientY - iconEditor.drag.y) * ratio;
    iconEditor.drag = { x: event.clientX, y: event.clientY };
    drawIconEditor();
  });
  view.addEventListener("pointerup", () => (iconEditor.drag = null));
  view.addEventListener("wheel", (event) => {
    event.preventDefault();
    zoomIcon(event.deltaY < 0 ? 1.1 : 1 / 1.1);
  });
  $("#icon-zoom").addEventListener("input", (event) => {
    iconEditor.zoom = Number(event.target.value);
    drawIconEditor();
  });
  $("#icon-pixel").addEventListener("change", (event) => {
    iconEditor.pixel = event.target.checked;
    drawIconEditor();
  });
  $("#icon-other").onclick = () => $("#hs-icon-file").click();
  $("#icon-save").onclick = () => saveEditedIcon();
  $("#icon-close").onclick = () => closeModal("#icon-dialog");
  $("#icon-dialog").onmousedown = (event) => {
    if (event.target.id === "icon-dialog") closeModal("#icon-dialog");
  };
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
  bindIconEditor();
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
// ---------- Página Servidores: tarjetas con el icono de cada servidor y una vista de detalle ----------
// El icono, el mensaje y los jugadores salen del propio servidor (estado_servidores: como la lista de
// Multijugador del juego). El último icono conocido de cada uno se recuerda para cuando esté apagado.
const SERVER_ICON_KEY = "parabellum-iconos-servidores";

function rememberedIcons() {
  try {
    return JSON.parse(localStorage.getItem(SERVER_ICON_KEY) ?? "{}");
  } catch {
    return {};
  }
}

function rememberIcon(key, favicon) {
  if (!key || !favicon) return;
  const icons = rememberedIcons();
  if (icons[key] === favicon) return;
  icons[key] = favicon;
  // Solo los últimos 40.
  const keys = Object.keys(icons);
  for (const old of keys.slice(0, Math.max(0, keys.length - 40))) delete icons[old];
  try {
    localStorage.setItem(SERVER_ICON_KEY, JSON.stringify(icons));
  } catch {
    // Sin almacenamiento: solo se ve mientras responde.
  }
}

/** Dirección para preguntar el estado de cada partida de amigos. */
function activityAddress(item) {
  if (item.kind === "hosted") return item.server.address ?? null;
  const p = item.friend?.presence ?? {};
  if (item.kind === "world") return p.address ?? p.lan ?? null;
  if (item.kind === "playing") return p.server ?? null;
  return null;
}

function statusFor(address) {
  return address ? state.serverStatus?.[address.trim().toLowerCase()] : null;
}

/** Pregunta el estado de todos los servidores de la página (cada 20 s mientras está abierta). */
async function refreshServerStatus() {
  if (!tauri) return;
  const addresses = [...friendActivity().map(activityAddress), ...(state.settings.servers ?? []).map((s) => s.address)].filter(Boolean);
  if (!addresses.length) return;
  try {
    const found = await invoke("estado_servidores", { direcciones: addresses });
    state.serverStatus = { ...(state.serverStatus ?? {}), ...found };
    renderFriendServers();
    renderServers();
    if (!$("#srv-dialog").classList.contains("hidden")) renderServerDetail();
  } catch {
    // Sin conexión: se queda lo que había.
  }
}

function enterServers() {
  refreshServerStatus();
  clearInterval(state.serverStatusTimer);
  state.serverStatusTimer = setInterval(() => {
    if (state.page === "servers" && document.visibilityState === "visible") refreshServerStatus();
    else if (state.page !== "servers") clearInterval(state.serverStatusTimer);
  }, 20000);
}

function serverIconHtml(favicon, fallback) {
  return `<div class="srv-icon">${favicon ? `<img src="${escapeHtml(favicon)}" alt="">` : icon(fallback)}</div>`;
}

/** Lo que muestra una tarjeta (y su detalle) de una partida de amigos. */
function activityInfo(item) {
  const address = activityAddress(item);
  const status = statusFor(address);
  if (status?.favicon) rememberIcon(item.key, status.favicon);
  const favicon = status?.favicon ?? rememberedIcons()[item.key] ?? null;
  const pill = (on, label) => `<span class="state-pill ${on ? "ready" : "off"}">${escapeHtml(label)}</span>`;
  if (item.kind === "hosted") {
    const sv = item.server;
    const players = status?.online ? `${status.players}/${status.maxPlayers}` : sv.online ? `${sv.players}/${sv.maxPlayers}` : null;
    return {
      favicon,
      fallback: "broadcast",
      name: sv.name,
      host: sv.hostName,
      hostLine: t(`Servidor de ${sv.hostName}`, `${sv.hostName}'s server`),
      pill: pill(sv.online, sv.online ? t("Encendido", "On") : t("Apagado", "Off")),
      players,
      canJoin: sv.online && !!(sv.address || sv.p2p || sv.lan),
      address: sv.address,
      pack: sv.pack,
      version: [sv.loader, sv.minecraft].filter(Boolean).join(" "),
      status,
      note: sv.online ? (sv.address || sv.p2p || sv.lan ? null : t("Sin conexión todavía", "No connection yet")) : t("Te invitó: se puede entrar cuando lo encienda", "Invited you: you can join when they start it"),
    };
  }
  const f = item.friend;
  const p = f.presence ?? {};
  const base = { favicon, host: f.name, address, pack: p.pack, version: p.minecraft ? `Minecraft ${p.minecraft}` : "", status, players: status?.online ? `${status.players}/${status.maxPlayers}` : null };
  if (item.kind === "world")
    return { ...base, fallback: "globe", name: t(`Mundo de ${f.name}`, `${f.name}'s world`), hostLine: t(`Mundo abierto de ${f.name}`, `${f.name}'s open world`), pill: pill(true, t("Abierto", "Open")), canJoin: true, note: p.address ? null : t("Solo en tu red (LAN)", "Only on your network (LAN)") };
  if (item.kind === "playing") return { ...base, fallback: "server", name: p.server, hostLine: t(`${f.name} está aquí`, `${f.name} is here`), pill: pill(true, t("Jugando", "Playing")), canJoin: true };
  return { ...base, fallback: "broadcast", name: p.hosting, hostLine: t(`Servidor de ${f.name}`, `${f.name}'s server`), pill: pill(true, t("Encendido", "On")), canJoin: false, note: t("Pídele que te invite para ver la dirección", "Ask them to invite you to see the address") };
}

function serverCardHtml({ key, info, button, dim }) {
  const head = info.host ? `<canvas width="8" height="8" class="head" data-head="${escapeHtml(info.host)}"></canvas>` : "";
  const players = info.players ? `<span class="srv-players">${icon("user")}${escapeHtml(info.players)}</span>` : "";
  return `<div class="srv-card ${dim ? "dim" : ""}" data-srv-open="${escapeHtml(key)}" data-key="s:${escapeHtml(key)}" tabindex="0" role="button" title="${t("Ver detalles", "See details")}">
      ${serverIconHtml(info.favicon, info.fallback)}
      <div class="grow srv-text">
        <div class="srv-name">${escapeHtml(info.name)}</div>
        <div class="srv-meta">${head}<span class="srv-host">${escapeHtml(info.hostLine)}</span>${info.pill ?? ""}${players}</div>
      </div>
      ${button}
    </div>`;
}

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
      const info = activityInfo(item);
      const button = `<button class="btn primary play" data-friend-join="${escapeHtml(item.key)}" ${info.canJoin ? "" : "disabled"}>${icon("play")}${t("Unirse", "Join")}</button>`;
      return serverCardHtml({ key: item.key, info, button, dim: !item.online });
    })
    .join("");
  setHtml(box, html);
  for (const canvas of $$("#friend-servers canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
}

/** Lo que muestra una tarjeta de un servidor guardado. */
function savedInfo(server) {
  const status = statusFor(server.address);
  const key = `saved:${server.address.toLowerCase()}`;
  if (status?.favicon) rememberIcon(key, status.favicon);
  const instance = state.instances.find((i) => i.id === server.instance);
  const on = !!status?.online;
  return {
    favicon: status?.favicon ?? rememberedIcons()[key] ?? null,
    fallback: "server",
    name: server.name || server.address,
    host: null,
    hostLine: server.name ? server.address : instance ? instance.name : "",
    pill: status ? `<span class="state-pill ${on ? "ready" : "off"}">${on ? t("En línea", "Online") : t("Sin respuesta", "No answer")}</span>` : "",
    players: on ? `${status.players}/${status.maxPlayers}` : null,
    canJoin: !!instance,
    address: server.address,
    pack: instance?.name ?? t("instancia no encontrada", "instance not found"),
    version: status?.version ?? "",
    status,
  };
}

// ---------- Detalle de un servidor ----------
function openServerDetail(key) {
  state.serverDetail = key;
  renderServerDetail();
  openModal("#srv-dialog");
}

function currentServerDetail() {
  const key = state.serverDetail;
  if (!key) return null;
  if (key.startsWith("saved:")) {
    const index = Number(key.slice(6));
    const server = (state.settings.servers ?? [])[index];
    return server ? { kind: "saved", index, server, info: savedInfo(server) } : null;
  }
  const item = findActivity(key);
  return item ? { kind: "activity", item, info: activityInfo(item) } : null;
}

function renderServerDetail() {
  const detail = currentServerDetail();
  if (!detail) return closeModal("#srv-dialog");
  const { info } = detail;
  const status = info.status;
  $("#srv-icon").outerHTML = serverIconHtml(info.favicon, info.fallback).replace('class="srv-icon"', 'class="srv-icon big" id="srv-icon"');
  $("#srv-name").textContent = info.name;
  $("#srv-sub").innerHTML = `${info.host ? `<canvas width="8" height="8" class="head" data-head="${escapeHtml(info.host)}"></canvas>` : ""}<span>${escapeHtml(info.hostLine)}</span>${info.pill ?? ""}`;
  for (const canvas of $$("#srv-sub canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
  // El mensaje (MOTD) como en la lista de Multijugador.
  const motd = status?.motd ?? "";
  $("#srv-motd-box").classList.toggle("hidden", !motd);
  $("#srv-motd").innerHTML = motdHtml(motd);
  const rows = [
    [t("Anfitrión", "Host"), info.host],
    [t("Modpack", "Modpack"), info.pack],
    [t("Versión", "Version"), info.version || status?.version],
    [t("Jugadores", "Players"), info.players ? `${info.players}${status?.sample?.length ? ` · ${status.sample.join(", ")}` : ""}` : null],
    [t("Dirección", "Address"), info.address],
    [t("Respuesta", "Ping"), status?.online && status.latencyMs ? `${status.latencyMs} ms` : null],
    ["TPS", detail.kind === "activity" && detail.item.kind === "hosted" && detail.item.server.tps != null ? `${Number(detail.item.server.tps).toFixed(1)} ${detail.item.server.tps >= 18.5 ? t("(fluido)", "(smooth)") : t("(se está atrasando)", "(falling behind)")}` : null],
    [t("Nota", "Note"), info.note],
  ].filter(([, value]) => value);
  $("#srv-facts").innerHTML =
    rows
      .map(([label, value]) => `<div class="srv-fact"><span>${escapeHtml(label)}</span><b>${escapeHtml(value)}</b>${label === t("Dirección", "Address") ? `<button class="circle-btn small" data-copy-address="${escapeHtml(value)}" title="${t("Copiar", "Copy")}">${icon("copy")}</button>` : ""}</div>`)
      .join("") + routesFactHtml(detail);
  measureDetailRoutes(detail);
  const actions = [];
  if (detail.kind === "activity") {
    const friend = detail.item.friend ?? state.social?.friends?.friends?.find((f) => f.uid === detail.item.server?.host);
    if (friend) actions.push(`<button class="btn" data-srv-action="message">${icon("chat")}${t(`Mensaje a ${friend.name}`, `Message ${friend.name}`)}</button>`);
    if (detail.item.kind === "hosted") actions.push(`<button class="btn" data-srv-action="leave">${icon("trash")}${t("Quitar de mi lista", "Remove from my list")}</button>`);
    actions.push(`<button class="btn primary" data-srv-action="join" ${info.canJoin ? "" : "disabled"}>${icon("play")}${t("Unirse", "Join")}</button>`);
  } else {
    actions.push(`<button class="btn" data-srv-action="remove">${icon("trash")}${t("Quitar", "Remove")}</button>`);
    actions.push(`<button class="btn primary" data-srv-action="play" ${info.canJoin ? "" : "disabled"}>${icon("play")}${t("Jugar", "Play")}</button>`);
  }
  $("#srv-actions").innerHTML = actions.join("");
}

/** Las rutas a la partida con su tiempo (se miden al abrir el detalle). */
function routesFactHtml(detail) {
  if (detail.kind !== "activity") return "";
  const routes = routesFor(detail.item);
  if (routes.length < 2) return "";
  const measured = state.routeTimes?.[detail.item.key];
  const chips = measured
    ? measured
        .map((r, i) => `<span class="route-chip ${i === 0 && r.latencyMs != null ? "best" : ""}">${escapeHtml(routeName(r.kind, r.via))} · ${r.latencyMs != null ? `${r.latencyMs} ms` : t("no responde", "no answer")}</span>`)
        .join("")
    : `<span class="route-chip"><span class="spinner tiny"></span>${t("Midiendo…", "Measuring…")}</span>`;
  return `<div class="srv-fact"><span>${t("Conexión", "Connection")}</span><div class="route-list">${chips}</div></div>`;
}

async function measureDetailRoutes(detail) {
  if (detail.kind !== "activity" || !detail.item.online) return;
  const key = detail.item.key;
  const routes = routesFor(detail.item);
  if (routes.length < 2 || state.routeMeasuring === key) return;
  const last = state.routeTimes?.[key];
  if (last && Date.now() - (last.at ?? 0) < 15000) return;
  state.routeMeasuring = key;
  const measured = await invoke("mejor_ruta", { rutas: routes, maxJugadores: detail.item.server?.maxPlayers || null }).catch(() => null);
  state.routeMeasuring = null;
  if (!measured) return;
  measured.at = Date.now();
  state.routeTimes = { ...(state.routeTimes ?? {}), [key]: measured };
  if (state.serverDetail === key && !$("#srv-dialog").classList.contains("hidden")) renderServerDetail();
}

async function serverDetailAction(action) {
  const detail = currentServerDetail();
  if (!detail) return;
  if (action === "join") {
    closeModal("#srv-dialog");
    return joinActivity(detail.item);
  }
  if (action === "play") {
    closeModal("#srv-dialog");
    return play(detail.server.instance, detail.server.address);
  }
  if (action === "message") {
    const friend = detail.item.friend ?? state.social?.friends?.friends?.find((f) => f.uid === detail.item.server?.host);
    closeModal("#srv-dialog");
    if (friend) messageFriend(friend.uid);
    return;
  }
  if (action === "remove") {
    closeModal("#srv-dialog");
    return saveServers(state.settings.servers.filter((_, i) => i !== detail.index));
  }
  if (action === "leave") {
    closeModal("#srv-dialog");
    try {
      await invoke("salir_de_servidor", { anfitrion: detail.item.server.host, clave: detail.item.server.key });
    } catch (error) {
      toast(String(error), "error");
    }
  }
}

function wireServerDetail() {
  $("#srv-close").onclick = () => closeModal("#srv-dialog");
  $("#srv-dialog").onmousedown = (event) => {
    if (event.target.id === "srv-dialog") closeModal("#srv-dialog");
  };
  $("#srv-actions").onclick = (event) => {
    const button = event.target.closest("[data-srv-action]");
    if (button) serverDetailAction(button.dataset.srvAction);
  };
  // Clic (o Enter) en una tarjeta que no sea en sus botones: el detalle.
  const open = (event) => {
    if (event.target.closest("button")) return;
    const card = event.target.closest("[data-srv-open]");
    if (card) openServerDetail(card.dataset.srvOpen);
  };
  for (const list of ["#friend-servers", "#server-list"]) {
    $(list).addEventListener("click", open);
    $(list).addEventListener("keydown", (event) => {
      if (event.key === "Enter" && event.target.matches("[data-srv-open]")) open(event);
    });
  }
}

function findActivity(key) {
  return friendActivity().find((item) => item.key === key);
}

/** Entrar a la partida de un amigo: servidor hosteado, mundo abierto o el servidor donde juega. */
function joinActivity(item) {
  if (item.kind === "hosted") return joinFriendServer(item.server);
  const p = item.friend.presence;
  if (item.kind === "world") return joinFriendWorld(item.friend);
  if (item.kind === "playing") return joinFriend(item.friend, p.server);
}

/** Entrar al mundo abierto de un amigo por la ruta más rápida (red local, P2P o playit.gg). */
async function joinFriendWorld(friend) {
  const p = friend.presence ?? {};
  const routes = routesFor({ kind: "world", friend });
  if (routes.length > 1) toast(t("Buscando la conexión más rápida…", "Looking for the fastest connection…"));
  const route = await fastestRoute(routes);
  if (route?.latencyMs != null) toast(t(`Entrando por ${routeName(route.kind, route.via)} (${route.latencyMs} ms).`, `Joining through ${routeName(route.kind, route.via)} (${route.latencyMs} ms).`));
  return joinFriend(friend, route?.address ?? p.address ?? p.lan);
}

/** Nombre de una ruta para mostrarla (P2P: directo entre las dos PCs o por el relevo). */
function routeName(kind, via) {
  if (kind === "p2p") return via === "relay" ? t("P2P por relevo", "P2P via relay") : t("P2P directo", "Direct P2P");
  return { local: t("Este PC", "This PC"), lan: t("Red local", "Local network"), ipv6: t("IPv6 directo", "Direct IPv6"), playit: "playit.gg" }[kind] ?? kind;
}

/** Las rutas para llegar a una partida (las directas primero; al unirse se prueban todas). */
function routesFor(item) {
  if (item.kind === "hosted") {
    const sv = item.server;
    return [
      sv.lan && { address: sv.lan, kind: "lan" },
      sv.p2p && { address: `p2p:${sv.p2p}#${sv.key}`, kind: "p2p" },
      sv.address && { address: sv.address, kind: "playit" },
    ].filter(Boolean);
  }
  const p = item.friend?.presence ?? {};
  if (item.kind === "world") return [p.lan && { address: p.lan, kind: "lan" }, p.p2p && { address: `p2p:${p.p2p}#world`, kind: "p2p" }, p.address && { address: p.address, kind: "playit" }].filter(Boolean);
  if (item.kind === "playing" && p.server) return [{ address: p.server, kind: "direct" }];
  return [];
}

/** Prueba las rutas a la vez y devuelve la más rápida que responda (o null). */
async function fastestRoute(routes, maxPlayers) {
  if (routes.length <= 1 || !tauri) return routes[0] ? { ...routes[0], latencyMs: null } : null;
  const measured = await invoke("mejor_ruta", { rutas: routes, maxJugadores: maxPlayers || null }).catch(() => []);
  return measured.find((r) => r.latencyMs != null) ?? null;
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
  // La ruta más rápida que responda: misma red o IPv6 directo antes que el túnel de playit.gg.
  toast(t("Buscando la conexión más rápida…", "Looking for the fastest connection…"));
  const route = await fastestRoute(routesFor({ kind: "hosted", server }), server.maxPlayers);
  const address = route?.address ?? server.address;
  if (!address) return toast(t("El servidor todavía no tiene dirección.", "The server has no address yet."), "error");
  if (route?.latencyMs != null) toast(t(`Entrando por ${routeName(route.kind, route.via)} (${route.latencyMs} ms).`, `Joining through ${routeName(route.kind, route.via)} (${route.latencyMs} ms).`));
  if (state.running.size) {
    await copyText(address, t(`Ya tienes Minecraft abierto. Copié la dirección: en el juego ve a Multijugador › Conexión directa y pégala.`, `Minecraft is already open. I copied the address: in the game go to Multiplayer › Direct Connection and paste it.`));
    return;
  }
  play(instance.id, address);
}

/** El anfitrión entra a su propio servidor desde este mismo PC (sin pasar por el túnel). */
function playOwnServer(s) {
  const instance = state.instances.find((i) => i.id === s.instance);
  if (!instance) return toast(t("No encuentro la instancia de este servidor.", "I cannot find this server's instance."), "error");
  const address = `127.0.0.1:${s.port}`;
  if (state.running.size) return copyText(address, t(`Ya tienes Minecraft abierto. Copié la dirección (${address}): en el juego ve a Multijugador › Conexión directa y pégala.`, `Minecraft is already open. I copied the address (${address}): in the game go to Multiplayer › Direct Connection and paste it.`));
  play(instance.id, address);
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
  wireServerDetail();
  $("#host-new-server").onclick = () => openHostCreate();
  $("#world-playit").onchange = async (event) => {
    const on = event.target.checked;
    await invoke("compartir_mundo", { activo: on }).catch((e) => toast(String(e), "error"));
    if (on && host.view && !host.view.playit.connected) {
      navigate("host");
      connectPlayit();
    }
  };
  $("#hosted-connections").addEventListener("change", (event) => {
    const toggle = event.target.closest("[data-connection]");
    if (toggle) setConnection(toggle.dataset.connection, toggle.checked);
  });
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
    if (event.target.closest("#tunnel-retry, [data-playit-retry]")) return void retryPlayit();
    const menu = event.target.closest("#tunnel-menu");
    if (menu) return void tunnelMenu(menu);
    if (event.target.closest("#hosted-firewall")) return void openFirewall();
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
  $("#hosted-play").onclick = () => {
    const s = hostServer(host.id);
    if (s) playOwnServer(s);
  };
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
