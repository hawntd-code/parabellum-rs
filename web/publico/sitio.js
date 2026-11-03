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

/** La demostración pesa: se carga al pulsar «Abrir la demostración». */
function demo() {
  const frame = document.querySelector("[data-demo]");
  frame?.querySelector("[data-demo-start]")?.addEventListener("click", () => {
    const iframe = document.createElement("iframe");
    iframe.src = "/demo/";
    iframe.title = english ? "Parabellum demo" : "Demostración de Parabellum";
    frame.replaceChildren(iframe);
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
tour();
demo();
footerReveal();
