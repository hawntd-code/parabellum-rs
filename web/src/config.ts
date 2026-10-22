// Lo que Juan puede querer cambiar sin tocar los componentes.

export const SITE = {
  // Dirección pública de la web (canonical, Open Graph, sitemap). También está en astro.config.mjs.
  url: 'https://parabellum-roan.vercel.app',
  // Repositorio de donde salen las versiones para descargar.
  repo: 'juanrojasgarcialin/parabellum-rs',
  // Si se llena (por ejemplo '/demo/' de la web actual), la capa de Funciones muestra la demo real
  // del launcher en un iframe en vez de la maqueta con datos de ejemplo.
  launcherDemoUrl: '',
};

// Respaldo por si la API de GitHub no responde al compilar. Son las versiones publicadas el 2026-10-22;
// el navegador las vuelve a pedir al abrir la página, así que una versión nueva aparece sola.
export const FALLBACK_RELEASES = {
  windows: {
    tag: 'v0.35.0',
    version: '0.35.0',
    url: 'https://github.com/juanrojasgarcialin/parabellum-rs/releases/download/v0.35.0/Parabellum-0.35.0.zip',
    page: 'https://github.com/juanrojasgarcialin/parabellum-rs/releases/tag/v0.35.0',
    size: 10042136,
    date: '2026-10-22T05:17:25Z',
  },
  mac: {
    tag: 'mac-v0.33.0',
    version: '0.33.0',
    url: 'https://github.com/juanrojasgarcialin/parabellum-rs/releases/download/mac-v0.33.0/Parabellum-Mac-0.33.0.zip',
    page: 'https://github.com/juanrojasgarcialin/parabellum-rs/releases/tag/mac-v0.33.0',
    size: 9641615,
    date: '2026-10-21T05:30:23Z',
  },
};
