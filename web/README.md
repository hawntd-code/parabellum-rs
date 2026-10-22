# Web de Parabellum · «Excavación»

La landing de Parabellum en Astro, **lista casi al 100 %** para que Juan la termine a su gusto y decida
dónde publicarla. La hizo Diego (con su agente) a partir del prototipo «Excavación», el que eligió Juan.

La idea: la página es un corte del mundo y **bajar es excavar**. Arriba está la superficie de noche; cada
sección es una capa más profunda (tierra, cueva, grava, redstone, geoda, portal, pizarra, diamantes y
roca madre), dibujada con arte pixel original hecho con código. Un minero baja por una escalera al ritmo del
scroll y cada sección se descubre haciendo algo: minar, pulsar O, mover una palanca, unirse a una comunidad.

- Español en `/` y en inglés en `/en/`.
- Las descargas salen solas de las releases de GitHub (`v*` para Windows, `mac-v*` para Mac).
- No necesita servidor: es un sitio estático.

> **Ojo con dónde vive.** Según `CLAUDE.md`, la web oficial está fuera del repo, en `../landing-servidor`
> (Astro, publicada en Vercel). Esta carpeta `web/` es para que Juan la tome: puede publicarla desde aquí o
> copiarla a `landing-servidor`. Ver «Decisiones para Juan».

## Correrla

Hace falta **Node 22.23.4 o más nuevo** (lo pide Astro 7).

```bash
cd web
npm install
npm run dev       # http://localhost:4321, se recarga al guardar
npm run build     # deja el sitio en web/dist
npm run preview   # sirve web/dist
npm run check     # revisa tipos y plantillas (debe dar 0 errores)
```

Al compilar se piden las versiones a la API de GitHub. Sin internet usa las de respaldo de
`src/config.ts` y avisa en la consola; no falla.

## Publicarla en Vercel

**Opción A, desde este repo:** en Vercel, «Add New Project» con este repositorio y:

| Ajuste | Valor |
|---|---|
| Root Directory | `web` |
| Framework Preset | Astro |
| Build Command | `npm run build` |
| Output Directory | `dist` |
| Node.js Version | 22 o más nuevo |

**Opción B, en `landing-servidor`:** copiar el contenido de `web/` (sin `node_modules` ni `dist`). No depende
de nada fuera de su carpeta.

En los dos casos:
- Si el dominio no es `parabellum-roan.vercel.app`, cámbialo en **dos lugares**: `site` de
  `astro.config.mjs` y `SITE.url` de `src/config.ts` (los usan el canonical, Open Graph y el sitemap).
- Opcional: una variable `GITHUB_TOKEN` (un token sin permisos basta) para que la API de GitHub no se quede
  sin cuota al compilar. Sin token son 60 consultas por hora; cada compilación usa una.

## Stack

| Parte | Qué se usa | Por qué |
|---|---|---|
| Sitio | Astro 7, salida estática | El mismo stack que la web actual; sin servidor |
| Componentes | `.astro` (HTML en el servidor) | Sin React ni otro framework: lo interactivo es JavaScript plano |
| Interacción | Un módulo JS (`src/scripts/excavacion.js`) | Lo empaqueta Astro; sin bibliotecas |
| Arte | `src/scripts/pixel.js`: canvas con semilla | Original, sin imágenes que bajar; se ve igual en cada visita |
| Fuentes | `@fontsource`: Bungee, Inter y VT323 | Se sirven desde la web, sin Google Fonts |
| Idiomas | `src/i18n/es.ts` y `en.ts` | Cada idioma es una página (`/` y `/en/`) con `hreflang` |
| Descargas | API de GitHub al compilar y en el navegador | Una versión nueva aparece sin volver a publicar |
| SEO | Canonical, Open Graph, `og.png`, sitemap | `@astrojs/sitemap` genera `sitemap-index.xml` |
| Tipos | TypeScript estricto + `astro check` | `en.ts` no compila si le falta un texto de `es.ts` |

## Carpetas

```
web/
  astro.config.mjs         dominio del sitio y el sitemap
  public/                  logo.png (el de ui/logo.png), favicon, apple-touch-icon y og.png (1200 × 630)
  src/
    config.ts              dominio, repo, demo del launcher y versiones de respaldo  ← lo primero que hay que revisar
    pages/index.astro      español (/)
    pages/en/index.astro   inglés (/en/)
    layouts/Base.astro     <head>: título, descripción, idiomas, Open Graph, íconos y fuentes
    components/
      Landing.astro        arma la página de un idioma
      Header.astro         logo, menú, HUD tipo F3, idioma, sonido, pausa y «Descargar»
      Hero.astro           la superficie
      Layer.astro          la envoltura de cada capa (su material, profundidad e ícono)
      Footer.astro
      capas/               una por sección: Funciones, Amigos, Hostear, Mods, Comunidades,
                           Escuela, Detalles, Descargar y Preguntas
    i18n/es.ts, en.ts      TODOS los textos visibles
    lib/layers.ts          orden de las capas: material, profundidad «Y» e ícono
    lib/releases.ts        versiones al compilar
    lib/releases-core.js   elegir la release de cada sistema (lo comparten compilación y navegador)
    scripts/pixel.js       el arte: texturas, minerales, bloques isométricos, criaturas y el corte de cada capa
    scripts/excavacion.js  todo lo que se mueve
    scripts/releases-client.js  pone al día las descargas en el navegador (caché de 1 hora)
    styles/global.css      estilos, con los colores en variables al inicio
```

## Las capas

| # | Capa (id) | Material | Y | Contenido |
|---|---|---|---|---|
| — | Superficie (`inicio`) | noche, pasto y tierra | 64 | Título, descargas, criaturas, bloque del logo |
| 1 | Tierra (`funciones`) | tierra y grava | 52 | Qué es; el launcher de ejemplo (o la demo real) |
| 2 | Cueva (`amigos`) | piedra, carbón, hierro, antorchas | 36 | Amigos, chat y el panel con O dentro del juego |
| 3 | Grava (`hostear`) | grava, piedra, cobre | 20 | Palanca del servidor, ajustes tipo Aternos y la carrera de rutas |
| 4 | Veta de redstone (`mods`) | piedra con redstone que brilla | 4 | Mesa de mods con dependencias y el doctor |
| 5 | Geoda (`comunidades`) | amatista y calcita | −12 | Unirse con código, «Jugar» con el paquete oficial y soporte con tarjeta de ajuste |
| 6 | Portal (`escuela`) | piedra, obsidiana, lapislázuli | −26 | Modo escuela: Mojang bloqueado, todo por el portal de Drive |
| 7 | Pizarra profunda (`detalles`) | pizarra con oro, diamante, esmeralda… | −40 | Cinco detalles y «Lo que viene» |
| 8 | Diamantes (`descargar`) | pizarra con diamantes y lava | −54 | Descargas con versión, tamaño y fecha; pasos para instalar |
| 9 | Roca madre (`preguntas`) | roca madre | −60 | Preguntas frecuentes |

Para agregar o mover una capa: `src/lib/layers.ts` (orden, material, Y e ícono), su componente en
`components/capas/`, sus textos en `es.ts`/`en.ts` y su línea en `Landing.astro`. La barra rápida tiene 9
casillas (teclas 1 a 9), una por capa.

## Diseño

| Papel | Color | Variable |
|---|---|---|
| Noche | `#120a28` | `--night` |
| Morado Parabellum | `#7f50f2` | `--purple` |
| Lila | `#b79bff` | `--lilac` |
| Texto del HUD | `#d8ff9a` | (directo) |
| Experiencia | `#80ff20` | `--xp` |
| Diamante | `#5ef2e8` | `--diamond` |

Los materiales de cada capa ponen el resto del color. En **modo escuela** el acento de toda la página pasa a
turquesa (`#1fb5a3`).

- **Bungee:** titulares.
- **Inter:** texto y botones (la fuente de la app). El logotipo va en minúsculas con el degradado de la app.
- **VT323:** lo que «habla» el juego: HUD, coordenadas, logros, tooltips.

| Ancho | Bloque | Paredes | Pozo y minero | Barra rápida |
|---|---|---|---|---|
| 1100 px o más | 48 px | 2 a 4 bloques | Sí | De pie, en la pared derecha |
| 700 a 1099 px | 40 px | 1 bloque | No | Abajo, se esconde al bajar |
| Menos de 700 px | 32 px | Solo en las orillas | No | Abajo, se esconde al bajar |

El texto empieza siempre después de la pared (`--wall`), así que los bloques nunca lo tapan.

## Interacciones

| Acción | Cómo | Qué pasa |
|---|---|---|
| Profundidad | Scroll | El HUD F3 cambia «Y» y la capa; la barra de experiencia avanza y sube de nivel; el minero baja por la escalera |
| Saltar de capa | HUD (lista), barra rápida o teclas 1–9 | Lleva a la capa |
| Cambiar de idioma | ES / EN del encabezado | Abre la otra página en la misma capa |
| Texto amarillo | Clic | Ocho frases (todas reales) |
| Bloque del logo | Clic | Vuela al logo del encabezado y suelta experiencia |
| Criaturas | Acercar el cursor o hacer clic | Huyen; la oveja cambia de lana; el cerdo, corazones; la gallina, plumas; el creeper se infla y no explota |
| Minerales | Mantener pulsado (o Enter/Espacio) | Grietas en cinco etapas, partículas, objeto y experiencia; vuelven a los 9 s |
| Launcher | Pestañas (con flechas) | Biblioteca, Explorar, Servidores y Mensajes |
| Panel del juego | Tecla O en cualquier parte o el botón; Esc cierra | El panel como el real: Amigos, Mensajes y Capturas |
| Invitación | Llega sola al ver el juego | «Alex te invitó» con «Unirse» |
| Palanca | Clic | Redstone, lámpara, servidor encendido y entran dos amigos |
| Ajustes del servidor | Interruptores | Cambia la línea real (`gamerule …`, `max-players`) |
| Carrera de rutas | «Unirse» y las casillas | Red local sin respuesta, P2P gana con 36 ms, playit.gg llega con 150 ms |
| Mesa de mods | «+ Iris», «+ Mod Menu», «+ Lithium» | La dependencia llega sola con la marca «dep» |
| Doctor | «Buscar el mod culpable» | Cuatro pruebas por mitades hasta «Roto»; «Desactivar» y el juego abre |
| Comunidad | «Unirme» y «Jugar» | Tarjeta de la comunidad y tres pasos hasta «¡Adentro!» |
| Soporte | «Aplicar» y «Deshacer» | La memoria pasa de 4 a 6 GB y vuelve |
| Modo escuela | Interruptor | Barreras frente a Mojang, portal activo y la página en turquesa |
| Diamante | Mantener pulsado | La experiencia vuela al botón de Windows |
| Pausa y sonido | Encabezado | Pausa toda la animación; sonidos sintetizados, apagados al empezar |

## Textos

Todos los textos visibles están en `src/i18n/es.ts` y `src/i18n/en.ts`, con la misma forma (TypeScript avisa
si falta uno). Los campos que terminan en `Html` llevan etiquetas (`<b>`, `<code>`, `<kbd>`). Los que usa el
JavaScript están en `ui` y llegan a la página como JSON (`#ui-strings`).

## Qué es real, qué es de ejemplo y qué falta

**Real (sale del repo):**

| Dato | De dónde |
|---|---|
| Versiones, tamaño, fecha y enlaces de descarga | Releases de GitHub, al compilar y en el navegador |
| 75 versiones (1.8 a 26.3), Vanilla/Fabric/Quilt/Forge/NeoForge, Java automático | `ROADMAP.md` |
| 36 ms de ping y 0 paquetes perdidos de 43 617 | `ROADMAP.md` (0.26.3) |
| 53 s en modo escuela, Fabric 1.20.1 | `ROADMAP.md` (0.30.0) |
| 11 modpacks y 678 mods | `ROADMAP.md` (0.16.0) |
| Panel con O de la 1.16.5 a la 26.3 | `mod/README.md` |
| «Roto» pide Minecraft 1.19.2 (prueba del doctor) e Iris pide Sodium | `ROADMAP.md` |
| Comunidades, soporte, copias de seguridad, temas | `README.md` |
| Pasos para instalar y preguntas frecuentes | `README.md` (mantenerlos iguales) |
| «Lo que viene» (mundos en Drive, ajustes del juego, O después de F2) | `docs/plan-noviembre.md` |

**De ejemplo (para ilustrar, cámbialos si quieres):**

| Qué | Dónde |
|---|---|
| Amigos Alex, Mar y Dani, sus mensajes y caras | `es.ts`/`en.ts` (`friends`, `features.mock`) y `pixel.js` (`face-*`) |
| Instancias de la Biblioteca (Survival con amigos, Better MC, Creativo, Cobbleverse) | `features.mock.instances` |
| La comunidad «Los de la cueva», el código `CUEVA-7Q2`, 12/40 jugadores y el paquete 1.2.0 | `community` |
| La tarjeta de soporte (memoria de 4 a 6 GB) | `community` y `excavacion.js` (`setupCommunity`) |
| 150 ms de playit.gg (el ROADMAP midió 140–180 ms) y «tu amigo está en otra casa» | `excavacion.js` (`ROUTES`) |
| Que Mod Menu traiga Fabric API | `excavacion.js` (`MODS`) |
| Los otros 15 mods del doctor | `ui.doctorMods` |

**Por confirmar o decidir:**
- El dominio (`astro.config.mjs` y `src/config.ts`).
- Si se muestra la demo real del launcher en Funciones: `SITE.launcherDemoUrl` (por ejemplo `'/demo/'` de la
  web actual) cambia la maqueta por un iframe.
- La revisión del inglés por Juan.
- Si se quiere analítica (no hay ninguna; tampoco cookies). Lo único que se guarda es la caché de versiones en
  `localStorage` del visitante, una hora.
- `og.png` es la superficie en español; se puede hacer una en inglés.

## Decisiones para Juan

- [ ] **Dónde vive:** publicarla desde `web/` (opción A) o copiarla a `landing-servidor` (opción B). Si se queda
  aquí, actualizar la línea de la web en `CLAUDE.md`.
- [ ] **Qué pasa con la web actual:** reemplazarla, o tomar partes de cada una. La actual tiene páginas aparte
  (`/funciones/`, `/descargas/`, `/ayuda/`) y la demo en `/demo/`; esta es una sola página.
- [ ] **Versión de Astro:** esta usa Astro 7; si `landing-servidor` usa otra, igualarlas.
- [ ] **Dominio, analítica e inglés** (arriba).

## Lista para publicar

- [ ] `npm run check` da 0 errores y `npm run build` termina.
- [ ] Las dos páginas (`/` y `/en/`) abren sin errores en la consola.
- [ ] Los botones bajan la versión más nueva de cada sistema.
- [ ] Probada en Safari, Firefox y Chrome, en escritorio y teléfono.
- [ ] Lighthouse (rendimiento y accesibilidad) en la URL publicada.
- [ ] El dominio está bien en los dos archivos y `og.png` se ve al compartir el enlace.

## Lo que ya se revisó

- `npm run check`: 0 errores, 0 advertencias. `npm run build`: las dos páginas, sin avisos.
- Chromium (Playwright) a 1440, 1280, 768 y 390 px, en español e inglés: sin errores en la consola y sin
  desplazamiento horizontal; las descargas se ponen al día desde GitHub.
- Teclado: foco visible, pestañas con flechas, Esc cierra el panel y la lista de capas, minerales con Enter.
- `prefers-reduced-motion` y el botón de pausa detienen toda la animación.

**No se revisó:** Safari, Firefox y Lighthouse.

## Créditos

- Fuentes Inter, Bungee y VT323 bajo la SIL Open Font License (vienen en sus paquetes de `@fontsource`).
- El arte pixel es original y se dibuja con código: no usa texturas, logos, fuentes ni capturas de
  Minecraft. El pie lleva el aviso de que Parabellum no es un producto oficial de Minecraft.
