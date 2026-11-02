"use strict";

// Tu skin de Parabellum (skins.rs): sale de una cuenta premium, firmada por Mojang, y la ven los jugadores
// de Parabellum en los mundos que se abren por LAN o P2P, y en la lista de amigos.
window.skins = (() => {
  const html = escapeHtml;
  /** Nombre en minúsculas → imagen de su skin, ya cargada (para drawHead). */
  const faces = new Map();
  const asked = new Set();
  let mine = null;
  let preview = null;
  let busy = false;

  const overlay = document.createElement("div");
  overlay.id = "skin-dialog";
  overlay.className = "overlay hidden";
  overlay.innerHTML = `<div class="modal sk-modal" role="dialog" aria-modal="true" aria-labelledby="sk-title" tabindex="-1">
    <div class="modal-head cm-dialog-head"><span class="cm-dialog-icon">${icon("user")}</span><div class="grow"><h2 id="sk-title">${t("Tu skin", "Your skin")}</h2><p class="faint sp-subtitle">${t("La ven los jugadores de Parabellum", "Parabellum players see it")}</p></div><button class="circle-btn" id="sk-close" aria-label="${t("Cerrar", "Close")}">${icon("close")}</button></div>
    <div class="modal-body" id="sk-body"></div></div>`;
  document.body.insertBefore(overlay, $("#cm-dialog"));

  const visible = () => !overlay.classList.contains("hidden") && !overlay.classList.contains("closing");
  const load = (src) => new Promise((resolve, reject) => { const img = new Image(); img.onload = () => resolve(img); img.onerror = reject; img.src = src; });

  /** Dibuja el frente del jugador (16×32) con su skin; sin skin, una figura con la cara de su nombre. */
  async function paint(canvas, view, name) {
    const ctx = canvas.getContext("2d");
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, 16, 32);
    if (!view) {
      const head = document.createElement("canvas");
      head.width = head.height = 8;
      drawHead(head, name, { real: false });
      ctx.drawImage(head, 4, 0);
      ctx.fillStyle = "#3a6ea5"; ctx.fillRect(4, 8, 8, 12);
      ctx.fillStyle = "#c9a07a"; ctx.fillRect(0, 8, 4, 12); ctx.fillRect(12, 8, 4, 12);
      ctx.fillStyle = "#3b3270"; ctx.fillRect(4, 20, 8, 12);
      return;
    }
    const img = await load(view.image);
    const legacy = img.height === 32;
    const arm = view.slim ? 3 : 4;
    const part = (sx, sy, w, h, dx, dy, mirror = false) => {
      if (!mirror) return ctx.drawImage(img, sx, sy, w, h, dx, dy, w, h);
      ctx.save(); ctx.translate(dx + w, dy); ctx.scale(-1, 1); ctx.drawImage(img, sx, sy, w, h, 0, 0, w, h); ctx.restore();
    };
    part(8, 8, 8, 8, 4, 0);
    part(20, 20, 8, 12, 4, 8);
    part(44, 20, arm, 12, 4 - arm, 8);
    if (legacy) part(44, 20, arm, 12, 12, 8, true); else part(36, 52, arm, 12, 12, 8);
    part(4, 20, 4, 12, 4, 20);
    if (legacy) part(4, 20, 4, 12, 8, 20, true); else part(20, 52, 4, 12, 8, 20);
    // La segunda capa (gorro, chaqueta, mangas, pantalón).
    part(40, 8, 8, 8, 4, 0);
    if (!legacy) {
      part(20, 36, 8, 12, 4, 8);
      part(44, 36, arm, 12, 4 - arm, 8);
      part(52, 52, arm, 12, 12, 8);
      part(4, 36, 4, 12, 4, 20);
      part(4, 52, 4, 12, 8, 20);
    }
  }

  async function remember(name, view) {
    if (!name) return;
    if (view) faces.set(name.toLowerCase(), await load(view.image).catch(() => null));
    else faces.delete(name.toLowerCase());
    redrawHeads();
  }
  function redrawHeads() {
    for (const canvas of $$("canvas[data-head]")) drawHead(canvas, canvas.dataset.head);
    renderSidebar();
  }
  const myName = () => state.social?.friends?.name ?? state.settings.username;

  /** Las caras de los amigos con skin de Parabellum (una vez por sesión de cada uno). */
  async function refreshFaces() {
    if (!state.social?.connected) return;
    const names = [myName(), ...(state.social.friends?.friends ?? []).map((f) => f.name)].filter((n) => n && n !== "…" && !asked.has(n.toLowerCase()));
    if (!names.length) return;
    for (const name of names) asked.add(name.toLowerCase());
    try {
      const found = await invoke("skins_de", { nombres: names });
      await Promise.all(Object.entries(found ?? {}).map(async ([name, view]) => faces.set(name, await load(view.image).catch(() => null))));
      if (Object.keys(found ?? {}).length) redrawHeads();
    } catch {
      for (const name of names) asked.delete(name.toLowerCase());
    }
  }

  function render() {
    const shown = preview?.view ?? mine;
    const signedIn = state.social?.connected;
    const status = preview
      ? `<h3>${t(`La skin de ${html(preview.name)}`, `${html(preview.name)}'s skin`)}</h3><p class="faint">${preview.view.slim ? t("Brazos delgados (Alex).", "Slim arms (Alex).") : t("Brazos normales (Steve).", "Classic arms (Steve).")} ${t("Así te verán si la usas.", "This is how you will look if you use it.")}</p>`
      : mine
        ? `<h3>${t("Tu skin de Parabellum", "Your Parabellum skin")}</h3><p class="faint">${mine.from ? t(`Es la de ${html(mine.from)}.`, `It is ${html(mine.from)}'s.`) : ""} ${t("Para cambiarla, busca otra cuenta.", "To change it, look up another account.")}</p>`
        : `<h3>${t("Usas la skin de siempre", "You use the default skin")}</h3><p class="faint">${t("Steve, Alex u otra de las de Minecraft, según tu nombre. Elige la de una cuenta premium para que tus amigos te vean con ella.", "Steve, Alex or another of Minecraft's, depending on your name. Choose a premium account's skin so your friends see you with it.")}</p>`;
    $("#sk-body").innerHTML = `<div class="sk-layout">
      <div class="sk-stage"><canvas id="sk-figure" width="16" height="32" aria-label="${t("Vista de la skin", "Skin preview")}"></canvas></div>
      <div class="sk-side">${status}
        ${signedIn ? `<label class="label" for="sk-name">${t("Nombre de una cuenta premium", "Name of a premium account")}</label>
        <div class="sk-search"><input id="sk-name" maxlength="16" autocomplete="off" spellcheck="false" placeholder="${t("Por ejemplo, Notch", "For example, Notch")}" value="${html(preview?.name ?? "")}"><button class="btn" data-sk="search">${icon("search")}${t("Buscar", "Look up")}</button></div>
        <div class="field-error hidden" id="sk-error"></div>
        <div class="sk-actions">${preview ? `<button class="btn primary" data-sk="use">${icon("check")}${t("Usar esta skin", "Use this skin")}</button><button class="btn" data-sk="cancel">${t("Volver a la mía", "Back to mine")}</button>` : mine ? `<button class="btn" data-sk="remove">${icon("trash")}${t("Volver a la de siempre", "Back to the default")}</button>` : ""}</div>`
        : `<div class="wb-notice">${icon("user")}<span><b>${t("Inicia sesión con Google", "Sign in with Google")}</b>${t("Tu skin se guarda en tu cuenta de Parabellum, para que la vean tus amigos en cualquier PC.", "Your skin is saved in your Parabellum account, so your friends see it on any computer.")}</span></div><div class="sk-actions"><button class="btn primary" data-sk="sign-in">${t("Iniciar sesión", "Sign in")}</button></div>`}
        <p class="faint sk-where">${t("Se ve en los mundos que abre un jugador de Parabellum (LAN o P2P) desde Minecraft 1.16, con o sin mods, y en la lista de amigos. En servidores de otros, no.", "It shows in worlds opened by a Parabellum player (LAN or P2P) from Minecraft 1.16, with or without mods, and in the friends list. Not on other people's servers.")}</p>
      </div></div>`;
    paint($("#sk-figure"), shown, myName()).catch(() => {});
    $("#sk-name")?.addEventListener("keydown", (event) => { if (event.key === "Enter") $('[data-sk="search"]', overlay)?.click(); });
  }
  function fail(error) {
    const box = $("#sk-error");
    if (!box) return toast(String(error), "error");
    box.innerHTML = window.errorRecovery.panel(error);
    box.classList.remove("hidden");
  }

  async function open() {
    preview = null;
    openModal("#skin-dialog");
    $("#sk-body").innerHTML = `<div class="content-loading"><span class="spinner small"></span></div>`;
    try { mine = state.social?.connected ? await invoke("skin_mia") : null; }
    catch (error) { mine = null; render(); return fail(error); }
    render();
    $("#sk-name")?.focus();
  }
  function close() { closeModal("#skin-dialog"); }

  overlay.addEventListener("click", async (event) => {
    if (event.target === overlay) return close();
    const target = event.target.closest("[data-sk]");
    if (!target || busy) return;
    const action = target.dataset.sk;
    if (action === "cancel") { preview = null; return render(); }
    if (action === "sign-in") { close(); return signInGoogle(); }
    busy = true;
    target.disabled = true;
    try {
      if (action === "search") {
        const name = $("#sk-name").value.trim();
        if (!name) return fail(t("Escribe el nombre de una cuenta premium.", "Type the name of a premium account."));
        preview = { name, view: await invoke("skin_buscar", { nombre: name }) };
        render();
      }
      if (action === "use") {
        mine = await invoke("skin_usar", { nombre: preview.name });
        preview = null;
        render();
        await remember(myName(), mine);
        toast(t("Listo: tus amigos te verán con ella desde la próxima partida.", "Done: your friends will see you with it from the next game."));
      }
      if (action === "remove") {
        const yes = await confirmAction({ title: t("¿Volver a la skin de siempre?", "Back to the default skin?"), text: t("Tus amigos te verán con la skin de Minecraft que sale de tu nombre.", "Your friends will see you with the Minecraft skin that comes from your name."), ok: t("Quitar mi skin", "Remove my skin") });
        if (!yes) return;
        await invoke("skin_quitar");
        mine = null;
        render();
        await remember(myName(), null);
      }
    } catch (error) { fail(error); }
    finally { busy = false; if (target.isConnected) target.disabled = false; }
  });
  $("#sk-close").onclick = close;
  listen("social", () => setTimeout(refreshFaces, 500));
  setTimeout(refreshFaces, 3000);

  return { open, close, visible, face: (name) => faces.get(String(name ?? "").toLowerCase()) ?? null };
})();

// Datos aislados para revisar la ventana en el navegador; nunca se usan dentro de Tauri.
window.skinsDemo = async (command, args) => {
  if (tauri) throw new Error("Solo demostración");
  const demo = (window.demoSkin ??= { mine: null });
  const image = () => {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 64;
    const ctx = canvas.getContext("2d");
    const fill = (color, x, y, w, h) => { ctx.fillStyle = color; ctx.fillRect(x, y, w, h); };
    fill("#e8b48f", 8, 8, 8, 8); fill("#3d2a1c", 8, 8, 8, 2); fill("#fff", 9, 12, 2, 1); fill("#fff", 13, 12, 2, 1); fill("#5b3fd1", 10, 12, 1, 1); fill("#5b3fd1", 13, 12, 1, 1);
    fill("#8b5cf6", 20, 20, 8, 12); fill("#e8b48f", 44, 20, 4, 12); fill("#e8b48f", 36, 52, 4, 12);
    fill("#26215c", 4, 20, 4, 12); fill("#26215c", 20, 52, 4, 12);
    return canvas.toDataURL();
  };
  if (command === "skin_mia") return demo.mine;
  if (command === "skin_buscar") return { image: image(), slim: false, from: args.nombre, at: Date.now() };
  if (command === "skin_usar") return (demo.mine = { image: image(), slim: false, from: args.nombre, at: Date.now() });
  if (command === "skin_quitar") { demo.mine = null; return; }
  if (command === "skins_de") return {};
  throw new Error("Demostración de skins: comando desconocido");
};
