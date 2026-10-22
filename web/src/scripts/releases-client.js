// En el navegador: vuelve a pedir las versiones a GitHub (con una hora de caché) y pone al día los
// botones de descarga. Así una versión nueva aparece sin volver a publicar la web.
import { formatDate, formatSize, pickReleases, releasesApi } from '../lib/releases-core.js';

const CACHE_KEY = 'parabellum-releases';
const MAX_AGE = 60 * 60 * 1000;

function readCache() {
  try {
    const cached = JSON.parse(localStorage.getItem(CACHE_KEY) || 'null');
    if (cached && Date.now() - cached.at < MAX_AGE) return cached.releases;
  } catch {
    // Sin almacenamiento (modo privado): se pide a GitHub.
  }
  return null;
}

function writeCache(releases) {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), releases }));
  } catch {
    // No pasa nada si no se puede guardar.
  }
}

function apply(releases) {
  const lang = document.documentElement.dataset.lang || 'es';
  for (const [system, release] of Object.entries(releases)) {
    if (!release) continue;
    document.querySelectorAll(`[data-release="${system}"]`).forEach(link => { link.href = release.url; });
    document.querySelectorAll(`[data-release-page="${system}"]`).forEach(link => { link.href = release.page; });
    document.querySelectorAll(`[data-release-version="${system}"]`).forEach(el => { el.textContent = release.version; });
    document.querySelectorAll(`[data-release-size="${system}"]`).forEach(el => { el.textContent = formatSize(release.size); });
    document.querySelectorAll(`[data-release-date="${system}"]`).forEach(el => { el.textContent = formatDate(release.date, lang); });
  }
}

export async function refreshReleases(repo) {
  const cached = readCache();
  if (cached) {
    apply(cached);
    return;
  }
  try {
    const response = await fetch(releasesApi(repo), { headers: { Accept: 'application/vnd.github+json' } });
    if (!response.ok) return;
    const releases = pickReleases(await response.json());
    if (!releases.windows && !releases.mac) return;
    writeCache(releases);
    apply(releases);
  } catch {
    // Sin internet: se quedan las versiones de la compilación.
  }
}
