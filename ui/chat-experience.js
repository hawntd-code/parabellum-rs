/* Continuidad del chat: perfiles, escritura y galería. Conserva los componentes de Parabellum. */
const chatExperience = {
  profiles: new Map(),
  messageMarkup: new Map(),
  profileRequest: 0,
  syncClock() {
    if (this.clockSource !== state.social && Number.isFinite(state.social?.serverNow)) {
      this.clockSource = state.social; state.socialClockOffset = state.social.serverNow - Date.now();
    }
  },
  newMessageId() {
    this.syncClock();
    const bytes = crypto.getRandomValues(new Uint8Array(7));
    const suffix = Array.from(bytes, n => "abcdefghijklmnopqrstuvwxyz0123456789"[n % 36]).join("");
    return `${Math.max(0, Math.round(Date.now() + (state.socialClockOffset ?? 0))).toString().padStart(13, "0")}${suffix}`;
  },
  /* Reutiliza las filas que no cambiaron: conserva selección, foco y las imágenes ya cargadas. */
  updateMessages(box, html, {same, stick, known}) {
    const anchor = same && !stick ? Array.from(box.children).find(n => n.getBoundingClientRect().bottom > box.getBoundingClientRect().top) : null;
    const anchorKey = anchor?.dataset.key;
    const anchorY = anchor?.getBoundingClientRect().top;
    const template = document.createElement("template");
    template.innerHTML = html;
    const existing = same ? new Map(Array.from(box.children, n => [n.dataset.key, n])) : new Map();
    if (!same) { box.replaceChildren(); this.messageMarkup.clear(); }
    const wanted = new Set();
    const selected = new Set();
    let cursor = box.firstElementChild;
    for (const fresh of Array.from(template.content.children)) {
      const key = fresh.dataset.key;
      const markup = fresh.outerHTML;
      const old = existing.get(key);
      const node = old && this.messageMarkup.get(key) === markup ? old : fresh;
      wanted.add(key);
      selected.add(node);
      this.messageMarkup.set(key, markup);
      if (node !== cursor) box.insertBefore(node, cursor);
      cursor = node.nextElementSibling;
      if (node === fresh && !old && same && known && node.classList.contains("bubble-row") && motionOk()) {
        node.animate([{opacity:0, transform:`translate(${node.classList.contains("mine") ? 6 : -6}px, 6px)`}, {opacity:1, transform:"none"}], {duration:200,easing:EASE_OUT});
      }
    }
    for (const node of Array.from(box.children)) if (!selected.has(node)) node.remove();
    for (const key of this.messageMarkup.keys()) if (!wanted.has(key)) this.messageMarkup.delete(key);
    if (stick) box.scrollTop = box.scrollHeight;
    else if (anchorKey) {
      const next = Array.from(box.children).find(n => n.dataset.key === anchorKey);
      if (next) box.scrollTop += next.getBoundingClientRect().top - anchorY;
    }
    box.dataset.scope = state.chatOpen ?? "";
    return true;
  },
  async profile(uid) {
    if (!uid) return;
    const request = ++this.profileRequest;
    const friend = state.social?.friends?.friends?.find(f => f.uid === uid);
    const member = allChats().flatMap(c => c.members).find(m => m.uid === uid);
    const name = friend?.name ?? member?.name ?? t("Jugador", "Player");
    $("#player-name").textContent = name;
    $("#player-status").textContent = friend ? friendStatus(friend) : t("Miembro de la conversación", "Conversation member");
    $("#player-status").classList.toggle("is-online", !!friend?.online);
    $("#player-avatar").innerHTML = presenceHead(name, friend);
    for (const canvas of $$("#player-avatar canvas")) drawHead(canvas, canvas.dataset.head);
    $("#player-details").innerHTML = `<p class="muted">${t("Cargando perfil…", "Loading profile…")}</p>`;
    const groups = allChats().filter(c => c.kind === "grupo" && c.members.some(m => m.uid === uid));
    $("#player-shared").innerHTML = `<h3>${t("Grupos en común", "Groups in common")}</h3>${groups.length ? groups.map(c => `<button class="profile-group" data-profile-chat="${escapeHtml(c.id)}">${chatAvatar(c)}<span>${escapeHtml(c.name)}</span>${icon("right")}</button>`).join("") : `<p class="muted">${t("Todavía no comparten un grupo.", "No shared groups yet.")}</p>`}`;
    const direct = allChats().find(c => c.kind === "directo" && c.members.some(m => m.uid === uid));
    const message = $("#player-message");
    message.classList.toggle("hidden", !direct && !friend);
    message.onclick = () => { closeModal("#player-dialog"); direct ? openChat(direct.id) : messageFriend(uid); };
    openModal("#player-dialog");
    try {
      const cached = this.profiles.get(uid);
      const profile = cached && Date.now() - cached.at < 60000 ? cached.value : await invoke("perfil_jugador", {uid});
      this.profiles.set(uid, {at:Date.now(),value:profile});
      if (request !== this.profileRequest) return;
      $("#player-name").textContent = profile.name || name;
      const rows = [[t("Nombre en Minecraft", "Minecraft name"),profile.name || name]];
      if (friend?.presence?.pack) rows.push([t("Jugando", "Playing"),friend.presence.pack]);
      if (friend?.presence?.minecraft) rows.push([t("Versión de Minecraft", "Minecraft version"),friend.presence.minecraft]);
      if (friend && !friend.online && friend.lastSeen) rows.push([t("Última actividad", "Last active"),new Date(friend.lastSeen).toLocaleString(locale(),{dateStyle:"medium",timeStyle:"short"})]);
      $("#player-details").innerHTML = `<dl class="profile-facts">${rows.map(([label,value]) => `<div><dt>${escapeHtml(label)}</dt><dd>${escapeHtml(value)}</dd></div>`).join("")}</dl>`;
    } catch (error) {
      if (request === this.profileRequest) $("#player-details").innerHTML = `<p class="inline-error">${escapeHtml(friendlyError(String(error)))}</p><button class="btn small" id="player-retry">${t("Reintentar", "Retry")}</button>`;
      $("#player-retry")?.addEventListener("click", () => this.profile(uid));
    }
  },
  openHeader() {
    const chat = currentChat();
    if (!chat) return;
    if (chat.kind === "grupo") openGroupDialog("members");
    else this.profile(chat.members.find(m => !m.you)?.uid);
  },
};

const mediaGallery = {
  items: [], index:0, chat:null, more:false, cursor:null, session:0, request:0, previousFocus:null, zoom:false,
  visible() { return !$("#lightbox").classList.contains("hidden"); },
  item(message, chat) {
    return {key:message.id, name:message.name, at:message.at, caption:t("Imagen compartida", "Shared image"),
      src:state.chatImages?.[message.id], load:async () => {
        const data = await invoke("imagen_chat", {chat,mensaje:message.id});
        (state.chatImages ??= {})[message.id] = data;
        return data;
      }};
  },
  async openChat(messageId = null) {
    const chat = state.chatOpen;
    if (!chat) return;
    const messages = state.chatMessages ?? [];
    const items = messages.filter(m => m.image).map(m => this.item(m,chat));
    this.open(items, Math.max(0,items.findIndex(m => m.key === messageId)), {
      chat, cursor:messages.map(m => m.id).sort()[0] ?? null, more:messages.length >= 150 || !messages.length,
    });
    const session = this.session;
    if (!items.length) {
      try {
        await this.older();
        if (session !== this.session) return;
        if (this.items.length) this.show(this.items.length-1);
        else this.status(t("Todavía no hay imágenes compartidas.", "No shared images yet."));
      } catch (error) { if (session === this.session) this.status(friendlyError(String(error)),true); }
    }
  },
  open(items, index=0, options={}) {
    this.session++; this.request++;
    this.items = items; this.index = index; this.chat = options.chat ?? null;
    this.cursor = options.cursor ?? null; this.more = options.more ?? false; this.olderPromise = null;
    this.previousFocus = document.activeElement;
    this.zoom = false; $("#media-canvas").classList.remove("zoomed");
    $("#lightbox").classList.remove("hidden");
    $("#media-close").focus({preventScroll:true});
    if (items.length) this.show(index);
    else { $("#lightbox-image").hidden = true; this.metadata(); this.status(t("Buscando imágenes…", "Finding images…")); }
  },
  close() {
    this.session++; this.request++;
    $("#lightbox").classList.add("hidden");
    if (this.previousFocus?.isConnected) this.previousFocus.focus({preventScroll:true});
  },
  status(text, retry=false) {
    const box = $("#media-status");
    box.classList.toggle("hidden", !text);
    box.innerHTML = text ? `<span>${escapeHtml(text)}</span>${retry ? `<button class="btn small" id="media-retry">${t("Reintentar", "Retry")}</button>` : ""}` : "";
    $("#media-retry")?.addEventListener("click", () => this.items.length ? this.show(this.index) : this.openChat());
  },
  metadata() {
    const focusedThumbnail = $("#media-thumbnails").contains(document.activeElement) ? document.activeElement.dataset.mediaIndex : null;
    const item = this.items[this.index];
    $("#media-author").textContent = item?.name ?? t("Imágenes compartidas", "Shared images");
    $("#media-date").textContent = item?.at ? new Date(item.at).toLocaleString(locale(),{dateStyle:"long",timeStyle:"short"}) : "";
    $("#media-count").textContent = this.items.length ? t(`${this.index+1} de ${this.items.length}`,`${this.index+1} of ${this.items.length}`) : "";
    $("#lightbox-caption").textContent = item?.caption ?? "";
    $("#media-prev").disabled = this.index === 0 && !this.more;
    $("#media-next").disabled = this.index >= this.items.length-1;
    $("#media-prev").title = this.index === 0 && this.more ? t("Buscar imágenes anteriores (←)","Find earlier images (←)") : t("Imagen anterior (←)","Previous image (←)");
    $("#media-zoom").disabled = !item;
    const start = Math.max(0, Math.min(this.index-4,this.items.length-9));
    const thumbs = this.items.slice(start,start+9);
    $("#media-thumbnails").innerHTML = thumbs.map((m,i) => `<button class="media-thumbnail ${start+i === this.index ? "active" : ""}" data-media-index="${start+i}" aria-label="${escapeHtml(t(`Ver imagen ${start+i+1}`,`View image ${start+i+1}`))}" ${start+i === this.index ? 'aria-current="true"' : ""}>${m.src ? `<img src="${escapeHtml(m.src)}" alt="">` : icon("image")}</button>`).join("");
    if (focusedThumbnail !== null) $(`#media-thumbnails [data-media-index="${focusedThumbnail}"]`)?.focus({preventScroll:true});
  },
  async show(index, direction=0) {
    const item = this.items[index];
    if (!item) return;
    const request = ++this.request;
    this.index = index;
    this.zoom = false; $("#media-canvas").classList.remove("zoomed");
    this.metadata();
    const image = $("#lightbox-image");
    image.hidden = true;
    this.status(t("Cargando imagen…", "Loading image…"));
    try {
      const src = item.src || await item.load();
      const probe = new Image(); probe.src = src;
      await probe.decode();
      if (request !== this.request || !this.visible()) return;
      item.src = src;
      image.src = src; image.alt = item.name ? t(`Imagen enviada por ${item.name}`,`Image sent by ${item.name}`) : item.caption;
      image.hidden = false; this.status(""); this.metadata();
      if (motionOk()) image.animate([{opacity:0,transform:`translateX(${direction*22}px) scale(.985)`},{opacity:1,transform:"none"}],{duration:190,easing:EASE_OUT});
    } catch (error) {
      if (request === this.request && this.visible()) this.status(friendlyError(String(error)),true);
    }
  },
  async older() {
    if (this.olderPromise) return this.olderPromise;
    if (!this.chat || !this.more) return 0;
    const session = this.session;
    this.olderPromise = (async () => {
      let added = 0;
      for (let page=0; page<6 && this.more && !added; page++) {
        const data = await invoke("imagenes_chat",{chat:this.chat,antes:this.cursor});
        if (session !== this.session) return 0;
        const oldCursor = this.cursor;
        this.cursor = data.cursor;
        this.more = !!data.more && !!this.cursor && this.cursor !== oldCursor;
        const known = new Set(this.items.map(m => m.key));
        const items = data.images.filter(m => !known.has(m.id)).map(m => this.item(m,this.chat));
        this.items.unshift(...items); added += items.length; this.index += items.length;
      }
      return added;
    })().finally(() => { if (session === this.session) this.olderPromise = null; });
    return this.olderPromise;
  },
  async move(direction) {
    const session = this.session;
    if (direction < 0 && this.index === 0 && this.more) {
      this.status(t("Buscando imágenes anteriores…", "Finding earlier images…"));
      try {
        const added = await this.older();
        if (session !== this.session) return;
        if (!added) { this.status(this.more ? t("Sigue hacia atrás para buscar más imágenes.","Continue back to find more images.") : ""); this.metadata(); return; }
      } catch (error) { if (session === this.session) this.status(friendlyError(String(error))); return; }
    }
    const next = this.index + direction;
    if (next >= 0 && next < this.items.length) this.show(next,direction);
  },
};

function initChatExperience() {
  $("#player-close").onclick = () => closeModal("#player-dialog");
  $("#player-dialog").onmousedown = event => { if (event.target.id === "player-dialog") closeModal("#player-dialog"); };
  $("#player-shared").onclick = event => { const button = event.target.closest("[data-profile-chat]"); if (button) { closeModal("#player-dialog"); openChat(button.dataset.profileChat); } };
  $("#chat-title").onclick = () => chatExperience.openHeader();
  $("#chat-media").onclick = () => mediaGallery.openChat();
  $("#media-close").onclick = () => mediaGallery.close();
  $("#media-prev").onclick = () => mediaGallery.move(-1);
  $("#media-next").onclick = () => mediaGallery.move(1);
  $("#media-zoom").onclick = () => { mediaGallery.zoom = !mediaGallery.zoom; $("#media-canvas").classList.toggle("zoomed",mediaGallery.zoom); };
  $("#lightbox-image").onclick = () => { if (mediaGallery.zoom) $("#media-zoom").click(); };
  $("#media-thumbnails").onclick = event => { const button=event.target.closest("[data-media-index]"); if (button) mediaGallery.show(Number(button.dataset.mediaIndex),Math.sign(Number(button.dataset.mediaIndex)-mediaGallery.index)); };
  window.addEventListener("keydown", event => {
    if (!mediaGallery.visible()) return;
    if (!["Escape","ArrowLeft","ArrowRight","Tab","Home","End"].includes(event.key)) return;
    event.preventDefault(); event.stopImmediatePropagation();
    if (event.key === "Escape") mediaGallery.close();
    else if (event.key === "ArrowLeft") mediaGallery.move(-1);
    else if (event.key === "ArrowRight") mediaGallery.move(1);
    else if (event.key === "Home") mediaGallery.show(0,-1);
    else if (event.key === "End") mediaGallery.show(mediaGallery.items.length-1,1);
    else {
      const controls = focusables($("#lightbox"));
      const index = controls.indexOf(document.activeElement);
      controls[(index+(event.shiftKey ? -1 : 1)+controls.length)%controls.length]?.focus();
    }
  },true);
  listen("chat-escribiendo",({payload}) => { if (payload.chat === state.chatOpen) { state.chatTyping = payload.users ?? {}; renderTyping(); } });
  if (!tauri) window.addEventListener("demo-escribiendo",({detail}) => { if (detail.chat === state.chatOpen) { state.chatTyping=detail.users; renderTyping(); } });
  setInterval(() => { if (state.chatOpen && state.page === "chat") renderTyping(); },1000);
}
