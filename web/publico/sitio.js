"use strict";
// La web de Parabellum: descargas al día con GitHub, entradas al hacer scroll, el recorrido, la demostración
// (se carga solo al pedirla) y el pie que se descubre al llegar al final.

const REPO = "hawntd-code/parabellum-rs";
const english = document.documentElement.lang === "en";

/** Las descargas de la release más nueva: el instalador y el zip de Windows (v…) y el zip de Mac (mac-v…). */
async function updateDownloads() {
  try {
    const response = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=40`);
    if (!response.ok) return;
    const stable = (await response.json()).filter((r) => !r.draft && !r.prerelease);
    const asset = (release, end) => release?.assets.find((a) => a.name.toLowerCase().endsWith(end));
    const windows = stable.find((r) => /^v\d/.test(r.tag_name) && asset(r, "-setup.exe"));
    const mac = stable.find((r) => /^mac-v\d/.test(r.tag_name) && asset(r, ".zip"));
    const mb = (a) => (a.size / 1e6).toFixed(1).replace(/\.0$/, "");
    if (windows) {
      const setup = asset(windows, "-setup.exe");
      const zip = asset(windows, ".zip");
      const version = windows.tag_name.slice(1);
      document.querySelectorAll('[data-dl="windows"]').forEach((a) => (a.href = setup.browser_download_url));
      if (zip) document.querySelectorAll('[data-dl="portable"]').forEach((a) => (a.href = zip.browser_download_url));
      document.querySelectorAll('[data-dl-info="windows"]').forEach((s) => (s.textContent = `${english ? "Installer" : "Instalador"} v${version} · ${mb(setup)} MB`));
    }
    if (mac) {
      const zip = asset(mac, ".zip");
      document.querySelectorAll('[data-dl="mac"]').forEach((a) => (a.href = zip.browser_download_url));
      document.querySelectorAll('[data-dl-info="mac"]').forEach((s) => (s.textContent = `${english ? "Version" : "Versión"} ${mac.tag_name.slice(5)} · ${mb(zip)} MB`));
    }
  } catch {
    // Sin GitHub se quedan las de cuando se armó la página.
  }
}

/** Lo que entra al hacer scroll. */
function revealOnScroll() {
  const items = document.querySelectorAll(".reveal");
  if (!("IntersectionObserver" in window)) return items.forEach((el) => el.classList.add("in"));
  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      entry.target.classList.add("in");
      observer.unobserve(entry.target);
    }
  }, { rootMargin: "0px 0px -8% 0px", threshold: 0.12 });
  items.forEach((el) => observer.observe(el));
}

/** El recorrido: la sección se queda fija y el scroll la va pasando paso por paso. `s` va de -HOLD a
 * (pasos - 1 + HOLD): en cada entero se ve un paso entero, y entre uno y otro la pantalla siguiente sube
 * sobre la anterior. Lo demás (la inclinación de llegada, el índice) lo dibuja estilos.css con las
 * variables que se ponen aquí. */
function tour() {
  const root = document.querySelector("[data-tour]");
  if (!root) return;
  const track = root.querySelector(".tour-track");
  const steps = [...root.querySelectorAll(".tour-step")];
  const shots = [...root.querySelectorAll(".tour-shot")];
  const bars = [...root.querySelectorAll(".tour-bars i")];
  const count = shots.length;
  const HOLD = 0.35;
  const still = matchMedia("(prefers-reduced-motion: reduce)");
  const clamp = (x) => Math.min(1, Math.max(0, x));
  const ease = (x) => (x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2);
  // La entrada de la pantalla i ocurre entre s = i - 0.8 y s = i - 0.2: así el paso se queda quieto un rato.
  const entry = (s, i) => (i <= 0 ? 1 : i >= count ? 0 : ease(clamp((s - i + 0.8) / 0.6)));
  let active = -1;
  let pending = false;

  const geometry = () => {
    const rect = track.getBoundingClientRect();
    return { rect, travel: Math.max(1, rect.height - innerHeight) };
  };

  function update() {
    pending = false;
    const { rect, travel } = geometry();
    const rise = still.matches ? 1 : clamp(1 - rect.top / innerHeight);
    const s = clamp(-rect.top / travel) * (count - 1 + 2 * HOLD) - HOLD;
    root.style.setProperty("--rise", rise.toFixed(4));
    shots.forEach((shot, i) => {
      let t = entry(s, i);
      let over = entry(s, i + 1);
      if (still.matches) [t, over] = [Math.round(t), 0];
      // En el celular la captura va recortada y se recorre de izquierda a derecha mientras está en pantalla.
      const from = i === 0 ? -HOLD : i - 0.8;
      const to = i === count - 1 ? count - 1 + HOLD : i + 0.8;
      shot.style.setProperty("--t", t.toFixed(4));
      shot.style.setProperty("--dim", over.toFixed(4));
      shot.style.setProperty("--pan", clamp((s - from) / (to - from)).toFixed(4));
    });
    // Cada línea del índice se llena mientras su paso está en pantalla; la última, hasta el final.
    steps.forEach((step, i) => {
      const span = i === count - 1 ? 0.5 + HOLD : 1;
      const fill = clamp((s - i + 0.5) / span).toFixed(4);
      step.style.setProperty("--fill", fill);
      bars[i]?.style.setProperty("--fill", fill);
    });
    const current = Math.min(count - 1, Math.max(0, Math.round(s)));
    if (current !== active) {
      active = current;
      steps.forEach((step, i) => {
        step.classList.toggle("is-active", i === current);
        const tab = step.querySelector(".tour-tab");
        if (i === current) tab.setAttribute("aria-current", "step");
        else tab.removeAttribute("aria-current");
      });
    }
  }

  const schedule = () => {
    if (!pending) {
      pending = true;
      requestAnimationFrame(update);
    }
  };
  // Pulsar un paso lleva el scroll justo a ese paso.
  steps.forEach((step, i) => {
    step.querySelector(".tour-tab").addEventListener("click", () => {
      const { rect, travel } = geometry();
      const y = scrollY + rect.top + ((i + HOLD) / (count - 1 + 2 * HOLD)) * travel;
      scrollTo({ top: Math.round(y), behavior: still.matches ? "auto" : "smooth" });
    });
  });
  addEventListener("scroll", schedule, { passive: true });
  addEventListener("resize", schedule, { passive: true });
  still.addEventListener?.("change", schedule);
  update();
}

const still = matchMedia("(prefers-reduced-motion: reduce)");
const clamp01 = (x) => Math.min(1, Math.max(0, x));

/** Llama a `update` en el siguiente cuadro cada vez que hay scroll o cambia el tamaño. */
function onScrollFrame(update) {
  let pending = false;
  const schedule = () => {
    if (pending) return;
    pending = true;
    requestAnimationFrame(() => { pending = false; update(); });
  };
  addEventListener("scroll", schedule, { passive: true });
  addEventListener("resize", schedule, { passive: true });
  update();
}

/** La demostración: una ventana encima de la página que se abre desde la barra. El launcher (pesado) se
 * carga la primera vez que se abre y se queda cargado. */
function demo() {
  const dialog = document.querySelector("[data-demo-dialog]");
  if (!dialog) return;
  const body = dialog.querySelector("[data-demo-body]");
  const open = () => {
    if (!body.firstChild) {
      const iframe = document.createElement("iframe");
      iframe.src = english ? "/demo/?en=1" : "/demo/";
      iframe.title = english ? "Parabellum demo" : "Demostración de Parabellum";
      body.append(iframe);
    }
    dialog.showModal();
    requestAnimationFrame(() => requestAnimationFrame(() => dialog.classList.add("is-open")));
  };
  const close = () => {
    dialog.classList.remove("is-open");
    setTimeout(() => dialog.close(), still.matches ? 0 : 320);
  };
  document.querySelectorAll("[data-demo-open]").forEach((button) => button.addEventListener("click", open));
  dialog.querySelector("[data-demo-close]").addEventListener("click", close);
  dialog.addEventListener("cancel", (event) => { event.preventDefault(); close(); });
  // Un clic fuera de la ventana (en el fondo) también la cierra.
  dialog.addEventListener("click", (event) => { if (event.target === dialog) close(); });
}

/** La vitrina bajo la portada: se endereza mientras sube hasta el centro de la pantalla; el borde lila se
 * enciende en el último tramo. */
function showcase() {
  const root = document.querySelector("[data-showcase]");
  if (!root || still.matches) return;
  const frame = root.querySelector(".showcase-frame");
  onScrollFrame(() => {
    // Con transform puesto, la caja medida cambia: se mide la sección, que no se mueve.
    const rect = root.getBoundingClientRect();
    // 0 con su borde de arriba a 3/4 de la pantalla (donde queda al abrir la página); 1 cuando ese borde
    // llega cerca de la barra y la ventana ya se ve entera.
    const p = clamp01((innerHeight * 0.75 - rect.top) / (innerHeight * 0.64));
    const eased = 1 - (1 - p) ** 2;
    root.style.setProperty("--p", eased.toFixed(4));
    root.style.setProperty("--ring", clamp01((p - 0.55) / 0.35).toFixed(3));
  });
}

/** El engrane de Rust gira con el scroll mientras la sección está en pantalla; su órbita se dibuja al
 * entrar. */
function rust() {
  const root = document.querySelector("[data-rust]");
  if (!root || still.matches) return;
  onScrollFrame(() => {
    const rect = root.getBoundingClientRect();
    const p = (innerHeight - rect.top) / (innerHeight + rect.height);
    if (p < -0.1 || p > 1.1) return;
    root.style.setProperty("--spin", (p * 150).toFixed(2));
    root.style.setProperty("--orbit", clamp01(p * 1.8).toFixed(4));
  });
}

/** La memoria: al entrar, el número cuenta desde cero, la curva se dibuja y las barras crecen. Sobre la
 * curva, el cursor muestra el valor de cada medición. */
function memory() {
  const root = document.querySelector("[data-memory]");
  if (!root) return;
  const counters = [...root.querySelectorAll("[data-count]")];
  const play = () => {
    root.style.setProperty("--grow", 1);
    const start = performance.now();
    const step = (now) => {
      const t = clamp01((now - start) / 1400);
      const eased = 1 - (1 - t) ** 3;
      root.style.setProperty("--draw", eased.toFixed(4));
      counters.forEach((el) => (el.textContent = Math.round(Number(el.dataset.count) * eased)));
      if (t < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  };
  if (!still.matches && "IntersectionObserver" in window) {
    root.style.setProperty("--draw", 0);
    root.style.setProperty("--grow", 0);
    counters.forEach((el) => (el.textContent = "0"));
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) return;
      observer.disconnect();
      play();
    }, { threshold: 0.3 });
    observer.observe(root.querySelector(".mem-grid"));
  }

  const svg = root.querySelector(".mem-svg");
  if (!svg) return;
  const points = svg.dataset.curve.split(",").map((pair) => pair.split(":").map(Number));
  const [first, last] = [points[0][0], points[points.length - 1][0]];
  const x = (s) => 8 + ((s - first) / (last - first)) * 384;
  const y = (mb) => 120 - (mb / 300) * 108;
  const hover = svg.querySelector(".mem-hover");
  const [line, dot, label] = [hover.querySelector("line"), hover.querySelector("circle"), hover.querySelector("text")];
  svg.addEventListener("pointermove", (event) => {
    const box = svg.getBoundingClientRect();
    const px = ((event.clientX - box.left) / box.width) * 400;
    const [s, mb] = points.reduce((best, p) => (Math.abs(x(p[0]) - px) < Math.abs(x(best[0]) - px) ? p : best));
    const cx = x(s);
    line.setAttribute("x1", cx); line.setAttribute("x2", cx);
    dot.setAttribute("cx", cx); dot.setAttribute("cy", y(mb));
    label.setAttribute("x", Math.min(370, Math.max(30, cx)));
    label.textContent = `${mb} MB · ${s < 60 ? `${s} s` : `${s / 60} min`}`;
    svg.classList.add("is-hover");
  });
  svg.addEventListener("pointerleave", () => svg.classList.remove("is-hover"));
}

/** Las preguntas se abren y se cierran animando su altura (un <details> normal salta de golpe). */
function faq() {
  document.querySelectorAll(".qa").forEach((item) => {
    const summary = item.querySelector("summary");
    const body = item.querySelector(".qa-body");
    let running = null;
    summary.addEventListener("click", (event) => {
      if (still.matches || !body.animate) return;
      event.preventDefault();
      running?.cancel();
      const from = body.offsetHeight;
      if (item.open && !item.classList.contains("is-closing")) {
        item.classList.add("is-closing");
        running = body.animate([{ height: `${from}px`, opacity: 1 }, { height: "0px", opacity: 0 }], { duration: 380, easing: "cubic-bezier(.22, 1, .36, 1)" });
        running.onfinish = () => { item.open = false; item.classList.remove("is-closing"); running = null; };
      } else {
        // Si se estaba cerrando, se vuelve a abrir desde donde iba.
        const start = item.classList.contains("is-closing") ? from : 0;
        item.classList.remove("is-closing");
        item.open = true;
        const to = body.scrollHeight;
        running = body.animate([{ height: `${start}px`, opacity: start ? 1 : 0 }, { height: `${to}px`, opacity: 1 }], { duration: 520, easing: "cubic-bezier(.22, 1, .36, 1)" });
        running.onfinish = () => { running = null; };
      }
    });
  });
}

/** El pie va fijo detrás de la página y se descubre al final, si cabe en la pantalla. */
function footerReveal() {
  const footer = document.querySelector(".footer");
  const main = document.querySelector("main");
  if (!footer || !main) return;
  const fit = () => document.body.classList.toggle("reveal-footer", footer.offsetHeight <= innerHeight - 64);
  const update = () => {
    const height = footer.offsetHeight;
    const hidden = main.getBoundingClientRect().bottom - (innerHeight - height);
    footer.style.setProperty("--reveal", String(Math.min(1, Math.max(0, 1 - hidden / height))));
  };
  fit();
  update();
  addEventListener("resize", () => { fit(); update(); }, { passive: true });
  addEventListener("scroll", update, { passive: true });
}

updateDownloads();
revealOnScroll();
showcase();
tour();
rust();
memory();
faq();
demo();
footerReveal();
