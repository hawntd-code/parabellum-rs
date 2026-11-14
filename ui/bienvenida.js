"use strict";

const welcomeGuide = (() => {
  const steps = [
    { icon: "user", label: () => t("Cuenta de Google", "Google account") },
    { icon: "pencil", label: () => t("Nombre de jugador", "Player name") },
    { icon: "sparkle", label: () => t("Skin de Minecraft", "Minecraft skin") },
    { icon: "gauge", label: () => t("Memoria para jugar", "Memory for Minecraft") },
    { icon: "community", label: () => t("Comunidades", "Communities") },
  ];
  const dialog = $("#setup-dialog");
  const content = $("#setup-content");
  const next = $("#setup-next");
  const back = $("#setup-back");
  const later = $("#setup-later");
  const close = $("#setup-close");
  const errorBox = $("#setup-error");
  let step = 0;
  let busy = false;
  let session = 0;
  let remember = true;
  let name = "";
  let memory = 4;
  let lookup = "";
  let invite = null;
  let selected = null;
  let members = [];
  let directory = [];
  let cursor = null;
  let loading = false;
  let loadError = "";
  let codeError = "";
  let codeFor = "";
  let searchingCode = false;

  const visible = () => !dialog.matches(".hidden, .closing");
  const connected = () => !!state.social?.connected && !state.social?.connecting;
  const setText = (id, text) => { $(id).textContent = text; };
  const showError = error => {
    errorBox.textContent = error ? friendlyError(String(error)) : "";
    errorBox.classList.toggle("hidden", !error);
  };
  const safeMemory = () => Math.max(1, Math.min(32, state.systemRam - 3));
  const recommended = () => Math.min(safeMemory(), state.systemRam < 8 ? 2 : state.systemRam < 12 ? 4 : state.systemRam < 16 ? 6 : 8);
  const isBusy = () => busy || (step === 2 && window.skins.busy());

  function navigation() {
    const connecting = step === 0 && !!(state.social?.signingIn || state.social?.connecting);
    const restoring = step === 0 && !!state.social?.restoring;
    const locked = isBusy();
    const previewing = step === 2 && window.skins.pending();
    close.disabled = locked && step !== 0;
    back.disabled = locked;
    back.classList.toggle("hidden", step === 0);
    later.classList.toggle("hidden", step !== 0);
    let label = t("Guardar y continuar", "Save and continue");
    if (step === 0) label = connected() ? t("Continuar", "Continue")
      : state.social?.signedIn ? t("Reconectar", "Reconnect") : t("Continuar con Google", "Continue with Google");
    if (step === 2) label = !previewing && !window.skins.active() && !window.skins.loading()
      ? t("Omitir skin", "Skip skin") : t("Continuar", "Continue");
    if (step === 4) label = selected ? t("Abrir comunidad", "Open community") : t("Finalizar", "Finish");
    next.innerHTML = (locked || connecting || restoring ? '<span class="spinner small" aria-hidden="true"></span>' : "") + label
      + (locked || connecting || restoring ? "" : icon(step === 4 ? "check" : "right"));
    next.disabled = locked || connecting || restoring || (step === 0 && state.social?.available === false)
      || previewing;
    content.setAttribute("aria-busy", String(locked));
  }

  function google() {
    const social = state.social;
    const ready = connected();
    const waiting = social?.restoring || social?.signingIn || social?.connecting || (busy && !ready);
    setText("#setup-title", t("Bienvenido a Parabellum", "Welcome to Parabellum"));
    setText("#setup-description", t("Conecta Google para guardar tu perfil y entrar a comunidades.", "Connect Google to save your profile and join communities."));
    const body = social?.available === false
      ? t("El inicio de sesión con Google no está disponible en esta copia.", "Google sign-in is unavailable in this copy.")
      : waiting ? (social?.connecting || social?.restoring ? t("Cargando tu perfil…", "Loading your profile…") : t("Elige tu cuenta en el navegador. La guía seguirá aquí al volver.", "Choose your account in the browser. This guide will be here when you return."))
      : ready ? t("Cuenta conectada", "Account connected")
      : social?.signedIn ? t("Vuelve a conectar tu perfil", "Reconnect your profile")
      : t("Inicia sesión en tu navegador", "Sign in in your browser");
    content.innerHTML = `<div class="setup-google" role="status">
      <span class="setup-symbol">${icon(ready ? "check" : "user")}</span>
      <div><strong>${escapeHtml(body)}</strong>${social?.email ? `<p class="faint">${escapeHtml(social.email)}</p>` : ""}</div>
    </div>
    ${!social?.signedIn && social?.available !== false && !waiting ? `<label class="check setup-remember"><input type="checkbox" id="setup-remember" ${remember ? "checked" : ""}><span>${t("Recordar la sesión en este equipo", "Keep me signed in on this computer")}</span></label>` : ""}`;
    $("#setup-remember")?.addEventListener("change", event => { remember = event.target.checked; });
    if (!waiting && !ready && social?.error) showError(social.error);
    navigation();
  }

  function playerName() {
    const current = state.settings.username;
    setText("#setup-title", t("¿Cómo te llamas en Minecraft?", "What's your Minecraft name?"));
    setText("#setup-description", t("Con este nombre te verán otros jugadores. Usa de 1 a 16 letras, números o guiones bajos.", "This is the name other players will see. Use 1 to 16 letters, numbers or underscores."));
    content.innerHTML = `<div class="setup-name"><canvas class="head" id="setup-name-avatar" width="8" height="8"></canvas>
      ${textField({ id: "setup-name", label: t("Nombre de jugador", "Player name"), value: name, max: 16, placeholder: t("Por ejemplo, Hawntd", "For example, Hawntd") })}
    </div>
    ${current !== "Jugador" ? `<p class="setup-hint faint">${t("Si ya juegas en Retorcidos, usa el mismo nombre para entrar con tu progreso.", "If you already play on Retorcidos, keep the same name to use your existing progress.")}</p>` : ""}`;
    const input = $("#setup-name");
    const avatar = $("#setup-name-avatar");
    drawHead(avatar, name || current);
    wireCounters(content);
    input.oninput = () => {
      name = input.value;
      drawHead(avatar, name || current);
      showError(null);
    };
    input.onkeydown = event => { if (event.key === "Enter") next.click(); };
    if (state.social?.nameConflict) showError(t("Elige otro nombre. El actual está en uso.", "Choose another name. Your current name is taken."));
  }

  function skin() {
    setText("#setup-title", t("Elige tu apariencia", "Choose your appearance"));
    setText("#setup-description", t("Importa una skin o conserva la que usas ahora.", "Import a skin or keep your current appearance."));
    content.innerHTML = '<div id="setup-skin"></div>';
    window.skins.mountIn($("#setup-skin"));
  }

  function ram() {
    const total = state.systemRam;
    const minimum = total < 6 ? 1 : 2;
    const limit = Math.max(minimum, state.settings.setupCompleted ? state.settings.ramGb : 0, safeMemory());
    memory = Math.min(limit, Math.max(minimum, memory));
    setText("#setup-title", t("¿Cuánta RAM usará Minecraft?", "How much RAM should Minecraft use?"));
    setText("#setup-description", t("Elige la memoria predeterminada para tus juegos. Puedes ajustarla en cada instancia.", "Choose the default memory for your games. You can adjust it for each instance."));
    content.innerHTML = `<div class="setup-memory">
      <div class="setup-memory-head">
        <label for="setup-ram">${t("Memoria asignada", "Allocated memory")}</label>
        <output id="setup-ram-value" for="setup-ram"></output>
      </div>
      <input type="range" id="setup-ram" min="${minimum}" max="${limit}" step="1" value="${memory}" ${limit === minimum ? "disabled" : ""} aria-describedby="setup-memory-total">
      <div class="setup-memory-foot"><span id="setup-memory-total" class="faint">${t(`${total} GB de RAM en este equipo`, `${total} GB of RAM on this computer`)}</span>
        <button type="button" class="btn small" id="setup-recommended">${t(`Usar ${recommended()} GB recomendados`, `Use recommended ${recommended()} GB`)}</button></div>
    </div>
    <p class="faint setup-hint">${t("Minecraft sin mods suele usar entre 2 y 4 GB. Los paquetes grandes, como Retorcidos, suelen necesitar más.", "Minecraft without mods usually uses 2–4 GB. Large packs like Retorcidos usually need more.")}</p>`;
    const range = $("#setup-ram");
    const update = () => {
      memory = Number(range.value);
      range.setAttribute("aria-valuetext", `${memory} GB`);
      setText("#setup-ram-value", `${memory} GB`);
    };
    range.oninput = update;
    $("#setup-recommended").onclick = () => { range.value = recommended(); update(); range.focus(); };
    update();
  }

  function communityRow(card, fromInvite = false) {
    const hasMembership = card.member || members.some(member => member.id === card.id);
    const action = hasMembership ? "select" : fromInvite || card.access === "open" ? "join" : "code";
    const label = hasMembership ? t("Elegir", "Select") : action === "join" ? t("Unirme", "Join") : t("Usar código", "Use code");
    const isSelected = card.id === selected?.id;
    return `<article class="setup-community ${isSelected ? "selected" : ""}">
      ${communityImage(card, "small")}
      <div class="setup-community-text"><b>${escapeHtml(card.name)}</b>${card.description ? `<p class="faint">${escapeHtml(card.description)}</p>` : ""}</div>
      <button type="button" class="btn small" data-setup-community="${escapeHtml(card.id)}" data-action="${action}" ${isSelected ? "disabled" : ""}
        aria-label="${escapeHtml(`${label} · ${card.name}`)}">${isSelected ? icon("check") + t("Elegida", "Selected") : label}</button>
    </article>`;
  }

  function renderCommunities() {
    const list = $("#setup-communities");
    if (!list) return;
    const ownIds = new Set(members.map(card => card.id));
    const compare = (a, b) => a.name.localeCompare(b.name, locale());
    const available = directory.filter(card => !ownIds.has(card.id)).sort(compare);
    const section = (label, cards) => cards.length ? `<section><h3>${label}</h3>${cards.map(communityRow).join("")}</section>` : "";
    list.innerHTML = section(t("Tus comunidades", "Your communities"), [...members].sort(compare))
      + section(t("En el directorio", "In the directory"), available)
      + (loading ? `<p class="setup-loading faint" role="status"><span class="spinner small"></span>${t("Cargando comunidades…", "Loading communities…")}</p>` : "")
      + (loadError ? `<p class="field-error" role="alert">${escapeHtml(loadError)}</p><button type="button" id="setup-communities-retry" class="btn small">${t("Reintentar", "Try again")}</button>` : "")
      + (!loading && !loadError && !members.length && !available.length ? `<p class="setup-hint faint">${t("Todavía no hay comunidades en el directorio. Puedes entrar con un código.", "There are no communities in the directory yet. You can join with an invite code.")}</p>` : "")
      + (!loading && cursor ? `<button type="button" id="setup-communities-more" class="btn small">${t("Ver más comunidades", "Show more communities")}</button>` : "");
    $("#setup-communities-retry")?.addEventListener("click", () => loadCommunities());
    $("#setup-communities-more")?.addEventListener("click", () => loadCommunities(cursor));
  }

  function invitePreview() {
    const box = $("#setup-invite");
    if (!box) return;
    box.innerHTML = invite ? communityRow(invite, true) : "";
    box.classList.toggle("hidden", !invite);
    const error = $("#setup-code-error");
    error.textContent = codeError;
    error.classList.toggle("hidden", !codeError);
  }

  function communities() {
    setText("#setup-title", t("Encuentra tu comunidad", "Find your community"));
    setText("#setup-description", t("Elige dónde jugar o usa un código de invitación. También puedes hacerlo después.", "Choose where to play or use an invite code. You can also do this later."));
    content.innerHTML = `<div id="setup-communities"></div>
      <form id="setup-code-form" class="setup-code">
        <label class="label" id="setup-code-label" for="setup-code"></label>
        <div class="cm-inline"><input id="setup-code" maxlength="12" value="${escapeHtml(lookup)}" placeholder="ABCD-2345" autocomplete="off" spellcheck="false">
          <button type="submit" class="btn">${icon("search")}${t("Buscar", "Look up")}</button></div>
        <p id="setup-code-error" class="field-error hidden" role="alert"></p>
        <div id="setup-invite" class="hidden"></div>
      </form>`;
    renderCommunities();
    invitePreview();
    updateCodeLabel();
    $("#setup-code").oninput = (event) => {
      lookup = event.target.value;
      invite = null;
      codeError = "";
      invitePreview();
    };
    $("#setup-code-form").onsubmit = event => {
      event.preventDefault();
      if (busy || searchingCode) return;
      const code = lookup.trim();
      if (!code) {
        codeError = t("Escribe el código que te compartieron.", "Enter the invite code you were given.");
        return invitePreview();
      }
      const request = session;
      const button = $('button[type="submit"]', $("#setup-code-form"));
      searchingCode = true;
      button.disabled = true;
      void (async () => {
        invite = null;
        codeError = "";
        invitePreview();
        try {
          const found = await invoke("ver_invitacion", { codigo: code });
          if (request !== session || step !== 4 || lookup.trim() !== code) return;
          invite = { ...found, code };
          invitePreview();
          $("[data-setup-community]", $("#setup-invite"))?.focus();
        } catch (error) {
          if (request === session && step === 4 && lookup.trim() === code) {
            codeError = friendlyError(String(error));
            invitePreview();
          }
        } finally {
          if (request === session) {
            searchingCode = false;
            button.disabled = false;
          }
        }
      })();
    };
  }

  function updateCodeLabel() {
    setText("#setup-code-label", codeFor
      ? t(`Código para ${codeFor}`, `Invite code for ${codeFor}`)
      : t("Código de invitación", "Invite code"));
  }

  async function loadCommunities(after = null) {
    if (loading) return;
    const request = session;
    loading = true;
    loadError = "";
    renderCommunities();
    const [mine, page] = await Promise.allSettled([
      after ? members : invoke("comunidades"),
      invoke("directorio_comunidades", { despues: after }),
    ]);
    if (request !== session) return;
    loading = false;
    if (mine.status === "fulfilled") members = mine.value;
    if (page.status === "fulfilled") {
      directory = after ? [...directory, ...page.value.entries] : page.value.entries;
      cursor = page.value.next;
    }
    const failure = [mine, page].find(result => result.status === "rejected");
    loadError = failure ? friendlyError(String(failure.reason)) : "";
    renderCommunities();
  }

  function show(index) {
    if (step === 2 && index !== 2) window.skins.mountIn(null);
    step = index;
    showError(null);
    $("#setup-steps").innerHTML = steps.map((entry, i) => `<li ${i === step ? 'aria-current="step"' : ""} class="${i < step ? "completed" : ""}">
      <span class="setup-step-icon">${icon(i < step ? "check" : entry.icon)}</span><span>${entry.label()}</span>
    </li>`).join("");
    [google, playerName, skin, ram, communities][step]();
    $(".setup-body").scrollTop = 0;
    navigation();
    if (step === 4) loadCommunities();
    requestAnimationFrame(() => {
      if (visible() && index === step) ($("#setup-name") ?? $("#setup-title")).focus({ preventScroll: true });
    });
  }

  function open() {
    if (visible()) return;
    session++;
    busy = false;
    remember = true;
    name = state.settings.username === "Jugador" && !state.social?.friends?.name ? "" : state.settings.username;
    memory = !state.settings.setupCompleted && state.settings.ramGb === 4 ? recommended() : state.settings.ramGb;
    members = [];
    directory = [];
    selected = null;
    lookup = "";
    invite = null;
    cursor = null;
    loading = false;
    loadError = "";
    codeError = "";
    codeFor = "";
    searchingCode = false;
    setText("#setup-label", t("Configura Parabellum", "Set up Parabellum"));
    close.ariaLabel = t("Cerrar la guía", "Close setup");
    setText("#setup-later", t("Hacerlo más tarde", "Set up later"));
    back.innerHTML = icon("left") + t("Atrás", "Back");
    openModal("#setup-dialog");
    show(connected() ? 1 : 0);
  }

  function hide() {
    session++;
    if (step === 2) window.skins.mountIn(null);
    closeModal("#setup-dialog");
    if (state.page === "account") window.skins.open();
  }

  function dismiss() {
    if (isBusy() && step !== 0) return;
    hide();
  }

  async function run(action) {
    if (isBusy()) return;
    const request = session;
    busy = true;
    showError(null);
    navigation();
    try {
      await action(() => request === session && visible());
    } catch (error) {
      if (request === session) showError(error);
    } finally {
      if (request === session) {
        busy = false;
        navigation();
      }
    }
  }

  async function signIn(alive) {
    if (!connected()) {
      if (state.social?.signingIn || state.social?.connecting || state.social?.restoring) return;
      google();
      try {
        state.social = state.social?.signedIn
          ? await invoke("reconectar_amigos", { configurarNombre: true })
          : await invoke("iniciar_sesion_google", { recordar: remember, configurarNombre: true });
      } catch (error) {
        if (alive()) { busy = false; google(); }
        throw error;
      }
      chatExperience.syncClock();
      renderFriends();
    }
    if (!alive()) return;
    if (!connected()) {
      busy = false;
      google();
      throw state.social?.error ?? t("No se pudo conectar con Parabellum.", "Could not connect to Parabellum.");
    }
    name = state.settings.username === "Jugador" && !state.social?.friends?.name ? "" : state.settings.username;
    memory = state.settings.ramGb === 4 && !state.settings.setupCompleted ? recommended() : state.settings.ramGb;
    show(1);
  }

  next.onclick = () => run(async alive => {
    if (step === 0) return signIn(alive);
    if (step === 1) {
      const chosen = name.trim();
      const input = $("#setup-name");
      input.disabled = true;
      try {
        if (!/^[A-Za-z0-9_]{1,16}$/.test(chosen)) throw t("Usa de 1 a 16 letras, números o guiones bajos.", "Use 1 to 16 letters, numbers or underscores.");
        if (chosen !== state.settings.username || state.social?.friends?.name !== chosen) await window.skins.saveName(chosen);
        name = chosen;
        show(2);
      } finally {
        input.disabled = false;
      }
      return;
    }
    if (step === 2) return show(3);
    if (step === 3) {
      await saveAccounts({ ramGb: memory });
      return show(4);
    }
    if (!state.settings.setupCompleted) await saveAccounts({ setupCompleted: true });
    const chosen = selected;
    hide();
    if (chosen) navigate("communities", chosen.id);
  });

  back.onclick = () => show(Math.max(0, step - 1));
  close.onclick = dismiss;
  later.onclick = () => {
    if (busy || state.social?.restoring || state.social?.signingIn || state.social?.connecting) return dismiss();
    run(async () => {
      if (!state.settings.setupCompleted) await saveAccounts({ setupCompleted: true });
      hide();
    });
  };
  content.addEventListener("click", event => {
    const button = event.target.closest("[data-setup-community]");
    if (!button || busy) return;
    const fromInvite = !!button.closest("#setup-invite");
    const id = button.dataset.setupCommunity;
    const card = fromInvite ? invite : members.find(item => item.id === id) ?? directory.find(item => item.id === id);
    if (!card) return;
    if (button.dataset.action === "code") {
      codeFor = card.name;
      updateCodeLabel();
      $("#setup-code").focus();
      return;
    }
    run(async () => {
      if (button.dataset.action === "join") {
        button.disabled = true;
        try {
          const joined = fromInvite
            ? await invoke("unirse_comunidad", { codigo: card.code })
            : await invoke("unirse_comunidad_publica", { id });
          if (joined !== id) throw t("La comunidad cambió. Busca otra vez la invitación.", "The community changed. Look up the invite again.");
          if (!members.some(item => item.id === id)) members.push({ ...card, member: true });
          if (fromInvite) {
            invite = null;
            lookup = "";
            codeFor = "";
            $("#setup-code").value = "";
            updateCodeLabel();
          }
        } finally {
          button.disabled = false;
        }
      }
      selected = card;
      renderCommunities();
      invitePreview();
      next.focus();
    });
  });
  window.skins.editor.addEventListener("skin-busy", () => { if (visible() && step === 2) navigation(); });
  window.skins.editor.addEventListener("skin-changed", () => { if (visible() && step === 2) navigation(); });
  window.skins.editor.addEventListener("click", () => {
    if (visible() && step === 2) queueMicrotask(navigation);
  });
  listen("social", () => { if (visible() && step === 0) google(); });

  return { open, dismiss, visible };
})();
