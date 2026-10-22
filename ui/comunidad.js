"use strict";

// ======================================================================
// Comunidades (docs/plan-diciembre.md, sección 6): la de Juan es la primera. Unirse con un código o una
// invitación por mensaje, anuncios, reglas, servidores con su estado y «Jugar» (prepara el paquete
// oficial y entra directo), y el staff con sus permisos. Los permisos los deciden las reglas de Firebase:
// aquí solo se muestran.
// Usa lo que define app.js: state, invoke, t, $, $$, icon, art, toast, navigate, openModal, confirmAction…
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
  /** Servidores que se están preparando para jugar y paquetes que se están instalando. */
  preparing: new Set(),
  /** El nombre de la instancia que se está publicando como paquete oficial. */
  publishing: null,
};

const roleNames = () => ({ owner: t("Dueño", "Owner"), admin: t("Administrador", "Admin"), member: t("Miembro", "Member") });
const capInfo = () => ({
  support: { name: t("Soporte", "Support"), text: t("Atiende las solicitudes de ayuda de los jugadores.", "Handles the players' help requests."), icon: "chat" },
  publish: { name: t("Publica paquetes", "Publishes packages"), text: t("Elige y publica el paquete oficial.", "Chooses and publishes the official package."), icon: "upload" },
  servers: { name: t("Servidores", "Servers"), text: t("Agrega y edita los servidores de la comunidad.", "Adds and edits the community's servers."), icon: "server" },
});
const isAdminRole = (role) => role === "owner" || role === "admin";
const myCaps = () => community.detail?.caps ?? {};
const canAdmin = () => isAdminRole(community.detail?.role);
const canPublish = () => canAdmin() || !!myCaps().publish;
const prettyCode = (code) => `${code.slice(0, 4)}-${code.slice(4)}`;

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
    const summary = community.list?.find((c) => c.id === id);
    if (summary) Object.assign(summary, { name: detail.name, image: detail.image, description: detail.description });
  } catch (error) {
    community.detail = null;
    community.error = String(error);
  } finally {
    community.loading = false;
  }
  renderCommunities();
  refreshCommunityStatus();
  if (community.detail?.id === id) {
    loadSupport(id);
    prefetchPackages();
  }
}

/** El paquete oficial que falta o tiene versión nueva empieza a bajar desde ya: al pulsar «Jugar» ya va
 * avanzado (el motor decide si conviene: uno a la vez, con tope y nunca mientras se juega). */
function prefetchPackages() {
  for (const local of Object.values(community.detail?.local ?? {})) {
    if (local.catalogId && (local.state === "falta" || local.state === "desactualizada")) {
      invoke("precargar_paquete", { id: local.catalogId }).catch(() => {});
    }
  }
}

/** El estado de los servidores (ping). */
function refreshCommunityStatus() {
  const addresses = community.detail?.servers.map((s) => s.address) ?? [];
  if (!addresses.length) return;
  invoke("estado_servidores", { direcciones: addresses })
    .then((statuses) => {
      community.statuses = { ...community.statuses, ...statuses };
      if (state.page === "communities") renderCommunityServers();
    })
    .catch(() => {});
}

function serverStatus(address) {
  return community.statuses[address?.trim().toLowerCase()] ?? null;
}

function communityImage(c, size = "") {
  return c?.image
    ? `<img class="cm-image ${size}" src="${escapeHtml(c.image)}" alt="">`
    : `<span class="cm-image ${size} cm-image-empty">${icon("community")}</span>`;
}

/** El icono de un paquete: el del catálogo, o el de la instancia de origen, o un cubo. */
function packageArt(p) {
  const info = community.detail?.catalog?.[p.id];
  if (info?.icon) return `<img class="cm-pack-art" src="${escapeHtml(info.icon)}" alt="">`;
  const local = community.detail?.local?.[p.id];
  const instance = local?.instance && state.instances.find((i) => i.id === local.instance);
  return `<span class="cm-pack-art">${art(instance ?? { name: p.name })}</span>`;
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
  $("#cm-tabs").classList.toggle("hidden", list.length < 2);
  $("#cm-tabs").innerHTML = list
    .map(
      (c) =>
        `<button class="cm-tab ${c.id === community.id ? "active" : ""} ${support.notices?.communities?.[c.id] ? "notice" : ""}" data-community="${escapeHtml(c.id)}">${communityImage(c, "tiny")}<span>${escapeHtml(c.name)}</span></button>`,
    )
    .join("");
  const body = $("#cm-body");
  if (community.list === null || (community.loading && !community.detail)) {
    body.innerHTML = `<div class="cm-skeleton"><div class="cm-hero"><span class="skel cm-image large"></span><div class="grow"><span class="skel line w30 tall"></span><span class="skel line w60"></span><span class="skel line w25 thin"></span></div></div><div class="cm-grid"><div class="cm-col"><div class="cm-section"><span class="skel line w20"></span><span class="skel block"></span><span class="skel block"></span></div></div><div class="cm-col side"><div class="cm-section"><span class="skel line w40"></span><span class="skel block short"></span></div><div class="cm-section"><span class="skel line w30"></span><span class="skel line w80"></span><span class="skel line w60"></span></div></div></div></div>`;
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
      ${canAdmin() ? `<button class="cm-hero-image" data-cm="image" title="${t("Cambiar la imagen", "Change the image")}">${communityImage(d, "large")}<span class="cm-camera">${icon("camera")}</span></button>` : communityImage(d, "large")}
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
        ${supportStaff() ? supportSectionHtml() : ""}
        <section class="cm-section"><div class="cm-section-head"><h3>${t("Anuncios", "Announcements")}</h3>${
          canAdmin() ? `<button class="btn small" data-cm="add-post">${icon("plus")}${t("Nuevo anuncio", "New announcement")}</button>` : ""
        }</div>${renderPosts(d)}</section>
      </div>
      <div class="cm-col side">
        ${renderPackages(d)}
        ${supportStaff() ? "" : supportSectionHtml()}
        <section class="cm-section"><div class="cm-section-head"><h3>${t("Reglas", "Rules")}</h3>${
          canAdmin() ? `<button class="circle-btn small" data-cm="edit-rules" title="${t("Editar las reglas", "Edit the rules")}">${icon("pencil")}</button>` : ""
        }</div>${d.rules ? `<div class="markdown cm-markdown">${markdownHtml(d.rules)}</div>` : `<p class="side-note no-top">${t("Todavía no hay reglas.", "There are no rules yet.")}</p>`}</section>
        ${canAdmin() ? renderInvites(d) : ""}
        <section class="cm-section"><div class="cm-section-head"><h3>${t("Miembros", "Members")}</h3><span class="faint">${members}</span></div>${renderMembers(d)}</section>
      </div>
    </div>`;
  renderCommunityServers();
  renderSupportSection();
  for (const canvas of $$("#cm-body canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
}

/** Markdown de quien administra (negritas, listas, enlaces), ya limpio (Markdown.toFragment). */
function markdownHtml(text) {
  const box = document.createElement("div");
  box.appendChild(Markdown.toFragment(text));
  // Sin imágenes de otros sitios (cargarlas avisaría a ese sitio de quién abrió la comunidad).
  for (const media of $$("img, .video-link", box)) media.remove();
  return box.innerHTML;
}

function renderPosts(d) {
  if (!d.posts.length) return `<p class="side-note no-top">${t("Sin anuncios por ahora.", "No announcements for now.")}</p>`;
  return d.posts
    .map(
      (p) => `<article class="cm-post">
        <div class="cm-post-head"><span class="cm-post-dot"></span><b>${escapeHtml(p.title)}</b><span class="faint">${escapeHtml(p.authorName)} · ${escapeHtml(relativeDate(new Date(p.at).toISOString()))}</span>${
          canAdmin() ? `<button class="circle-btn small" data-cm-delete-post="${escapeHtml(p.id)}" title="${t("Borrar", "Delete")}">${icon("trash")}</button>` : ""
        }</div>
        ${p.text ? `<div class="markdown cm-markdown">${markdownHtml(p.text)}</div>` : ""}
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
      const pack = d.packages.find((p) => p.id === s.package);
      const ready = pack && d.local?.[pack.id];
      const busy = community.preparing.has(s.id) || (pack && community.preparing.has(pack.id));
      const online = status?.online;
      const note = !pack
        ? t("Entras con la instancia que elijas.", "You join with the instance you choose.")
        : !ready
          ? ""
          : ready.state === "falta"
            ? t(`Al pulsar «Jugar» se instala ${pack.name}.`, `Pressing «Play» installs ${pack.name}.`)
            : ready.state === "desactualizada"
              ? t(`Al pulsar «Jugar» se actualiza ${pack.name} (tus mundos y controles se quedan).`, `Pressing «Play» updates ${pack.name} (your worlds and controls stay).`)
              : ready.state === "modificada"
                ? t(`Tu ${pack.name} tiene cambios (${changesSummary(ready.changes)}): al pulsar «Jugar» se ofrece restaurarlo.`, `Your ${pack.name} has changes (${changesSummary(ready.changes)}): pressing «Play» offers to restore it.`)
                : t(`Con ${pack.name}, listo en este PC.`, `With ${pack.name}, ready on this PC.`);
      return `<div class="cm-server ${online ? "on" : ""}">
        <div class="cm-server-main">
          <span class="cm-server-icon">${icon("server")}<i class="${online ? "on" : status ? "off" : ""}"></i></span>
          <div class="grow">
            <b>${escapeHtml(s.name)}</b>
            <div class="cm-server-lines">
              <button class="cm-address" data-copy="${escapeHtml(s.address)}" title="${t("Copiar la dirección", "Copy the address")}">${icon("copy")}<span>Java · ${escapeHtml(s.address)}</span></button>
              ${s.bedrock ? `<button class="cm-address" data-copy="${escapeHtml(s.bedrock)}" title="${t("Copiar la dirección", "Copy the address")}">${icon("copy")}<span>Bedrock · ${escapeHtml(s.bedrock)}</span></button>` : ""}
            </div>
          </div>
          <span class="cm-state ${online ? "on" : status ? "off" : ""}">${online ? `${status.players}/${status.maxPlayers} ${t("jugando", "playing")}` : status ? t("Apagado", "Offline") : "…"}</span>
          ${editable ? `<button class="circle-btn small" data-cm-edit-server="${escapeHtml(s.id)}" title="${t("Editar", "Edit")}">${icon("pencil")}</button>` : ""}
          <button class="btn primary" data-cm-play="${escapeHtml(s.id)}" ${busy ? "disabled" : ""}>${icon("play")}<span>${busy ? t("Preparando…", "Preparing…") : t("Jugar", "Play")}</span></button>
        </div>
        ${note ? `<p class="cm-server-note ${ready?.state === "modificada" ? "warn" : ""}">${pack ? icon(ready?.state === "lista" ? "check-circle" : ready?.state === "modificada" ? "alert" : "download") : ""}${escapeHtml(note)}</p>` : ""}
      </div>`;
    })
    .join("");
}

/** Estado del paquete en este PC, con lo que se puede hacer. */
function packageStateHtml(p) {
  const local = community.detail.local?.[p.id];
  if (!local) return "";
  const busy = community.preparing.has(p.id);
  if (local.source) {
    return local.changes
      ? `<span class="cm-chip warn" title="${escapeHtml(changesSummary(local.changes))}">${icon("alert")}${t("Cambió desde que la publicaste", "Changed since you published it")}</span>`
      : `<span class="cm-chip ok">${icon("check")}${t("Tu instancia (la publicaste tú)", "Your instance (you published it)")}</span>`;
  }
  if (local.state === "lista") return `<span class="cm-chip ok">${icon("check")}${t("Instalado", "Installed")}</span>`;
  if (local.state === "modificada") {
    return `<button class="cm-chip warn as-button" data-cm-changes="${escapeHtml(p.id)}" title="${t("Ver qué cambió", "See what changed")}">${icon("alert")}${t("Modificado", "Modified")} · ${escapeHtml(changesSummary(local.changes))}</button><button class="btn small" data-cm-install="${escapeHtml(p.id)}" ${busy ? "disabled" : ""}>${icon("refresh")}${busy ? t("Restaurando…", "Restoring…") : t("Restaurar", "Restore")}</button>`;
  }
  const label = local.state === "falta" ? t("Instalar", "Install") : t("Actualizar", "Update");
  return `<span class="cm-chip ${local.state === "falta" ? "" : "warn"}">${local.state === "falta" ? t("No instalado", "Not installed") : t("Hay versión nueva", "New version")}</span><button class="btn small primary" data-cm-install="${escapeHtml(p.id)}" ${busy ? "disabled" : ""}>${icon("download")}${busy ? t("Instalando…", "Installing…") : label}</button>`;
}

function renderPackages(d) {
  const publish = canPublish();
  if (!d.packages.length && !publish) return "";
  return `<section class="cm-section"><div class="cm-section-head"><h3>${t("Paquete oficial", "Official package")}</h3>${
    publish ? `<button class="btn small" data-cm="add-package">${icon(d.packages.length ? "plus" : "upload")}${d.packages.length ? t("Otro", "Another") : t("Elegir", "Choose")}</button>` : ""
  }</div>${
    d.packages.length
      ? d.packages
          .map((p) => {
            const info = d.catalog?.[p.id];
            const facts = [p.minecraft, p.loader, info?.mods ? plural(info.mods, "mod", "mods") : null, info?.size ? formatSize(info.size) : null].filter(Boolean).join(" · ");
            return `<div class="cm-pack">
              ${packageArt(p)}
              <div class="grow">
                <b>${escapeHtml(p.name)}</b>
                <span class="faint">${escapeHtml(facts)}</span>
                <div class="cm-pack-state">${packageStateHtml(p)}</div>
              </div>
              ${publish ? `<button class="circle-btn small" data-cm-package="${escapeHtml(p.id)}" title="${t("Opciones del paquete", "Package options")}">${icon("more")}</button>` : ""}
            </div>`;
          })
          .join("")
      : community.publishing
        ? ""
        : `<div class="cm-pack-empty">${icon("cubes")}<p>${t("Elige una de tus instancias: se sube sola a Drive y los jugadores la instalan al pulsar «Jugar».", "Choose one of your instances: it uploads itself to Drive and players install it by pressing «Play».")}</p></div>`
  }${
    community.publishing
      ? `<div class="cm-pack-busy"><div class="spinner small"></div><span>${escapeHtml(t(`Publicando ${community.publishing}… (el avance está en Descargas)`, `Publishing ${community.publishing}… (the progress is in Downloads)`))}</span></div>`
      : ""
  }</section>`;
}

function renderInvites(d) {
  return `<section class="cm-section"><div class="cm-section-head"><h3>${t("Códigos de invitación", "Invite codes")}</h3><button class="btn small" data-cm="invite">${icon("user-plus")}${t("Invitar", "Invite")}</button></div>${
    d.invites.length
      ? d.invites
          .map(
            (i) => `<div class="cm-line"><code class="cm-code">${escapeHtml(prettyCode(i.code))}</code><span class="grow faint">${escapeHtml(
              t(`vence ${new Date(i.expiresAt).toLocaleDateString(locale(), { day: "numeric", month: "short" })}`, `expires ${new Date(i.expiresAt).toLocaleDateString(locale(), { day: "numeric", month: "short" })}`),
            )}</span><button class="circle-btn small" data-copy="${escapeHtml(i.code)}" title="${t("Copiar", "Copy")}">${icon("copy")}</button><button class="circle-btn small" data-cm-delete-code="${escapeHtml(
              i.code,
            )}" title="${t("Borrar", "Delete")}">${icon("trash")}</button></div>`,
          )
          .join("")
      : `<p class="side-note no-top">${t("Sin códigos vigentes. «Invitar» crea uno y lo manda a quien elijas.", "No valid codes. «Invite» creates one and sends it to whoever you choose.")}</p>`
  }</section>`;
}

function renderMembers(d) {
  const owner = d.role === "owner";
  return `<div class="cm-members">${d.members
    .map((m) => {
      const caps = Object.entries(m.caps ?? {})
        .filter(([, on]) => on)
        .map(([cap]) => `<span class="tag">${escapeHtml(capInfo()[cap]?.name ?? cap)}</span>`)
        .join("");
      // El dueño administra a todos menos a sí mismo; un administrador, solo a los miembros.
      const manage = !m.you && m.role !== "owner" && (owner || (d.role === "admin" && m.role === "member"));
      return `<${manage ? `button data-cm-member="${escapeHtml(m.uid)}" title="${t("Rol y permisos", "Role and permissions")}"` : "div"} class="cm-member ${manage ? "manage" : ""}">
        <canvas width="8" height="8" class="head" data-head="${escapeHtml(m.name)}"></canvas>
        <span class="grow"><b>${escapeHtml(m.name)}${m.you ? ` <span class="faint">(${t("tú", "you")})</span>` : ""}</b>${
          m.role !== "member" || caps ? `<span class="cm-badges">${m.role !== "member" ? `<span class="tag accent">${escapeHtml(roleNames()[m.role])}</span>` : ""}${caps}</span>` : ""
        }</span>
        ${manage ? icon("right") : ""}
      </${manage ? "button" : "div"}>`;
    })
    .join("")}</div>`;
}

// ---------- Ventanas de la comunidad (un diálogo, cada uno con su forma) ----------
/**
 * Abre #cm-dialog: cabecera con icono, título y subtítulo; `body` (HTML); abajo, quitar/borrar
 * (`danger`), Cancelar y el botón principal. `onOk` y `onDanger` devuelven true para cerrar (un error
 * se muestra dentro, sin cerrar). `mount(el)` conecta lo de adentro. Devuelve una promesa que se
 * resuelve al cerrar.
 */
function cmDialog({ title, subtitle = "", glyph = "community", wide = false, body, ok = null, danger = null, secondary = null, cancel = t("Cancelar", "Cancel"), onOk, onDanger, mount }) {
  return new Promise((resolve) => {
    const dialog = $("#cm-dialog");
    dialog.querySelector(".modal").classList.toggle("wide", wide);
    $("#cm-dialog-icon").innerHTML = icon(glyph);
    $("#cm-dialog-title").textContent = title;
    $("#cm-dialog-subtitle").textContent = subtitle;
    $("#cm-dialog-subtitle").classList.toggle("hidden", !subtitle);
    $("#cm-dialog-body").innerHTML = body;
    $("#cm-dialog-error").classList.add("hidden");
    const okButton = $("#cm-dialog-ok");
    okButton.classList.toggle("hidden", !ok);
    okButton.disabled = false;
    if (ok) okButton.innerHTML = ok;
    $("#cm-dialog-danger").classList.toggle("hidden", !danger);
    if (danger) $("#cm-dialog-danger").innerHTML = danger;
    // Un segundo camino (se resuelve con "secundario"): por ejemplo, «Jugar así».
    $("#cm-dialog-secondary").classList.toggle("hidden", !secondary);
    if (secondary) $("#cm-dialog-secondary").innerHTML = secondary;
    $("#cm-dialog-cancel").textContent = cancel;
    const finish = (value) => {
      closeModal("#cm-dialog");
      resolve(value);
    };
    const run = async (handler, button) => {
      if (!handler) return finish(true);
      button.disabled = true;
      $("#cm-dialog-error").classList.add("hidden");
      try {
        if (await handler()) finish(true);
      } catch (error) {
        $("#cm-dialog-error").textContent = String(error);
        $("#cm-dialog-error").classList.remove("hidden");
      } finally {
        button.disabled = false;
      }
    };
    okButton.onclick = () => run(onOk, okButton);
    $("#cm-dialog-danger").onclick = () => run(onDanger, $("#cm-dialog-danger"));
    $("#cm-dialog-cancel").onclick = () => finish(false);
    $("#cm-dialog-secondary").onclick = () => finish("secundario");
    $("#cm-dialog-close").onclick = () => finish(false);
    community.cancelDialog = () => finish(false);
    // Cada ventana tiene su número: lo que llega tarde (como el catálogo) no pinta sobre otra.
    community.dialogSeq = (community.dialogSeq ?? 0) + 1;
    openModal("#cm-dialog");
    mount?.($("#cm-dialog-body"));
    requestAnimationFrame(() => $("#cm-dialog-body [data-autofocus]")?.focus());
  });
}

/** Un campo de texto con su contador. */
function textField({ id, label, value = "", max, placeholder = "", rows = 0, hint = "", autofocus = false }) {
  const input = rows
    ? `<textarea id="${id}" rows="${rows}" maxlength="${max}" placeholder="${escapeHtml(placeholder)}" ${autofocus ? "data-autofocus" : ""}>${escapeHtml(value)}</textarea>`
    : `<input id="${id}" maxlength="${max}" value="${escapeHtml(value)}" placeholder="${escapeHtml(placeholder)}" autocomplete="off" spellcheck="false" ${autofocus ? "data-autofocus" : ""}>`;
  return `<label class="cm-field"><span class="cm-field-top"><span class="label">${escapeHtml(label)}</span><span class="faint cm-count" data-count="${id}">${value.length}/${max}</span></span>${input}${
    hint ? `<span class="faint cm-hint">${hint}</span>` : ""
  }</label>`;
}

/** Los contadores de letras de los campos de un diálogo. */
function wireCounters(root) {
  for (const counter of $$("[data-count]", root)) {
    const input = $(`#${counter.dataset.count}`, root);
    input.addEventListener("input", () => (counter.textContent = `${input.value.length}/${input.maxLength}`));
  }
}

function pickImageFile() {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*,.heic,.heif";
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.click();
  });
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

/** Crear o editar la comunidad: la imagen se cambia con un clic sobre ella. */
async function communityForm(existing = null) {
  let image = existing?.image ?? null;
  let changed = false;
  const imageHtml = () => (image ? `<img src="${escapeHtml(image)}" alt="">` : icon("community"));
  const saved = await cmDialog({
    title: existing ? t("Editar la comunidad", "Edit the community") : t("Crear comunidad", "Create community"),
    subtitle: existing ? existing.name : t("Tú serás su dueño; después invitas a los demás.", "You will be its owner; then you invite the others."),
    glyph: existing ? "pencil" : "plus",
    body: `<div class="cm-identity">
        <button class="cm-image-pick ${image ? "has-image" : ""}" id="cm-image-pick" title="${t("Cambiar la imagen", "Change the image")}">${imageHtml()}<span>${icon("camera")}${t("Cambiar", "Change")}</span></button>
        <div class="grow">${textField({ id: "cm-name", label: t("Nombre", "Name"), value: existing?.name ?? "", max: 40, autofocus: true })}</div>
      </div>
      ${textField({ id: "cm-description", label: t("Descripción", "Description"), value: existing?.description ?? "", max: 500, rows: 3, placeholder: t("De qué se trata, qué versión, cuándo abre…", "What it is about, which version, when it opens…") })}`,
    ok: existing ? t("Guardar", "Save") : t("Crear comunidad", "Create community"),
    mount: (root) => {
      wireCounters(root);
      $("#cm-image-pick", root).onclick = async () => {
        const file = await pickImageFile();
        if (!file) return;
        try {
          image = await communityImageFrom(file);
          changed = true;
          $("#cm-image-pick", root).classList.add("has-image");
          $("#cm-image-pick", root).innerHTML = `${imageHtml()}<span>${icon("camera")}${t("Cambiar", "Change")}</span>`;
        } catch {
          toast(t("No se pudo abrir esa imagen.", "That image could not be opened."), "error");
        }
      };
    },
    onOk: async () => {
      const name = $("#cm-name").value.trim();
      const description = $("#cm-description").value.trim();
      if (name.length < 2) throw t("Escribe un nombre (2 letras o más).", "Write a name (2 characters or more).");
      if (existing) {
        await invoke("editar_comunidad", { id: existing.id, nombre: name, descripcion: description, imagen: changed ? image : null });
      } else {
        community.id = await invoke("crear_comunidad", { nombre: name, descripcion: description });
        if (changed) await invoke("editar_comunidad", { id: community.id, nombre: name, descripcion: description, imagen: image });
      }
      return true;
    },
  });
  if (!saved) return;
  toast(existing ? t("Guardado.", "Saved.") : t("Comunidad creada: ahora invita a los demás.", "Community created: now invite the others."));
  enterCommunities(community.id);
}

/** Cambiar la imagen directo (clic en la imagen de arriba). */
async function quickImage() {
  const d = community.detail;
  const file = await pickImageFile();
  if (!file) return;
  try {
    const image = await communityImageFrom(file);
    await invoke("editar_comunidad", { id: d.id, nombre: d.name, descripcion: d.description, imagen: image });
    toast(t("Imagen cambiada.", "Image changed."));
    loadCommunity(d.id);
  } catch (error) {
    toast(String(error), "error");
  }
}

/** Anuncio o reglas: texto con formato y su vista previa. */
function markdownForm({ title, subtitle, glyph, withTitle, text = "", max, ok, onSave }) {
  return cmDialog({
    title,
    subtitle,
    glyph,
    wide: true,
    body: `${withTitle ? textField({ id: "cm-post-title", label: t("Título", "Title"), max: 80, autofocus: true, placeholder: t("Por ejemplo: ¡Abrimos el 1 de diciembre!", "For example: We open on December 1!") }) : ""}
      <div class="cm-editor">
        ${textField({ id: "cm-text", label: withTitle ? t("Texto", "Text") : t("Lo verán todos los miembros", "All members will see it"), value: text, max, rows: withTitle ? 7 : 12, autofocus: !withTitle, hint: t("Admite **negritas**, *cursivas*, listas con «-» y enlaces.", "Supports **bold**, *italics*, lists with «-» and links.") })}
        <div class="cm-preview"><span class="label">${t("Vista previa", "Preview")}</span><div class="markdown cm-markdown" id="cm-preview"></div></div>
      </div>`,
    ok,
    mount: (root) => {
      wireCounters(root);
      const update = () => {
        const value = $("#cm-text", root).value.trim();
        const heading = withTitle ? $("#cm-post-title", root).value.trim() : "";
        $("#cm-preview", root).innerHTML =
          (heading ? `<b class="cm-preview-title">${escapeHtml(heading)}</b>` : "") + (value ? markdownHtml(value) : `<p class="faint">${t("Escribe algo para verlo aquí.", "Write something to see it here.")}</p>`);
      };
      for (const input of $$("input, textarea", root)) input.addEventListener("input", update);
      update();
    },
    onOk: () => onSave($("#cm-text").value.trim(), withTitle ? $("#cm-post-title").value.trim() : ""),
  });
}

async function addPost() {
  const ok = await markdownForm({
    title: t("Nuevo anuncio", "New announcement"),
    subtitle: t("Lo verán todos los miembros arriba de la comunidad.", "All members will see it at the top of the community."),
    glyph: "broadcast",
    withTitle: true,
    max: 2000,
    ok: `${icon("send")}${t("Publicar", "Publish")}`,
    onSave: async (text, heading) => {
      if (!heading) throw t("Escribe un título.", "Write a title.");
      await invoke("publicar_anuncio", { id: community.id, titulo: heading, texto: text });
      return true;
    },
  });
  if (ok) {
    toast(t("Anuncio publicado.", "Announcement published."));
    loadCommunity(community.id);
  }
}

async function editRules() {
  const ok = await markdownForm({
    title: t("Reglas", "Rules"),
    subtitle: community.detail.name,
    glyph: "flag",
    text: community.detail.rules,
    max: 4000,
    ok: t("Guardar", "Save"),
    onSave: async (text) => {
      await invoke("guardar_reglas", { id: community.id, texto: text });
      return true;
    },
  });
  if (ok) {
    toast(t("Reglas guardadas.", "Rules saved."));
    loadCommunity(community.id);
  }
}

/** Servidor: nombre, direcciones (con «Probar») y el paquete que piden, en tarjetas. */
async function editServer(server = null) {
  const d = community.detail;
  const packageOption = (p) => {
    const info = d.catalog?.[p.id];
    return `<label class="cm-option"><input type="radio" name="cm-server-package" value="${escapeHtml(p.id)}" ${server?.package === p.id || (!server && d.packages.length === 1) ? "checked" : ""}>${packageArt(p)}<span class="grow"><b>${escapeHtml(p.name)}</b><span class="faint">${escapeHtml(
      [p.minecraft, p.loader, info?.mods ? plural(info.mods, "mod", "mods") : null].filter(Boolean).join(" · "),
    )}</span></span></label>`;
  };
  const saved = await cmDialog({
    title: server ? t("Editar servidor", "Edit server") : t("Agregar servidor", "Add server"),
    subtitle: d.name,
    glyph: "server",
    body: `${textField({ id: "cm-server-name", label: t("Nombre", "Name"), value: server?.name ?? "", max: 40, autofocus: true, placeholder: t("Por ejemplo: Survival", "For example: Survival") })}
      <div class="cm-field"><span class="label">${t("Dirección para Java", "Address for Java")}</span>
        <div class="cm-inline"><input id="cm-server-address" maxlength="100" value="${escapeHtml(server?.address ?? "")}" placeholder="mc.ejemplo.com" autocomplete="off" spellcheck="false"><button class="btn" id="cm-server-test" type="button">${icon("refresh")}${t("Probar", "Test")}</button></div>
        <span class="cm-test faint" id="cm-server-test-result"></span>
      </div>
      ${textField({ id: "cm-server-bedrock", label: t("Dirección para Bedrock (opcional)", "Address for Bedrock (optional)"), value: server?.bedrock ?? "", max: 100, placeholder: "mc.ejemplo.com:19132", hint: t("Solo se muestra para copiarla: Bedrock entra con su propio juego.", "It is only shown to copy it: Bedrock joins with its own game.") })}
      <div class="cm-field"><span class="label">${t("Paquete que piden para entrar", "Package required to join")}</span>
        <div class="cm-options">
          <label class="cm-option"><input type="radio" name="cm-server-package" value="" ${!server?.package && !(!server && d.packages.length === 1) ? "checked" : ""}><span class="cm-pack-art cm-pack-none">${icon("cubes")}</span><span class="grow"><b>${t("Ninguno", "None")}</b><span class="faint">${t("Cada quien entra con la instancia que elija.", "Everyone joins with the instance they choose.")}</span></span></label>
          ${d.packages.map(packageOption).join("")}
        </div>
      </div>`,
    ok: t("Guardar", "Save"),
    danger: server ? `${icon("trash")}${t("Quitar servidor", "Remove server")}` : null,
    mount: (root) => {
      wireCounters(root);
      $("#cm-server-test", root).onclick = async () => {
        const address = $("#cm-server-address", root).value.trim();
        const result = $("#cm-server-test-result", root);
        if (!address) return;
        result.textContent = t("Probando…", "Testing…");
        result.className = "cm-test faint";
        try {
          const statuses = await invoke("estado_servidores", { direcciones: [address] });
          const status = statuses[address.toLowerCase()];
          result.textContent = status?.online
            ? t(`Responde: ${status.players}/${status.maxPlayers} jugando.`, `It answers: ${status.players}/${status.maxPlayers} playing.`)
            : t("No responde ahora (puede estar apagado).", "It does not answer now (it may be off).");
          result.className = `cm-test ${status?.online ? "ok" : "warn"}`;
        } catch (error) {
          result.textContent = String(error);
        }
      };
    },
    onOk: async () => {
      const name = $("#cm-server-name").value.trim();
      const address = $("#cm-server-address").value.trim();
      if (!name || !address) throw t("Escribe el nombre y la dirección.", "Write the name and the address.");
      await invoke("guardar_servidor_comunidad", {
        id: d.id,
        servidor: {
          id: server?.id ?? "",
          name,
          address,
          bedrock: $("#cm-server-bedrock").value.trim() || null,
          package: $("input[name=cm-server-package]:checked")?.value || null,
          order: server?.order ?? null,
        },
      });
      return true;
    },
    onDanger: async () => {
      await invoke("borrar_servidor_comunidad", { id: d.id, servidor: server.id });
      return true;
    },
  });
  if (saved) loadCommunity(d.id);
}

/** Invitar: el código (para copiarlo) y a quién mandarlo por mensaje, amigos o grupos, varios a la vez. */
async function inviteToCommunity() {
  const d = community.detail;
  const friends = state.social?.friends?.friends ?? [];
  const groups = (state.chats ?? []).filter((c) => c.kind === "grupo");
  let invite = d.invites.find((i) => i.expiresAt - Date.now() > 86400e3) ?? null;
  const days = () => Math.max(1, Math.round((invite.expiresAt - Date.now()) / 86400e3));
  const targetRow = (kind, id, name, avatar, sub) =>
    `<label class="cm-target" data-name="${escapeHtml(name.toLowerCase())}"><input type="checkbox" value="${kind}:${escapeHtml(id)}">${avatar}<span class="grow"><b>${escapeHtml(name)}</b><span class="faint">${escapeHtml(sub)}</span></span></label>`;
  const sent = await cmDialog({
    title: t(`Invitar a ${d.name}`, `Invite to ${d.name}`),
    subtitle: t("Les llega un mensaje con «Unirse»; o comparte el código.", "They get a message with «Join»; or share the code."),
    glyph: "user-plus",
    wide: true,
    body: `<div class="cm-invite-code">
        <div class="grow"><span class="label">${t("Código", "Code")}</span><code id="cm-invite-code">${invite ? escapeHtml(prettyCode(invite.code)) : "····-····"}</code><span class="faint" id="cm-invite-expiry"></span></div>
        <button class="btn" id="cm-invite-copy" type="button">${icon("copy")}${t("Copiar", "Copy")}</button>
      </div>
      <div class="cm-field"><span class="label">${t("Vence en", "Expires in")}</span>
        <div class="segmented" id="cm-invite-days"><button type="button" data-days="1">${t("1 día", "1 day")}</button><button type="button" data-days="7">${t("7 días", "7 days")}</button><button type="button" data-days="30">${t("30 días", "30 days")}</button></div>
      </div>
      <div class="cm-field"><span class="label">${t("Mandar por mensaje a", "Send by message to")}</span>
        <div class="search small cm-target-search">${icon("search")}<input id="cm-invite-search" placeholder="${t("Buscar amigos y grupos", "Search friends and groups")}" autocomplete="off" spellcheck="false"></div>
        <div class="cm-targets" id="cm-targets">
          ${groups.length ? `<p class="cm-targets-label">${t("Grupos", "Groups")}</p>${groups.map((g) => targetRow("c", g.id, g.name, `<span class="cm-target-avatar">${groupAvatar(g)}</span>`, plural(g.members?.length ?? 0, t("miembro", "member"), t("miembros", "members")))).join("")}` : ""}
          ${friends.length ? `<p class="cm-targets-label">${t("Amigos", "Friends")}</p>${friends.map((f) => targetRow("f", f.uid, f.name, `<canvas width="8" height="8" class="head" data-head="${escapeHtml(f.name)}"></canvas>`, friendStatus(f))).join("")}` : ""}
          ${!groups.length && !friends.length ? `<p class="side-note no-top">${t("Todavía no tienes amigos ni grupos: comparte el código.", "You have no friends or groups yet: share the code.")}</p>` : ""}
        </div>
      </div>`,
    ok: `${icon("send")}${t("Listo", "Done")}`,
    cancel: t("Cerrar", "Close"),
    mount: async (root) => {
      for (const canvas of $$("canvas[data-head]", root)) drawHead(canvas, canvas.dataset.head);
      const showCode = () => {
        $("#cm-invite-code", root).textContent = invite ? prettyCode(invite.code) : "····-····";
        $("#cm-invite-expiry", root).textContent = invite ? t(`Vence en ${plural(days(), "día", "días")}`, `Expires in ${plural(days(), "day", "days")}`) : "";
        const wanted = invite ? (days() <= 1 ? 1 : days() <= 7 ? 7 : 30) : 7;
        for (const b of $$("[data-days]", root)) b.classList.toggle("active", Number(b.dataset.days) === wanted);
      };
      const fresh = async (count) => {
        try {
          invite = await invoke("crear_invitacion", { id: d.id, dias: count });
          showCode();
        } catch (error) {
          toast(String(error), "error");
        }
      };
      for (const b of $$("[data-days]", root)) b.onclick = () => fresh(Number(b.dataset.days));
      $("#cm-invite-copy", root).onclick = () => invite && navigator.clipboard?.writeText(invite.code).then(() => toast(t(`Código ${prettyCode(invite.code)} copiado.`, `Code ${prettyCode(invite.code)} copied.`)));
      $("#cm-invite-search", root).oninput = (event) => {
        const query = event.target.value.trim().toLowerCase();
        for (const row of $$(".cm-target", root)) row.classList.toggle("hidden", !!query && !row.dataset.name.includes(query));
      };
      const updateOk = () => {
        const count = $$(".cm-target input:checked", root).length;
        $("#cm-dialog-ok").innerHTML = count ? `${icon("send")}${t(`Mandar invitación (${count})`, `Send invitation (${count})`)}` : t("Listo", "Done");
      };
      for (const input of $$(".cm-target input", root)) input.onchange = updateOk;
      showCode();
      if (!invite) await fresh(7);
    },
    onOk: async () => {
      const targets = $$(".cm-target input:checked").map((i) => i.value);
      if (!targets.length) return true;
      const text = `${INVITE_PREFIX}${JSON.stringify({ t: "comunidad", n: d.name, c: invite.code })}`;
      for (const target of targets) {
        const chat = target.startsWith("f:") ? await invoke("chat_con_amigo", { uid: target.slice(2) }) : target.slice(2);
        await invoke("enviar_mensaje", { chat, texto: text, respuesta: null });
      }
      toast(t(`Invitación mandada (${targets.length}).`, `Invitation sent (${targets.length}).`));
      return true;
    },
  });
  if (sent) loadCommunity(d.id);
  else if (!d.invites.length) loadCommunity(d.id); // el código que se creó al abrir
}

/** Rol y permisos de un miembro, explicados. */
async function manageMember(uid) {
  const d = community.detail;
  const m = d.members.find((x) => x.uid === uid);
  if (!m) return;
  const owner = d.role === "owner";
  let role = m.role;
  const removed = { value: false };
  const saved = await cmDialog({
    title: m.name,
    subtitle: t(`En la comunidad desde ${new Date(m.joinedAt).toLocaleDateString(locale(), { day: "numeric", month: "long" })}`, `In the community since ${new Date(m.joinedAt).toLocaleDateString(locale(), { day: "numeric", month: "long" })}`),
    glyph: "crown",
    body: `${
      owner
        ? `<div class="cm-field"><span class="label">${t("Rol", "Role")}</span><div class="segmented" id="cm-role"><button type="button" data-role="member">${roleNames().member}</button><button type="button" data-role="admin">${roleNames().admin}</button></div><span class="faint cm-hint" id="cm-role-hint"></span></div>`
        : ""
    }
      <div class="cm-field"><span class="label">${t("Permisos", "Permissions")}</span>
        ${Object.entries(capInfo())
          .map(
            ([cap, info]) => `<label class="cm-cap"><span class="cm-cap-icon">${icon(info.icon)}</span><span class="grow"><b>${escapeHtml(info.name)}</b><span class="faint">${escapeHtml(info.text)}</span></span><span class="toggle"><input type="checkbox" data-cap="${cap}" ${m.caps?.[cap] ? "checked" : ""}><span></span></span></label>`,
          )
          .join("")}
      </div>`,
    ok: t("Guardar", "Save"),
    danger: `${icon("trash")}${t("Sacar de la comunidad", "Remove from the community")}`,
    mount: (root) => {
      if (!owner) return;
      const paint = () => {
        for (const b of $$("[data-role]", root)) b.classList.toggle("active", b.dataset.role === role);
        $("#cm-role-hint", root).textContent =
          role === "admin"
            ? t("Publica anuncios, cambia las reglas, invita y da permisos a los miembros.", "Publishes announcements, changes the rules, invites and gives permissions to members.")
            : t("Juega y participa; tiene solo los permisos que le des abajo.", "Plays and takes part; only has the permissions you give below.");
      };
      for (const b of $$("[data-role]", root)) b.onclick = () => ((role = b.dataset.role), paint());
      paint();
    },
    onOk: async () => {
      const caps = Object.fromEntries($$("[data-cap]").map((input) => [input.dataset.cap, input.checked]));
      await invoke("cambiar_miembro", { id: d.id, uid, rol: role, permisos: caps });
      return true;
    },
    onDanger: async () => {
      const ok = await confirmAction({
        title: t(`Sacar a ${m.name}`, `Remove ${m.name}`),
        text: t("Podrá volver si alguien le da otro código.", "They can come back if someone gives them another code."),
        ok: t("Sacar", "Remove"),
      });
      if (!ok) return false;
      await invoke("sacar_de_comunidad", { id: d.id, uid });
      removed.value = true;
      return true;
    },
  });
  if (saved) {
    toast(removed.value ? t(`${m.name} ya no está en la comunidad.`, `${m.name} is no longer in the community.`) : t("Permisos guardados.", "Permissions saved."));
    loadCommunity(d.id);
  }
}

/** El paquete oficial: una de tus instancias (se publica sola en Drive) o un modpack que ya está en Drive. */
async function choosePackage(replace = null) {
  const d = community.detail;
  const instances = [...state.instances].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0));
  let tab = "mine";
  let pick = null;
  let publish = null;
  // La ventana abre ya; el catálogo de Drive y la cuenta para publicar llegan después (tardan unos segundos).
  let drivePacks = null;
  let connected = state.drivePublisher ? true : null;
  let repaint = () => {};
  invoke("catalogo", { recargar: false })
    .then((catalog) => catalog.entries.filter((e) => e.kind === "modpack" && e.source === "drive"))
    .catch(() => [])
    .then((packs) => ((drivePacks = packs), repaint()));
  if (connected === null) {
    invoke("drive_publicar")
      .catch(() => null)
      .then((email) => ((connected = !!email), repaint()));
  }
  const loading = (text) => `<div class="cm-pick-loading"><div class="spinner"></div><span>${escapeHtml(text)}</span></div>`;
  const saved = await cmDialog({
    title: replace ? t(`Nueva versión de ${replace.name}`, `New version of ${replace.name}`) : t("Paquete oficial", "Official package"),
    subtitle: t("Lo que deben tener para entrar a los servidores de la comunidad.", "What they must have to join the community's servers."),
    glyph: "cubes",
    wide: true,
    body: `<div class="segmented" id="cm-pack-tabs"><button type="button" data-tab="mine">${icon("cubes")}${t("Mis instancias", "My instances")}</button><button type="button" data-tab="drive">${icon("cloud")}${t("Ya en Drive", "Already on Drive")}</button></div>
      <p class="faint cm-hint" id="cm-pack-hint"></p>
      <div class="cm-pick-grid" id="cm-pack-list"></div>`,
    ok: t("Elegir", "Choose"),
    mount: (root) => {
      const paint = () => {
        for (const b of $$("[data-tab]", root)) b.classList.toggle("active", b.dataset.tab === tab);
        const hint = $("#cm-pack-hint", root);
        hint.innerHTML =
          tab === "mine"
            ? connected === null
              ? t("Revisando tu cuenta de Drive…", "Checking your Drive account…")
              : connected
              ? t("Se exporta y se sube sola a tu Drive (sin Minecraft, que ya está en el espejo). El avance está en Descargas.", "It is exported and uploaded to your Drive on its own (without Minecraft, which is already in the mirror). The progress is in Downloads.")
              : `${t("Para subirla hace falta conectar Drive.", "To upload it you need to connect Drive.")} <button class="link-btn" id="cm-connect-drive" type="button">${t("Ir a Ajustes › Acerca de", "Go to Settings › About")}</button>`
            : t("Los modpacks del catálogo de Drive. Se pide la versión que está ahora.", "The Drive catalog modpacks. The version there now is required.");
        $("#cm-connect-drive", root)?.addEventListener("click", () => {
          closeModal("#cm-dialog");
          openSettings("about");
        });
        if (tab === "drive" && drivePacks === null) {
          $("#cm-pack-list", root).innerHTML = loading(t("Leyendo el catálogo de Drive…", "Reading the Drive catalog…"));
          $("#cm-dialog-ok").disabled = true;
          return;
        }
        const items =
          tab === "mine"
            ? instances.map((i) => ({ key: `i:${i.id}`, art: art(i), name: i.name, sub: `${i.loader} ${i.minecraftVersion}` }))
            : drivePacks.map((p) => ({ key: `d:${p.id}`, art: p.icon ? `<img src="${escapeHtml(p.icon)}" alt="">` : `<span class="cm-pack-none">${icon("cubes")}</span>`, name: p.name, sub: `${p.minecraftVersion} · ${p.loader}${p.modCount ? ` · ${plural(p.modCount, "mod", "mods")}` : ""}` }));
        $("#cm-pack-list", root).innerHTML = items.length
          ? items
              .map(
                (item) => `<button type="button" class="cm-pick ${pick === item.key ? "active" : ""}" data-pick="${escapeHtml(item.key)}"><span class="cm-pick-art">${item.art}</span><b>${escapeHtml(item.name)}</b><span class="faint">${escapeHtml(item.sub)}</span></button>`,
              )
              .join("")
          : `<p class="side-note no-top">${tab === "mine" ? t("Todavía no tienes instancias.", "You have no instances yet.") : t("No hay modpacks en el catálogo de Drive.", "There are no modpacks in the Drive catalog.")}</p>`;
        $("#cm-dialog-ok").innerHTML = pick?.startsWith("i:") ? `${icon("upload")}${t("Publicar y usar", "Publish and use")}` : t("Usar este", "Use this one");
        $("#cm-dialog-ok").disabled = !pick || (pick.startsWith("i:") && !connected);
      };
      for (const b of $$("[data-tab]", root)) b.onclick = () => ((tab = b.dataset.tab), (pick = null), paint());
      $("#cm-pack-list", root).onclick = (event) => {
        const item = event.target.closest("[data-pick]");
        if (item) (pick = item.dataset.pick), paint();
      };
      paint();
      const seq = community.dialogSeq;
      repaint = () => community.dialogSeq === seq && !$("#cm-dialog").classList.contains("hidden") && paint();
    },
    onOk: async () => {
      if (!pick) return false;
      if (pick.startsWith("i:")) {
        // Se publica fuera de la ventana (tarda): la ventana se cierra ya.
        publish = instances.find((i) => i.id === pick.slice(2));
      } else {
        const entry = drivePacks.find((p) => p.id === pick.slice(2));
        await invoke("guardar_paquete_comunidad", {
          id: d.id,
          paquete: { id: replace?.id ?? "", name: entry.name, catalogId: entry.id, version: entry.version ?? "", minecraft: entry.minecraftVersion, loader: entry.loader ?? "", loaderVersion: "", at: 0 },
        });
      }
      return true;
    },
  });
  if (!saved) return;
  if (publish && !(await publishInstance(publish, replace))) return;
  await linkServersToOnlyPackage();
  loadCommunity(d.id);
}

/** Exporta la instancia, la sube al catálogo de Drive y la deja como paquete oficial. */
async function publishInstance(instance, replace = null) {
  if (community.publishing) {
    toast(t("Ya se está publicando un paquete: espera a que termine.", "A package is already being published: wait for it to finish."), "error");
    return false;
  }
  const cid = community.id;
  community.publishing = instance.name;
  renderCommunities();
  toast(t("Publicando el paquete oficial: el avance está en Descargas.", "Publishing the official package: the progress is in Downloads."));
  try {
    await invoke("publicar_paquete_comunidad", { id: cid, instancia: instance.id, paquete: replace?.id ?? null, tarea: newJob() });
    toast(t(`${instance.name} es el paquete oficial: al pulsar «Jugar», los demás lo instalan.`, `${instance.name} is the official package: pressing «Play» installs it for the others.`));
    return true;
  } catch (error) {
    toast(String(error), "error");
    return false;
  } finally {
    community.publishing = null;
    if (community.id === cid) loadCommunity(cid);
  }
}

/** Con un solo paquete, los servidores que no piden ninguno pasan a pedirlo. */
async function linkServersToOnlyPackage() {
  const fresh = await invoke("comunidad", { id: community.id }).catch(() => null);
  if (!fresh || fresh.packages.length !== 1 || !(isAdminRole(fresh.role) || fresh.caps?.servers)) return;
  const pk = fresh.packages[0].id;
  const loose = fresh.servers.filter((s) => !s.package);
  for (const server of loose) await invoke("guardar_servidor_comunidad", { id: fresh.id, servidor: { ...server, package: pk } }).catch(() => {});
  if (loose.length) toast(t(`Los servidores ahora piden ${fresh.packages[0].name}.`, `The servers now require ${fresh.packages[0].name}.`));
}

function packageMenu(anchor, pk) {
  const d = community.detail;
  const p = d.packages.find((x) => x.id === pk);
  const local = d.local?.[pk];
  const info = d.catalog?.[pk];
  const newer = info?.version && p.version && info.version !== p.version;
  const items = [
    local?.source ? { label: t("Publicar una versión nueva desde mi instancia", "Publish a new version from my instance"), icon: "upload", action: () => publishAgain(p, local.instance) } : null,
    newer ? { label: t("Pedir la versión que está ahora en Drive", "Require the version on Drive now"), icon: "refresh", action: () => requireDriveVersion(p) } : null,
    { label: t("Cambiar por otro paquete", "Change to another package"), icon: "cubes", action: () => choosePackage(p) },
    { label: t("Quitar el paquete", "Remove the package"), icon: "trash", danger: true, action: () => removePackage(p) },
  ].filter(Boolean);
  contextMenu(anchor, items);
}

function publishAgain(p, instanceId) {
  const instance = state.instances.find((i) => i.id === instanceId);
  if (instance) publishInstance(instance, p);
}

async function requireDriveVersion(p) {
  const info = community.detail.catalog?.[p.id];
  try {
    await invoke("guardar_paquete_comunidad", { id: community.id, paquete: { ...p, version: info.version } });
    toast(t("Ahora se pide la versión de Drive.", "The Drive version is now required."));
    loadCommunity(community.id);
  } catch (error) {
    toast(String(error), "error");
  }
}

async function removePackage(p) {
  const ok = await confirmAction({
    title: t(`Quitar ${p.name}`, `Remove ${p.name}`),
    text: t("Los servidores que lo pedían dejarán de pedir paquete. No se borra de Drive.", "The servers that required it will no longer require a package. It is not deleted from Drive."),
    ok: t("Quitar", "Remove"),
  });
  if (!ok) return;
  try {
    await invoke("borrar_paquete_comunidad", { id: community.id, paquete: p.id });
    loadCommunity(community.id);
  } catch (error) {
    toast(String(error), "error");
  }
}

/** Instalar o actualizar el paquete oficial (sin entrar al servidor). Devuelve la instancia. */
async function installPackage(pk) {
  const d = community.detail;
  const local = d.local?.[pk];
  if (!local?.catalogId) return local?.instance ?? null;
  if (local.state === "lista") return local.instance;
  const restoring = local.state === "modificada";
  community.preparing.add(pk);
  renderCommunities();
  try {
    toast(
      local.state === "falta"
        ? t("Instalando el paquete oficial: el avance está en Descargas.", "Installing the official package: the progress is in Downloads.")
        : restoring
          ? t("Restaurando el paquete oficial: lo que se cambia queda en una copia (Detalles de la instancia).", "Restoring the official package: what changes is kept in a backup (instance Details).")
          : t("Actualizando el paquete oficial (tus mundos y controles se quedan).", "Updating the official package (your worlds and controls stay)."),
    );
    const result = await invoke("instalar_de_catalogo", { id: local.catalogId, reparar: restoring, tarea: newJob() });
    await reloadInstances();
    if (restoring) toast(t("Listo: el paquete quedó igual al oficial.", "Done: the package is the same as the official one again."));
    return result.id ?? local.instance;
  } finally {
    community.preparing.delete(pk);
  }
}

// ---------- Otras acciones ----------
async function joinWithCode(code) {
  code = (code ?? "").trim();
  if (!code) return toast(t("Escribe el código de la comunidad.", "Type the community code."), "error");
  try {
    const preview = await invoke("ver_invitacion", { codigo: code });
    if (!preview.member) {
      const joined = await cmDialog({
        title: preview.name,
        subtitle: t("Te invitaron a esta comunidad", "You were invited to this community"),
        glyph: "community",
        body: `<div class="cm-join-preview">${communityImage(preview, "large")}<p>${escapeHtml(preview.description || t("Vas a entrar como miembro.", "You will join as a member."))}</p></div>`,
        ok: `${icon("check")}${t("Unirme", "Join")}`,
      });
      if (!joined) return;
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
    // Con cambios en el paquete: restaurar el oficial antes de entrar, o entrar así.
    const local = server.package ? d.local?.[server.package] : null;
    if (local?.state === "modificada") {
      const pack = d.packages.find((p) => p.id === server.package);
      const choice = await showPackageChanges(pack, local.changes, { play: true });
      if (!choice) return;
      if (choice === "secundario") return void (await play(local.instance, server.address));
    }
    // Si el paquete que pide ya no está, se elige con qué instancia entrar.
    const instance = server.package && d.local?.[server.package] ? await installPackage(server.package) : await chooseInstanceFor(server);
    if (!instance) return;
    await play(instance, server.address);
  } catch (error) {
    toast(String(error), "error");
  } finally {
    community.preparing.delete(sid);
    loadCommunity(d.id);
  }
}

/** Qué cambió en el paquete oficial de este PC. Con `play`: «Restaurar y jugar» o «Jugar así». */
function showPackageChanges(pack, changes, { play = false } = {}) {
  return cmDialog({
    title: t(`Tu ${pack?.name ?? "paquete"} tiene cambios`, `Your ${pack?.name ?? "package"} has changes`),
    subtitle: t("Comparado con el paquete oficial de la comunidad.", "Compared with the community's official package."),
    glyph: "alert",
    wide: true,
    body: `${packageChangesHtml(changes)}<p class="faint cm-hint">${t(
      "Restaurar deja tus mods como los del paquete oficial. Lo que cambie queda en una copia que puedes recuperar en Detalles de la instancia. Tus mundos y opciones no se tocan.",
      "Restoring makes your mods match the official package. Whatever changes is kept in a backup you can recover from the instance Details. Your worlds and options are not touched.",
    )}</p>`,
    ok: play ? `${icon("refresh")}${t("Restaurar y jugar", "Restore and play")}` : null,
    secondary: play ? `${icon("play")}${t("Jugar así", "Play as is")}` : null,
    cancel: play ? t("Cancelar", "Cancel") : t("Cerrar", "Close"),
  });
}

/** Sin paquete oficial: con qué instancia entrar (la última que jugaste primero). */
async function chooseInstanceFor(server) {
  const instances = [...state.instances].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0));
  if (!instances.length) {
    toast(t("Primero crea una instancia.", "Create an instance first."), "error");
    return null;
  }
  let pick = instances[0].id;
  const ok = await cmDialog({
    title: t(`Entrar a ${server.name}`, `Join ${server.name}`),
    subtitle: t("Este servidor no pide un paquete: elige con qué instancia entrar.", "This server does not require a package: choose which instance to join with."),
    glyph: "play",
    wide: true,
    body: `<div class="cm-pick-grid">${instances
      .map((i) => `<button type="button" class="cm-pick ${i.id === pick ? "active" : ""}" data-pick="${escapeHtml(i.id)}"><span class="cm-pick-art">${art(i)}</span><b>${escapeHtml(i.name)}</b><span class="faint">${escapeHtml(`${i.loader} ${i.minecraftVersion}`)}</span></button>`)
      .join("")}</div>`,
    ok: `${icon("play")}${t("Jugar", "Play")}`,
    mount: (root) => {
      root.onclick = (event) => {
        const item = event.target.closest("[data-pick]");
        if (!item) return;
        pick = item.dataset.pick;
        for (const b of $$("[data-pick]", root)) b.classList.toggle("active", b === item);
      };
    },
  });
  return ok ? pick : null;
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
  $("#cm-create").onclick = () => communityForm(null);
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
    if (target.dataset.cmPackage) return packageMenu(target, target.dataset.cmPackage);
    if (target.dataset.cmChanges) {
      const pack = d.packages.find((p) => p.id === target.dataset.cmChanges);
      return showPackageChanges(pack, d.local?.[pack?.id]?.changes);
    }
    if (target.dataset.cmInstall) {
      return installPackage(target.dataset.cmInstall)
        .catch((error) => toast(String(error), "error"))
        .finally(() => loadCommunity(d.id));
    }
    if (target.dataset.cmDeletePost) {
      return confirmAction({ title: t("Borrar el anuncio", "Delete the announcement"), text: t("Nadie lo verá más.", "Nobody will see it anymore."), ok: t("Borrar", "Delete") }).then(
        (ok) => ok && invoke("borrar_anuncio", { id: d.id, anuncio: target.dataset.cmDeletePost }).then(() => loadCommunity(d.id), (e) => toast(String(e), "error")),
      );
    }
    if (target.dataset.cmDeleteCode) {
      return invoke("borrar_invitacion", { id: d.id, codigo: target.dataset.cmDeleteCode }).then(() => loadCommunity(d.id), (e) => toast(String(e), "error"));
    }
    const actions = {
      invite: inviteToCommunity,
      edit: () => communityForm(d),
      image: quickImage,
      more: () => communityMenu(target),
      "add-server": () => editServer(null),
      "add-post": addPost,
      "edit-rules": editRules,
      "add-package": () => choosePackage(null),
    };
    actions[target.dataset.cm]?.();
  };
}

wireCommunityEvents();
