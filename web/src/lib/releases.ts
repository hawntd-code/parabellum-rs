// Las versiones para descargar, leídas de GitHub al compilar. Si GitHub no responde (sin internet o sin
// cuota de la API), se usan las de config.ts y el navegador las pone al día al abrir la página.
import { FALLBACK_RELEASES, SITE } from '../config';
import { pickReleases, releasesApi } from './releases-core.js';

export type Release = {
  tag: string;
  version: string;
  url: string;
  page: string;
  size: number;
  date: string;
};

export type Releases = { windows: Release; mac: Release; fromGitHub: boolean };

let pending: Promise<Releases> | null = null;

async function load(): Promise<Releases> {
  try {
    const headers: Record<string, string> = { Accept: 'application/vnd.github+json', 'User-Agent': 'parabellum-web' };
    // Opcional: con GITHUB_TOKEN en Vercel la API no se queda sin cuota (60 consultas por hora sin token).
    const token = process.env.GITHUB_TOKEN;
    if (token) headers.Authorization = `Bearer ${token}`;
    const response = await fetch(releasesApi(SITE.repo), { headers, signal: AbortSignal.timeout(8000) });
    if (!response.ok) throw new Error(`GitHub respondió ${response.status}`);
    const picked = pickReleases(await response.json());
    return {
      windows: picked.windows ?? FALLBACK_RELEASES.windows,
      mac: picked.mac ?? FALLBACK_RELEASES.mac,
      fromGitHub: Boolean(picked.windows && picked.mac),
    };
  } catch (error) {
    console.warn(`[releases] Se usan las versiones de respaldo: ${(error as Error).message}`);
    return { ...FALLBACK_RELEASES, fromGitHub: false };
  }
}

// Una sola consulta por compilación, aunque la pidan las dos páginas.
export function getReleases(): Promise<Releases> {
  pending ??= load();
  return pending;
}
