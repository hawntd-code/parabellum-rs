"use strict";

// ======================================================================
// Soporte de una comunidad (docs/plan-diciembre.md, sección 7): «Pedir ayuda» con el diagnóstico (el
// jugador lo ve antes de mandarlo), la cola que lee todo el equipo de soporte, la conversación y las
// tarjetas de ajustes, que el jugador aplica con un clic (soporte.rs). Quién ve y escribe qué lo deciden
// las reglas de Firebase.
// Usa lo de app.js y comunidad.js: state, invoke, listen, t, $, $$, icon, community, cmDialog, textField…
// ======================================================================

const support = {
  /** La cola de la comunidad abierta: { community, staff, tickets }. */
  view: null,
  /** Filtro del equipo: «abiertas», «mias» o «cerradas». */
  filter: "abiertas",
  /** La solicitud abierta en la ventana: { community, id }. */
  open: null,
  messages: null,
  /** Los ajustes que se pueden mandar, con el valor de este PC (ajustes_compartibles). */
  settings: null,
  /** Lo que pasó con cada tarjeta en este PC (datos/tarjetas.json). */
  applied: {},
  /** Tarjetas que se están aplicando o deshaciendo. */
  busy: new Set(),
  sending: false,
  /** La comunidad cuya cola se escucha. */
  watching: null,
};

const ticketStates = () => ({ abierto: t("Abierta", "Open"), atendido: t("Atendida", "Answered"), cerrado: t("Cerrada", "Closed") });
const myUid = () => community.detail?.members.find((m) => m.you)?.uid ?? null;
const isStaffMember = (m) => !!m && (m.role === "owner" || m.role === "admin" || !!m.caps?.support);
/** Con lo que ya se sabe de la comunidad (antes de que llegue la cola). */
const supportStaff = () => support.view?.community === community.id ? support.view.staff : isStaffMember(community.detail?.members.find((m) => m.you));
const openTicket = () => (support.open && support.view?.community === support.open.community ? support.view.tickets.find((x) => x.id === support.open.id) : null) ?? support.open?.ticket ?? null;

/** La cola de la comunidad (y la escucha, si cambió de comunidad). */
async function loadSupport(cid) {
  if (support.watching !== cid) {
    support.watching = cid;
    invoke("soporte_escuchar_cola", { id: cid }).catch(() => {});
  }
  let view;
  try {
    view = await invoke("soporte", { id: cid });
  } catch {
    return;
  }
  if (community.id !== cid) return;
  const before = support.view?.community === cid ? new Set(support.view.tickets.filter((x) => x.unread).map((x) => x.id)) : null;
  support.view = { community: cid, ...view };
  // Te respondieron (y no la tienes abierta): un aviso.
  for (const ticket of view.tickets) {
    if (before && ticket.unread && !before.has(ticket.id) && support.open?.id !== ticket.id) {
      toast(t(`El soporte te respondió: «${ticket.topic}»`, `Support answered you: «${ticket.topic}»`));
    }
  }
  if (state.page === "communities") renderSupportSection();
  if (support.open?.community === cid) renderTicketHead();
}

// ---------- La sección en la comunidad ----------
/** Para el equipo, la cola con filtros (en la columna ancha); para el jugador, sus solicitudes. */
function supportSectionHtml() {
  return `<section class="cm-section" id="cm-support"></section>`;
}

function renderSupportSection() {
  const box = $("#cm-support");
  const d = community.detail;
  if (!box || !d) return;
  const view = support.view?.community === d.id ? support.view : null;
  const staff = supportStaff();
  const ask = `<button class="btn small" data-sp="ask">${icon("chat")}${t("Pedir ayuda", "Ask for help")}</button>`;
  if (!view) {
    box.innerHTML = `<div class="cm-section-head"><h3>${staff ? t("Solicitudes de ayuda", "Help requests") : t("Ayuda", "Help")}</h3>${staff ? "" : ask}</div><span class="skel block short"></span>`;
    return;
  }
  const me = myUid();
  if (!staff) {
    box.innerHTML = `<div class="cm-section-head"><h3>${t("Ayuda", "Help")}</h3>${ask}</div>${
      view.tickets.length
        ? `<div class="sp-list compact">${view.tickets.map((x) => ticketRowHtml(x, { compact: true })).join("")}</div>`
        : `<p class="side-note no-top">${escapeHtml(
            t(`¿Algo no funciona? Pide ayuda: la lee el equipo de soporte de ${d.name}.`, `Something not working? Ask for help: ${d.name}'s support team reads it.`),
          )}</p>`
    }`;
    return;
  }
  const groups = {
    abiertas: view.tickets.filter((x) => x.state !== "cerrado"),
    mias: view.tickets.filter((x) => x.assignee === me && x.state !== "cerrado"),
    cerradas: view.tickets.filter((x) => x.state === "cerrado"),
  };
  const waiting = groups.abiertas.filter((x) => x.state === "abierto").length;
  const shown = groups[support.filter] ?? groups.abiertas;
  const chip = (key, label) => `<button class="sp-filter ${support.filter === key ? "active" : ""}" data-sp-filter="${key}">${label}<span>${groups[key].length}</span></button>`;
  const empty = {
    abiertas: t("No hay solicitudes abiertas. Cuando alguien pida ayuda, aparece aquí.", "There are no open requests. When someone asks for help, it shows up here."),
    mias: t("No tienes solicitudes tomadas. Abre una y pulsa «Tomar».", "You have no requests taken. Open one and press «Take»."),
    cerradas: t("Todavía no se cerró ninguna.", "None has been closed yet."),
  }[support.filter];
  box.innerHTML = `<div class="cm-section-head"><h3>${t("Solicitudes de ayuda", "Help requests")}</h3>${
    waiting ? `<span class="sp-waiting">${escapeHtml(plural(waiting, t("esperando", "waiting"), t("esperando", "waiting")))}</span>` : ""
  }${ask}</div>
    <div class="sp-filters">${chip("abiertas", t("Abiertas", "Open"))}${chip("mias", t("Mías", "Mine"))}${chip("cerradas", t("Cerradas", "Closed"))}</div>
    ${shown.length ? `<div class="sp-list">${shown.map((x) => ticketRowHtml(x)).join("")}</div>` : `<div class="sp-empty">${icon("check-circle")}<p>${escapeHtml(empty)}</p></div>`}`;
  for (const canvas of $$("canvas[data-head]", box)) drawHead(canvas, canvas.dataset.head);
}

function ticketRowHtml(x, { compact = false } = {}) {
  const fresh = x.state === "abierto" && !x.assignee;
  const who = compact ? "" : `<canvas width="8" height="8" class="head" data-head="${escapeHtml(x.byName)}"></canvas>`;
  const last = x.last ? `${x.lastFrom === x.by ? (compact ? t("Tú", "You") : x.byName) : t("Soporte", "Support")}: ${x.last}` : "";
  return `<button class="sp-ticket ${x.unread ? "unread" : ""} ${fresh && !compact ? "fresh" : ""}" data-sp-ticket="${escapeHtml(x.id)}">
    ${who}
    <span class="grow">
      <span class="sp-ticket-top"><b>${escapeHtml(x.topic)}</b><span class="faint">${escapeHtml(chatTime(x.lastAt || x.updated || x.at))}</span></span>
      <span class="sp-ticket-last">${escapeHtml(last)}</span>
      <span class="sp-ticket-tags"><span class="sp-state ${x.state}">${escapeHtml(ticketStates()[x.state] ?? x.state)}</span>${
        !compact ? `<span class="faint">${escapeHtml(x.byName)}${x.instance ? ` · ${escapeHtml(x.instance)}` : ""}</span>` : ""
      }${x.assigneeName ? `<span class="faint">${icon("user")}${escapeHtml(x.assigneeName)}</span>` : ""}</span>
    </span>
    ${x.unread ? `<span class="sp-dot" title="${t("Respuesta nueva", "New answer")}"></span>` : ""}
  </button>`;
}

// ---------- Pedir ayuda ----------
/** El formulario: tema, qué pasó, la instancia y su diagnóstico (se ve antes de mandarlo). */
async function askForHelp({ cid = community.id, instance = null, note = "" } = {}) {
  if (community.detail?.id !== cid) await loadCommunity(cid);
  const d = community.detail;
  if (!d) return;
  const team = d.members.filter(isStaffMember).length;
  const instances = [...state.instances].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0));
  // La que falla: la que se pidió, la del paquete oficial o la última que se jugó.
  const official = Object.values(d.local ?? {}).find((l) => l.instance && instances.some((i) => i.id === l.instance))?.instance;
  const picked = instance ?? official ?? instances[0]?.id ?? null;
  let diagnosis = null;
  let seq = 0;
  const loadDiagnosis = async (root) => {
    const id = $("#sp-instance", root)?.value || null;
    const mine = ++seq;
    diagnosis = null;
    $("#sp-diagnosis", root).innerHTML = `<span class="spinner tiny"></span>${t("Revisando la instancia…", "Checking the instance…")}`;
    try {
      const report = await invoke("reporte_problema", { id });
      if (mine !== seq) return;
      diagnosis = report.text;
      $("#sp-diagnosis", root).textContent = diagnosis;
    } catch (error) {
      if (mine === seq) $("#sp-diagnosis", root).textContent = String(error);
    }
  };
  const tid = await new Promise((resolve) => {
    let opened = null;
    cmDialog({
      title: t("Pedir ayuda", "Ask for help"),
      subtitle: t(`Lo leerá el equipo de soporte de ${d.name} (${plural(team, "persona", "personas")}).`, `${d.name}'s support team will read it (${plural(team, "person", "people")}).`),
      glyph: "chat",
      wide: true,
      body: `${textField({ id: "sp-topic", label: t("Tema", "Topic"), max: 80, placeholder: t("Por ejemplo: se cierra al entrar al servidor", "For example: it closes when joining the server"), autofocus: true })}
        ${textField({ id: "sp-text", label: t("¿Qué pasó?", "What happened?"), value: note, max: 2000, rows: 4, placeholder: t("Qué hacías, qué esperabas y qué pasó. Si sale un error, cópialo aquí.", "What you were doing, what you expected and what happened. If there is an error, copy it here.") })}
        ${
          instances.length
            ? `<label class="cm-field"><span class="label">${t("Instancia", "Instance")}</span><select id="sp-instance">${instances
                .map((i) => `<option value="${escapeHtml(i.id)}" ${i.id === picked ? "selected" : ""}>${escapeHtml(i.name)}</option>`)
                .join("")}</select></label>`
            : ""
        }
        <div class="cm-field">
          <label class="check sp-attach"><input type="checkbox" id="sp-attach" checked><span>${t("Adjuntar el diagnóstico", "Attach the diagnosis")}</span></label>
          <pre class="report-preview" id="sp-diagnosis"></pre>
          <span class="faint cm-hint">${t(
            "Va tal cual lo ves: versión, sistema, la instancia, el último error y lo que encontró el doctor. Sin tu carpeta de usuario, correos ni claves.",
            "It goes exactly as you see it: version, system, the instance, the last error and what the doctor found. Without your user folder, emails or keys.",
          )}</span>
        </div>`,
      ok: `${icon("send")}${t("Enviar", "Send")}`,
      mount: (root) => {
        wireCounters(root);
        $("#sp-instance", root)?.addEventListener("change", () => loadDiagnosis(root));
        $("#sp-attach", root).addEventListener("change", (event) => $("#sp-diagnosis", root).classList.toggle("off", !event.target.checked));
        loadDiagnosis(root);
      },
      onOk: async () => {
        const root = $("#cm-dialog-body");
        const tema = $("#sp-topic", root).value.trim();
        const texto = $("#sp-text", root).value.trim();
        if (!tema) throw t("Escribe el tema.", "Write the topic.");
        if (!texto) throw t("Cuenta qué pasó.", "Tell what happened.");
        const chosen = state.instances.find((i) => i.id === $("#sp-instance", root)?.value);
        const attach = $("#sp-attach", root).checked;
        if (attach && diagnosis === null) throw t("Espera a que termine el diagnóstico (o quítalo).", "Wait for the diagnosis to finish (or remove it).");
        opened = await invoke("soporte_abrir", { id: cid, tema, texto, instancia: chosen?.name ?? null, diagnostico: attach ? diagnosis : null });
        return true;
      },
    }).then(() => resolve(opened));
  });
  if (!tid) return;
  toast(t("Listo: el equipo de soporte ya la ve.", "Done: the support team can see it now."));
  await loadSupport(cid);
  openSupportTicket(tid);
}

// ---------- La conversación ----------
async function openSupportTicket(tid) {
  const cid = community.id;
  support.open = { community: cid, id: tid, ticket: support.view?.tickets.find((x) => x.id === tid) ?? null };
  support.messages = null;
  $("#sp-card-form").classList.add("hidden");
  $("#sp-input").value = "";
  renderTicketHead();
  renderTicketMessages();
  openModal("#support-dialog");
  setTimeout(() => $("#sp-input").focus(), 60);
  try {
    const [messages, applied, settings] = await Promise.all([invoke("soporte_mensajes", { id: cid, solicitud: tid }), invoke("tarjetas_aplicadas"), invoke("ajustes_compartibles")]);
    if (support.open?.id !== tid) return;
    Object.assign(support, { messages, applied: applied ?? {}, settings });
    renderTicketMessages();
    invoke("soporte_escuchar", { id: cid, solicitud: tid }).catch(() => {});
    // Leída: se apaga el punto en la lista.
    const ticket = openTicket();
    if (ticket?.unread) {
      ticket.unread = false;
      renderSupportSection();
    }
  } catch (error) {
    if (support.open?.id !== tid) return;
    support.messages = [];
    $("#sp-messages").innerHTML = `<p class="chat-hint">${escapeHtml(String(error))}</p>`;
  }
}

function closeSupportTicket() {
  if (!support.open) return;
  support.open = null;
  closeModal("#support-dialog");
  invoke("soporte_escuchar", {}).catch(() => {});
}

function renderTicketHead() {
  const x = openTicket();
  if (!x) {
    $("#sp-title").textContent = t("Solicitud de ayuda", "Help request");
    $("#sp-subtitle").textContent = "";
    $("#sp-actions").innerHTML = "";
    return;
  }
  const staff = supportStaff();
  const mine = x.by === myUid();
  $("#sp-title").textContent = x.topic;
  const facts = [staff && !mine ? x.byName : null, x.instance, x.assigneeName ? t(`La atiende ${x.assigneeName}`, `Handled by ${x.assigneeName}`) : staff ? t("Sin tomar", "Not taken") : null].filter(Boolean);
  $("#sp-subtitle").innerHTML = `<span class="sp-state ${x.state}">${escapeHtml(ticketStates()[x.state] ?? x.state)}</span>${escapeHtml(facts.join(" · "))}`;
  const take = staff && x.assignee !== myUid() && x.state !== "cerrado" ? `<button class="btn small" data-sp-head="take">${icon("user")}${t("Tomar", "Take")}</button>` : "";
  const toggle =
    x.state === "cerrado"
      ? `<button class="btn small" data-sp-head="reopen">${icon("refresh")}${t("Reabrir", "Reopen")}</button>`
      : `<button class="btn small" data-sp-head="close" title="${mine ? t("Ya se resolvió", "It is solved") : t("Ya se atendió", "It was handled")}">${icon("check")}${mine ? t("Resuelta", "Solved") : t("Cerrar", "Close")}</button>`;
  $("#sp-actions").innerHTML = take + toggle;
  $("#sp-card-toggle").classList.toggle("hidden", !staff || mine);
}

/** Cómo se lee un valor de ajuste. */
function settingValue(id, value) {
  if (value === null || value === undefined) return "—";
  if (typeof value === "boolean") return value ? t("Activado", "On") : t("Desactivado", "Off");
  if (id === "launcher.ram_gb") return `${value} GB`;
  if (id === "launcher.download_limit_mb") return Number(value) === 0 ? t("Sin límite", "No limit") : `${value} MB/s`;
  return String(value);
}

/** Cuándo se nota (lo dice la tarjeta). */
function settingEffect(id) {
  return (
    {
      "launcher.ram_gb": t("Se usa la próxima vez que abras el juego. Las instancias con memoria propia no cambian.", "It is used the next time you open the game. Instances with their own memory do not change."),
      "launcher.download_limit_mb": t("Desde la próxima descarga.", "From the next download."),
      "launcher.download_parallel": t("Desde la próxima descarga.", "From the next download."),
      "launcher.game_panel": t("La próxima vez que abras el juego.", "The next time you open the game."),
      "launcher.school_mode": t("Al instante: las descargas van por el espejo de Drive.", "Right away: downloads go through the Drive mirror."),
    }[id] ?? t("Al instante.", "Right away.")
  );
}

const sameValue = (a, b) => (typeof a === "number" || typeof b === "number" ? Math.abs(Number(a) - Number(b)) < 1e-9 : a === b);

function cardHtml(m, acks) {
  const c = m.card ?? {};
  const def = support.settings?.find((s) => s.id === c.setting);
  const owner = openTicket()?.by === myUid();
  const record = support.applied?.[c.op];
  const busy = support.busy.has(c.op);
  // Sin lo de entre paréntesis («Memoria para el juego (GB)»): el valor ya lleva su unidad.
  const label = (def?.label ?? c.setting).replace(/\s*\(.*\)$/, "");
  let values;
  let actions;
  if (owner) {
    values = `<span class="sp-val"><small>${t("Ahora en este PC", "Now on this PC")}</small><b>${escapeHtml(settingValue(c.setting, def?.value))}</b></span>${icon("right")}<span class="sp-val new"><small>${t(
      "Recomendado",
      "Recommended",
    )}</small><b>${escapeHtml(settingValue(c.setting, c.value))}</b></span>`;
    // Lo mismo que revisa aplicar_tarjeta: que sea de esta comunidad y que quien la mandó siga en el equipo
    // (si salió de la comunidad, tampoco). Lo ya aplicado se puede deshacer igual.
    const valid = c.v === 1 && c.kind === "setting" && c.scope === "launcher" && c.official === support.open?.community;
    const known = community.detail?.id === support.open?.community;
    const stale = known && !isStaffMember(community.detail.members.find((x) => x.uid === m.from));
    if (!valid) actions = `<span class="cm-chip warn">${icon("alert")}${t("Esta tarjeta no es válida", "This card is not valid")}</span>`;
    else if (record?.result === "aplicada")
      actions = `<span class="cm-chip ok">${icon("check")}${t("Aplicada en este PC", "Applied on this PC")}</span><button class="btn small" data-sp-undo="${escapeHtml(c.op)}" ${busy ? "disabled" : ""}>${icon("reply")}${t("Deshacer", "Undo")}</button>`;
    else if (record?.result === "ya-estaba") actions = `<span class="cm-chip ok">${icon("check")}${t("Ya lo tenías así", "You already had it like that")}</span>`;
    else if (stale) actions = `<span class="cm-chip warn">${icon("alert")}${t("Quien la mandó ya no es del equipo de soporte", "Whoever sent it is no longer on the support team")}</span>`;
    else if (!def) actions = `<span class="cm-chip warn">${icon("update")}${t("Actualiza Parabellum para aplicarla", "Update Parabellum to apply it")}</span>`;
    else
      actions = `${record?.result === "deshecha" ? `<span class="cm-chip">${t("La deshiciste", "You undid it")}</span>` : ""}<button class="btn small primary" data-sp-apply="${escapeHtml(m.id)}" ${busy ? "disabled" : ""}>${icon("check")}${
        busy ? t("Aplicando…", "Applying…") : record ? t("Aplicar otra vez", "Apply again") : t("Aplicar", "Apply")
      }</button>`;
  } else {
    values = `<span class="sp-val new"><small>${t("Propuesto", "Proposed")}</small><b>${escapeHtml(settingValue(c.setting, c.value))}</b></span>`;
    const ack = acks[c.op];
    actions = ack
      ? `<span class="cm-chip ${ack === "deshecha" ? "" : "ok"}">${icon(ack === "deshecha" ? "reply" : "check")}${
          { aplicada: t("La aplicó en su PC", "Applied on their PC"), "ya-estaba": t("Ya lo tenía así", "They already had it"), deshecha: t("La deshizo", "They undid it") }[ack]
        }</span>`
      : `<span class="cm-chip">${icon("clock")}${t("Sin aplicar todavía", "Not applied yet")}</span>`;
  }
  return `<div class="sp-card">
    <div class="sp-card-head"><span class="sp-card-icon">${icon("sliders")}</span><span class="grow"><b>${escapeHtml(label)}</b><span class="faint">${escapeHtml(
      t(`Ajuste de Parabellum · lo propone ${m.name}`, `Parabellum setting · proposed by ${m.name}`),
    )}</span></span></div>
    <div class="sp-card-values">${values}</div>
    <p class="sp-card-effect">${escapeHtml(settingEffect(c.setting))}${owner ? ` ${escapeHtml(t("Solo cambia si pulsas «Aplicar».", "It only changes if you press «Apply»."))}` : ""}</p>
    <div class="sp-card-actions">${actions}</div>
  </div>`;
}

function renderTicketMessages() {
  const box = $("#sp-messages");
  if (support.messages === null) {
    box.innerHTML = `<div class="sp-loading"><span class="spinner small"></span></div>`;
    return;
  }
  const nearBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 140;
  const first = !box.querySelector(".bubble-row, .sp-event");
  const members = Object.fromEntries((community.detail?.members ?? []).map((m) => [m.uid, m]));
  // Lo último que pasó con cada tarjeta (el acuse más nuevo).
  const acks = {};
  for (const m of support.messages) if (m.kind === "acuse" && m.ack) acks[m.ack.op] = m.ack.result;
  let html = "";
  let previous = null;
  for (const m of support.messages) {
    const day = new Date(m.at).toDateString();
    if (!previous || new Date(previous.at).toDateString() !== day) html += `<div class="chat-day">${escapeHtml(dayLabel(m.at))}</div>`;
    if (m.kind === "acuse") {
      const result = m.ack?.result;
      const what = m.mine
        ? { aplicada: t("Aplicaste el ajuste en tu PC", "You applied the setting on your PC"), "ya-estaba": t("Ya tenías el ajuste así", "You already had the setting like that"), deshecha: t("Deshiciste el ajuste", "You undid the setting") }[result]
        : { aplicada: t(`${m.name} aplicó el ajuste en su PC`, `${m.name} applied the setting on their PC`), "ya-estaba": t(`${m.name} ya tenía el ajuste así`, `${m.name} already had the setting like that`), deshecha: t(`${m.name} deshizo el ajuste`, `${m.name} undid the setting`) }[result];
      html += `<div class="sp-event">${icon(result === "deshecha" ? "reply" : "check-circle")}<span>${escapeHtml(what ?? m.text)}</span><span class="faint">${escapeHtml(chatTime(m.at))}</span></div>`;
      previous = m;
      continue;
    }
    const continued = previous && previous.kind !== "acuse" && previous.from === m.from && m.at - previous.at < 5 * 60000 && new Date(previous.at).toDateString() === day;
    const staffTag = isStaffMember(members[m.from]) ? `<span class="tag accent">${t("Soporte", "Support")}</span>` : "";
    const author = !m.mine && !continued ? `<span class="bubble-author">${escapeHtml(m.name)}${staffTag}</span>` : "";
    const time = new Date(m.at).toLocaleTimeString(locale(), { hour: "numeric", minute: "2-digit" });
    let body;
    if (m.kind === "tarjeta") body = cardHtml(m, acks);
    else if (m.kind === "diagnostico") body = `<span class="sp-diag-label">${icon("wrench")}${t("Diagnóstico", "Diagnosis")}</span><pre class="sp-diag">${escapeHtml(m.text)}</pre>`;
    else body = `<span class="bubble-text">${messageTextHtml(m.text)}</span>`;
    html += `<div class="bubble-row ${m.mine ? "mine" : ""} ${continued ? "continued" : ""} ${m.kind !== "texto" ? `sp-${m.kind}` : ""}"><div class="bubble-stack"><div class="bubble">${author}${body}<span class="bubble-time">${escapeHtml(time)}</span></div></div></div>`;
    previous = m;
  }
  box.innerHTML = html || `<p class="chat-hint">${t("Todavía no hay mensajes.", "No messages yet.")}</p>`;
  if (first || nearBottom) box.scrollTop = box.scrollHeight;
}

async function sendSupportMessage() {
  const input = $("#sp-input");
  const texto = input.value.trim();
  if (!texto || support.sending || !support.open) return;
  const { community: id, id: solicitud } = support.open;
  support.sending = true;
  $("#sp-send").disabled = true;
  try {
    await invoke("soporte_enviar", { id, solicitud, texto });
    input.value = "";
    input.style.height = "";
    if (!tauri) {
      support.messages.push({ id: `m${Date.now()}`, from: myUid(), name: state.social?.name ?? "Tú", text: texto, at: Date.now(), kind: "texto", mine: true });
      renderTicketMessages();
    }
  } catch (error) {
    toast(String(error), "error");
  } finally {
    support.sending = false;
    $("#sp-send").disabled = false;
    input.focus();
  }
}

async function headAction(action) {
  const x = openTicket();
  if (!x) return;
  const id = support.open.community;
  try {
    if (action === "take") await invoke("soporte_tomar", { id, solicitud: x.id });
    // Al reabrir: quien la pidió la deja «abierta» (las reglas no le dejan otra); el equipo, «atendida» si ya tiene a alguien.
    else await invoke("soporte_estado", { id, solicitud: x.id, estado: action === "close" ? "cerrado" : supportStaff() && x.by !== myUid() && x.assignee ? "atendido" : "abierto" });
    if (!tauri) {
      if (action === "take") Object.assign(x, { assignee: myUid(), assigneeName: community.detail.members.find((m) => m.you)?.name, state: "atendido" });
      else x.state = action === "close" ? "cerrado" : "abierto";
      renderTicketHead();
      renderSupportSection();
    } else await loadSupport(id);
  } catch (error) {
    toast(String(error), "error");
  }
}

async function applyCard(mid) {
  const m = support.messages?.find((x) => x.id === mid);
  if (!m?.card || !support.open) return;
  const op = m.card.op;
  support.busy.add(op);
  renderTicketMessages();
  try {
    const result = await invoke("aplicar_tarjeta", { id: support.open.community, solicitud: support.open.id, mensaje: mid });
    support.applied[op] = { ...result, setting: m.card.setting };
    support.settings = await invoke("ajustes_compartibles");
    toast(result.result === "ya-estaba" ? t("Ya lo tenías así: no cambió nada.", "You already had it like that: nothing changed.") : t("Ajuste aplicado.", "Setting applied."));
  } catch (error) {
    toast(String(error), "error");
  } finally {
    support.busy.delete(op);
    renderTicketMessages();
  }
}

async function undoCard(op) {
  support.busy.add(op);
  renderTicketMessages();
  try {
    const result = await invoke("deshacer_tarjeta", { op });
    support.applied[op] = { ...support.applied[op], ...result };
    support.settings = await invoke("ajustes_compartibles");
    toast(t("Volvió a como estaba.", "It is back to how it was."));
  } catch (error) {
    toast(String(error), "error");
  } finally {
    support.busy.delete(op);
    renderTicketMessages();
  }
}

// ---------- Tarjeta nueva (el equipo) ----------
function openCardForm() {
  const form = $("#sp-card-form");
  if (!form.classList.contains("hidden")) return form.classList.add("hidden");
  const settings = support.settings ?? [];
  $("#sp-card-setting").innerHTML = settings.map((s) => `<option value="${escapeHtml(s.id)}">${escapeHtml(s.label)}</option>`).join("");
  renderCardValue();
  form.classList.remove("hidden");
  $("#sp-card-setting").focus();
}

function renderCardValue() {
  const def = support.settings?.find((s) => s.id === $("#sp-card-setting").value);
  if (!def) return;
  const box = $("#sp-card-value");
  if (def.kind === "bool") box.innerHTML = `<select id="sp-card-input"><option value="true">${t("Activado", "On")}</option><option value="false">${t("Desactivado", "Off")}</option></select>`;
  else if (def.kind === "range") box.innerHTML = `<input id="sp-card-input" type="number" min="${def.min}" max="${def.max}" step="1" value="${escapeHtml(String(def.value ?? def.min))}">`;
  else box.innerHTML = `<select id="sp-card-input">${def.options.map((o) => `<option value="${o}">${escapeHtml(settingValue(def.id, o))}</option>`).join("")}</select>`;
}

async function sendCard() {
  const def = support.settings?.find((s) => s.id === $("#sp-card-setting").value);
  if (!def || !support.open) return;
  const raw = $("#sp-card-input").value;
  const valor = def.kind === "bool" ? raw === "true" : Number(raw);
  if (def.kind === "range" && (!Number.isInteger(valor) || valor < def.min || valor > def.max)) return toast(t(`Elige un número de ${def.min} a ${def.max}.`, `Choose a number from ${def.min} to ${def.max}.`), "error");
  $("#sp-card-send").disabled = true;
  try {
    await invoke("soporte_tarjeta", { id: support.open.community, solicitud: support.open.id, ajuste: def.id, valor });
    $("#sp-card-form").classList.add("hidden");
    if (!tauri) {
      support.messages.push({ id: `m${Date.now()}`, from: myUid(), name: state.social?.name ?? "Tú", text: "", at: Date.now(), kind: "tarjeta", card: { v: 1, op: `o${Date.now()}`, kind: "setting", setting: def.id, value: valor, scope: "launcher", official: support.open.community }, mine: true });
      renderTicketMessages();
    }
  } catch (error) {
    toast(String(error), "error");
  } finally {
    $("#sp-card-send").disabled = false;
  }
}

// ---------- Eventos ----------
function wireSupportEvents() {
  $("#cm-body").addEventListener("click", (event) => {
    const target = event.target.closest("button");
    if (!target || !target.closest("#cm-support")) return;
    if (target.dataset.sp === "ask") return askForHelp();
    if (target.dataset.spTicket) return openSupportTicket(target.dataset.spTicket);
    if (target.dataset.spFilter) {
      support.filter = target.dataset.spFilter;
      renderSupportSection();
    }
  });
  $("#sp-close").onclick = closeSupportTicket;
  $("#support-dialog").addEventListener("click", (event) => {
    if (event.target.id === "support-dialog") return closeSupportTicket();
    const target = event.target.closest("button");
    if (!target) return;
    if (target.dataset.spHead) return headAction(target.dataset.spHead);
    if (target.dataset.spApply) return applyCard(target.dataset.spApply);
    if (target.dataset.spUndo) return undoCard(target.dataset.spUndo);
  });
  $("#sp-send").onclick = sendSupportMessage;
  const input = $("#sp-input");
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      sendSupportMessage();
    }
  });
  // Crece con lo que se escribe (hasta 160 px, como el chat).
  input.addEventListener("input", () => {
    input.style.height = "";
    input.style.height = `${Math.min(input.scrollHeight, 160)}px`;
  });
  $("#sp-card-toggle").onclick = openCardForm;
  $("#sp-card-setting").onchange = renderCardValue;
  $("#sp-card-send").onclick = sendCard;
  $("#sp-card-cancel").onclick = () => $("#sp-card-form").classList.add("hidden");

  listen("soporte-mensajes", ({ payload }) => {
    if (support.open?.community !== payload.community || support.open?.id !== payload.ticket) return;
    support.messages = payload.messages;
    renderTicketMessages();
    if (openTicket()?.by === myUid() && document.hasFocus()) invoke("soporte_visto", { id: payload.community, solicitud: payload.ticket }).catch(() => {});
  });
  let pending = null;
  listen("soporte-cola", ({ payload }) => {
    if (payload.community !== community.id) return;
    clearTimeout(pending);
    pending = setTimeout(() => loadSupport(payload.community), 400);
  });
}

wireSupportEvents();
