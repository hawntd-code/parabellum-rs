"use strict";
// La web de Parabellum: descargas al día con GitHub, entradas al hacer scroll, la galería, la demostración
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

function gallery() {
  for (const root of document.querySelectorAll("[data-gallery]")) {
    const tabs = [...root.querySelectorAll("[data-shot]")];
    const images = [...root.querySelectorAll("[data-shot-img]")];
    const show = (index) => {
      tabs.forEach((tab, i) => tab.setAttribute("aria-selected", String(i === index)));
      images.forEach((img, i) => (img.hidden = i !== index));
    };
    tabs.forEach((tab, i) => {
      tab.addEventListener("click", () => show(i));
      tab.addEventListener("keydown", (event) => {
        const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
        if (!step) return;
        const next = (i + step + tabs.length) % tabs.length;
        tabs[next].focus();
        show(next);
      });
    });
  }
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
gallery();
demo();
footerReveal();
