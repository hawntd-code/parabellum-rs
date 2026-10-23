"use strict";

// Copias privadas de mundos. La nube nunca sustituye el original sin una elección del jugador.
window.worldBackups = (() => {
  const html = escapeHtml;
  let view = { mode: "library", source: null, world: null };
  let seq = 0;
  let cloud = [];
  let status = null;
  let working = false;
  const date = (at) => new Date(at).toLocaleString(locale(), { dateStyle: "medium", timeStyle: "short" });
  const size = (n) => n >= 1024 ** 3 ? `${(n / 1024 ** 3).toFixed(2)} GB` : `${(n / 1024 ** 2).toFixed(1)} MB`;
  const visible = () => !$("#worlds-dialog").classList.contains("hidden") && !$("#worlds-dialog").classList.contains("closing");
  const button = (action, label, glyph = "cloud", primary = false, extra = "") => `<button class="btn small ${primary ? "primary" : ""}" data-wb="${action}" ${extra}>${icon(glyph)}${html(label)}</button>`;
  const empty = (text) => `<p class="wb-empty">${html(text)}</p>`;

  const overlay = document.createElement("div");
  overlay.id = "worlds-dialog";
  overlay.className = "overlay hidden";
  overlay.innerHTML = `<div class="modal wb-modal" role="dialog" aria-modal="true" aria-labelledby="wb-title" tabindex="-1">
    <div class="modal-head cm-dialog-head"><span class="cm-dialog-icon">${icon("cloud")}</span><div class="grow"><h2 id="wb-title"></h2><p id="wb-subtitle" class="faint sp-subtitle"></p></div><button class="circle-btn" id="wb-close" aria-label="${t("Cerrar", "Close")}">${icon("close")}</button></div>
    <div class="modal-body" id="wb-body" aria-live="polite"></div><div class="wb-footer" id="wb-footer"></div></div>`;
  document.body.insertBefore(overlay, $("#cm-dialog"));

  function close() { seq++; closeModal("#worlds-dialog"); }
  function head(title, subtitle = "") { $("#wb-title").textContent = title; $("#wb-subtitle").textContent = subtitle; }
  function show(next) {
    view = next;
    head(t("Mundos en Drive", "Worlds in Drive"));
    openModal("#worlds-dialog");
    refresh();
  }
  async function connect() {
    if (working) return;
    working = true;
    try { await invoke("mundos_conectar"); toast(t("Drive conectado para tus mundos.", "Drive connected for your worlds.")); }
    catch (error) { toast(String(error), "error"); }
    finally { working = false; if (visible()) refresh(); }
  }
  function account() {
    if (!status?.connected) return `<div class="wb-intro"><h3>${t("Continúa tu mundo en otro equipo", "Continue your world on another computer")}</h3><p>${t("Guarda copias privadas en tu Google Drive. Tú eliges qué mundos respaldar; al cerrar el juego se prepara la siguiente copia.", "Save private backups in your Google Drive. You choose which worlds to back up; the next backup is prepared when you close the game.")}</p><p class="faint">${t("Parabellum pide acceso a los archivos que crea para tus mundos. Hasta 5 GB y cinco revisiones por mundo; los conflictos se conservan hasta que los resuelvas.", "Parabellum requests access to the files it creates for your worlds. Up to 5 GB and five revisions per world; conflicts are kept until you resolve them.")}</p>${button("connect", t("Conectar Google Drive", "Connect Google Drive"), "cloud", true)}</div>`;
    const used = cloud.reduce((n, w) => n + w.used, 0);
    return `<div class="wb-account"><span class="grow"><b>${html(status.email)}</b><span class="faint">${html(size(used))} / 5 GB · ${t("Copias privadas", "Private backups")}</span></span>${button("refresh", t("Actualizar", "Refresh"), "refresh")}${button("disconnect", t("Desconectar", "Disconnect"), "close")}</div><progress class="wb-quota" max="${status.limit}" value="${used}" aria-label="${t("Espacio de mundos usado", "World backup storage used")}"></progress>`;
  }
  function pending() {
    if (!status?.pending?.length) return "";
    return `<section class="wb-section"><h3>${t("Copias pendientes", "Pending backups")}</h3>${status.pending.map((p) => `<div class="wb-row"><span class="grow"><b>${html(p.name)}</b><span class="faint">${html(size(p.bytes))} · ${p.paused ? t("Pausada", "Paused") : t("Pendiente de subir", "Waiting to upload")}</span></span>${button("resume", t("Reanudar", "Resume"), "play", false, `data-revision="${html(p.id)}"`)}</div>`).join("")}<p class="faint">${t("El progreso y la opción de pausar están en Descargas.", "Progress and the pause option are in Downloads.")}</p></section>`;
  }
  function history(world) {
    if (!world) return empty(t("Este mundo todavía no tiene copias en Drive.", "This world has no Drive backups yet."));
    return `${world.conflict ? `<div class="wb-notice" role="status">${icon("alert")}<span><b>${t("Hay avances de distintos equipos", "There is progress from different computers")}</b>${t("Compara las fechas y el equipo. Elige con qué revisión continuar; las otras copias siguen en el historial.", "Compare dates and computers. Choose which revision to continue with; the other backups stay in history.")}</span></div>` : ""}<div class="wb-history">${world.revisions.map((entry) => {
      const r = entry.revision;
      const current = world.heads.includes(r.id);
      return `<div class="wb-revision"><div class="grow"><b>${html(date(r.at))}</b>${current ? `<span class="cm-chip ${world.conflict ? "warn" : "ok"}">${world.conflict ? t("Rama por resolver", "Unresolved branch") : t("Más reciente", "Latest")}</span>` : ""}<span class="faint">${html(r.device)} · ${html(size(r.size))}</span><span class="faint">Minecraft ${html(r.recipe.minecraft)} · ${html(r.recipe.loader)} ${html(r.recipe.loaderVersion ?? "")}${r.recipe.packageVersion ? ` · ${html(r.recipe.packageVersion)}` : ""}</span></div><div class="wb-row-actions">${button("restore", world.conflict && current ? t("Continuar con esta", "Continue with this") : t("Restaurar…", "Restore…"), "reply", current, `data-world="${html(world.worldId)}" data-revision="${html(r.id)}" data-resolve="${world.conflict && current}"`)}${world.conflict && current && r.source.kind === "instancia" ? button("restore", t("Crear otro mundo…", "Create another world…"), "plus", false, `data-world="${html(world.worldId)}" data-revision="${html(r.id)}" data-resolve="false"`) : ""}${button("export", t("Guardar ZIP", "Save ZIP"), "download", false, `data-world="${html(world.worldId)}" data-revision="${html(r.id)}"`)}</div></div>`;
    }).join("")}</div>`;
  }
  async function refresh() {
    if (!visible() || view.mode === "restore") return;
    const mine = ++seq;
    const focused = document.activeElement?.dataset?.wb;
    $("#wb-body").innerHTML = `<div class="content-loading"><span class="spinner small"></span></div>`;
    $("#wb-footer").innerHTML = "";
    try {
      status = await invoke("mundos_estado");
      const worlds = status.connected ? await invoke("mundos_nube") : [];
      if (mine !== seq) return;
      cloud = worlds ?? [];
      if (view.mode === "local") {
        const source = view.source;
        const [locals, previous] = await Promise.all([invoke("mundos_locales", { tipo: source.kind, id: source.id }), invoke("mundos_anteriores", { origen: source })]);
        if (mine !== seq) return;
        const local = locals?.find((l) => l.source.world === source.world);
        const world = cloud.find((w) => w.worldId === local?.worldId);
        head(source.world, t("Respaldos del mundo", "World backups"));
        $("#wb-body").innerHTML = `${account()}${status.connected ? `<section class="wb-section"><div class="setting"><div class="grow"><b>${t("Respaldar al cerrar", "Back up when closing")}</b><span>${t("Solo con el juego o el servidor cerrado. Las copias no mezclan avances de otros equipos.", "Only with the game or server closed. Backups do not merge progress from other computers.")}</span></div><label class="toggle"><input type="checkbox" id="wb-auto" ${local?.enabled ? "checked" : ""} aria-label="${t("Respaldar al cerrar", "Back up when closing")}"><span></span></label></div><div class="wb-toolbar">${button("backup", t("Respaldar ahora", "Back up now"), "cloud", true)}${local?.lastAt ? `<span class="faint">${t("Última copia", "Last backup")}: ${html(date(local.lastAt))}</span>` : ""}</div>${local?.error ? `<p class="wb-error">${html(friendlyError(window.errorRecovery.present(local.error).message))}</p>` : ""}</section><section class="wb-section"><h3>${t("Historial en Drive", "Drive history")}</h3>${history(world)}</section>` : ""}${previous?.length ? `<section class="wb-section"><h3>${t("Antes de restaurar", "Before restoring")}</h3><p class="faint">${t("Se conservaron en este equipo cuando reemplazaste el mundo.", "Kept on this computer when you replaced the world.")}</p>${previous.map((p) => `<div class="wb-row"><span class="grow">${html(date(p.at))}</span>${button("previous", t("Volver a esta copia", "Return to this backup"), "reply", false, `data-backup="${html(p.id)}"`)}</div>`).join("")}</section>` : ""}${pending()}`;
        $("#wb-auto")?.addEventListener("change", async (event) => {
          event.target.disabled = true;
          try { await invoke("mundos_configurar", { origen: source, activo: event.target.checked }); }
          catch (e) { toast(String(e), "error"); }
          finally { refresh(); }
        });
      } else if (view.mode === "history") {
        const world = cloud.find((w) => w.worldId === view.world);
        head(world?.name ?? t("Historial", "History"), t("Cada revisión guarda el mundo completo.", "Each revision saves the complete world."));
        $("#wb-body").innerHTML = history(world);
      } else {
        head(t("Mundos en Drive", "Worlds in Drive"), t("Tus copias, en tus equipos", "Your backups, on your computers"));
        $("#wb-body").innerHTML = `${account()}${status.connected ? `<section class="wb-section">${cloud.length ? cloud.map((w) => `<button class="wb-world" data-wb="history" data-world="${html(w.worldId)}">${icon("globe")}<span class="grow"><b>${html(w.name)}</b><span class="faint">${html(date(w.revisions[0].revision.at))} · ${html(w.revisions[0].revision.device)}</span></span>${w.conflict ? `<span class="cm-chip warn">${t("Revisar conflicto", "Review conflict")}</span>` : `<span class="faint">${w.revisions.length} ${t("revisiones", "revisions")}</span>`}${icon("right")}</button>`).join("") : empty(t("Abre una instancia → Contenido → Mundos → Respaldos para elegir tu primer mundo.", "Open an instance → Content → Worlds → Backups to choose your first world."))}</section>${pending()}` : ""}`;
      }
      $("#wb-footer").innerHTML = `${view.mode !== "library" ? button("library", t("Todos mis mundos", "All my worlds"), "left") : `<span class="faint">${t("Las copias usan espacio de tu Google Drive.", "Backups use your Google Drive storage.")}</span>`}${button("close", t("Cerrar", "Close"), "check")}`;
      if (focused) $(`[data-wb="${focused}"]`, overlay)?.focus({ preventScroll: true });
    } catch (error) {
      if (mine !== seq) return;
      $("#wb-body").innerHTML = `<p class="wb-error" role="alert">${html(friendlyError(window.errorRecovery.present(String(error)).message))}</p>${button("refresh", t("Reintentar", "Retry"), "update")}${button("connect", t("Conectar Drive", "Connect Drive"))}`;
    }
  }
  async function restore(worldId, revisionId, resolve) {
    const entry = cloud.find((w) => w.worldId === worldId)?.revisions.find((e) => e.revision.id === revisionId);
    if (!entry) return;
    const mine = ++seq;
    const destinations = await invoke("mundos_destinos", { mundo: worldId, revision: revisionId });
    if (mine !== seq || !visible()) return;
    view = { mode: "restore", entry, resolve };
    const r = entry.revision;
    head(t("Restaurar mundo", "Restore world"), `${r.name} · ${date(r.at)}`);
    if (!destinations.length) {
      $("#wb-body").innerHTML = `<div class="wb-notice">${icon("alert")}<span><b>${t("Hace falta una instalación compatible", "A compatible installation is needed")}</b>${t("Prepara una instancia o servidor con estas versiones y los mismos mods. Después vuelve al historial.", "Prepare an instance or server with these versions and the same mods. Then return to history.")}</span></div><dl class="wb-recipe"><dt>Minecraft</dt><dd>${html(r.recipe.minecraft)}</dd><dt>Loader</dt><dd>${html(r.recipe.loader)} ${html(r.recipe.loaderVersion ?? "")}</dd><dt>${t("Paquete", "Package")}</dt><dd>${html(r.recipe.package ?? r.recipe.instanceName)} ${html(r.recipe.packageVersion ?? "")}</dd><dt>${t("Contenido", "Content")}</dt><dd class="mono">${html(r.recipe.contentHash)}</dd></dl>${button("library-page", t("Ir a mi biblioteca", "Go to my library"), "folder")}`;
      $("#wb-footer").innerHTML = button("library", t("Volver a mis mundos", "Back to my worlds"), "left");
      return;
    }
    const server = r.source.kind === "servidor";
    $("#wb-body").innerHTML = `<label class="cm-field"><span class="label">${t("Restaurar en", "Restore into")}</span><select id="wb-destination">${destinations.map((d, i) => `<option value="${i}">${html(d.name)}</option>`).join("")}</select></label>
      ${!server && !resolve ? `<label class="cm-field"><span class="label">${t("Cómo restaurar", "How to restore")}</span><select id="wb-copy"><option value="copy">${t("Crear otro mundo (recomendado)", "Create another world (recommended)")}</option><option value="replace">${t("Reemplazar un mundo", "Replace a world")}</option></select></label>` : ""}
      <label class="cm-field"><span class="label">${t("Carpeta del mundo", "World folder")}</span><input id="wb-world-name" maxlength="100" value="${html(server ? destinations[0].world : resolve ? r.source.world : `${r.name} - ${t("copia", "copy")}`)}" ${server ? "readonly" : ""} list="wb-existing"><datalist id="wb-existing"></datalist></label>
      <p class="wb-notice">${icon("reply")}<span>${t("Cierra Minecraft o el servidor. Si reemplazas un mundo, conservamos el anterior en este equipo y podrás volver a él desde Respaldos.", "Close Minecraft or the server. If you replace a world, we keep the previous one on this computer and you can return to it from Backups.")}</span></p>
      <label class="check wb-consent"><input type="checkbox" id="wb-player"><span>${t("Revisé la copia: incluye los inventarios y datos de jugador de ese equipo. Si cambió mi UUID de Minecraft, mi inventario puede ser distinto.", "I checked the backup: it includes that computer's inventories and player data. If my Minecraft UUID changed, my inventory may differ.")}</span></label>
      <p id="wb-restore-error" class="wb-error hidden" role="alert"></p>`;
    $("#wb-footer").innerHTML = `${button("library", t("Cancelar", "Cancel"), "left")}${button("confirm-restore", resolve ? t("Restaurar y continuar con esta", "Restore and continue with this") : t("Restaurar", "Restore"), "reply", true)}`;
    const names = async () => {
      const d = destinations[Number($("#wb-destination").value)];
      if (server) $("#wb-world-name").value = d.world;
      const local = await invoke("mundos_locales", { tipo: d.kind, id: d.id }).catch(() => []);
      if (view.mode !== "restore") return;
      $("#wb-existing").innerHTML = local.map((l) => `<option value="${html(l.source.world)}"></option>`).join("");
    };
    $("#wb-destination").onchange = names;
    $("#wb-copy")?.addEventListener("change", () => { $("#wb-world-name").value = $("#wb-copy").value === "copy" ? `${r.name} - ${t("copia", "copy")}` : r.source.world; });
    names();
    $('[data-wb="confirm-restore"]', overlay).onclick = async (event) => {
      if (working) return;
      const errorBox = $("#wb-restore-error");
      errorBox.classList.add("hidden");
      if (!$("#wb-player").checked) { errorBox.textContent = t("Revisa y confirma el aviso de los datos de jugador.", "Review and confirm the player data notice."); errorBox.classList.remove("hidden"); return; }
      const d = destinations[Number($("#wb-destination").value)];
      const destination = { kind: d.kind, id: d.id, world: $("#wb-world-name").value.trim() };
      const copy = !server && !resolve && $("#wb-copy").value === "copy";
      working = true;
      event.currentTarget.disabled = true;
      try {
        const result = await invoke("mundos_restaurar", { mundo: worldId, revision: revisionId, destino: destination, nuevo: copy, confirmarJugador: true, resolver: resolve });
        toast(result.warning ?? (result.backup ? t("Mundo restaurado. La copia anterior está disponible en Respaldos.", "World restored. The previous backup is available in Backups.") : t("Mundo restaurado y listo para abrir.", "World restored and ready to open.")), result.warning ? "error" : "info");
        view = { mode: "local", source: destination }; refresh();
        if (state.current?.id === destination.id) loadContent();
      } catch (e) { if (visible()) { errorBox.textContent = friendlyError(window.errorRecovery.present(String(e)).message); errorBox.classList.remove("hidden"); } else toast(String(e), "error"); }
      finally { working = false; $('[data-wb="confirm-restore"]', overlay)?.removeAttribute("disabled"); }
    };
  }
  overlay.addEventListener("click", async (event) => {
    if (event.target === overlay) return close();
    const target = event.target.closest("[data-wb]");
    if (!target || target.dataset.wb === "confirm-restore") return;
    const action = target.dataset.wb;
    if (action === "close") return close();
    if (working) return;
    if (action === "library") return show({ mode: "library" });
    if (action === "library-page") { close(); return navigate("home"); }
    if (action === "history") return show({ mode: "history", world: target.dataset.world });
    if (action === "refresh") return refresh();
    if (action === "connect") return connect();
    target.disabled = true;
    try {
      if (action === "restore") return await restore(target.dataset.world, target.dataset.revision, target.dataset.resolve === "true");
      if (action === "export") { await invoke("mundos_exportar", { mundo: target.dataset.world, revision: target.dataset.revision }); return; }
      if (action === "disconnect") await invoke("mundos_desconectar");
      if (action === "backup") { await invoke("mundos_respaldar", { origen: view.source }); toast(t("Copia confirmada en Drive.", "Backup confirmed in Drive.")); }
      if (action === "resume") await invoke("mundos_reanudar", { revision: target.dataset.revision });
      if (action === "previous") {
        const source = view.source;
        const yes = await confirmAction({ title: t("¿Volver a esta copia local?", "Return to this local backup?"), text: t("El mundo actual también se conservará. Cierra el juego antes de continuar.", "The current world will also be kept. Close the game before continuing."), ok: t("Restaurar", "Restore") });
        if (!yes) return;
        await invoke("mundos_volver_anterior", { origen: source, respaldo: target.dataset.backup });
        toast(t("Se recuperó la copia local.", "Local backup recovered."));
      }
      refresh();
    } catch (e) { toast(String(e), "error"); }
    finally { target.disabled = false; }
  });
  $("#wb-close").onclick = close;
  $("#worlds-library").onclick = () => show({ mode: "library" });
  $("#worlds-settings").onclick = () => show({ mode: "library" });
  $("#worlds-server").onclick = async () => {
    try {
      const list = await invoke("mundos_locales", { tipo: "servidor", id: host.id });
      if (!list.length) return toast(t("Enciende el servidor una vez para crear su mundo y luego apágalo.", "Start the server once to create its world, then stop it."));
      show({ mode: "local", source: list[0].source });
    } catch (e) { toast(String(e), "error"); }
  };
  document.addEventListener("click", (event) => {
    const target = event.target.closest("[data-world-cloud]");
    if (target && state.current) show({ mode: "local", source: { kind: "instancia", id: state.current.id, world: target.dataset.worldCloud } });
  });
  let refreshTimer;
  listen("mundos-cambiados", () => { clearTimeout(refreshTimer); refreshTimer = setTimeout(() => { if (!working) refresh(); }, 200); });
  window.addEventListener("operacion-recuperada", ({ detail }) => { if (detail.command.startsWith("mundos_")) refresh(); });
  return { connect, close, openLibrary: () => show({ mode: "library" }) };
})();

// Datos aislados para revisar el recorrido en el navegador; nunca se usan dentro de Tauri.
window.worldsDemo = async (command, args) => {
  if (tauri) throw new Error("Solo demostración");
  const root = "1".repeat(32), left = "2".repeat(32), right = "3".repeat(32), world = "a".repeat(32);
  window.demoWorlds ??= { connected: true, enabled: true, previous: false };
  const d = window.demoWorlds;
  const source = { kind: "instancia", id: state.current?.id ?? "Vanilla 1.21.1", world: "Mi mundo" };
  const revision = (id, parents, at, device) => ({ id, worldId: world, owner: "demo", name: "Mi mundo", parents, at, device, playerUuid: "jugador", recipe: { minecraft: "1.21.1", loader: "Vanilla", loaderVersion: null, instanceName: "Vanilla 1.21.1", package: null, packageVersion: null, contentHash: "sha256:" + "a".repeat(64) }, source, size: 180e6 });
  const entries = [revision(right, [root], Date.now() - 3600e3, "PC de casa"), revision(left, [root], Date.now() - 7200e3, "Laptop"), revision(root, [], Date.now() - 86400e3, "PC de casa")].map((r) => ({ file: r.id, revision: r, bytes: r.size }));
  if (command === "mundos_estado") return { connected: d.connected, email: d.connected ? "jugador@example.test" : null, limit: 5 * 1024 ** 3, pending: [] };
  if (command === "mundos_conectar") { d.connected = true; return "jugador@example.test"; }
  if (command === "mundos_desconectar") { d.connected = false; return; }
  if (command === "mundos_nube") return [{ worldId: world, name: "Mi mundo", conflict: !d.resolved, heads: d.resolved ? [right] : [right, left], used: 540e6, revisions: entries }];
  if (command === "mundos_locales") return [{ source: { ...source, kind: args.tipo, id: args.id }, worldId: world, enabled: d.enabled, lastAt: Date.now() - 3600e3, error: null }];
  if (command === "mundos_configurar") { d.enabled = args.activo; return; }
  if (command === "mundos_anteriores") return d.previous ? [{ id: root, at: Date.now() - 60e3 }] : [];
  if (command === "mundos_destinos") return [{ kind: "instancia", id: source.id, name: "Vanilla 1.21.1", world: null }];
  if (command === "mundos_restaurar") { d.previous = true; d.resolved = args.resolver; return { backup: "respaldo anterior", source: args.destino, revision: args.revision }; }
  if (["mundos_respaldar", "mundos_reanudar", "mundos_volver_anterior", "mundos_exportar"].includes(command)) return {};
  throw new Error("Demostración de mundos: comando desconocido");
};
