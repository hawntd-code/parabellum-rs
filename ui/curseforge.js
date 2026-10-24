/* Asistente de recepción desde el portal oficial: usa los componentes y colores de Parabellum. */
const curseforgeAssistant = {
  queues: [],
  instance: null,
  refreshing: null,
  notices: new Map(),
  async refresh() {
    if (this.refreshing) return this.refreshing;
    this.refreshing = invoke("curseforge_pendientes").then(async queues => {
      const previous = this.queues;
      this.queues = queues ?? [];
      this.render();
      const completed = previous.filter(q => q.files.some(f => !this.queues.some(next => next.id === q.id && next.files.some(n => n.fileId === f.fileId))));
      if (!completed.length) return;
      state.content = null;
      if (state.page === "instance" && completed.some(q => q.id === state.current?.id)) loadContent();
      if (["explore", "project"].includes(state.page) && completed.some(q => q.id === state.explore.target)) {
        const installed = await invoke("proyectos_instalados", { id: state.explore.target });
        state.explore.installed = new Set(installed.projects);
        state.explore.installedVersions = new Set(installed.versions);
        refreshInstallButtons();
      }
    }).finally(() => { this.refreshing = null; });
    return this.refreshing;
  },
  render() {
    const queue = this.queues.find(q => q.id === this.instance);
    const files = queue?.files ?? [];
    const banner = $("#curseforge-pending");
    const total = this.queues.reduce((n, q) => n + q.files.length, 0);
    banner.classList.toggle("hidden", !total);
    if (banner.dataset.total !== String(total)) {
      banner.dataset.total = String(total);
      banner.innerHTML = total ? `<span class="cf-pending-icon">${icon("download")}</span><span class="grow"><b>${t("Completa tu instalación de CurseForge", "Complete your CurseForge installation")}</b><span>${t(`${plural(total, "archivo pendiente", "archivos pendientes")} · puedes continuar cuando quieras.`, `${total} pending ${total === 1 ? "file" : "files"} · continue whenever you like.`)}</span></span><button class="btn small" id="cf-resume">${t("Continuar instalación", "Continue installation")}</button>` : "";
    }
    if (total) $("#cf-resume").onclick = () => this.open(this.queues[0].id);
    const detail = $("#cf-detail-pending");
    const current = this.queues.find(q => q.id === state.current?.id);
    detail.classList.toggle("hidden", !current);
    const detailKey = `${current?.id ?? ""}:${current?.files.length ?? 0}`;
    if (detail.dataset.queue !== detailKey) {
      detail.dataset.queue = detailKey;
      detail.innerHTML = current ? `<span class="grow">${icon("download")} ${t(`${plural(current.files.length, "archivo de CurseForge pendiente", "archivos de CurseForge pendientes")}.`, `${current.files.length} CurseForge ${current.files.length === 1 ? "file is" : "files are"} pending.`)}</span><button class="btn small">${t("Completar instalación", "Complete installation")}</button>` : "";
    }
    if (current) $("button", detail).onclick = () => this.open(current.id);
    if (!this.instance || $("#curseforge-dialog").classList.contains("hidden")) return;
    $("#cf-assistant-title").textContent = files.length ? t("Completar instalación", "Complete installation") : t("Instalación completada", "Installation complete");
    $("#cf-assistant-instance").textContent = queue?.name ?? state.instances.find(i => i.id === this.instance)?.name ?? this.instance;
    $("#cf-assistant-text").textContent = files.length
      ? t("Abre el archivo y pulsa Descargar en CurseForge. Parabellum lo verificará y lo colocará en esta instancia.", "Open the file and click Download on CurseForge. Parabellum will verify it and place it in this instance.")
      : t("Todos los archivos pendientes se verificaron e instalaron. Ya puedes continuar.", "All pending files have been verified and installed. You can continue.");
    $("#cf-assistant-count").textContent = files.length ? t(`${plural(files.length, "archivo pendiente", "archivos pendientes")}`, `${files.length} pending ${files.length === 1 ? "file" : "files"}`) : t("Todo instalado", "Everything installed");
    // No se reemplazan los botones enfocados con cada consulta periódica.
    const signature = JSON.stringify(files.map(f => [f.fileId, this.notices.get(f.fileId)]));
    const list = $("#cf-assistant-list");
    if (list.dataset.signature === signature) return;
    list.dataset.signature = signature;
    list.innerHTML = files.map(file => {
      const notice = this.notices.get(file.fileId);
      const active = ["opening", "downloading", "verifying"].includes(notice?.stage);
      return `<div class="cf-file ${notice?.stage === "error" ? "has-error" : ""}"><span class="cf-file-icon">${icon(active ? "clock" : "puzzle")}</span><div class="grow"><b>${escapeHtml(file.name)}</b><span class="cf-file-name">${escapeHtml(file.fileName)}</span><span class="cf-file-status">${escapeHtml(notice?.message ?? t("Pendiente de descargar", "Waiting for download"))}</span><div class="cf-file-actions"><button class="btn primary small" data-cf-open="${file.fileId}" ${active ? "disabled" : ""}>${icon("external")}${t("Obtener en CurseForge", "Get on CurseForge")}</button><button class="btn small" data-cf-pick="${file.fileId}" ${active ? "disabled" : ""}>${t("Ya lo descargué", "Already downloaded")}</button></div></div></div>`;
    }).join("") || `<div class="cf-complete">${icon("check")}<b>${t("Archivos verificados e instalados", "Files verified and installed")}</b></div>`;
  },
  async open(id) {
    this.instance = id;
    $("#cf-assistant-list").dataset.signature = "";
    await this.refresh();
    openModal("#curseforge-dialog");
    this.render();
  },
  async action(command, fileId) {
    const key = Number(fileId);
    if (command === "curseforge_abrir_portal") {
      this.notices.set(key, { stage: "opening", message: t("Abriendo CurseForge…", "Opening CurseForge…") });
      this.render();
    }
    try {
      await invoke(command, { id: this.instance, archivo: key });
      if (this.notices.get(key)?.stage === "opening") this.notices.delete(key);
      await this.refresh();
    } catch (error) {
      this.notices.set(Number(fileId), { stage: "error", message: friendlyError(String(error)) });
      this.render();
    }
  },
  init() {
    $("#cf-assistant-list").onclick = event => {
      const open = event.target.closest("[data-cf-open]");
      const pick = event.target.closest("[data-cf-pick]");
      if (open) this.action("curseforge_abrir_portal", open.dataset.cfOpen);
      else if (pick) this.action("curseforge_elegir_archivo", pick.dataset.cfPick);
    };
    $("#cf-assistant-close").onclick = $("#cf-assistant-later").onclick = () => closeModal("#curseforge-dialog");
    listen("curseforge-manual", async ({payload}) => {
      this.notices.set(payload.fileId, payload);
      if (payload.stage === "installed") {
        await this.refresh();
      } else this.render();
    });
    window.addEventListener("focus", () => this.refresh().catch(() => {}));
    setInterval(() => {
      if (!$("#curseforge-dialog").classList.contains("hidden")) this.refresh().catch(() => {});
    }, 2500);
    this.refresh().catch(error => toast(String(error), "error"));
  },
};
