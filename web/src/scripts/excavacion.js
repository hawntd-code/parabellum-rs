// Todo lo que se mueve en la página: el mundo de cada capa, las criaturas, el HUD, la barra rápida
// y las demostraciones (panel con O, carrera de rutas, mesa de mods, doctor, comunidad, soporte,
// modo escuela y minerales). Los datos de las demostraciones son de ejemplo; los textos vienen del
// idioma de la página (src/i18n), en el JSON #ui-strings.
import { Pixel as P } from './pixel.js';
import { refreshReleases } from './releases-client.js';

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];
const S = JSON.parse($('#ui-strings').textContent);
const fmt = (text, values) => text.replace(/\{(\w+)\}/g, (_, key) => values[key] ?? '');
const clamp = (value, min, max) => Math.min(max, Math.max(min, value));
const body = document.body;
const reducedQuery = window.matchMedia('(prefers-reduced-motion: reduce)');

const state = {
  paused: false,
  reduced: reducedQuery.matches,
  sound: false,
  metrics: null,
  active: null,
  level: 0,
  discovered: new Set(),
  ready: false,
  width: 0,
};
const motion = () => !state.paused && !state.reduced;
body.classList.toggle('reduced', state.reduced);

const hero = $('#inicio');
const layers = $$('.layer');
const sections = [hero, ...layers];
const fx = $('#fx');
const hotbar = $('#hotbar');

// ---------------------------------------------------------------- medidas del mundo

function computeMetrics() {
  const width = document.documentElement.clientWidth;
  if (width >= 1100) {
    const bs = 48;
    return { bs, walls: clamp(Math.floor((width - 1120) / 2 / bs), 2, 4), shaft: true, groundRows: 4, width };
  }
  if (width >= 700) return { bs: 40, walls: 1, shaft: false, groundRows: 4, width };
  return { bs: 32, walls: 0, shaft: false, groundRows: 3, width };
}

function applyMetrics() {
  const m = (state.metrics = computeMetrics());
  const root = document.documentElement.style;
  root.setProperty('--bs', `${m.bs}px`);
  root.setProperty('--wall', `${m.walls * m.bs}px`);
  root.setProperty('--ground', `${m.groundRows * m.bs}px`);
  body.classList.toggle('has-shaft', m.shaft);
}

// ---------------------------------------------------------------- sprites declarados en el HTML

// «sprite:moon», «tex:cobble» o «iso:CARA_DE_ARRIBA|LADOS».
function spriteCanvas(spec) {
  const [kind, ...rest] = spec.split(':');
  const value = rest.join(':');
  if (kind === 'sprite') return P.sprite(value);
  if (kind === 'tex') return P.texture(value);
  if (kind === 'iso') {
    const [top, side] = value.split('|');
    return P.iso(top, side || top, 64);
  }
  throw new Error(`Sprite sin tipo: ${spec}`);
}
const pxUrl = spec => `url(${P.url(spriteCanvas(spec))})`;

function hydrate(root = document) {
  $$('[data-px]', root).forEach(el => el.style.setProperty('--px', pxUrl(el.dataset.px)));
}

// ---------------------------------------------------------------- las capas

function glow(className, point) {
  const el = document.createElement('i');
  el.className = className;
  el.style.left = `${point.x}px`;
  el.style.top = `${point.y}px`;
  el.style.animationDelay = `${-(Math.random() * 3).toFixed(2)}s`;
  return el;
}

function renderLayer(section, force) {
  const m = state.metrics;
  const width = section.clientWidth;
  const height = section.offsetHeight;
  if (!width || !height) return;
  const key = `${width}x${height}x${m.bs}x${m.walls}`;
  if (!force && section.dataset.rendered === key) return;
  section.dataset.rendered = key;

  const result = P.world({
    material: section.dataset.material,
    width,
    height,
    bs: m.bs,
    walls: m.walls,
    shaft: m.shaft,
    seed: P.hash(section.id),
  });
  const canvas = result.canvas;
  canvas.className = 'world';
  canvas.setAttribute('aria-hidden', 'true');
  canvas.style.width = `${result.cssWidth}px`;
  canvas.style.height = `${result.cssHeight}px`;
  const old = section.querySelector(':scope > canvas.world');
  if (old) old.replaceWith(canvas);
  else section.prepend(canvas);

  let layerFx = section.querySelector(':scope > .fx');
  if (!layerFx) {
    layerFx = document.createElement('div');
    layerFx.className = 'fx';
    layerFx.setAttribute('aria-hidden', 'true');
    canvas.after(layerFx);
  }
  layerFx.replaceChildren(
    ...result.torches.map(p => glow('torch-glow', p)),
    ...result.lava.map(p => glow('lava-glow', p)),
    ...result.glows.map(p => glow('ore-glow', p)),
    ...result.crystals.slice(0, 8).map(p => glow('crystal-glint', p)),
  );
}

const renderTimers = new Map();
function scheduleRender(section) {
  clearTimeout(renderTimers.get(section));
  renderTimers.set(section, setTimeout(() => renderLayer(section), 140));
}
const layerObserver = new ResizeObserver(entries => entries.forEach(entry => scheduleRender(entry.target)));

// ---------------------------------------------------------------- la superficie

function renderHero() {
  const m = state.metrics;
  const width = hero.clientWidth;
  const ground = P.ground({ width, bs: m.bs, rows: m.groundRows, shaft: m.shaft, seed: 77 });
  const groundEl = $('#ground');
  groundEl.width = ground.canvas.width;
  groundEl.height = ground.canvas.height;
  const ctx = groundEl.getContext('2d');
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(ground.canvas, 0, 0);
  groundEl.style.width = `${ground.cssWidth}px`;
  groundEl.style.height = `${ground.cssHeight}px`;

  drawSky();
  drawHills($('#hills-far'), 11, 0.42, 0.95, '#291760', '#33207a');
  drawHills($('#hills-near'), 23, 0.18, 0.62, '#1c0f46', '#24145a');
  placeFlowers();
}

function drawSky() {
  const sky = $('#sky');
  const scale = 3;
  const w = Math.ceil(hero.clientWidth / scale);
  const h = Math.ceil(hero.offsetHeight / scale);
  sky.width = w;
  sky.height = h;
  const ctx = sky.getContext('2d');
  const random = P.mulberry32(91);
  const count = Math.round((w * h) / 260);
  for (let i = 0; i < count; i++) {
    const x = Math.floor(random() * w);
    const y = Math.floor(random() * h * 0.8);
    // Menos estrellas detrás del texto, para que se lea bien.
    if (x < w * 0.55 && y > h * 0.12 && y < h * 0.72 && random() < 0.85) continue;
    ctx.fillStyle = ['#ffffff', '#e9dcff', '#c9b6ff', '#a58cff'][Math.floor(random() * 4)];
    ctx.globalAlpha = 0.35 + random() * 0.65;
    ctx.fillRect(x, y, 1, 1);
    if (random() < 0.08) {
      ctx.fillRect(x - 1, y, 3, 1);
      ctx.fillRect(x, y - 1, 1, 3);
    }
  }
  ctx.globalAlpha = 1;
}

function drawHills(canvas, seed, minHeight, maxHeight, color, rim) {
  const step = state.metrics.bs / 4;
  const w = Math.ceil(hero.clientWidth / step);
  const h = Math.ceil((canvas.offsetHeight || 260) / step);
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d');
  const random = P.mulberry32(seed);
  const knots = [];
  for (let i = 0; i <= w / 6 + 2; i++) knots.push(minHeight + random() * (maxHeight - minHeight));
  for (let x = 0; x < w; x++) {
    const k = Math.floor(x / 6);
    const t = (x % 6) / 6;
    const top = h - Math.round((knots[k] * (1 - t) + knots[k + 1] * t) * h);
    ctx.fillStyle = color;
    ctx.fillRect(x, top, 1, h - top);
    ctx.fillStyle = rim;
    ctx.fillRect(x, top, 1, 1);
  }
}

function placeFlowers() {
  const m = state.metrics;
  const scale = m.bs >= 48 ? 3 : 2;
  const width = hero.clientWidth;
  const random = P.mulberry32(5);
  const kinds = ['poppy', 'dandelion', 'tuft', 'tuft', 'poppy', 'tuft', 'dandelion', 'tuft'];
  $('#flowers').replaceChildren(
    ...kinds.map((kind, index) => {
      const canvas = P.sprite(kind);
      const el = document.createElement('i');
      el.style.setProperty('--px', `url(${P.url(canvas)})`);
      el.style.width = `${canvas.width * scale}px`;
      el.style.height = `${canvas.height * scale}px`;
      el.style.left = `${clamp(((index + 0.5) / kinds.length) * width + (random() - 0.5) * 60, m.bs * 1.5, width - 40)}px`;
      return el;
    }),
  );
}

const pointer = { x: -9999, y: -9999, nx: 0.5 };
hero.addEventListener('pointermove', event => {
  pointer.x = event.clientX;
  pointer.y = event.clientY;
  pointer.nx = event.clientX / innerWidth;
  requestFrame();
});
hero.addEventListener('pointerleave', () => {
  pointer.x = pointer.y = -9999;
});

function updateParallax() {
  if (!motion()) return;
  const offset = Math.min(scrollY, hero.offsetHeight);
  const shift = pointer.nx - 0.5;
  $('#hills-far').style.transform = `translate(${shift * -14}px, ${offset * 0.12}px)`;
  $('#hills-near').style.transform = `translate(${shift * -28}px, ${offset * 0.06}px)`;
  $('.moon').style.transform = `translate(${shift * 10}px, ${offset * 0.3}px)`;
}

// ---------------------------------------------------------------- criaturas

const MOB_INFO = {
  sheep: { w: 20, h: 14, speed: 26 },
  pig: { w: 18, h: 12, speed: 30 },
  chicken: { w: 12, h: 13, speed: 36 },
  creeper: { w: 8, h: 16, speed: 0 },
};
const WOOL = ['#ece8f2', '#c9a8ff', '#7ff3ea', '#ffd84a', '#ff9fc3', '#8fe082', '#5a4a72'];
const mobs = [];
const mobScale = () => (state.metrics.bs >= 48 ? 4 : 3);

function setMobFrames(mob) {
  mob.el.style.setProperty('--f0', `url(${P.url(P.mob(mob.kind, 0, mob.color))})`);
  if (mob.kind !== 'creeper') mob.el.style.setProperty('--f1', `url(${P.url(P.mob(mob.kind, 1, mob.color))})`);
}

function sizeMob(mob) {
  const scale = mobScale();
  const info = MOB_INFO[mob.kind];
  mob.w = info.w * scale;
  mob.el.style.width = `${info.w * scale}px`;
  mob.el.style.height = `${info.h * scale}px`;
}

const placeMob = mob => {
  mob.el.style.transform = `translateX(${mob.x}px) scaleX(${mob.dir})`;
};

function makeMob(kind, lane, options = {}) {
  const el = document.createElement('button');
  el.type = 'button';
  el.className = `mob mob-${kind}`;
  el.tabIndex = -1;
  el.setAttribute('aria-hidden', 'true');
  const mob = { kind, el, lane, x: 0, dir: options.dir || 1, state: 'idle', timer: 0.5 + Math.random() * 2, color: options.color, frame: 0, frameTime: 0, flee: 0, w: 0 };
  setMobFrames(mob);
  sizeMob(mob);
  $('#mobs').append(el);
  mobs.push(mob);
  return mob;
}

function layoutMobs() {
  const width = $('#mobs').clientWidth;
  mobs.forEach(mob => {
    sizeMob(mob);
    mob.x = clamp(mob.lane * width, 10, width - mob.w - 10);
    placeMob(mob);
  });
}

function hop(el) {
  el.classList.remove('hop');
  void el.offsetWidth;
  el.classList.add('hop');
}

function setupMobs() {
  const wide = state.metrics.width >= 700;
  const creeper = makeMob('creeper', wide ? 0.62 : 0.66, { dir: 1 });
  const sheep = makeMob('sheep', wide ? 0.73 : 0.08, { dir: -1, color: WOOL[0] });
  const pig = makeMob('pig', wide ? 0.88 : 0.4, { dir: 1 });
  const chicken = makeMob('chicken', wide ? 0.5 : 0.82, { dir: -1 });
  layoutMobs();

  // El creeper se infla si lo miras mucho, pero aquí nada explota.
  let primeTimer;
  let fizzleTimer;
  const calm = () => {
    clearTimeout(primeTimer);
    clearTimeout(fizzleTimer);
    creeper.el.classList.remove('primed');
  };
  const prime = () => {
    clearTimeout(primeTimer);
    primeTimer = setTimeout(() => {
      creeper.el.classList.add('primed');
      sound('fuse');
      clearTimeout(fizzleTimer);
      fizzleTimer = setTimeout(() => {
        calm();
        toast(S.creeperTitle, S.creeperText, 'sprite:creeper');
      }, 1800);
    }, state.reduced ? 0 : 600);
  };
  creeper.el.addEventListener('pointerenter', prime);
  creeper.el.addEventListener('pointerleave', calm);
  creeper.el.addEventListener('click', prime);

  sheep.el.addEventListener('click', () => {
    sheep.wool = ((sheep.wool || 0) + 1) % WOOL.length;
    sheep.color = WOOL[sheep.wool];
    setMobFrames(sheep);
    hop(sheep.el);
    burst(sheep.el, [sheep.color, '#ffffff'], 8, { size: 6 });
    sound('pop');
  });
  pig.el.addEventListener('click', () => {
    hop(pig.el);
    burst(pig.el, [], 5, { px: pxUrl('sprite:heart'), cls: 'heart', rise: true, duration: 1100 });
    sound('pop');
  });
  chicken.el.addEventListener('click', () => {
    hop(chicken.el);
    burst(chicken.el, [], 6, { px: pxUrl('sprite:feather'), cls: 'feather', duration: 900 });
    sound('pop');
  });
}

let heroVisible = true;
let lastTime = 0;
function mobLoop(time) {
  const dt = Math.min(0.05, (time - lastTime) / 1000 || 0);
  lastTime = time;
  if (motion() && heroVisible && mobs.length) {
    const lane = $('#mobs').getBoundingClientRect();
    const minX = state.metrics.shaft ? state.metrics.bs * 1.5 : 6;
    const maxX = lane.width - 6;
    for (const mob of mobs) {
      if (mob.kind === 'creeper') continue;
      const info = MOB_INFO[mob.kind];
      const center = lane.left + mob.x + mob.w / 2;
      const near = Math.abs(pointer.x - center) < 85 && pointer.y > lane.top - 150 && pointer.y < lane.top + 30;
      if (near && mob.flee <= 0) {
        mob.flee = 0.9;
        mob.dir = pointer.x < center ? 1 : -1;
        mob.state = 'walk';
      }
      if (mob.flee > 0) mob.flee -= dt;
      mob.timer -= dt;
      if (mob.timer <= 0 && mob.flee <= 0) {
        if (mob.state === 'walk') {
          mob.state = 'idle';
          mob.timer = 1 + Math.random() * 2.5;
          mob.el.classList.remove('step');
        } else {
          mob.state = 'walk';
          mob.dir = Math.random() < 0.5 ? 1 : -1;
          mob.timer = 1.2 + Math.random() * 2.6;
        }
      }
      if (mob.state === 'walk') {
        mob.x += mob.dir * info.speed * (mob.flee > 0 ? 3.4 : 1) * (mobScale() / 4) * dt;
        if (mob.x < minX) {
          mob.x = minX;
          mob.dir = 1;
        }
        if (mob.x > maxX - mob.w) {
          mob.x = maxX - mob.w;
          mob.dir = -1;
        }
        mob.frameTime += dt;
        if (mob.frameTime > (mob.flee > 0 ? 0.08 : 0.17)) {
          mob.frameTime = 0;
          mob.frame ^= 1;
          mob.el.classList.toggle('step', mob.frame === 1);
        }
      }
      placeMob(mob);
    }
  }
  requestAnimationFrame(mobLoop);
}

// ---------------------------------------------------------------- el bloque del logo

function setupDropLogo() {
  const drop = $('#drop-logo');
  drop.addEventListener('click', () => {
    if (drop.classList.contains('taken')) return;
    const img = $('img', drop);
    const from = img.getBoundingClientRect();
    const to = $('#brand img').getBoundingClientRect();
    drop.classList.add('taken');
    sound('pop');
    toast(S.picked, S.logoBlock, 'url(/logo.png)');
    if (motion()) {
      const ghost = img.cloneNode();
      ghost.style.cssText = `position:absolute;left:0;top:0;width:${from.width}px;height:${from.height}px;`;
      fx.append(ghost);
      ghost.animate(
        [
          { transform: `translate(${from.left}px, ${from.top}px) scale(1)` },
          { transform: `translate(${(from.left + to.left) / 2}px, ${Math.min(from.top, to.top) - 60}px) scale(.8)`, offset: 0.5 },
          { transform: `translate(${to.left - (from.width - to.width) / 2}px, ${to.top - (from.height - to.height) / 2}px) scale(${to.width / from.width})` },
        ],
        { duration: 650, easing: 'cubic-bezier(.4,0,.2,1)' },
      ).onfinish = () => {
        ghost.remove();
        hop($('#brand img'));
        xpBurst($('#brand'), $('.xp'), 4);
      };
    }
    setTimeout(() => {
      drop.classList.remove('taken');
      drop.classList.add('respawn');
      setTimeout(() => drop.classList.remove('respawn'), 900);
    }, 4200);
  });
}

// ---------------------------------------------------------------- scroll: profundidad, HUD y minero

let framePending = false;
function requestFrame() {
  if (framePending) return;
  framePending = true;
  requestAnimationFrame(() => {
    framePending = false;
    onFrame();
  });
}

let lastScroll = scrollY;
let idleTimer;

function onFrame() {
  const y = scrollY;
  const max = Math.max(1, document.documentElement.scrollHeight - innerHeight);
  $('#xp-fill').style.width = `${clamp(y / max, 0, 1) * 100}%`;

  const probe = innerHeight * 0.42;
  let active = sections[0];
  for (const section of sections) {
    if (section.getBoundingClientRect().top <= probe) active = section;
  }
  if (active !== state.active) setActive(active);

  const delta = y - lastScroll;
  lastScroll = y;
  hotbar.classList.toggle('in-hero', y < hero.offsetHeight * 0.55);
  if (delta > 8) hotbar.classList.add('tucked');
  else if (delta < -8) hotbar.classList.remove('tucked');
  clearTimeout(idleTimer);
  idleTimer = setTimeout(() => hotbar.classList.remove('tucked'), 650);

  updateMiner();
  updateParallax();
}

function setActive(section) {
  state.active = section;
  const index = sections.indexOf(section);
  $('#f3-y').textContent = `Y ${section.dataset.y}`;
  $('#f3-biome').textContent = section.dataset.biome;
  $('#xp-level').textContent = String(index);
  if (index > state.level) {
    state.level = index;
    gainXp();
  }
  $$('a', hotbar).forEach(link => link.classList.toggle('active', link.hash === `#${section.id}`));
  $$('#biome-list a').forEach(link => link.classList.toggle('active', link.hash === `#${section.id}`));
  // Bajar a una capa nueva sube el nivel de la barra de experiencia (sin avisos que tapen el texto).
  if (state.ready && index > 0 && !state.discovered.has(section.id)) {
    state.discovered.add(section.id);
    sound('level');
  }
}

const miner = $('#miner');
const minerFrames = {};
function updateMiner() {
  if (!state.metrics.shaft) return;
  if (!minerFrames.front) {
    for (const name of ['front', 'climbA', 'climbB']) minerFrames[name] = pxUrl(`sprite:miner-${name}`);
  }
  const height = 60;
  const groundTop = $('#ground').getBoundingClientRect().top;
  const bottom = layers[layers.length - 1].getBoundingClientRect().bottom;
  const fixedY = innerHeight * 0.48;
  let top = Math.max(fixedY, groundTop - height + 6);
  top = Math.min(top, bottom - height - 8);
  const onSurface = groundTop - height + 6 >= fixedY - 1;
  const frame = onSurface ? 'front' : Math.floor(scrollY / 28) % 2 ? 'climbA' : 'climbB';
  miner.style.transform = `translateY(${top}px)`;
  if (miner.dataset.frame !== frame) {
    miner.dataset.frame = frame;
    miner.style.setProperty('--miner', minerFrames[frame]);
  }
}

// ---------------------------------------------------------------- barra rápida y lista de capas

function buildNavigation() {
  layers.forEach((section, index) => {
    const link = document.createElement('a');
    link.href = `#${section.id}`;
    link.setAttribute('aria-label', `${index + 1}: ${section.dataset.label}`);
    link.dataset.tip = `${section.dataset.label}|${fmt(S.key, { n: index + 1 })} · ${section.dataset.biome}`;
    const number = document.createElement('span');
    number.textContent = String(index + 1);
    const icon = document.createElement('i');
    icon.style.setProperty('--px', pxUrl(section.dataset.icon));
    link.append(number, icon);
    hotbar.append(link);
  });
  hotbar.addEventListener('focusin', () => hotbar.classList.remove('tucked'));
  document.addEventListener(
    'pointermove',
    event => {
      if (event.clientY > innerHeight - 90) hotbar.classList.remove('tucked');
    },
    { passive: true },
  );

  const list = $('#biome-list');
  sections.forEach(section => {
    const link = document.createElement('a');
    link.href = `#${section.id}`;
    const name = document.createElement('b');
    name.textContent = section.dataset.label;
    const y = document.createElement('span');
    y.textContent = `Y ${section.dataset.y}`;
    link.append(name, y);
    link.addEventListener('click', () => setBiomes(false));
    list.append(link);
  });
  $('#f3-chip').addEventListener('click', () => setBiomes(list.hidden));
  document.addEventListener('click', event => {
    if (!event.target.closest('.f3')) setBiomes(false);
  });
}

function setBiomes(open) {
  $('#biome-list').hidden = !open;
  $('#f3-chip').setAttribute('aria-expanded', String(open));
}

// ---------------------------------------------------------------- logros, tooltip y efectos

const iconValue = icon => (!icon ? null : icon.startsWith('url(') ? icon : pxUrl(icon));

function toast(title, text, icon) {
  const box = $('#toasts');
  box.replaceChildren();
  const el = document.createElement('div');
  el.className = 'toast';
  const value = iconValue(icon);
  if (value) {
    const i = document.createElement('i');
    i.style.setProperty('--px', value);
    el.append(i);
  }
  const textBox = document.createElement('span');
  const small = document.createElement('small');
  small.textContent = title;
  const bold = document.createElement('b');
  bold.textContent = text;
  textBox.append(small, bold);
  el.append(textBox);
  box.append(el);
  setTimeout(() => el.classList.add('out'), 2500);
  setTimeout(() => el.remove(), 2850);
}

const tooltip = $('#tooltip');
function showTip(el) {
  const [title, sub] = el.dataset.tip.split('|');
  tooltip.replaceChildren(document.createTextNode(title));
  if (sub) {
    const small = document.createElement('small');
    small.textContent = sub;
    tooltip.append(small);
  }
  tooltip.hidden = false;
  const rect = el.getBoundingClientRect();
  const tip = tooltip.getBoundingClientRect();
  if (el.closest('.hotbar') && body.classList.contains('has-shaft')) {
    tooltip.style.left = `${rect.left - tip.width - 12}px`;
    tooltip.style.top = `${rect.top + rect.height / 2 - tip.height / 2}px`;
    return;
  }
  let top = rect.top - tip.height - 10;
  if (top < 72) top = rect.bottom + 10;
  tooltip.style.left = `${clamp(rect.left + rect.width / 2 - tip.width / 2, 8, innerWidth - tip.width - 8)}px`;
  tooltip.style.top = `${top}px`;
}
const hideTip = () => {
  tooltip.hidden = true;
};
document.addEventListener('pointerover', event => {
  const target = event.target.closest('[data-tip]');
  if (target && event.pointerType === 'mouse') showTip(target);
});
document.addEventListener('pointerout', event => {
  if (event.target.closest('[data-tip]')) hideTip();
});
document.addEventListener('focusin', event => {
  const target = event.target.closest('[data-tip]');
  if (target) showTip(target);
  else hideTip();
});
addEventListener('scroll', hideTip, { passive: true });

function center(el) {
  const rect = el.getBoundingClientRect();
  return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
}

function burst(el, colors, count = 10, options = {}) {
  if (!motion()) return;
  const origin = center(el);
  for (let i = 0; i < count; i++) {
    const p = document.createElement('i');
    p.className = `particle${options.cls ? ` ${options.cls}` : ''}`;
    if (options.px) {
      p.style.setProperty('--px', options.px);
    } else {
      const size = options.size || 5 + Math.random() * 5;
      p.style.width = p.style.height = `${size}px`;
      p.style.background = colors[i % colors.length];
    }
    fx.append(p);
    const angle = Math.random() * Math.PI * 2;
    const distance = 26 + Math.random() * 48;
    const dx = Math.cos(angle) * distance;
    const dy = options.rise ? -40 - Math.random() * 40 : Math.sin(angle) * distance - 26;
    const fall = options.rise ? -50 : 90;
    p.animate(
      [
        { transform: `translate(${origin.x}px, ${origin.y}px)`, opacity: 1 },
        { transform: `translate(${origin.x + dx * 0.7}px, ${origin.y + dy}px)`, opacity: 1, offset: 0.45 },
        { transform: `translate(${origin.x + dx}px, ${origin.y + dy + fall}px)`, opacity: 0 },
      ],
      { duration: options.duration || 750, easing: 'cubic-bezier(.2,.6,.4,1)' },
    ).onfinish = () => p.remove();
  }
}

function xpBurst(fromEl, toEl, count = 6, done) {
  if (!motion()) {
    gainXp();
    if (done) done();
    return;
  }
  const a = center(fromEl);
  const b = center(toEl);
  const orbUrl = pxUrl('sprite:orb');
  for (let i = 0; i < count; i++) {
    const orb = document.createElement('i');
    orb.className = 'orb';
    orb.style.setProperty('--px', orbUrl);
    fx.append(orb);
    const mid = { x: (a.x + b.x) / 2 + (Math.random() - 0.5) * 140, y: Math.min(a.y, b.y) - 60 - Math.random() * 80 };
    orb.animate(
      [
        { transform: `translate(${a.x - 7}px, ${a.y - 7}px) scale(.4)` },
        { transform: `translate(${mid.x - 7}px, ${mid.y - 7}px) scale(1.1)`, offset: 0.5 },
        { transform: `translate(${b.x - 7}px, ${b.y - 7}px) scale(.7)` },
      ],
      { duration: 700 + i * 50, delay: i * 70, easing: 'ease-in-out', fill: 'both' },
    ).onfinish = () => {
      orb.remove();
      sound('xp');
      if (i === count - 1) {
        gainXp();
        if (done) done();
      }
    };
  }
}

function gainXp() {
  const xp = $('.xp');
  xp.classList.remove('gain');
  void xp.offsetWidth;
  xp.classList.add('gain');
}

// ---------------------------------------------------------------- sonidos (apagados al empezar)

let audio = null;
function audioContext() {
  if (!audio) audio = new (window.AudioContext || /** @type {any} */ (window).webkitAudioContext)();
  return audio;
}
function beep(freq, duration, { type = 'square', volume = 0.035, to } = {}) {
  const a = audioContext();
  const osc = a.createOscillator();
  const gain = a.createGain();
  osc.type = type;
  osc.frequency.setValueAtTime(freq, a.currentTime);
  if (to) osc.frequency.exponentialRampToValueAtTime(to, a.currentTime + duration);
  gain.gain.setValueAtTime(volume, a.currentTime);
  gain.gain.exponentialRampToValueAtTime(0.0001, a.currentTime + duration);
  osc.connect(gain).connect(a.destination);
  osc.start();
  osc.stop(a.currentTime + duration + 0.02);
}
function noise(duration, volume, freq) {
  const a = audioContext();
  const buffer = a.createBuffer(1, Math.ceil(a.sampleRate * duration), a.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < data.length; i++) data[i] = (Math.random() * 2 - 1) * (1 - i / data.length);
  const source = a.createBufferSource();
  source.buffer = buffer;
  const filter = a.createBiquadFilter();
  filter.type = 'bandpass';
  filter.frequency.value = freq;
  const gain = a.createGain();
  gain.gain.value = volume;
  source.connect(filter).connect(gain).connect(a.destination);
  source.start();
}
const SOUNDS = {
  hit: () => noise(0.05, 0.5, 1300),
  break: () => noise(0.18, 0.7, 650),
  pop: () => beep(480, 0.09, { type: 'sine', to: 920, volume: 0.06 }),
  xp: () => beep(1100 + Math.random() * 500, 0.05, { type: 'sine', volume: 0.025 }),
  level: () => {
    beep(523, 0.09);
    setTimeout(() => beep(784, 0.14), 95);
  },
  msg: () => {
    beep(880, 0.08, { type: 'sine', volume: 0.05 });
    setTimeout(() => beep(1320, 0.1, { type: 'sine', volume: 0.05 }), 90);
  },
  click: () => beep(640, 0.03, { volume: 0.03 }),
  fuse: () => noise(0.6, 0.25, 3200),
};
function sound(name) {
  if (!state.sound) return;
  try {
    SOUNDS[name]();
  } catch {
    // Sin audio en este navegador: la página sigue igual.
  }
}

// ---------------------------------------------------------------- minerales

const mined = new Set();
function setupMine(button) {
  const block = button.dataset.block;
  const base = block.startsWith('ore:') ? block.split(':')[1] : block;
  const label = button.getAttribute('aria-label');
  const isDiamond = button.id === 'diamond-mine';
  button.style.setProperty('--tex', `url(${P.url(P.texture(block, 1))})`);
  button.style.setProperty('--air', `url(${P.url(P.shaded(base, 2, 0.32))})`);
  button.dataset.tip = `${button.dataset.name}|${S.holdToMine}`;
  const mine = { stage: 0, broken: false, timer: null };

  const stop = () => {
    clearInterval(mine.timer);
    mine.timer = null;
  };
  const hit = () => {
    if (mine.broken) return;
    mine.stage++;
    button.style.setProperty('--crack', `url(${P.url(P.crack(mine.stage))})`);
    button.classList.remove('hit');
    void button.offsetWidth;
    button.classList.add('hit');
    sound('hit');
    burst(button, P.colorsOf(block), 3, { size: 5 });
    if (mine.stage >= 5) breakBlock();
  };
  const breakBlock = () => {
    stop();
    mine.broken = true;
    button.classList.add('broken');
    button.style.removeProperty('--crack');
    button.setAttribute('aria-label', fmt(S.mined, { name: button.dataset.name }));
    burst(button, P.colorsOf(block), 14);
    sound('break');
    const dropCanvas = P.dropFor(block);
    const drop = document.createElement('i');
    drop.className = `drop${button.classList.contains('mine-big') ? ' big' : ''}`;
    drop.style.setProperty('--px', `url(${P.url(dropCanvas)})`);
    button.parentElement.append(drop);
    const first = !mined.has(block);
    mined.add(block);
    if (!isDiamond) toast(first ? S.achievement : S.picked, button.dataset.name, `url(${P.url(dropCanvas)})`);
    setTimeout(() => {
      const target = isDiamond ? $('#dl-windows') : $('.xp');
      xpBurst(drop, target, isDiamond ? 9 : 5, () => {
        if (!isDiamond) return;
        target.classList.add('pulse');
        setTimeout(() => target.classList.remove('pulse'), 2000);
      });
      drop.classList.add('gone');
      setTimeout(() => drop.remove(), 400);
    }, 1300);
    setTimeout(() => {
      mine.broken = false;
      mine.stage = 0;
      button.classList.remove('broken');
      button.classList.add('restore');
      button.setAttribute('aria-label', label);
      setTimeout(() => button.classList.remove('restore'), 600);
    }, 9000);
  };

  button.addEventListener('pointerdown', event => {
    if (event.button !== 0 || mine.broken) return;
    event.preventDefault();
    hit();
    stop();
    mine.timer = setInterval(hit, 190);
  });
  ['pointerup', 'pointerleave', 'pointercancel'].forEach(type => button.addEventListener(type, stop));
  // Con teclado (Enter o Espacio) cada pulsación es un golpe.
  button.addEventListener('click', event => {
    if (event.detail === 0) hit();
  });
}

// ---------------------------------------------------------------- el launcher de ejemplo

function setupLauncher() {
  const tabs = $$('[data-ltab]');
  if (!tabs.length) return;
  const crumb = $('#l-crumb');
  const crumbs = JSON.parse(crumb.dataset.crumbs);
  const select = name => {
    tabs.forEach(tab => {
      tab.setAttribute('aria-selected', String(tab.dataset.ltab === name));
      tab.tabIndex = tab.dataset.ltab === name ? 0 : -1;
    });
    $$('[data-lpanel]').forEach(panel => {
      panel.hidden = panel.dataset.lpanel !== name;
    });
    $$('[data-lrail]').forEach(icon => icon.classList.toggle('active', icon.dataset.lrail === name));
    crumb.textContent = crumbs[name];
    sound('click');
  };
  tabs.forEach((tab, index) => {
    tab.tabIndex = index === 0 ? 0 : -1;
    tab.addEventListener('click', () => select(tab.dataset.ltab));
    tab.addEventListener('keydown', event => {
      if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
      const next = tabs[(index + (event.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length];
      next.focus();
      select(next.dataset.ltab);
    });
  });
}

// ---------------------------------------------------------------- amigos y el panel con O

const game = $('#game');
const panel = $('#game-panel');
const openButton = $('#game-open');

// Un paisaje de bloques para la «pantalla» del juego y las capturas.
function drawGameScene(canvas, seed, options = {}) {
  const W = 240;
  const H = 150;
  const B = 10;
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext('2d');
  ctx.imageSmoothingEnabled = false;
  const random = P.mulberry32(seed);

  const sky = ctx.createLinearGradient(0, 0, 0, H);
  sky.addColorStop(0, options.sunset ? '#ff8f6a' : options.night ? '#1a1240' : '#6fadf2');
  sky.addColorStop(1, options.sunset ? '#ffd2a0' : options.night ? '#3a2a78' : '#cfe6ff');
  ctx.fillStyle = sky;
  ctx.fillRect(0, 0, W, H);
  ctx.fillStyle = options.night ? '#ece7c9' : '#fff6c0';
  ctx.fillRect(184, 16, 16, 16);
  ctx.fillStyle = options.night ? '#cdc6a2' : '#ffe680';
  ctx.fillRect(186, 18, 12, 12);

  ctx.fillStyle = options.night ? 'rgba(200,190,255,.35)' : 'rgba(255,255,255,.88)';
  for (let i = 0; i < 4; i++) {
    const x = Math.floor(random() * W);
    const y = 8 + Math.floor(random() * 30);
    ctx.fillRect(x, y, 26 + Math.floor(random() * 26), 6);
    ctx.fillRect(x + 8, y - 4, 14, 4);
  }

  ctx.fillStyle = options.night ? '#2a2060' : options.sunset ? '#c58a8a' : '#93b4d6';
  for (let x = 0; x < W; x += 4) {
    const h = 22 + Math.round(Math.sin(x / 23 + seed) * 10 + Math.sin(x / 9) * 4);
    ctx.fillRect(x, 92 - h, 4, h + 20);
  }

  const cols = W / B;
  const heights = [];
  for (let c = 0; c < cols; c++) heights.push(8 + Math.round(Math.sin(c / 3 + seed) * 1.4 + random() * 0.8));
  const friendCol = Math.round(cols * 0.63);
  for (let c = friendCol - 2; c <= friendCol + 2; c++) heights[c] = 9;
  const waterLevel = 10;
  for (let c = 2; c < 7; c++) heights[c] = 11;

  for (let c = 0; c < cols; c++) {
    for (let r = heights[c]; r < H / B; r++) {
      const name = r === heights[c] ? 'grass' : r < heights[c] + 3 ? 'dirt' : 'stone';
      ctx.drawImage(P.texture(name, (c + r) % 4), c * B, r * B, B, B);
    }
    if (heights[c] > waterLevel) {
      ctx.globalAlpha = 0.8;
      for (let r = waterLevel; r < heights[c]; r++) ctx.drawImage(P.texture('water', c % 4), c * B, r * B + 2, B, B);
      ctx.globalAlpha = 1;
    }
  }

  const treeCol = Math.round(cols * 0.86);
  const treeTop = heights[treeCol] - 4;
  for (let r = treeTop + 1; r < heights[treeCol]; r++) ctx.drawImage(P.texture('log', r % 4), treeCol * B, r * B, B, B);
  for (let dx = -2; dx <= 2; dx++) {
    for (let dy = -1; dy <= 1; dy++) {
      if (Math.abs(dx) === 2 && dy === -1) continue;
      ctx.drawImage(P.texture('leaves', (dx + dy + 4) % 4), (treeCol + dx) * B, (treeTop + dy) * B, B, B);
    }
  }

  if (!options.friend) return null;
  const fx0 = friendCol * B - 1;
  const fy0 = heights[friendCol] * B - 18;
  ctx.drawImage(P.sprite('friend-alex'), fx0, fy0, 11, 18);
  return { x: (fx0 + 5.5) / W, y: (fy0 - 6) / H };
}

function setPanel(open, focus = true) {
  panel.hidden = !open;
  game.classList.toggle('panel-open', open);
  openButton.setAttribute('aria-expanded', String(open));
  if (open) {
    sound('pop');
    if (focus) $('[aria-selected="true"]', panel).focus();
  } else if (focus) {
    openButton.focus();
  }
}

function setupGame() {
  const friend = drawGameScene($('#game-scene'), 3, { friend: true });
  const tag = $('#nametag');
  tag.style.left = `${friend.x * 100}%`;
  tag.style.top = `${friend.y * 100}%`;
  tag.style.transform = 'translate(-50%, -100%)';
  $$('.shot-canvas').forEach((canvas, index) => drawGameScene(canvas, 10 + index * 7, index === 1 ? { sunset: true } : index === 2 ? { night: true } : {}));

  const hearts = $('#g-hearts');
  for (let i = 0; i < 10; i++) {
    const heart = document.createElement('i');
    heart.style.setProperty('--px', pxUrl(i === 9 ? 'sprite:halfHeart' : 'sprite:heart'));
    hearts.append(heart);
  }

  openButton.addEventListener('click', () => setPanel(panel.hidden));
  $('#game-close').addEventListener('click', () => setPanel(false));

  const tabs = $$('[data-gp]', panel);
  tabs.forEach(tab =>
    tab.addEventListener('click', () => {
      tabs.forEach(item => item.setAttribute('aria-selected', String(item === tab)));
      $$('[data-gpview]', panel).forEach(view => {
        view.hidden = view.dataset.gpview !== tab.dataset.gp;
      });
      sound('click');
    }),
  );

  $$('.shot', panel).forEach(shot =>
    shot.addEventListener('click', () => {
      toast(S.shotTitle, S.shotText, null);
      sound('msg');
    }),
  );

  // El aviso de invitación sale abajo a la derecha la primera vez que se ve el juego.
  let invited = false;
  new IntersectionObserver(
    entries => {
      if (invited || !entries[0].isIntersecting) return;
      invited = true;
      setTimeout(() => {
        $('#game-toast').hidden = false;
        sound('msg');
      }, 1300);
    },
    { threshold: 0.6 },
  ).observe(game);
}

function setupFriends() {
  const reactions = $$('[data-reaction]');
  reactions.forEach(button =>
    button.addEventListener('click', () => {
      // Una reacción por persona, como en la app: otra la reemplaza y la misma la quita.
      const pressed = button.getAttribute('aria-pressed') === 'true';
      reactions.forEach(item => item.setAttribute('aria-pressed', 'false'));
      button.setAttribute('aria-pressed', String(!pressed));
      if (!pressed) burst(button, [], 1, { px: pxUrl('sprite:heart'), cls: 'heart', rise: true, duration: 900 });
      sound('click');
    }),
  );

  $$('.invite-btn').forEach(button =>
    button.addEventListener('click', () => {
      button.textContent = S.invited;
      button.disabled = true;
      toast(S.inviteTitle, S.inviteText, 'sprite:face-mar');
      sound('msg');
    }),
  );

  $$('.join-trigger').forEach(button =>
    button.addEventListener('click', () => {
      if (!panel.hidden) setPanel(false, false);
      $('#race').scrollIntoView({ behavior: motion() ? 'smooth' : 'auto', block: 'center' });
      setTimeout(startRace, motion() ? 700 : 0);
    }),
  );

  const typing = $('#typing');
  let typingOn = true;
  setInterval(() => {
    if (!motion()) return;
    typingOn = !typingOn;
    typing.style.visibility = typingOn ? 'visible' : 'hidden';
  }, 4200);
}

// ---------------------------------------------------------------- hostear: palanca, ajustes y carrera

function setupHosting() {
  const lever = $('#lever');
  const card = $('#server-card');
  const heads = $('#srv-heads');
  let timers = [];
  let maxPlayers = 10;
  let leverToasted = false;
  const setPlayers = count => {
    $('#srv-players').textContent = `${count}/${maxPlayers}`;
  };

  lever.addEventListener('click', () => {
    const on = lever.getAttribute('aria-checked') !== 'true';
    lever.setAttribute('aria-checked', String(on));
    lever.setAttribute('aria-label', on ? S.leverOn : S.leverOff);
    $('#dust').classList.toggle('on', on);
    $('#lamp').classList.toggle('on', on);
    card.classList.toggle('on', on);
    $('#srv-state').textContent = on ? S.on : S.off;
    $('#srv-tps').textContent = on ? '20' : '—';
    timers.forEach(clearTimeout);
    timers = [];
    heads.replaceChildren();
    setPlayers(0);
    sound('click');
    if (!on) return;
    if (!leverToasted) {
      leverToasted = true;
      toast(S.achievement, S.serverAchievement, 'iso:lampOn|lampOn');
    }
    ['face-alex', 'face-mar'].forEach((face, index) => {
      timers.push(
        setTimeout(() => {
          const head = document.createElement('i');
          head.style.setProperty('--px', pxUrl(`sprite:${face}`));
          heads.append(head);
          setPlayers(index + 1);
          sound('pop');
        }, 900 + index * 900),
      );
    });
  });

  const codes = [on => `gamerule keepInventory ${on}`, on => `gamerule doDaylightCycle ${on}`, on => `max-players=${on ? 20 : 10}`];
  $$('.rule input').forEach((input, index) =>
    input.addEventListener('change', () => {
      $('code', input.closest('.rule')).textContent = codes[index](input.checked);
      if (index === 2) {
        maxPlayers = input.checked ? 20 : 10;
        setPlayers(heads.children.length);
      }
      sound('click');
    }),
  );

  $('#race-start').addEventListener('click', startRace);
  $$('[data-route-toggle]').forEach(input =>
    input.addEventListener('change', () => {
      $(`.track[data-route="${input.dataset.routeToggle}"]`).classList.toggle('off', !input.checked);
    }),
  );
}

// Ejemplo: tu amigo está en otra casa, así que la red local no responde. 36 ms es la mediana medida con
// P2P; 150 ms está dentro de lo que se midió por playit.gg (140–180 ms).
const ROUTES = {
  local: { duration: 950, fails: true },
  p2p: { duration: 1150, result: '36 ms' },
  playit: { duration: 2000, result: '150 ms' },
};
let raceAnimations = [];
let raceTimers = [];
let raceToasted = false;

function startRace() {
  raceAnimations.forEach(animation => animation.cancel());
  raceTimers.forEach(clearTimeout);
  raceAnimations = [];
  raceTimers = [];
  const enabled = {};
  $$('[data-route-toggle]').forEach(input => {
    enabled[input.dataset.routeToggle] = input.checked;
  });
  const winner = enabled.p2p ? 'p2p' : enabled.playit ? 'playit' : null;
  const result = $('#race-result');

  if (!Object.values(enabled).some(Boolean)) {
    result.textContent = S.raceNone;
    return;
  }
  result.textContent = S.raceTesting;
  sound('click');

  $$('.track').forEach(track => {
    const name = track.dataset.route;
    const route = ROUTES[name];
    const cart = $('.cart', track);
    const rail = $('.rail', track);
    const out = $('.track-result', track);
    track.classList.remove('win', 'fail');
    track.classList.toggle('off', !enabled[name]);
    cart.style.transform = 'translateX(0)';
    if (!enabled[name]) {
      out.textContent = S.raceOff;
      return;
    }
    out.textContent = S.raceProbing;
    const distance = rail.clientWidth - cart.offsetWidth - 34;
    const target = route.fails ? distance * 0.42 : distance;
    const finish = () => {
      cart.style.transform = `translateX(${target}px)`;
      out.textContent = route.fails ? S.raceNoAnswer : route.result;
      if (route.fails) {
        track.classList.add('fail');
        if (motion()) cart.animate([{ translate: '0 0' }, { translate: '-4px 0' }, { translate: '4px 0' }, { translate: '0 0' }], { duration: 260 });
      }
      if (name === winner) declareWinner(track, name);
    };
    if (motion()) {
      const animation = cart.animate([{ transform: 'translateX(0)' }, { transform: `translateX(${target}px)` }], {
        duration: route.duration,
        easing: route.fails ? 'cubic-bezier(.3,.6,.2,1)' : 'cubic-bezier(.5,0,.3,1)',
        fill: 'forwards',
      });
      raceAnimations.push(animation);
      animation.onfinish = finish;
    } else {
      raceTimers.push(setTimeout(finish, 10));
    }
  });

  if (!winner) {
    raceTimers.push(setTimeout(() => (result.textContent = S.raceNoWinner), motion() ? 1000 : 20));
  }
}

function declareWinner(track, name) {
  track.classList.add('win');
  $('#race-result').textContent = fmt(S.raceWinner, { route: S.routeNames[name], result: ROUTES[name].result });
  sound('level');
  if (!raceToasted) {
    raceToasted = true;
    toast(S.achievement, S.raceAchievement, 'sprite:minecart-p2p');
  }
}

// ---------------------------------------------------------------- mods: la mesa

// Iris pide Sodium (lo dice el ROADMAP); que Mod Menu traiga Fabric API es de ejemplo.
const MODS = {
  iris: { name: 'Iris Shaders', tex: 'tint:#ff7ad9,#c94fb0,#ff9ff0,#8a2f78', deps: ['sodium'] },
  sodium: { name: 'Sodium', tex: 'tint:#e9f1ff,#bcd0ee,#ffffff,#8fa6c9', deps: [] },
  modmenu: { name: 'Mod Menu', tex: 'tint:#ffd84a,#e0b02a,#fff09a,#a87a12', deps: ['fabricapi'] },
  fabricapi: { name: 'Fabric API', tex: 'tint:#d9c7a4,#b8a37c,#eee0c2,#8c7856', deps: [] },
  lithium: { name: 'Lithium', tex: 'tint:#7cf0b0,#3fc883,#b5ffd6,#1f8a55', deps: [] },
};

function setupCrafting() {
  const grid = $('#craft-grid');
  const log = $('#craft-log');
  const result = $('#craft-result');
  let installed = [];
  const deps = new Set();
  let timers = [];
  const names = list => list.map(id => MODS[id].name).join(S.and);

  const render = () => {
    grid.replaceChildren();
    for (let i = 0; i < 9; i++) {
      const slot = document.createElement('span');
      slot.className = 'slot';
      const id = installed[i];
      if (id) {
        const item = document.createElement('i');
        item.style.setProperty('--px', pxUrl(`iso:${MODS[id].tex}|${MODS[id].tex}`));
        slot.append(item);
        slot.dataset.tip = `${MODS[id].name}|${deps.has(id) ? S.craftDep : S.craftMine}`;
        slot.setAttribute('aria-label', MODS[id].name);
        if (deps.has(id)) slot.classList.add('dep');
      }
      grid.append(slot);
    }
    $('#craft-count').textContent = String(installed.length);
    result.classList.toggle('ready', installed.length > 0);
    $('.craft-arrow').classList.toggle('ready', installed.length > 0);
    result.dataset.tip = installed.length ? fmt(S.craftResult, { n: installed.length }) : S.craftEmpty;
  };

  $$('[data-add-mod]').forEach(button =>
    button.addEventListener('click', () => {
      const id = button.dataset.addMod;
      const mod = MODS[id];
      if (installed.includes(id)) {
        log.textContent = fmt(S.craftHas, { name: mod.name });
        return;
      }
      const missing = mod.deps.filter(dep => !installed.includes(dep));
      if (installed.length + 1 + missing.length > 9) {
        log.textContent = S.craftFull;
        return;
      }
      installed.push(id);
      render();
      sound('pop');
      if (!missing.length) {
        log.textContent = fmt(S.craftInstalled, { name: mod.name });
        return;
      }
      log.textContent = fmt(S.craftNeeds, { name: mod.name, deps: names(missing) });
      timers.push(
        setTimeout(() => {
          missing.forEach(dep => {
            installed.push(dep);
            deps.add(dep);
          });
          render();
          sound('pop');
          log.textContent = fmt(S.craftArrived, { deps: names(missing) });
        }, motion() ? 450 : 0),
      );
    }),
  );

  $('#craft-clear').addEventListener('click', () => {
    timers.forEach(clearTimeout);
    timers = [];
    installed = [];
    deps.clear();
    render();
    log.textContent = S.craftIdle;
  });

  render();
}

// ---------------------------------------------------------------- el doctor

const CUBE_TINTS = [
  'tint:#b48cff,#9466e6,#c9a8ff,#7d4fd0',
  'tint:#7ff3ea,#4fe3db,#b5fff9,#1f9e98',
  'tint:#ffd84a,#e0b02a,#fff09a,#a87a12',
  'tint:#7cf0b0,#3fc883,#b5ffd6,#1f8a55',
  'tint:#ff9a6a,#e0764a,#ffc09a,#a8502a',
  'tint:#8fb0ff,#5a82ff,#c2d2ff,#2d55d6',
  'tint:#e9e2f7,#c9c0dc,#ffffff,#9a90b0',
  'tint:#ff7ad9,#c94fb0,#ff9ff0,#8a2f78',
];
// El culpable es el doceavo, «Roto» (como en la prueba del doctor del ROADMAP).
const CULPRIT = 11;

function setupDoctor() {
  const grid = $('#doctor-grid');
  const status = $('#doctor-status');
  const start = $('#doctor-start');
  const fix = $('#doctor-fix');
  const meter = $$('#doctor-meter span');
  S.doctorMods.forEach((name, index) => {
    const cube = document.createElement('span');
    cube.className = 'mod-cube';
    cube.dataset.name = name;
    cube.dataset.disabled = S.disabled;
    cube.dataset.tip = `${name}|${S.modExample}`;
    const tint = CUBE_TINTS[index % CUBE_TINTS.length];
    cube.style.setProperty('--px', pxUrl(`iso:${tint}|${tint}`));
    grid.append(cube);
  });
  const cubes = $$('.mod-cube', grid);
  const ranges = [[8, 15], [8, 11], [10, 11], [CULPRIT, CULPRIT]];
  let timers = [];

  start.addEventListener('click', () => {
    timers.forEach(clearTimeout);
    timers = [];
    start.disabled = true;
    fix.hidden = true;
    cubes.forEach(cube => {
      cube.className = 'mod-cube';
    });
    meter.forEach(span => span.classList.remove('done'));
    status.textContent = S.doctorFirst;
    sound('click');
    ranges.forEach((range, index) => {
      timers.push(
        setTimeout(() => {
          const last = index === ranges.length - 1;
          cubes.forEach((cube, cubeIndex) => {
            const inside = cubeIndex >= range[0] && cubeIndex <= range[1];
            cube.className = `mod-cube ${inside ? (last ? 'culprit' : 'in') : 'out'}`;
          });
          meter[index].classList.add('done');
          status.textContent = S.doctorSteps[index];
          sound(last ? 'level' : 'hit');
          if (last) {
            fix.hidden = false;
            start.disabled = false;
            start.textContent = S.doctorAgain;
          }
        }, (index + 1) * (motion() ? 1000 : 120)),
      );
    });
  });

  fix.addEventListener('click', () => {
    cubes.forEach((cube, index) => {
      cube.className = index === CULPRIT ? 'mod-cube disabled' : 'mod-cube';
    });
    status.textContent = S.doctorDone;
    fix.hidden = true;
    toast(S.achievement, S.doctorAchievement, `iso:${CUBE_TINTS[4]}|${CUBE_TINTS[4]}`);
    sound('level');
  });
}

// ---------------------------------------------------------------- comunidades y soporte

function setupCommunity() {
  const joinBox = $('#cd-join');
  const card = $('#cd-card');
  const play = $('#cd-play');
  const steps = $$('#cd-steps li');
  const fill = $('#cd-fill');
  const status = $('#cd-status');
  let timers = [];

  $('#cd-join-btn').addEventListener('click', () => {
    joinBox.hidden = true;
    card.hidden = false;
    hydrate(card);
    play.focus();
    toast(S.achievement, S.communityJoined, 'sprite:shard');
    sound('level');
  });

  play.addEventListener('click', () => {
    timers.forEach(clearTimeout);
    timers = [];
    play.disabled = true;
    steps.forEach(step => (step.className = ''));
    fill.style.width = '0';
    const delay = motion() ? 1100 : 60;
    steps.forEach((step, index) => {
      timers.push(
        setTimeout(() => {
          if (index > 0) steps[index - 1].className = 'done';
          step.className = 'doing';
          status.textContent = S.communityStep[index];
          fill.style.width = `${((index + 0.5) / steps.length) * 100}%`;
          sound('click');
        }, index * delay),
      );
    });
    timers.push(
      setTimeout(() => {
        steps.forEach(step => (step.className = 'done'));
        fill.style.width = '100%';
        status.textContent = S.communityDone;
        play.disabled = false;
        toast(S.achievement, S.communityAchievement, 'iso:serverTop|serverSide');
        xpBurst(play, $('.xp'), 5);
        sound('level');
      }, steps.length * delay),
    );
  });

  const settingCard = $('#setting-card');
  const apply = $('#sc-apply');
  const undo = $('#sc-undo');
  const now = $('#sc-now');
  const original = now.textContent;
  apply.addEventListener('click', () => {
    settingCard.classList.add('applied');
    now.textContent = '6 GB';
    apply.hidden = true;
    undo.hidden = false;
    undo.focus();
    $('#sc-status').textContent = S.supportApplied;
    toast(S.supportToast, S.supportApplied, 'iso:craftTop|craftSide');
    sound('level');
  });
  undo.addEventListener('click', () => {
    settingCard.classList.remove('applied');
    now.textContent = original;
    undo.hidden = true;
    apply.hidden = false;
    apply.focus();
    $('#sc-status').textContent = S.supportUndone;
    sound('click');
  });
}

// ---------------------------------------------------------------- modo escuela

function setupSchool() {
  const toggle = $('#school-toggle');
  const stage = $('#school-stage');
  const flow = $('#school-flow');
  const svg = $('.paths', stage);
  let schoolOn = false;
  let stageVisible = false;
  let anchors = null;

  const anchor = (el, stageRect) => {
    const rect = el.getBoundingClientRect();
    return { x: rect.left - stageRect.left + rect.width / 2, y: rect.top - stageRect.top + rect.height / 2 };
  };
  const layoutPaths = () => {
    const stageRect = stage.getBoundingClientRect();
    if (!stageRect.width) return;
    anchors = {
      mojang: anchor($('.node-mojang .node-block', stage), stageRect),
      portal: anchor($('.node-portal .portal', stage), stageRect),
      pc: anchor($('.node-pc .node-block', stage), stageRect),
      barrier: anchor($('.barriers', stage), stageRect),
    };
    svg.setAttribute('viewBox', `0 0 ${stageRect.width} ${stageRect.height}`);
    const curve = (a, b) => `M${a.x} ${a.y} Q ${a.x} ${b.y} ${b.x} ${b.y}`;
    $('#path-mojang').setAttribute('d', curve(anchors.mojang, anchors.pc));
    $('#path-drive').setAttribute('d', curve(anchors.portal, anchors.pc));
    $('#path-mojang').classList.toggle('active', !schoolOn);
    $('#path-drive').classList.toggle('active', schoolOn);
  };

  const quad = (a, b, t) => {
    const c = { x: a.x, y: b.y };
    const u = 1 - t;
    return { x: u * u * a.x + 2 * u * t * c.x + t * t * b.x, y: u * u * a.y + 2 * u * t * c.y + t * t * b.y };
  };

  const ITEMS = ['iso:grassTop|grass', 'iso:planks|planks', 'sprite:book', 'iso:ore:stone:iron|ore:stone:iron'];
  let itemIndex = 0;
  const spawn = () => {
    if (!motion() || !stageVisible || !anchors) return;
    const item = document.createElement('i');
    item.style.setProperty('--px', pxUrl(ITEMS[itemIndex++ % ITEMS.length]));
    flow.append(item);
    const source = schoolOn ? anchors.portal : anchors.mojang;
    const frames = [0, 0.25, 0.5, 0.75, 1].map(t => {
      const p = quad(source, anchors.pc, t);
      return { transform: `translate(${p.x - 14}px, ${p.y - 14}px)` };
    });
    frames[0].opacity = 0;
    frames[1].opacity = 1;
    frames[4].opacity = 0;
    item.animate(frames, { duration: 2200, easing: 'linear' }).onfinish = () => item.remove();

    // Con el modo escuela, lo que intenta venir de Mojang choca con la barrera.
    if (schoolOn && itemIndex % 2 === 0) {
      const blocked = document.createElement('i');
      blocked.style.setProperty('--px', pxUrl(ITEMS[(itemIndex + 1) % ITEMS.length]));
      flow.append(blocked);
      blocked.animate(
        [
          { transform: `translate(${anchors.mojang.x - 14}px, ${anchors.mojang.y - 14}px)`, opacity: 0 },
          { transform: `translate(${anchors.barrier.x - 30}px, ${anchors.barrier.y - 14}px)`, opacity: 1 },
        ],
        { duration: 900, easing: 'ease-in' },
      ).onfinish = () => {
        burst(blocked, ['#e8262d', '#ff6a6f', '#5a5a60'], 6, { size: 5 });
        blocked.remove();
      };
    }
  };
  setInterval(spawn, 900);

  new IntersectionObserver(entries => {
    stageVisible = entries[0].isIntersecting;
    if (stageVisible) layoutPaths();
  }).observe(stage);
  new ResizeObserver(layoutPaths).observe(stage);

  toggle.addEventListener('click', () => {
    schoolOn = toggle.getAttribute('aria-checked') !== 'true';
    toggle.setAttribute('aria-checked', String(schoolOn));
    body.classList.toggle('school-mode', schoolOn);
    $('#school-state').textContent = schoolOn ? S.schoolOn : S.schoolOff;
    $('#mojang-status').textContent = schoolOn ? S.mojangBlocked : S.mojangNormal;
    $('#drive-status').textContent = schoolOn ? S.driveOn : S.driveOff;
    $('#pc-status').textContent = schoolOn ? S.pcDrive : S.pcMojang;
    $('#school-quip').textContent = schoolOn ? S.quipOn : S.quipOff;
    $('#path-mojang').classList.toggle('active', !schoolOn);
    $('#path-drive').classList.toggle('active', schoolOn);
    $('meta[name="theme-color"]').setAttribute('content', schoolOn ? '#0d2a28' : '#120a28');
    sound(schoolOn ? 'level' : 'click');
    if (schoolOn) toast(S.schoolTitle, S.schoolToast, 'sprite:barrier');
  });
}

// ---------------------------------------------------------------- encabezado y teclado

function setupChrome() {
  let splashIndex = 0;
  $('#splash').addEventListener('click', () => {
    splashIndex = (splashIndex + 1) % S.splash.length;
    $('#splash').textContent = S.splash[splashIndex];
    sound('pop');
  });

  const pause = $('#pause-motion');
  pause.addEventListener('click', () => {
    state.paused = !state.paused;
    body.classList.toggle('paused', state.paused);
    const label = state.paused ? S.header.resume : S.header.pause;
    pause.setAttribute('aria-pressed', String(state.paused));
    pause.setAttribute('aria-label', label);
    pause.title = label;
  });

  const soundButton = $('#sound-toggle');
  soundButton.addEventListener('click', () => {
    state.sound = !state.sound;
    const label = state.sound ? S.header.soundOff : S.header.soundOn;
    soundButton.setAttribute('aria-pressed', String(state.sound));
    soundButton.setAttribute('aria-label', label);
    soundButton.title = label;
    if (state.sound) sound('msg');
  });

  // Al cambiar de idioma se queda en la misma capa (los id son iguales en los dos idiomas).
  const langLink = $('#lang-link');
  const base = langLink.getAttribute('href');
  langLink.addEventListener('click', () => {
    langLink.href = base + (state.active && state.active !== hero ? `#${state.active.id}` : '');
  });

  $$('.faq-list details').forEach(details => details.addEventListener('toggle', () => sound('click')));

  document.addEventListener('keydown', event => {
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    const active = document.activeElement;
    if (active && /INPUT|TEXTAREA|SELECT/.test(active.tagName) && active.type !== 'checkbox') return;
    const key = event.key.toLowerCase();
    if (key === 'o') {
      event.preventDefault();
      const open = panel.hidden;
      if (open) {
        const rect = game.getBoundingClientRect();
        if (rect.top < 0 || rect.bottom > innerHeight) game.scrollIntoView({ behavior: motion() ? 'smooth' : 'auto', block: 'center' });
      }
      setPanel(open);
    } else if (event.key === 'Escape') {
      if (!panel.hidden) setPanel(false);
      setBiomes(false);
      hideTip();
    } else if (/^[1-9]$/.test(event.key)) {
      const link = hotbar.children[Number(event.key) - 1];
      if (link) {
        event.preventDefault();
        hotbar.classList.remove('tucked');
        link.click();
      }
    }
  });

  reducedQuery.addEventListener('change', event => {
    state.reduced = event.matches;
    body.classList.toggle('reduced', state.reduced);
  });
}

// ---------------------------------------------------------------- pausar lo que no se ve

function observeVisibility() {
  const observer = new IntersectionObserver(
    entries => {
      entries.forEach(entry => {
        entry.target.classList.toggle('is-offscreen', !entry.isIntersecting);
        if (entry.target === hero) heroVisible = entry.isIntersecting;
      });
    },
    { rootMargin: '150px' },
  );
  sections.forEach(section => observer.observe(section));
}

// ---------------------------------------------------------------- arranque

function relayout() {
  const width = document.documentElement.clientWidth;
  if (width === state.width) return;
  state.width = width;
  applyMetrics();
  renderHero();
  layoutMobs();
  layers.forEach(section => renderLayer(section, true));
  requestFrame();
}

function init() {
  applyMetrics();
  state.width = state.metrics.width;
  hydrate();
  buildNavigation();
  renderHero();
  layers.forEach(section => renderLayer(section));
  layers.forEach(section => layerObserver.observe(section));
  setupMobs();
  setupDropLogo();
  $$('button.mine[data-block]').forEach(setupMine);
  setupLauncher();
  setupGame();
  setupFriends();
  setupHosting();
  setupCrafting();
  setupDoctor();
  setupCommunity();
  setupSchool();
  setupChrome();
  observeVisibility();
  refreshReleases(S.repo);

  addEventListener('scroll', requestFrame, { passive: true });
  let resizeTimer;
  addEventListener('resize', () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(relayout, 160);
    requestFrame();
  });
  document.fonts?.ready.then(() => {
    layers.forEach(scheduleRender);
    renderHero();
    requestFrame();
  });
  onFrame();
  requestAnimationFrame(mobLoop);
  setTimeout(() => {
    state.ready = true;
  }, 700);
}

init();
