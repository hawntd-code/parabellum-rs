"use strict";
// Recuperación con contexto local: un mensaje remoto nunca elige comandos ni sus argumentos.
window.errorRecovery = (() => {
  const contexts = new Map();
  let next = 0;
  const strip = (value) => String(value).replace(/⟦(?:accion|contexto):[^⟧]*⟧/g, "").replace(/^Error:\s*/, "").trim();
  const retryable = new Set([
    "listar_versiones", "versiones_compatibles", "versiones_loader", "contenido", "buscar_contenido", "buscar_actualizaciones",
    "verificar_instancia", "instalar_contenido", "actualizar_contenido", "instalar_de_catalogo",
    "mundos_nube", "mundos_locales", "mundos_respaldar", "mundos_reanudar", "mundos_destinos", "mundos_exportar",
    "iniciar_servidor",
  ]);
  function capture(command, args, error) {
    const original = String(error);
    const message = strip(original);
    if (/cancelad|cancelled/i.test(message)) return message;
    try {
      const serialized = JSON.stringify(retryable.has(command) ? args ?? {} : { id: args?.id, instancia: args?.instancia, origen: args?.origen, destino: args?.destino });
      if (serialized.length > 16384) return message;
      while (contexts.size >= 64) contexts.delete(contexts.keys().next().value);
      const key = String(++next);
      const ids = [...original.matchAll(/⟦accion:([a-z-]+)(?::[^⟧]*)?⟧/g)].map(m => m[1]);
      contexts.set(key, { command, args: JSON.parse(serialized), at: Date.now(), message, ids });
      return message;
    } catch { return message; }
  }
  function present(raw) {
    const text = String(raw);
    const contextId = text.match(/⟦contexto:(\d+)⟧/)?.[1];
    const saved = contexts.get(contextId) ?? [...contexts.values()].reverse().find(c => c.message === strip(text));
    const context = saved && Date.now() - saved.at < 30 * 60e3 ? saved : null;
    const args = context?.args ?? {};
    const candidate = args.origen?.id ?? args.destino?.id ?? args.instancia ?? args.id;
    const server = args.origen?.kind === "servidor" || args.destino?.kind === "servidor" || /servidor/.test(context?.command ?? "");
    const instance = !server && (state.instances?.find(i => i.id === candidate)
      ?? (state.current && !/soporte|comunidad/.test(context?.command ?? "") ? state.current : null));
    const ids = [...(context?.ids ?? []), ...[...text.matchAll(/⟦accion:([a-z-]+)(?::[^⟧]*)?⟧/g)].map(m => m[1])];
    const message = strip(text);
    if (suggestsSchool(message)) ids.push("escuela");
    if (/sin espacio|disco.*lleno|disk.*full|os error 112|not enough.*space/i.test(message)) ids.push("recursos");
    if (/Java.*(dañad|no está|no se encontr|missing|damaged|not found)/i.test(message)) ids.push("reparar-java");
    if (/archivo.*(dañad|no coincide)|checksum|hash mismatch|corrupt/i.test(message)) ids.push("verificar");
    if (/sesión.*(caduc|venci)|session.*expir|invalid_grant/i.test(message)) ids.push(context?.command.startsWith("mundos_") ? "conectar-mundos" : "sesion");
    if (/acceso denegado|no deja escribir|access.*denied|permission denied|os error 5\b/i.test(message)) ids.push("carpeta");
    if (/actualiza Parabellum|update Parabellum|versión.*más nueva de Parabellum/i.test(message)) ids.push("actualizar");
    if (/puerto.*ocupado|port.*in use/i.test(message)) ids.push("puerto");
    if (/conexión|internet|no respondió|no responde|network|connection|timed out/i.test(message)) ids.push("reintentar");
    const choices = [];
    function add(id, label, glyph, run) {
      if (!choices.some(a => a.id === id)) choices.push({ id, label, glyph, run });
    }
    for (const id of ids) {
      if (id === "reintentar" && context && retryable.has(context.command))
        add(id, t("Reintentar", "Retry"), "refresh", async () => {
          const result = await invoke(context.command, context.args);
          window.dispatchEvent(new CustomEvent("operacion-recuperada", { detail: { command: context.command, args: context.args, result } }));
          toast(t("La operación terminó.", "The operation completed."));
        });
      else if (id === "escuela") add(id, t("Activar modo escuela", "Turn on school mode"), "school", async () => { if (!state.settings.schoolMode) await toggleSchool(); });
      else if (id === "recursos") add(id, t("Ver espacio y recursos", "View storage and resources"), "sliders", () => openSettings("resources"));
      else if ((id === "verificar" || id === "reparar-java") && instance)
        add("verificar", id === "reparar-java" ? t("Reparar Java", "Repair Java") : t("Verificar la instancia", "Check instance"), "refresh", async () => {
          await invoke("verificar_instancia", { id: instance.id });
          toast(t("Instancia revisada.", "Instance checked."));
        });
      else if ((id === "carpeta" || id === "abrir-carpeta") && instance) add("carpeta", t("Abrir la carpeta", "Open folder"), "folder", () => invoke("abrir_carpeta", { id: instance.id }));
      else if ((id === "carpeta" || id === "abrir-carpeta") && server && candidate) add("carpeta", t("Abrir la carpeta", "Open folder"), "folder", () => invoke("carpeta_servidor", { id: candidate }));
      else if (id === "sesion") add(id, t("Volver a iniciar sesión", "Sign in again"), "user", () => signInGoogle());
      else if (id === "conectar-mundos") add(id, t("Conectar Drive de mundos", "Connect worlds Drive"), "cloud", () => window.worldBackups.connect());
      else if (id === "actualizar") add(id, t("Buscar actualización", "Check for update"), "update", async () => { openSettings("about"); await checkLauncherUpdate(true, false); });
      else if (id === "biblioteca") add(id, t("Abrir la biblioteca", "Open library"), "cubes", () => navigate("home"));
      else if (id === "cerrar-juego" && instance) add(id, t("Ver juego abierto", "View running game"), "play", () => {
        navigate("instance", instance.id);
        toast(t("Guarda y cierra la partida desde Minecraft para continuar.", "Save and close the game from Minecraft to continue."));
      });
      else if (id === "cerrar-juego" && server && candidate) add(id, t("Ver servidor abierto", "View running server"), "play", () => {
        navigate("hosted", candidate);
        toast(t("Apaga el servidor desde su consola antes de continuar.", "Stop the server from its console before continuing."));
      });
      else if (id === "puerto" && context && /servidor/.test(context.command) && retryable.has(context.command))
        add(id, t("Usar otro puerto", "Use another port"), "refresh", () => invoke(context.command, context.args));
    }
    return { message, actions: choices.slice(0, 2) };
  }
  const panels = new Map();
  function panel(raw) {
    const result = present(raw);
    const key = String(++next);
    while (panels.size >= 64) panels.delete(panels.keys().next().value);
    panels.set(key, result.actions);
    return `<div class="recovery-message">${escapeHtml(friendlyError(result.message))}</div>${result.actions.length ? `<div class="recovery-actions">${result.actions.map((action, index) => `<button class="btn small" data-recovery="${key}" data-recovery-index="${index}">${icon(action.glyph)}${escapeHtml(action.label)}</button>`).join("")}</div>` : ""}`;
  }
  document.addEventListener("click", async (event) => {
    const button = event.target.closest("[data-recovery]");
    if (!button) return;
    const action = panels.get(button.dataset.recovery)?.[Number(button.dataset.recoveryIndex)];
    if (!action) return;
    button.disabled = true;
    try { await action.run(); }
    catch (error) { toast(String(error), "error"); }
    finally { button.disabled = false; }
  });
  return { capture, present, panel };
})();
