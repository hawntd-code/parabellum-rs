// Elegir la versión más nueva de cada sistema entre las releases de GitHub. Lo usan la compilación
// (releases.ts) y el navegador (releases-client.js), así que tiene que ser JavaScript sin dependencias.
// Convención del repo: Windows se publica como «v<versión>» y Mac como «mac-v<versión>», con un zip.

const SYSTEMS = {
  windows: tag => /^v\d/.test(tag),
  mac: tag => /^mac-v\d/.test(tag),
};

/**
 * @typedef {{ tag: string, version: string, url: string, page: string, size: number, date: string }} Release
 */

/** @returns {Release | null} */
function toRelease(release) {
  const asset = (release.assets || []).find(item => item.name.toLowerCase().endsWith('.zip'));
  if (!asset) return null;
  return {
    tag: release.tag_name,
    version: release.tag_name.replace(/^(mac-)?v/, ''),
    url: asset.browser_download_url,
    page: release.html_url,
    size: asset.size,
    date: release.published_at,
  };
}

// `releases` es la lista que devuelve /repos/{repo}/releases (de la más nueva a la más vieja).
/** @returns {{ windows: Release | null, mac: Release | null }} */
export function pickReleases(releases) {
  /** @type {{ windows: Release | null, mac: Release | null }} */
  const picked = { windows: null, mac: null };
  for (const [system, matches] of Object.entries(SYSTEMS)) {
    const found = releases.find(release => !release.draft && !release.prerelease && matches(release.tag_name) && toRelease(release));
    picked[system] = found ? toRelease(found) : null;
  }
  return picked;
}

// 10042136 → «10 MB»; 9641615 → «9.6 MB» (como en el README).
export function formatSize(bytes) {
  const mb = bytes / 1e6;
  return `${mb >= 10 ? Math.round(mb) : mb.toFixed(1)} MB`;
}

export function formatDate(iso, lang) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '';
  return new Intl.DateTimeFormat(lang === 'en' ? 'en-US' : 'es-MX', { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' })
    .format(date)
    .replace('.', '');
}

export function releasesApi(repo) {
  return `https://api.github.com/repos/${repo}/releases?per_page=40`;
}
