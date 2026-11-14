"use strict";

// Perfil local y personalización de la cuenta social de Parabellum.
window.skins = (() => {
  const html = escapeHtml;
  /** Nombre en minúsculas → imagen de su skin, ya cargada (para drawHead). */
  const faces = new Map();
  const asked = new Set();
  let mine = null;
  let preview = null;
  let busy = false;
  /** El puente puede subir PNG propios (tiene la clave de MineSkin). */
  let uploads = false;

  const page = $("#page-account");
  const editor = $("#sk-body");
  const editorHome = editor.parentElement;
  let mountedForSetup = false;
  const bannerButton = $('[data-account="banner"]', page);
  const removeBannerButton = $('[data-account="remove-banner"]', page);
  let banner = null;
  let bannerBusy = false;
  let loadingProfile = false;
  let loadingSkin = false;
  let lastIdentity = null;
  let loadRequest = 0;
  const visible = () => state.page === "account";
  const load = (src) => new Promise((resolve, reject) => { const img = new Image(); img.onload = () => resolve(img); img.onerror = reject; img.src = src; });

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
  const accountIdentity = () => state.social?.connected ? state.social.email || "connected" : "";

  async function saveName(name) {
    clearTimeout(saveTimer);
    state.settings = await invoke("cambiar_nombre_perfil", { nombre: name });
    renderSidebar();
    renderSteps();
  }

  function setCover(cover, source) {
    cover.style.backgroundImage = source && /^data:image\/jpeg;base64,[A-Za-z0-9+/]+=*$/.test(source) ? `url("${source}")` : "";
  }

  function renderIdentity() {
    const title = $("#my-account-title");
    title.textContent = state.settings.username;
    $("#my-account-status").textContent = state.social?.connected
      ? t("Cuenta de Google conectada", "Google account connected")
      : t("Perfil local", "Local profile");
    const avatar = $("#my-account-avatar");
    avatar.dataset.head = state.settings.username;
    drawHead(avatar, state.settings.username);
  }

  function renderBanner() {
    setCover($("#my-account-cover"), banner);
    const signedIn = state.social?.connected;
    bannerButton.innerHTML = signedIn
      ? icon("image") + (banner ? t("Cambiar banner", "Change banner") : t("Añadir banner", "Add banner"))
      : icon("user") + t("Inicia sesión para añadir un banner", "Sign in to add a banner");
    removeBannerButton.innerHTML = icon("trash") + t("Quitar banner", "Remove banner");
    removeBannerButton.classList.toggle("hidden", !signedIn || !banner);
    for (const button of [bannerButton, removeBannerButton]) button.disabled = loadingProfile || bannerBusy;
  }

  function renderName() {
    $("#my-account-name-title").textContent = t("Nombre de jugador", "Player name");
    const form = $("#my-account-name-form");
    form.innerHTML = `${textField({ id: "my-player-name", label: t("Nombre en Minecraft", "Minecraft name"), value: state.settings.username, max: 16 })}
      <p class="faint account-note">${t("Retorcidos asocia tu progreso a tu nombre. Si lo cambias, el servidor puede recibirte como un jugador nuevo.", "Retorcidos ties your progress to your name. If you change it, the server may treat you as a new player.")}</p>
      <div id="my-account-name-error" class="field-error hidden" role="alert"></div>
      <div><button type="submit" class="btn primary">${icon("check")}${t("Guardar nombre", "Save name")}</button></div>`;
    wireCounters(form);
    const input = $("#my-player-name", form);
    const submit = $("button[type=submit]", form);
    const errorBox = $("#my-account-name-error", form);
    submit.disabled = true;
    input.oninput = () => {
      submit.disabled = input.value.trim() === state.settings.username;
      errorBox.classList.add("hidden");
    };
    form.onsubmit = async (event) => {
      event.preventDefault();
      const name = input.value.trim();
      if (submit.disabled || name === state.settings.username) return;
      submit.disabled = true;
      input.disabled = true;
      try {
        if (!/^[A-Za-z0-9_]{1,16}$/.test(name)) throw t("Usa de 1 a 16 letras, números o guiones bajos.", "Use 1 to 16 letters, numbers or underscores.");
        if (otherAccountNamed(name)) throw t("Ya tienes otra cuenta con ese nombre. Elígela en el menú «Jugando como».", "You already have an account with that name. Select it from the «Playing as» menu.");
        await saveName(name);
        await remember(name, mine);
        renderIdentity();
        render();
        toast(t(`Ahora te llamas ${name}.`, `Your name is now ${name}.`));
      } catch (error) {
        errorBox.textContent = String(error);
        errorBox.classList.remove("hidden");
        input.focus();
      } finally {
        input.disabled = false;
        submit.disabled = name === state.settings.username;
      }
    };
  }

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

  async function refreshFace(name) {
    if (faces.has(name.toLowerCase()) || !state.social?.connected) return;
    try {
      const found = await invoke("skins_de", { nombres: [name] });
      if (found?.[name.toLowerCase()]) await remember(name, found[name.toLowerCase()]);
    } catch {}
  }

  function render() {
    const shown = preview?.view ?? mine;
    const signedIn = state.social?.connected;
    const status = preview ? t(`Vista previa de ${preview.name}`, `Previewing ${preview.name}`)
      : mine?.from ? t(`Activa · importada de ${mine.from}`, `Active · imported from ${mine.from}`)
      : mine ? t("Skin activa", "Active skin") : t("Sin skin personalizada", "No custom skin");
    const turnButton = (turn, glyph, label) => `<button class="circle-btn small" type="button" data-turn="${turn}" aria-label="${label}" title="${label}">${icon(glyph)}</button>`;
    editor.innerHTML = `<div class="sk-layout">
      <section class="account-card sk-card-view" aria-labelledby="sk-view-title">
        <div class="sk-view-head">
          <h2 id="sk-view-title">${t("Visor de skin", "Skin viewer")}</h2>
          <p class="faint" role="status">${html(status)}</p>
        </div>
        <div class="sk-stage" id="sk-figure" tabindex="0" role="region" aria-label="${t("Visor de skin en 3D. Usa las flechas para girar.", "3D skin viewer. Use arrow keys to rotate.")}">
          <div class="sk-scene" aria-hidden="true"><div class="sk-model"></div></div>
          ${shown ? `<span class="sk-model-type">${shown.slim ? "Alex" : "Steve"}</span>` : ""}
        </div>
        <div class="sk-view-foot">
          <span class="faint">${t("Arrastra para girar", "Drag to rotate")}</span>
          <div id="sk-controls" class="sk-controls">
            ${turnButton(-45, "left", t("Girar a la izquierda", "Rotate left"))}
            ${turnButton("reset", "refresh", t("Restablecer vista", "Reset view"))}
            ${turnButton(45, "right", t("Girar a la derecha", "Rotate right"))}
          </div>
        </div>
      </section>
      <section class="account-card sk-card-editor account-fields" aria-labelledby="sk-editor-title">
        <h2 id="sk-editor-title">${t("Cambiar skin", "Change skin")}</h2>
        ${signedIn ? `<label class="label" for="sk-name">${t("Importar por nombre de Minecraft", "Import by Minecraft name")}</label>
          <div class="sk-search"><input id="sk-name" maxlength="16" autocomplete="off" spellcheck="false" placeholder="${t("Nombre de una cuenta premium", "Premium account name")}" value="${html(preview?.name ?? "")}"><button class="btn" type="button" data-sk="search">${icon("search")}${t("Buscar", "Look up")}</button></div>
          <div class="sk-actions">${preview ? `<button class="btn primary" type="button" data-sk="use">${icon("check")}${t("Usar esta skin", "Use this skin")}</button><button class="btn" type="button" data-sk="cancel">${t("Volver a la mía", "Back to mine")}</button>` : ""}</div>
          ${uploads ? `<div class="sk-upload">
            <div><b>${t("Usar un archivo", "Use a file")}</b><p class="faint">PNG · 64 × 64</p></div>
            <button class="btn" data-sk="upload" type="button">${icon("upload")}${t("Subir PNG…", "Upload PNG…")}</button>
            <label class="check"><input type="checkbox" id="sk-slim"><span>${t("Brazos delgados (Alex)", "Slim arms (Alex)")}</span></label>
          </div>` : ""}
          ${mine && !preview && !mountedForSetup ? `<div class="sk-actions"><button class="btn ghost" type="button" data-sk="remove">${icon("trash")}${t("Volver a la de siempre", "Back to the default")}</button></div>` : ""}`
          : `<div class="wb-notice">${icon("user")}<span><b>${t("Inicia sesión con Google", "Sign in with Google")}</b>${t("Tu skin se guarda en tu cuenta de Parabellum, para que la vean tus amigos en cualquier PC.", "Your skin is saved in your Parabellum account, so your friends see it on any computer.")}</span></div><div class="sk-actions"><button class="btn primary" type="button" data-sk="sign-in">${t("Iniciar sesión", "Sign in")}</button></div>`}
        <div class="field-error hidden" id="sk-error" role="alert"></div>
        <p class="faint sk-where">${t("Otros jugadores de Parabellum ven tu skin en Retorcidos y en las partidas compartidas desde el launcher.", "Other Parabellum players see your skin in Retorcidos and in games shared from the launcher.")}</p>
      </section>
    </div>`;
    SkinViewer.mount($("#sk-figure", editor), $("#sk-controls", editor), shown, state.settings.username).catch(error => fail(error));
    $("#sk-name")?.addEventListener("keydown", (event) => {
      if (event.key !== "Enter") return;
      event.preventDefault();
      $('[data-sk="search"]', editor)?.click();
    });
  }
  function fail(error) {
    const box = $("#sk-error");
    if (!box) return toast(String(error), "error");
    box.innerHTML = window.errorRecovery.panel(error);
    box.classList.remove("hidden");
  }

  async function open(resetName = true) {
    const request = ++loadRequest;
    const identity = accountIdentity();
    lastIdentity = identity;
    preview = null;
    mine = null;
    banner = null;
    if (resetName) renderName();
    renderIdentity();
    loadingProfile = true;
    renderBanner();
    loadingSkin = true;
    editor.innerHTML = `<div class="content-loading" role="status"><span class="spinner small"></span>${t("Cargando tu skin…", "Loading your skin…")}</div>`;
    const [skinResult, uploadsResult, bannerResult] = await Promise.allSettled(identity
      ? [invoke("skin_mia"), invoke("skin_png_disponible"), mountedForSetup ? null : invoke("banner_mio")] : [null, false, null]);
    if (request !== loadRequest || identity !== accountIdentity()) return;
    mine = skinResult.value ?? null;
    uploads = uploadsResult.value ?? false;
    banner = bannerResult.value ?? null;
    loadingProfile = false;
    loadingSkin = false;
    renderBanner();
    render();
    if (mine) remember(state.settings.username, mine);
    editor.dispatchEvent(new Event("skin-changed"));
    if (skinResult.status === "rejected") fail(skinResult.reason);
    if (bannerResult.status === "rejected") toast(String(bannerResult.reason), "error");
  }

  async function updateBanner(remove) {
    if (!state.social?.connected) return signInGoogle();
    if (bannerBusy) return;
    const identity = accountIdentity();
    bannerBusy = true;
    renderBanner();
    try {
      const file = remove ? null : await pickImageFile();
      if (!file && !remove) return;
      const image = file ? (await reduceImage(file, 1200, 200000)).data : null;
      if (identity !== accountIdentity()) return;
      await invoke("guardar_banner", { imagen: image });
      if (identity !== accountIdentity()) return;
      banner = image;
      toast(remove ? t("Banner quitado.", "Banner removed.") : t("Banner guardado.", "Banner saved."));
    } catch (error) {
      toast(String(error), "error");
    } finally {
      bannerBusy = false;
      renderBanner();
    }
  }
  bannerButton.onclick = () => updateBanner(false);
  removeBannerButton.onclick = () => updateBanner(true);

  editor.addEventListener("click", async (event) => {
    const target = event.target.closest("[data-sk]");
    if (!target || busy) return;
    const identity = accountIdentity();
    const action = target.dataset.sk;
    if (action === "cancel") { preview = null; return render(); }
    if (action === "sign-in") return signInGoogle();
    busy = true;
    for (const button of $$("button", editor)) button.disabled = true;
    editor.dispatchEvent(new Event("skin-busy"));
    try {
      if (action === "search") {
        const name = $("#sk-name").value.trim();
        if (!name) return fail(t("Escribe el nombre de una cuenta premium.", "Type the name of a premium account."));
        const view = await invoke("skin_buscar", { nombre: name });
        if (identity !== accountIdentity()) return;
        preview = { name, view };
        render();
        $('[data-sk="use"]', editor)?.focus();
      }
      if (action === "upload") {
        const uploaded = await invoke("skin_subir", { delgada: !!$("#sk-slim")?.checked });
        if (uploaded && identity === accountIdentity()) {
          mine = uploaded;
          preview = null;
          render();
          await remember(state.settings.username, mine);
          editor.dispatchEvent(new Event("skin-changed"));
          toast(t("Skin guardada.", "Skin saved."));
        }
      }
      if (action === "use") {
        const saved = await invoke("skin_usar", { nombre: preview.name });
        if (identity !== accountIdentity()) return;
        mine = saved;
        preview = null;
        render();
        await remember(state.settings.username, mine);
        editor.dispatchEvent(new Event("skin-changed"));
        toast(t("Skin guardada.", "Skin saved."));
      }
      if (action === "remove") {
        const yes = await confirmAction({ title: t("¿Volver a la skin de siempre?", "Back to the default skin?"), text: t("Tus amigos te verán con la skin de Minecraft que sale de tu nombre.", "Your friends will see you with the Minecraft skin that comes from your name."), ok: t("Quitar mi skin", "Remove my skin") });
        if (!yes || identity !== accountIdentity()) return;
        await invoke("skin_quitar");
        if (identity !== accountIdentity()) return;
        mine = null;
        render();
        await remember(state.settings.username, null);
        editor.dispatchEvent(new Event("skin-changed"));
      }
    } catch (error) { fail(error); }
    finally {
      busy = false;
      for (const button of $$("button", editor)) button.disabled = false;
      editor.dispatchEvent(new Event("skin-busy"));
    }
  });
  listen("social", () => setTimeout(() => {
    refreshFaces();
    if (visible() && !mountedForSetup && accountIdentity() !== lastIdentity) open(false);
  }, 500));
  setTimeout(refreshFaces, 3000);

  return {
    open, setCover, refreshFace, saveName, editor,
    busy: () => busy,
    loading: () => loadingSkin,
    pending: () => !!preview,
    active: () => mine,
    mountIn(parent) {
      mountedForSetup = !!parent;
      (parent ?? editorHome).appendChild(editor);
      if (parent) open();
      else if (visible()) render();
    },
    face: (name) => faces.get(String(name ?? "").toLowerCase()) ?? null,
  };
})();

// Datos aislados para revisar la ventana en el navegador; nunca se usan dentro de Tauri.
window.skinsDemo = async (command, args) => {
  if (tauri) throw new Error("Solo demostración");
  const demo = (window.demoSkin ??= { mine: null, banner: null });
  const image = () => SkinViewer.mannequin(args.nombre ?? "Parabellum", !!args.delgada, "#8b5cf6");
  if (command === "skin_mia") return demo.mine;
  if (command === "skin_buscar") return { image: image(), slim: false, from: args.nombre, at: Date.now() };
  if (command === "skin_usar") return (demo.mine = { image: image(), slim: false, from: args.nombre, at: Date.now() });
  if (command === "skin_quitar") { demo.mine = null; return; }
  if (command === "skins_de") return {};
  if (command === "skin_png_disponible") return true;
  if (command === "skin_subir") return (demo.mine = { image: image(), slim: args.delgada, from: null, at: Date.now() });
  if (command === "banner_mio") return demo.banner;
  if (command === "guardar_banner") { demo.banner = args.imagen; return; }
  throw new Error("Demostración de skins: comando desconocido");
};
