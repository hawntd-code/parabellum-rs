"use strict";

// ======================================================================
// Comunidades (docs/plan-diciembre.md, sección 6): la de Juan es la primera. Unirse con un código o una
// invitación por mensaje, anuncios, reglas, servidores con su estado y «Jugar» (prepara el paquete
// oficial y entra directo), y el staff con sus permisos. Los permisos los deciden las reglas de Firebase:
// aquí solo se muestran.
// Usa lo que define app.js: state, invoke, t, $, $$, icon, toast, navigate, openModal, confirmAction…
// ======================================================================

const community = {
  list: null,
  /** La comunidad abierta y todo lo suyo (comunidad.rs › Detail). */
  id: null,
  detail: null,
  loading: false,
  error: null,
  /** Estado de los servidores (ping) por dirección. */
  statuses: {},
  /** Cómo está el paquete oficial de cada servidor en este PC. */
  readiness: {},
  /** Servidores que se están preparando para jugar. */
  preparing: new Set(),
};

const roleNames = () => ({ owner: t("Dueño", "Owner"), admin: t("Administrador", "Admin"), member: t("Miembro", "Member") });
const capNames = () => ({ support: t("Soporte", "Support"), publish: t("Publica paquetes", "Publishes packages"), servers: t("Servidores", "Servers") });
const isAdminRole = (role) => role === "owner" || role === "admin";
const myCaps = () => community.detail?.caps ?? {};
const canAdmin = () => isAdminRole(community.detail?.role);

/** Al entrar a la página: tus comunidades y la abierta (o la primera). */
async function enterCommunities(id = null) {
  if (!state.social?.connected) return renderCommunities();
  if (id) community.id = id;
  try {
    community.list = await invoke("comunidades");
  } catch (error) {
    community.list = [];
    community.error = String(error);
  }
  if (!community.id || !community.list.some((c) => c.id === community.id)) community.id = community.list[0]?.id ?? null;
  renderCommunities();
  if (community.id) loadCommunity(community.id);
}

async function loadCommunity(id) {
  community.id = id;
  community.loading = true;
  community.error = null;
  renderCommunities();
  try {
    const detail = await invoke("comunidad", { id });
    if (community.id !== id) return;
    community.detail = detail;
    community.readiness = {};
  } catch (error) {
    community.detail = null;
    community.error = String(error);
  } finally {
    community.loading = false;
  }
  renderCommunities();
  refreshCommunityStatus();
}

/** El estado de los servidores (ping) y si el paquete oficial está listo en este PC. */
async function refreshCommunityStatus() {
  const detail = community.detail;
  if (!detail) return;
  const addresses = detail.servers.map((s) => s.address);
  if (addresses.length) {
    invoke("estado_servidores", { direcciones: addresses })
      .then((statuses) => {
        community.statuses = { ...community.statuses, ...statuses };
        if (state.page === "communities") renderCommunityServers();
      })
      .catch(() => {});
  }
  for (const server of detail.servers.filter((s) => s.package)) {
    invoke("paquete_comunidad", { id: detail.id, servidor: server.id })
      .then((ready) => {
        community.readiness[server.id] = ready;
        if (state.page === "communities") renderCommunityServers();
      })
      .catch(() => {});
  }
}

function serverStatus(address) {
  return community.statuses[address?.trim().toLowerCase()] ?? null;
}

function communityImage(c, size = "") {
  return c?.image
    ? `<img class="cm-image ${size}" src="${escapeHtml(c.image)}" alt="">`
    : `<span class="cm-image ${size} cm-image-empty">${icon("community")}</span>`;
}

// ---------- Pintar ----------
function renderCommunities() {
  const signedIn = state.social?.connected;
  $("#cm-signed-out").classList.toggle("hidden", !!signedIn);
  $("#cm-main").classList.toggle("hidden", !signedIn);
  $("#cm-create").classList.toggle("hidden", !signedIn || !state.creator);
  if (!signedIn) {
    $("#cm-signed-out").innerHTML = `<span class="chat-empty-icon">${icon("community")}</span><p>${escapeHtml(
      t("Inicia sesión con Google para unirte a comunidades.", "Sign in with Google to join communities."),
    )}</p><button class="btn primary" id="cm-sign-in">${t("Iniciar sesión con Google", "Sign in with Google")}</button>`;
    $("#cm-sign-in").onclick = signInGoogle;
    return;
  }
  const list = community.list ?? [];
  // Varias comunidades: una fila de pestañas arriba.
  $("#cm-tabs").classList.toggle("hidden", list.length < 2);
  $("#cm-tabs").innerHTML = list
    .map((c) => `<button class="cm-tab ${c.id === community.id ? "active" : ""}" data-community="${escapeHtml(c.id)}">${communityImage(c, "tiny")}<span>${escapeHtml(c.name)}</span></button>`)
    .join("");
  const body = $("#cm-body");
  if (community.list === null || (community.loading && !community.detail)) {
    body.innerHTML = `<div class="content-loading"><div class="spinner"></div></div>`;
    return;
  }
  if (!list.length) {
    body.innerHTML = `<div class="cm-empty">${icon("community")}<h3>${escapeHtml(t("Todavía no estás en ninguna comunidad", "You are not in any community yet"))}</h3><p>${escapeHtml(
      t("Escribe arriba el código que te dieron, o pulsa «Unirse» en una invitación que te llegue por mensaje.", "Type the code you were given above, or press «Join» on an invitation you get by message."),
    )}</p></div>`;
    return;
  }
  const d = community.detail;
  if (!d) {
    body.innerHTML = `<div class="content-empty">${escapeHtml(community.error ?? "")}</div>`;
    return;
  }
  const members = d.members.length;
  const staff = d.members.filter((m) => m.role !== "member" || Object.values(m.caps ?? {}).some(Boolean)).length;
  body.innerHTML = `
    <div class="cm-hero">
      ${communityImage(d, "large")}
      <div class="grow">
        <h2>${escapeHtml(d.name)}</h2>
        ${d.description ? `<p class="cm-description">${escapeHtml(d.description)}</p>` : ""}
        <div class="cm-meta">
          <span>${icon("users")}${escapeHtml(plural(members, t("miembro", "member"), t("miembros", "members")))}</span>
          ${staff ? `<span>${icon("crown")}${escapeHtml(t(`${staff} del staff`, `${staff} staff`))}</span>` : ""}
          <span class="tag ${isAdminRole(d.role) ? "accent" : ""}">${escapeHtml(roleNames()[d.role] ?? d.role)}</span>
        </div>
      </div>
      <div class="cm-hero-actions">
        ${canAdmin() ? `<button class="btn" data-cm="invite">${icon("user-plus")}${t("Invitar", "Invite")}</button><button class="circle-btn" data-cm="edit" title="${t("Editar la comunidad", "Edit the community")}">${icon("pencil")}</button>` : ""}
        <button class="circle-btn" data-cm="more" title="${t("Más", "More")}">${icon("more")}</button>
      </div>
    </div>
    <div class="cm-grid">
      <div class="cm-col">
        <section class="cm-section"><div class="cm-section-head"><h3>${t("Servidores", "Servers")}</h3>${
          canAdmin() || myCaps().servers ? `<button class="btn small" data-cm="add-server">${icon("plus")}${t("Agregar", "Add")}</button>` : ""
        }</div><div id="cm-servers"></div></section>
        <section class="cm-section"><div class="cm-section-head"><h3>${t("Anuncios", "Announcements")}</h3>${
          canAdmin() ? `<button class="btn small" data-cm="add-post">${icon("plus")}${t("Nuevo", "New")}</button>` : ""
        }</div>${renderPosts(d)}</section>
      </div>
      <div class="cm-col side">
        <section class="cm-section"><div class="cm-section-head"><h3>${t("Reglas", "Rules")}</h3>${
          canAdmin() ? `<button class="circle-btn small" data-cm="edit-rules" title="${t("Editar las reglas", "Edit the rules")}">${icon("pencil")}</button>` : ""
        }</div>${d.rules ? `<p class="cm-rules">${escapeHtml(d.rules)}</p>` : `<p class="side-note no-top">${t("Todavía no hay reglas.", "There are no rules yet.")}</p>`}</section>
        ${canAdmin() || myCaps().publish ? renderPackages(d) : ""}
        ${canAdmin() ? renderInvites(d) : ""}
        <section class="cm-section"><div class="cm-section-head"><h3>${t("Miembros", "Members")}</h3></div>${renderMembers(d)}</section>
      </div>
    </div>`;
  renderCommunityServers();
  for (const canvas of $$("#cm-body canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
}

function renderPosts(d) {
  if (!d.posts.length) return `<p class="side-note no-top">${t("Sin anuncios por ahora.", "No announcements for now.")}</p>`;
  return d.posts
    .map(
      (p) => `<article class="cm-post">
        <div class="cm-post-head"><b>${escapeHtml(p.title)}</b><span class="faint">${escapeHtml(p.authorName)} · ${escapeHtml(relativeDate(new Date(p.at).toISOString()))}</span>${
          canAdmin() ? `<button class="circle-btn small" data-cm-delete-post="${escapeHtml(p.id)}" title="${t("Borrar", "Delete")}">${icon("trash")}</button>` : ""
        }</div>
        ${p.text ? `<p>${messageTextHtml(p.text)}</p>` : ""}
      </article>`,
    )
    .join("");
}

function renderCommunityServers() {
  const box = $("#cm-servers");
  const d = community.detail;
  if (!box || !d) return;
  if (!d.servers.length) {
    box.innerHTML = `<p class="side-note no-top">${t("Todavía no hay servidores.", "There are no servers yet.")}</p>`;
    return;
  }
  const editable = canAdmin() || myCaps().servers;
  box.innerHTML = d.servers
    .map((s) => {
      const status = serverStatus(s.address);
      const ready = community.readiness[s.id];
      const busy = community.preparing.has(s.id);
      const pack = d.packages.find((p) => p.id === s.package);
      const online = status?.online;
      const players = online ? `${status.players}/${status.maxPlayers}` : "";
      const note = !s.package
        ? t("Entras con la instancia que elijas.", "You join with the instance you choose.")
        : !ready
          ? t("Revisando el paquete oficial…", "Checking the official package…")
          : ready.state === "falta"
            ? t(`Se instala ${pack?.name ?? t("el paquete oficial", "the official package")} antes de entrar.`, `${pack?.name ?? "The official package"} is installed before joining.`)
            : ready.state === "desactualizada"
              ? t(`Se actualiza a la ${ready.wanted} antes de entrar (tus mundos y controles se quedan).`, `It updates to ${ready.wanted} before joining (your worlds and controls stay).`)
              : t(`${pack?.name ?? t("Paquete oficial", "Official package")} listo.`, `${pack?.name ?? "Official package"} ready.`);
      return `<div class="cm-server">
        <div class="cm-server-main">
          <span class="srv-row-icon">${icon("server")}</span>
          <div class="grow">
            <b>${escapeHtml(s.name)}</b>
            <div class="cm-server-lines">
              <button class="cm-address" data-copy="${escapeHtml(s.address)}" title="${t("Copiar la dirección", "Copy the address")}">${icon("copy")}<span>Java · ${escapeHtml(s.address)}</span></button>
              ${s.bedrock ? `<button class="cm-address" data-copy="${escapeHtml(s.bedrock)}" title="${t("Copiar la dirección", "Copy the address")}">${icon("copy")}<span>Bedrock · ${escapeHtml(s.bedrock)}</span></button>` : ""}
            </div>
          </div>
          <span class="cm-state ${online ? "on" : status ? "off" : ""}">${online ? `${t("En línea", "Online")} · ${players}` : status ? t("Apagado", "Offline") : "…"}</span>
          ${editable ? `<button class="circle-btn small" data-cm-edit-server="${escapeHtml(s.id)}" title="${t("Editar", "Edit")}">${icon("pencil")}</button>` : ""}
          <button class="btn primary" data-cm-play="${escapeHtml(s.id)}" ${busy ? "disabled" : ""}>${icon("play")}<span>${busy ? t("Preparando…", "Preparing…") : t("Jugar", "Play")}</span></button>
        </div>
        <p class="cm-server-note">${escapeHtml(note)}</p>
      </div>`;
    })
    .join("");
}

function renderPackages(d) {
  return `<section class="cm-section"><div class="cm-section-head"><h3>${t("Paquete oficial", "Official package")}</h3><button class="btn small" data-cm="add-package">${icon(
    "plus",
  )}${t("Elegir", "Choose")}</button></div>${
    d.packages.length
      ? d.packages
          .map(
            (p) => `<div class="cm-line"><span class="grow"><b>${escapeHtml(p.name)}</b><span class="faint">${escapeHtml(
              [p.minecraft, p.loader, p.version && `v${p.version}`].filter(Boolean).join(" · "),
            )}</span></span><button class="circle-btn small" data-cm-refresh-package="${escapeHtml(p.id)}" title="${t(
              "Pedir la versión que está ahora en Drive",
              "Require the version that is on Drive now",
            )}">${icon("refresh")}</button></div>`,
          )
          .join("")
      : `<p class="side-note no-top">${t("Elige el modpack de Drive que deben tener para entrar.", "Choose the Drive modpack they must have to join.")}</p>`
  }</section>`;
}

function renderInvites(d) {
  return `<section class="cm-section"><div class="cm-section-head"><h3>${t("Códigos de invitación", "Invite codes")}</h3><button class="btn small" data-cm="new-code">${icon(
    "plus",
  )}${t("Nuevo", "New")}</button></div>${
    d.invites.length
      ? d.invites
          .map(
            (i) => `<div class="cm-line"><code class="cm-code">${escapeHtml(i.code.slice(0, 4))}-${escapeHtml(i.code.slice(4))}</code><span class="grow faint">${escapeHtml(
              t(`vence ${new Date(i.expiresAt).toLocaleDateString(locale(), { day: "numeric", month: "short" })}`, `expires ${new Date(i.expiresAt).toLocaleDateString(locale(), { day: "numeric", month: "short" })}`),
            )}</span><button class="circle-btn small" data-copy="${escapeHtml(i.code)}" title="${t("Copiar", "Copy")}">${icon("copy")}</button><button class="circle-btn small" data-cm-delete-code="${escapeHtml(
              i.code,
            )}" title="${t("Borrar", "Delete")}">${icon("trash")}</button></div>`,
          )
          .join("")
      : `<p class="side-note no-top">${t("Crea un código y compártelo, o usa «Invitar» para mandarlo por mensaje.", "Create a code and share it, or use «Invite» to send it by message.")}</p>`
  }</section>`;
}

function renderMembers(d) {
  const owner = d.role === "owner";
  return `<div class="cm-members">${d.members
    .map((m) => {
      const caps = Object.entries(m.caps ?? {})
        .filter(([, on]) => on)
        .map(([cap]) => `<span class="tag">${escapeHtml(capNames()[cap] ?? cap)}</span>`)
        .join("");
      // El dueño administra a todos menos a sí mismo; un administrador, solo a los miembros.
      const manage = !m.you && m.role !== "owner" && (owner || (d.role === "admin" && m.role === "member"));
      return `<div class="cm-member">
        <canvas width="8" height="8" class="head" data-head="${escapeHtml(m.name)}"></canvas>
        <span class="grow"><b>${escapeHtml(m.name)}${m.you ? ` <span class="faint">(${t("tú", "you")})</span>` : ""}</b><span class="cm-badges">${
          m.role !== "member" ? `<span class="tag accent">${escapeHtml(roleNames()[m.role])}</span>` : ""
        }${caps}</span></span>
        ${manage ? `<button class="circle-btn small" data-cm-member="${escapeHtml(m.uid)}" title="${t("Rol y permisos", "Role and permissions")}">${icon("more")}</button>` : ""}
      </div>`;
    })
    .join("")}</div>`;
}

// ---------- Formularios (un diálogo para todo) ----------
/**
 * Pide unos datos: fields = [{ key, label, type: "text"|"textarea"|"select"|"toggle", value, max, options, hint }].
 * Devuelve los valores o null si se canceló.
 */
function askForm({ title, fields, ok, danger = null }) {
  return new Promise((resolve) => {
    $("#cm-form-title").textContent = title;
    $("#cm-form-fields").innerHTML = fields
      .map((f) => {
        const id = `cm-f-${f.key}`;
        const value = escapeHtml(f.value ?? "");
        const input =
          f.type === "textarea"
            ? `<textarea id="${id}" rows="${f.rows ?? 5}" maxlength="${f.max ?? 2000}">${value}</textarea>`
            : f.type === "select"
              ? `<select id="${id}">${f.options.map((o) => `<option value="${escapeHtml(o.value)}" ${o.value === f.value ? "selected" : ""}>${escapeHtml(o.label)}</option>`).join("")}</select>`
              : f.type === "toggle"
                ? `<label class="toggle"><input type="checkbox" id="${id}" ${f.value ? "checked" : ""}><span></span>${escapeHtml(f.label)}</label>`
                : `<input id="${id}" maxlength="${f.max ?? 100}" value="${value}" autocomplete="off" spellcheck="false" ${f.placeholder ? `placeholder="${escapeHtml(f.placeholder)}"` : ""}>`;
        return f.type === "toggle"
          ? `<div class="cm-field">${input}${f.hint ? `<span class="faint">${escapeHtml(f.hint)}</span>` : ""}</div>`
          : `<label class="cm-field"><span class="label">${escapeHtml(f.label)}</span>${input}${f.hint ? `<span class="faint">${escapeHtml(f.hint)}</span>` : ""}</label>`;
      })
      .join("");
    $("#cm-form-ok").textContent = ok;
    $("#cm-form-danger").classList.toggle("hidden", !danger);
    if (danger) $("#cm-form-danger").textContent = danger;
    const finish = (value) => {
      closeModal("#cm-form-dialog");
      resolve(value);
    };
    $("#cm-form-ok").onclick = () => {
      const values = {};
      for (const f of fields) {
        const element = $(`#cm-f-${f.key}`);
        values[f.key] = f.type === "toggle" ? element.checked : element.value.trim();
      }
      finish(values);
    };
    $("#cm-form-danger").onclick = () => finish({ danger: true });
    $("#cm-form-cancel").onclick = () => finish(null);
    state.confirmCancel = () => finish(null);
    openModal("#cm-form-dialog");
    requestAnimationFrame(() => $("#cm-form-fields input, #cm-form-fields textarea")?.focus());
  });
}

async function act(promise, done) {
  try {
    await promise;
    if (done) toast(done);
    await loadCommunity(community.id);
  } catch (error) {
    toast(String(error), "error");
  }
}

// ---------- Acciones ----------
async function joinWithCode(code) {
  code = (code ?? "").trim();
  if (!code) return toast(t("Escribe el código de la comunidad.", "Type the community code."), "error");
  try {
    const preview = await invoke("ver_invitacion", { codigo: code });
    if (!preview.member) {
      const ok = await confirmAction({
        title: t(`Unirte a ${preview.name}`, `Join ${preview.name}`),
        text: preview.description || t("Vas a entrar como miembro.", "You will join as a member."),
        ok: t("Unirme", "Join"),
        primary: true,
      });
      if (!ok) return;
    }
    const id = await invoke("unirse_comunidad", { codigo: code });
    $("#cm-code").value = "";
    toast(preview.member ? t(`Ya eras parte de ${preview.name}.`, `You were already in ${preview.name}.`) : t(`¡Bienvenido a ${preview.name}!`, `Welcome to ${preview.name}!`));
    community.id = id;
    if (state.page !== "communities") navigate("communities", id);
    else enterCommunities(id);
  } catch (error) {
    toast(String(error), "error");
  }
}

async function createCommunity() {
  const values = await askForm({
    title: t("Crear comunidad", "Create community"),
    fields: [
      { key: "name", label: t("Nombre", "Name"), max: 40 },
      { key: "description", label: t("Descripción", "Description"), type: "textarea", max: 500, rows: 3 },
    ],
    ok: t("Crear", "Create"),
  });
  if (!values) return;
  try {
    community.id = await invoke("crear_comunidad", { nombre: values.name, descripcion: values.description });
    toast(t("Comunidad creada: ahora invita a los demás.", "Community created: now invite the others."));
    enterCommunities(community.id);
  } catch (error) {
    toast(String(error), "error");
  }
}

/** Una imagen cuadrada pequeña (256 px, JPEG) para la comunidad: cabe en Firebase. */
async function communityImageFrom(file) {
  const image = await loadImageFile(file);
  const side = Math.min(image.naturalWidth, image.naturalHeight);
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 256;
  const ctx = canvas.getContext("2d");
  ctx.fillStyle = "#1e1d23";
  ctx.fillRect(0, 0, 256, 256);
  ctx.drawImage(image, (image.naturalWidth - side) / 2, (image.naturalHeight - side) / 2, side, side, 0, 0, 256, 256);
  return canvas.toDataURL("image/jpeg", 0.85);
}

async function editCommunity() {
  const d = community.detail;
  const values = await askForm({
    title: t("Editar la comunidad", "Edit the community"),
    fields: [
      { key: "name", label: t("Nombre", "Name"), max: 40, value: d.name },
      { key: "description", label: t("Descripción", "Description"), type: "textarea", max: 500, rows: 3, value: d.description },
      { key: "image", label: t("Cambiar la imagen", "Change the image"), type: "toggle", value: false, hint: t("Al guardar eliges un archivo.", "When saving you choose a file.") },
    ],
    ok: t("Guardar", "Save"),
  });
  if (!values) return;
  let image = null;
  if (values.image) {
    const file = await pickImageFile();
    if (file) image = await communityImageFrom(file).catch(() => null);
  }
  await act(invoke("editar_comunidad", { id: d.id, nombre: values.name, descripcion: values.description, imagen: image }), t("Guardado.", "Saved."));
}

function pickImageFile() {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.click();
  });
}

async function newCode(days = 7) {
  const invite = await invoke("crear_invitacion", { id: community.id, dias: days });
  await loadCommunity(community.id);
  return invite;
}

/** Invitar: un código nuevo y, si quieres, mandarlo a un amigo por mensaje (con «Unirse»). */
async function inviteToCommunity() {
  const d = community.detail;
  const friends = state.social?.friends?.friends ?? [];
  const values = await askForm({
    title: t(`Invitar a ${d.name}`, `Invite to ${d.name}`),
    fields: [
      {
        key: "friend",
        label: t("Mandar por mensaje a", "Send by message to"),
        type: "select",
        value: "",
        options: [{ value: "", label: t("Nadie: solo quiero el código", "Nobody: I only want the code") }, ...friends.map((f) => ({ value: f.uid, label: f.name }))],
      },
      {
        key: "days",
        label: t("El código vence en", "The code expires in"),
        type: "select",
        value: "7",
        options: [
          { value: "1", label: t("1 día", "1 day") },
          { value: "7", label: t("7 días", "7 days") },
          { value: "30", label: t("30 días", "30 days") },
        ],
      },
    ],
    ok: t("Crear código", "Create code"),
  });
  if (!values) return;
  try {
    const invite = await newCode(Number(values.days));
    const pretty = `${invite.code.slice(0, 4)}-${invite.code.slice(4)}`;
    navigator.clipboard?.writeText(invite.code).catch(() => {});
    if (values.friend) {
      const chat = await invoke("chat_con_amigo", { uid: values.friend });
      await invoke("enviar_mensaje", { chat, texto: `${INVITE_PREFIX}${JSON.stringify({ t: "comunidad", n: d.name, c: invite.code })}`, respuesta: null });
      toast(t(`Invitación mandada a ${findFriend(values.friend)?.name ?? ""} (código ${pretty}).`, `Invitation sent to ${findFriend(values.friend)?.name ?? ""} (code ${pretty}).`));
    } else {
      toast(t(`Código ${pretty} copiado.`, `Code ${pretty} copied.`));
    }
  } catch (error) {
    toast(String(error), "error");
  }
}

async function editServer(server = null) {
  const d = community.detail;
  const values = await askForm({
    title: server ? t("Editar servidor", "Edit server") : t("Agregar servidor", "Add server"),
    fields: [
      { key: "name", label: t("Nombre", "Name"), max: 40, value: server?.name ?? "" },
      { key: "address", label: t("Dirección (Java)", "Address (Java)"), max: 100, value: server?.address ?? "", placeholder: "mc.ejemplo.com" },
      { key: "bedrock", label: t("Dirección para Bedrock (opcional)", "Address for Bedrock (optional)"), max: 100, value: server?.bedrock ?? "", placeholder: "mc.ejemplo.com:19132" },
      {
        key: "package",
        label: t("Paquete que piden para entrar", "Package required to join"),
        type: "select",
        value: server?.package ?? "",
        options: [{ value: "", label: t("Ninguno (cualquier instancia)", "None (any instance)") }, ...d.packages.map((p) => ({ value: p.id, label: `${p.name} (${p.minecraft})` }))],
      },
    ],
    ok: t("Guardar", "Save"),
    danger: server ? t("Quitar servidor", "Remove server") : null,
  });
  if (!values) return;
  if (values.danger) return act(invoke("borrar_servidor_comunidad", { id: d.id, servidor: server.id }), t("Servidor quitado.", "Server removed."));
  await act(
    invoke("guardar_servidor_comunidad", {
      id: d.id,
      servidor: { id: server?.id ?? "", name: values.name, address: values.address, bedrock: values.bedrock || null, package: values.package || null, order: server?.order ?? null },
    }),
    t("Servidor guardado.", "Server saved."),
  );
}

async function addPost() {
  const values = await askForm({
    title: t("Nuevo anuncio", "New announcement"),
    fields: [
      { key: "title", label: t("Título", "Title"), max: 80 },
      { key: "text", label: t("Texto", "Text"), type: "textarea", max: 2000 },
    ],
    ok: t("Publicar", "Publish"),
  });
  if (!values) return;
  await act(invoke("publicar_anuncio", { id: community.id, titulo: values.title, texto: values.text }), t("Anuncio publicado.", "Announcement published."));
}

async function editRules() {
  const values = await askForm({
    title: t("Reglas", "Rules"),
    fields: [{ key: "text", label: t("Las verán todos los miembros", "All members will see them"), type: "textarea", max: 4000, rows: 10, value: community.detail.rules }],
    ok: t("Guardar", "Save"),
  });
  if (!values) return;
  await act(invoke("guardar_reglas", { id: community.id, texto: values.text }), t("Reglas guardadas.", "Rules saved."));
}

/** El paquete oficial: un modpack del catálogo de Drive (su versión de ahora es la que se pide). */
async function choosePackage(existing = null) {
  let catalog;
  try {
    catalog = await invoke("catalogo", { recargar: false });
  } catch (error) {
    return toast(String(error), "error");
  }
  const packs = catalog.entries.filter((e) => e.kind !== "base");
  if (!packs.length) return toast(t("No hay modpacks en el catálogo de Drive. Sube primero el de la comunidad.", "There are no modpacks in the Drive catalog. Upload the community's first."), "error");
  const pick = existing
    ? { pack: existing.catalogId }
    : await askForm({
        title: t("Paquete oficial", "Official package"),
        fields: [{ key: "pack", label: t("Modpack de Drive", "Drive modpack"), type: "select", value: packs[0].id, options: packs.map((p) => ({ value: p.id, label: `${p.name} (${p.minecraftVersion})` })) }],
        ok: t("Elegir", "Choose"),
      });
  if (!pick) return;
  const entry = packs.find((p) => p.id === pick.pack);
  if (!entry) return toast(t("Ese modpack ya no está en el catálogo.", "That modpack is no longer in the catalog."), "error");
  await act(
    invoke("guardar_paquete_comunidad", {
      id: community.id,
      paquete: { id: existing?.id ?? "", name: entry.name, catalogId: entry.id, version: entry.version ?? "", minecraft: entry.minecraftVersion, loader: entry.loader ?? "", loaderVersion: "", at: 0 },
    }),
    existing ? t(`Ahora se pide la versión ${entry.version}.`, `Version ${entry.version} is now required.`) : t("Paquete oficial elegido.", "Official package chosen."),
  );
}

async function manageMember(uid) {
  const d = community.detail;
  const m = d.members.find((x) => x.uid === uid);
  if (!m) return;
  const owner = d.role === "owner";
  const fields = [];
  if (owner) {
    fields.push({
      key: "role",
      label: t("Rol", "Role"),
      type: "select",
      value: m.role,
      options: [
        { value: "member", label: roleNames().member },
        { value: "admin", label: t("Administrador (anuncios, reglas, invitaciones y miembros)", "Admin (announcements, rules, invites and members)") },
      ],
    });
  }
  for (const [cap, label] of Object.entries(capNames())) fields.push({ key: cap, label, type: "toggle", value: !!m.caps?.[cap] });
  const values = await askForm({ title: m.name, fields, ok: t("Guardar", "Save"), danger: t("Sacar de la comunidad", "Remove from the community") });
  if (!values) return;
  if (values.danger) {
    const ok = await confirmAction({
      title: t(`Sacar a ${m.name}`, `Remove ${m.name}`),
      text: t("Podrá volver si alguien le da otro código.", "They can come back if someone gives them another code."),
      ok: t("Sacar", "Remove"),
    });
    if (!ok) return;
    return act(invoke("sacar_de_comunidad", { id: d.id, uid }), t(`${m.name} ya no está en la comunidad.`, `${m.name} is no longer in the community.`));
  }
  await act(
    invoke("cambiar_miembro", { id: d.id, uid, rol: values.role ?? m.role, permisos: { support: values.support, publish: values.publish, servers: values.servers } }),
    t("Permisos guardados.", "Permissions saved."),
  );
}

async function leaveCommunity() {
  const d = community.detail;
  if (d.role === "owner") return toast(t("Eres el dueño: no puedes salir de tu comunidad.", "You are the owner: you cannot leave your community."), "error");
  const ok = await confirmAction({ title: t(`Salir de ${d.name}`, `Leave ${d.name}`), text: t("Para volver necesitarás otro código.", "To come back you will need another code."), ok: t("Salir", "Leave") });
  if (!ok) return;
  try {
    await invoke("salir_comunidad", { id: d.id });
    community.id = null;
    community.detail = null;
    enterCommunities();
  } catch (error) {
    toast(String(error), "error");
  }
}

/** «Jugar»: instala o actualiza el paquete oficial si hace falta y entra directo al servidor. */
async function playCommunityServer(sid) {
  const d = community.detail;
  const server = d?.servers.find((s) => s.id === sid);
  if (!server || community.preparing.has(sid)) return;
  community.preparing.add(sid);
  renderCommunityServers();
  try {
    let instance = null;
    if (server.package) {
      const ready = await invoke("paquete_comunidad", { id: d.id, servidor: sid });
      community.readiness[sid] = ready;
      instance = ready.instance;
      if (ready.state === "falta" || ready.state === "desactualizada") {
        toast(
          ready.state === "falta"
            ? t("Instalando el paquete oficial: el avance está en Descargas.", "Installing the official package: the progress is in Downloads.")
            : t("Actualizando el paquete oficial (tus mundos y controles se quedan).", "Updating the official package (your worlds and controls stay)."),
        );
        const result = await invoke("instalar_de_catalogo", { id: ready.catalogId, reparar: false, tarea: newJob() });
        await reloadInstances();
        instance = result.id ?? instance;
        community.readiness[sid] = { ...ready, state: "lista" };
      }
    } else {
      instance = await chooseInstanceFor(server);
    }
    if (!instance) return;
    await play(instance, server.address);
  } catch (error) {
    toast(String(error), "error");
  } finally {
    community.preparing.delete(sid);
    renderCommunityServers();
  }
}

/** Sin paquete oficial: con qué instancia entrar (la última que jugaste primero). */
async function chooseInstanceFor(server) {
  const instances = [...state.instances].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0));
  if (!instances.length) {
    toast(t("Primero crea una instancia.", "Create an instance first."), "error");
    return null;
  }
  const values = await askForm({
    title: t(`Entrar a ${server.name}`, `Join ${server.name}`),
    fields: [{ key: "instance", label: t("Instancia", "Instance"), type: "select", value: instances[0].id, options: instances.map((i) => ({ value: i.id, label: `${i.name} (${i.loader} ${i.minecraftVersion})` })) }],
    ok: t("Jugar", "Play"),
  });
  return values?.instance ?? null;
}

function communityMenu(anchor) {
  const d = community.detail;
  const items = [
    { label: t("Recargar", "Reload"), icon: "refresh", action: () => loadCommunity(d.id) },
    d.role !== "owner" ? { label: t("Salir de la comunidad", "Leave the community"), icon: "trash", danger: true, action: leaveCommunity } : null,
  ].filter(Boolean);
  contextMenu(anchor, items);
}

function wireCommunityEvents() {
  $("#cm-join").onclick = () => joinWithCode($("#cm-code").value);
  $("#cm-code").addEventListener("keydown", (event) => {
    if (event.key === "Enter") joinWithCode(event.target.value);
  });
  $("#cm-create").onclick = createCommunity;
  $("#cm-tabs").onclick = (event) => {
    const tab = event.target.closest("[data-community]");
    if (tab) loadCommunity(tab.dataset.community);
  };
  $("#cm-body").onclick = (event) => {
    const target = event.target.closest("button");
    if (!target) return;
    const d = community.detail;
    if (target.dataset.copy) {
      navigator.clipboard?.writeText(target.dataset.copy).then(() => toast(t("Copiado.", "Copied.")));
      return;
    }
    if (target.dataset.cmPlay) return playCommunityServer(target.dataset.cmPlay);
    if (target.dataset.cmEditServer) return editServer(d.servers.find((s) => s.id === target.dataset.cmEditServer));
    if (target.dataset.cmMember) return manageMember(target.dataset.cmMember);
    if (target.dataset.cmRefreshPackage) return choosePackage(d.packages.find((p) => p.id === target.dataset.cmRefreshPackage));
    if (target.dataset.cmDeletePost) {
      return confirmAction({ title: t("Borrar el anuncio", "Delete the announcement"), text: t("Nadie lo verá más.", "Nobody will see it anymore."), ok: t("Borrar", "Delete") }).then(
        (ok) => ok && act(invoke("borrar_anuncio", { id: d.id, anuncio: target.dataset.cmDeletePost })),
      );
    }
    if (target.dataset.cmDeleteCode) return act(invoke("borrar_invitacion", { id: d.id, codigo: target.dataset.cmDeleteCode }), t("Código borrado.", "Code deleted."));
    const actions = {
      invite: inviteToCommunity,
      edit: editCommunity,
      more: () => communityMenu(target),
      "add-server": () => editServer(null),
      "add-post": addPost,
      "edit-rules": editRules,
      "add-package": () => choosePackage(null),
      "new-code": () => newCode(7).then((i) => toast(t(`Código ${i.code.slice(0, 4)}-${i.code.slice(4)} creado.`, `Code ${i.code.slice(0, 4)}-${i.code.slice(4)} created.`))).catch((e) => toast(String(e), "error")),
    };
    actions[target.dataset.cm]?.();
  };
}

wireCommunityEvents();
