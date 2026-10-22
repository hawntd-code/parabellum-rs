// Arte pixel de la landing, hecho con código: texturas de 16 × 16, bloques isométricos,
// criaturas, objetos y el corte del mundo de cada capa. Todo sale de un generador con
// semilla, así que se ve igual en cada visita y no hay imágenes que descargar.
// Es arte original con estética de bloques: no usa texturas, logos ni fuentes de Minecraft.

const TILE = 16;
const cache = new Map();
const urls = new WeakMap();

// ---------------------------------------------------------------- utilidades

function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function hash(text) {
  let h = 2166136261;
  for (const ch of String(text)) {
    h ^= ch.codePointAt(0);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function makeCanvas(width, height) {
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(width));
  canvas.height = Math.max(1, Math.round(height));
  canvas.getContext('2d').imageSmoothingEnabled = false;
  return canvas;
}

const randomInt = (random, n) => Math.floor(random() * n);
const clamp = (value, min, max) => Math.min(max, Math.max(min, value));

function pick(random, colors, weights) {
  if (!weights) return colors[randomInt(random, colors.length)];
  let total = 0;
  for (const weight of weights) total += weight;
  let roll = random() * total;
  for (let i = 0; i < colors.length; i++) {
    roll -= weights[i];
    if (roll < 0) return colors[i];
  }
  return colors[colors.length - 1];
}

function px(ctx, x, y, color) {
  ctx.fillStyle = color;
  ctx.fillRect(x, y, 1, 1);
}

function rect(ctx, x, y, w, h, color) {
  ctx.fillStyle = color;
  ctx.fillRect(x, y, w, h);
}

function noiseRect(ctx, random, x0, y0, w, h, colors, weights) {
  for (let y = y0; y < y0 + h; y++) {
    for (let x = x0; x < x0 + w; x++) px(ctx, x, y, pick(random, colors, weights));
  }
}

function fillNoise(ctx, random, colors, weights) {
  noiseRect(ctx, random, 0, 0, TILE, TILE, colors, weights);
}

function blotches(ctx, random, color, count) {
  for (let i = 0; i < count; i++) {
    const x = randomInt(random, 15);
    const y = randomInt(random, 15);
    px(ctx, x, y, color);
    if (random() < 0.7) px(ctx, x + 1, y, color);
    if (random() < 0.5) px(ctx, x, y + 1, color);
  }
}

// Multiplica un color hexadecimal: < 1 oscurece, > 1 aclara.
function adjust(hex, factor) {
  const n = parseInt(hex.slice(1, 7), 16);
  const channel = shift => clamp(Math.round(((n >> shift) & 255) * factor), 0, 255);
  return `rgb(${channel(16)},${channel(8)},${channel(0)})`;
}

// Un valor suave por posición: nudos al azar cada `step` y recta entre ellos.
function wave(length, step, random) {
  const knots = [];
  for (let i = 0; i <= Math.ceil(length / step) + 1; i++) knots.push(random());
  const values = [];
  for (let i = 0; i < length; i++) {
    const k = Math.floor(i / step);
    const t = (i % step) / step;
    values.push(knots[k] * (1 - t) + knots[k + 1] * t);
  }
  return values;
}

// ---------------------------------------------------------------- texturas

const PALETTES = {
  dirt: ['#7a553a', '#6b4a31', '#8b6443', '#5d4029', '#9b7352'],
  grass: ['#6cbc3d', '#5ba633', '#80d04a', '#4d902b'],
  stone: ['#818188', '#75757c', '#8c8c93', '#6b6b72', '#97979e'],
  gravel: ['#857c78', '#716966', '#978e8a', '#605956', '#a39a95'],
  deepslate: ['#4d4d55', '#45454d', '#56565e', '#3c3c43', '#5e5e66'],
  bedrock: ['#3b3b3e', '#58585b', '#202022', '#707073', '#2c2c2e'],
  obsidian: ['#150c22', '#1e1131', '#28173f', '#0f0819'],
  calcite: ['#dedfd9', '#d0d1cb', '#e9eae4', '#c4c5bf'],
  amethyst: ['#7d4fd0', '#6a3cbd', '#9466e6', '#5a2fa6', '#b48cff'],
  basalt: ['#4a4a50', '#525258', '#424248', '#5a5a60'],
  leaves: ['#3f7d2a', '#336b21', '#4c9234', '#2a5a1b'],
  log: ['#6b4f2c', '#5b4224', '#7c5d35', '#4d371d'],
  planks: ['#a27a4a', '#956f42', '#b0875a'],
  lava: ['#ff9a1f', '#ff6a00', '#ffc23f', '#e84a00', '#ffd86a'],
  water: ['#3a5fd9', '#2f50c4', '#4a72ee', '#5b84ff'],
  cobble: ['#7b7b82', '#6a6a71', '#8d8d94', '#5a5a61']
};

const ORES = {
  coal: ['#2c2c30', '#4b4b50', '#141416'],
  iron: ['#d8a98a', '#f0cdb2', '#a8775a'],
  copper: ['#e07b45', '#f5a070', '#3e9e86'],
  gold: ['#f5d742', '#fff3a0', '#c79a1c'],
  redstone: ['#e8241f', '#ff6a5e', '#8f0c0c'],
  lapis: ['#2d55d6', '#5a82ff', '#16308f'],
  diamond: ['#4fe3db', '#b5fff9', '#1f9e98'],
  emerald: ['#2bd36a', '#8cffb4', '#118a3e']
};

const PAINTERS = {
  dirt(ctx, r) {
    fillNoise(ctx, r, PALETTES.dirt, [4, 4, 2, 2, 1]);
    blotches(ctx, r, '#4f3622', 3);
  },
  grass(ctx, r) {
    PAINTERS.dirt(ctx, r);
    for (let x = 0; x < TILE; x++) {
      const depth = 2 + (r() < 0.55 ? 1 : 0) + (r() < 0.22 ? 1 : 0);
      for (let y = 0; y < depth; y++) px(ctx, x, y, pick(r, PALETTES.grass, [4, 4, 2, 2]));
    }
  },
  grassTop(ctx, r) {
    fillNoise(ctx, r, PALETTES.grass, [4, 4, 2, 2]);
  },
  stone(ctx, r) {
    fillNoise(ctx, r, PALETTES.stone, [4, 4, 3, 2, 1]);
    blotches(ctx, r, '#64646b', 4);
  },
  cobble(ctx, r) {
    fillNoise(ctx, r, PALETTES.cobble, [3, 3, 2, 2]);
    for (let i = 0; i < 7; i++) {
      const x = randomInt(r, 13);
      const y = randomInt(r, 13);
      const w = 2 + randomInt(r, 3);
      rect(ctx, x, y, w, 1, '#4d4d53');
      rect(ctx, x, y, 1, 2 + randomInt(r, 2), '#4d4d53');
    }
  },
  gravel(ctx, r) {
    for (let y = 0; y < TILE; y += 2) {
      for (let x = 0; x < TILE; x += 2) {
        rect(ctx, x, y, 2, 2, pick(r, PALETTES.gravel, [4, 4, 3, 2, 1]));
        if (r() < 0.35) px(ctx, x + 1, y + 1, '#4f4946');
      }
    }
  },
  deepslate(ctx, r) {
    fillNoise(ctx, r, PALETTES.deepslate, [4, 4, 2, 3, 1]);
    for (let y = 1; y < TILE; y += 4) {
      for (let x = 0; x < TILE; x++) if (r() < 0.55) px(ctx, x, y, '#37373e');
    }
  },
  bedrock(ctx, r) {
    fillNoise(ctx, r, PALETTES.bedrock, [3, 3, 2, 1, 2]);
  },
  obsidian(ctx, r) {
    fillNoise(ctx, r, PALETTES.obsidian, [4, 3, 2, 3]);
    for (let i = 0; i < 6; i++) px(ctx, randomInt(r, TILE), randomInt(r, TILE), '#4a2a73');
  },
  calcite(ctx, r) {
    fillNoise(ctx, r, PALETTES.calcite, [4, 3, 2, 1]);
  },
  amethyst(ctx, r) {
    for (let y = 0; y < TILE; y++) {
      for (let x = 0; x < TILE; x++) {
        const band = (x + y + randomInt(r, 2)) % 6;
        px(ctx, x, y, band === 0 ? '#b48cff' : band === 1 ? '#9466e6' : pick(r, PALETTES.amethyst, [4, 4, 1, 3, 0]));
      }
    }
  },
  basalt(ctx, r) {
    fillNoise(ctx, r, PALETTES.basalt, [4, 3, 3, 1]);
  },
  leaves(ctx, r) {
    for (let y = 0; y < TILE; y++) {
      for (let x = 0; x < TILE; x++) {
        if (r() < 0.12) continue;
        px(ctx, x, y, pick(r, PALETTES.leaves, [4, 3, 2, 2]));
      }
    }
  },
  log(ctx, r) {
    for (let x = 0; x < TILE; x++) {
      const base = pick(r, PALETTES.log, [4, 3, 2, 1]);
      for (let y = 0; y < TILE; y++) px(ctx, x, y, r() < 0.72 ? base : pick(r, PALETTES.log));
    }
  },
  planks(ctx, r) {
    for (let plank = 0; plank < 4; plank++) {
      const base = pick(r, PALETTES.planks);
      for (let y = plank * 4; y < plank * 4 + 4; y++) {
        for (let x = 0; x < TILE; x++) {
          px(ctx, x, y, y === plank * 4 + 3 ? '#6f4f2c' : r() < 0.18 ? adjust(base, 0.9) : base);
        }
      }
      px(ctx, (plank * 5 + 3) % TILE, plank * 4 + 1, '#6f4f2c');
    }
  },
  lava(ctx, r) {
    fillNoise(ctx, r, PALETTES.lava, [4, 3, 3, 1, 1]);
    blotches(ctx, r, '#ffe28a', 3);
  },
  water(ctx, r) {
    fillNoise(ctx, r, PALETTES.water, [4, 3, 2, 1]);
  },
  ladder(ctx) {
    rect(ctx, 2, 0, 2, TILE, '#7a5a32');
    rect(ctx, 12, 0, 2, TILE, '#7a5a32');
    rect(ctx, 3, 0, 1, TILE, '#5e4424');
    rect(ctx, 13, 0, 1, TILE, '#5e4424');
    for (let y = 1; y < TILE; y += 4) {
      rect(ctx, 2, y, 12, 2, '#9a7243');
      rect(ctx, 2, y + 2, 12, 1, '#5e4424');
    }
  },
  lampOff(ctx, r) {
    fillNoise(ctx, r, ['#6a4529', '#5b3a22', '#4b2f1b'], [3, 3, 2]);
    lampGrid(ctx, '#3a2414');
  },
  lampOn(ctx, r) {
    fillNoise(ctx, r, ['#ffd889', '#f6c35c', '#fff0b8', '#e8a33e'], [3, 3, 2, 1]);
    lampGrid(ctx, '#a8611f');
  },
  craftTop(ctx, r) {
    PAINTERS.planks(ctx, r);
    rect(ctx, 0, 0, TILE, 1, '#4e3420');
    rect(ctx, 0, 15, TILE, 1, '#4e3420');
    rect(ctx, 0, 0, 1, TILE, '#4e3420');
    rect(ctx, 15, 0, 1, TILE, '#4e3420');
    for (const v of [5, 10]) {
      rect(ctx, v, 1, 1, 14, '#5d3f25');
      rect(ctx, 1, v, 14, 1, '#5d3f25');
    }
  },
  craftSide(ctx, r) {
    PAINTERS.planks(ctx, r);
    rect(ctx, 0, 0, TILE, 3, '#4e3420');
    rect(ctx, 3, 6, 4, 6, '#9b9ba3');
    rect(ctx, 3, 6, 4, 1, '#c9c9d0');
    rect(ctx, 10, 5, 2, 8, '#6b4a2b');
    rect(ctx, 9, 5, 4, 2, '#7d7d86');
  },
  serverSide(ctx, r) {
    fillNoise(ctx, r, ['#2f2b3a', '#28242f', '#363143'], [3, 3, 2]);
    for (let y = 2; y < TILE; y += 4) {
      rect(ctx, 2, y, 12, 2, '#1b1822');
      px(ctx, 12, y, '#3dd68c');
      px(ctx, 10, y, '#7f50f2');
    }
  },
  serverTop(ctx, r) {
    fillNoise(ctx, r, ['#3b3647', '#34303f', '#423c50'], [3, 3, 2]);
    rect(ctx, 3, 3, 10, 10, '#2a2633');
    rect(ctx, 6, 6, 4, 4, '#7f50f2');
  }
};

function lampGrid(ctx, color) {
  rect(ctx, 0, 0, TILE, 1, color);
  rect(ctx, 0, 15, TILE, 1, color);
  rect(ctx, 0, 0, 1, TILE, color);
  rect(ctx, 15, 0, 1, TILE, color);
  rect(ctx, 7, 1, 2, 14, color);
  rect(ctx, 1, 7, 14, 2, color);
}

const ORE_SHAPES = [
  [[0, 0], [1, 0], [0, 1], [1, 1]],
  [[0, 0], [1, 0], [1, 1], [2, 1]],
  [[1, 0], [0, 1], [1, 1], [2, 1], [1, 2]],
  [[0, 0], [1, 1], [2, 1]],
  [[0, 1], [1, 0], [1, 1], [2, 0]]
];

function paintOre(ctx, random, base, kind) {
  PAINTERS[base](ctx, random);
  const [mid, light, dark] = ORES[kind];
  const count = 4 + randomInt(random, 2);
  for (let i = 0; i < count; i++) {
    const cx = 1 + randomInt(random, 12);
    const cy = 1 + randomInt(random, 12);
    const shape = ORE_SHAPES[randomInt(random, ORE_SHAPES.length)];
    shape.forEach(([dx, dy], index) => px(ctx, cx + dx, cy + dy, index === 0 ? light : mid));
    const [lx, ly] = shape[shape.length - 1];
    px(ctx, clamp(cx + lx + 1, 0, 15), clamp(cy + ly, 0, 15), dark);
  }
}

// «tint:#a,#b,#c,#d»: un bloque de un solo color con textura, para portadas e íconos.
function paintTint(ctx, random, colors) {
  fillNoise(ctx, random, colors, [5, 3, 2, 1].slice(0, colors.length));
  for (let i = 0; i < 4; i++) {
    const x = randomInt(random, 7) * 2;
    const y = randomInt(random, 7) * 2;
    rect(ctx, x, y, 2, 2, colors[2] || colors[0]);
  }
}

function texture(name, variant = 0) {
  const key = `t|${name}|${variant}`;
  if (!cache.has(key)) {
    const canvas = makeCanvas(TILE, TILE);
    const ctx = canvas.getContext('2d');
    const random = mulberry32(hash(name) + variant * 7919);
    if (name.startsWith('ore:')) {
      const [, base, kind] = name.split(':');
      paintOre(ctx, random, base, kind);
    } else if (name.startsWith('tint:')) {
      paintTint(ctx, random, name.slice(5).split(','));
    } else if (PAINTERS[name]) {
      PAINTERS[name](ctx, random);
    } else {
      throw new Error(`Textura desconocida: ${name}`);
    }
    cache.set(key, canvas);
  }
  return cache.get(key);
}

function shadeCanvas(source, factor, key) {
  if (key && cache.has(key)) return cache.get(key);
  const canvas = makeCanvas(source.width, source.height);
  const ctx = canvas.getContext('2d');
  ctx.drawImage(source, 0, 0);
  const data = ctx.getImageData(0, 0, canvas.width, canvas.height);
  for (let i = 0; i < data.data.length; i += 4) {
    data.data[i] = Math.min(255, data.data[i] * factor);
    data.data[i + 1] = Math.min(255, data.data[i + 1] * factor);
    data.data[i + 2] = Math.min(255, data.data[i + 2] * factor);
  }
  ctx.putImageData(data, 0, 0);
  if (key) cache.set(key, canvas);
  return canvas;
}

function shaded(name, variant, factor) {
  return shadeCanvas(texture(name, variant), factor, `s|${name}|${variant}|${factor}`);
}

// Los colores con los que estalla un bloque al romperse.
function colorsOf(name) {
  if (name.startsWith('ore:')) {
    const [, base, kind] = name.split(':');
    return [ORES[kind][0], ORES[kind][1], PALETTES[base][0], PALETTES[base][1]];
  }
  if (name.startsWith('tint:')) return name.slice(5).split(',');
  return PALETTES[name] || ['#888888', '#666666'];
}

// ---------------------------------------------------------------- grietas al minar

const CRACK_DIRECTIONS = [[1, 0], [-1, -1], [0, 1], [-1, 1], [1, -1], [-1, 0], [0, -1], [1, 1]];
let crackStrokes = null;

function crack(stage) {
  const key = `crack|${stage}`;
  if (cache.has(key)) return cache.get(key);
  if (!crackStrokes) {
    const random = mulberry32(4242);
    crackStrokes = CRACK_DIRECTIONS.map(([dx, dy], index) => {
      let x = 7 + randomInt(random, 3);
      let y = 7 + randomInt(random, 3);
      const points = [[x, y]];
      for (let k = 0; k < 3 + index; k++) {
        x = clamp(x + dx + (random() < 0.35 ? randomInt(random, 3) - 1 : 0), 0, 15);
        y = clamp(y + dy + (random() < 0.35 ? randomInt(random, 3) - 1 : 0), 0, 15);
        points.push([x, y]);
      }
      return points;
    });
  }
  const canvas = makeCanvas(TILE, TILE);
  const ctx = canvas.getContext('2d');
  const strokes = Math.min(CRACK_DIRECTIONS.length, Math.round(stage * 1.6));
  for (let s = 0; s < strokes; s++) {
    crackStrokes[s].forEach(([x, y]) => px(ctx, x, y, 'rgba(12,8,18,.85)'));
  }
  cache.set(key, canvas);
  return canvas;
}

// ---------------------------------------------------------------- bloque isométrico

// Dibuja las tres caras visibles de un bloque con transformaciones afines del canvas.
function iso(top, side, size = 64) {
  const key = `iso|${top}|${side}|${size}`;
  if (cache.has(key)) return cache.get(key);
  const canvas = makeCanvas(size, size);
  const ctx = canvas.getContext('2d');
  const S = size;
  ctx.imageSmoothingEnabled = false;
  ctx.setTransform(S / 32, -S / 64, S / 32, S / 64, 0, S / 4);
  ctx.drawImage(texture(top), 0, 0);
  ctx.setTransform(S / 32, S / 64, 0, S / 32, 0, S / 4);
  ctx.drawImage(shaded(side, 0, 0.8), 0, 0);
  ctx.setTransform(S / 32, -S / 64, 0, S / 32, S / 2, S / 2);
  ctx.drawImage(shaded(side, 0, 0.6), 0, 0);
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  cache.set(key, canvas);
  return canvas;
}

// ---------------------------------------------------------------- sprites con mapa

function fromMap(rows, palette) {
  const canvas = makeCanvas(rows[0].length, rows.length);
  const ctx = canvas.getContext('2d');
  rows.forEach((row, y) => {
    [...row].forEach((char, x) => {
      if (palette[char]) px(ctx, x, y, palette[char]);
    });
  });
  return canvas;
}

const GEM_ROWS = [
  '....oooo....',
  '...ollllo...',
  '..olwllmmo..',
  '.olwllmmmdo.',
  'olllmmmmmddo',
  '.oddmmmmddo.',
  '..oddmmddo..',
  '...oddddo...',
  '....oddo....',
  '.....oo.....'
];

const LUMP_ROWS = [
  '..oooo..',
  '.ommmmo.',
  'omlmmmdo',
  'ommmmmdo',
  'ommmdddo',
  '.odddddo',
  '..oooo..'
];

const MAPS = {
  heart: {
    rows: ['.rr.rr.', 'rwrrrrr', 'rrrrrrr', '.rrrrr.', '..rrr..', '...r...'],
    palette: { r: '#e8242b', w: '#ff9a9a' }
  },
  halfHeart: {
    rows: ['.rr.oo.', 'rwrrooo', 'rrrrooo', '.rrroo.', '..rro..', '...r...'],
    palette: { r: '#e8242b', w: '#ff9a9a', o: '#3a1418' }
  },
  pickaxe: {
    rows: [
      '.hhhhhhhhh.',
      'hHHHHHHHHHh',
      'hHh..s..hHh',
      'hh...s...hh',
      '.....s.....',
      '.....s.....',
      '.....s.....',
      '.....s.....',
      '.....s.....',
      '.....b.....'
    ],
    palette: { h: '#1d3b45', H: '#7ff3ea', s: '#8a5d2c', b: '#5e3c1b' }
  },
  torch: {
    rows: ['..w..', '.yoy.', '.yYy.', '..o..', '..s..', '..s..', '..s..', '..s..', '..S..', '..S..'],
    palette: { w: '#fff6c8', y: '#ffd23f', o: '#ff8a1f', Y: '#ffe680', s: '#8a6a3a', S: '#5a3f1f' }
  },
  book: {
    rows: [
      '.oooooooooo.',
      'oCCCCCCCCCpo',
      'oCggggggCCpo',
      'oCCCCCCCCCpo',
      'oCCgggggCCpo',
      'oCCCCCCCCCpo',
      'oCCCCCCCCCpo',
      'oCCCCLLCCCpo',
      'oCCCCLLCCCpo',
      'oCCCCCCCCCpo',
      'oDDDDDDDDDpo',
      '.oooooooooo.'
    ],
    palette: { o: '#2a1a3d', C: '#6a3cc9', D: '#4c2a99', g: '#ffd84a', p: '#efe6d0', L: '#b79bff' }
  },
  shard: {
    rows: [
      '......oo..',
      '.....oLlo.',
      '....oLlmo.',
      '...oLlmdo.',
      '..oLlmdo..',
      '.oLlmdo...',
      'oLlmdo....',
      'olmdo.....',
      'oddo......',
      '.oo.......'
    ],
    palette: { o: '#2c1554', L: '#f1dcff', l: '#c8a2ff', m: '#9a6ae6', d: '#6a3cbd' }
  },
  bud: {
    rows: ['...o....', '..oLo.o.', '..olo.Lo', '.oLmooLo', '.olmdolo', 'oLmddmdo', 'oommddoo'],
    palette: { o: '#2c1554', L: '#f1dcff', l: '#c8a2ff', m: '#9a6ae6', d: '#6a3cbd' }
  },
  poppy: {
    rows: ['..rr...', '.rRrr..', '.rrrr..', '..rr...', '...g...', '..gg...', '...g.g.', '...gg..', '...g...', '...g...'],
    palette: { r: '#d6322b', R: '#ff6b5e', g: '#3f8f2a' }
  },
  dandelion: {
    rows: ['..yy...', '.yYyy..', '..yy...', '...g...', '..gg...', '...gg..', '...g...', '...g...'],
    palette: { y: '#ffd23f', Y: '#fff08a', g: '#3f8f2a' }
  },
  tuft: {
    rows: ['...g....', '.g.g..g.', '.g.gg.g.', 'gg.g.gg.', '.ggg.g..', '..gggg..'],
    palette: { g: '#4f9b32' }
  },
  feather: {
    rows: ['.w', 'ww', 'w.'],
    palette: { w: '#f4f4f4' }
  },
  orb: {
    rows: ['.yyy.', 'yGGgy', 'yGgGy', 'ygGGy', '.yyy.'],
    palette: { y: '#5c9d10', G: '#c8ff5a', g: '#80ff20' }
  }
};

const GEMS = {
  diamond: { o: '#0f3b3a', l: '#a8fff8', m: '#4fe3db', d: '#1f9e98', w: '#ffffff' },
  emerald: { o: '#0b3b1d', l: '#9dffbf', m: '#2bd36a', d: '#118a3e', w: '#ffffff' },
  lapis: { o: '#0c1a52', l: '#8fb0ff', m: '#2d55d6', d: '#16308f', w: '#dce6ff' },
  gold: { o: '#4d3a05', l: '#fff3a0', m: '#f5d742', d: '#c79a1c', w: '#ffffff' },
  redstone: { o: '#3d0505', l: '#ff8f86', m: '#e8241f', d: '#8f0c0c', w: '#ffd2cf' },
  amethyst: { o: '#2c1554', l: '#f1dcff', m: '#9a6ae6', d: '#6a3cbd', w: '#ffffff' }
};

const LUMPS = {
  coal: { o: '#0c0c0e', m: '#2c2c30', l: '#55555b', d: '#18181a' },
  iron: { o: '#4d2f1f', m: '#d8a98a', l: '#f0cdb2', d: '#a8775a' },
  copper: { o: '#4a2310', m: '#e07b45', l: '#f5a070', d: '#a8532a' }
};

// ---------------------------------------------------------------- sprites dibujados

function paint(width, height, seed, drawer) {
  const canvas = makeCanvas(width, height);
  drawer(canvas.getContext('2d'), mulberry32(seed));
  return canvas;
}

function sheep(frame, wool) {
  return paint(20, 14, 11, (ctx, r) => {
    const skin = '#d8b294';
    const woolColors = [wool, adjust(wool, 0.93), adjust(wool, 1.04), adjust(wool, 0.86)];
    [3, 6, 12, 15].forEach((x, i) => {
      const lift = i % 2 === frame ? 1 : 0;
      rect(ctx, x, 10, 2, 4 - lift, skin);
      rect(ctx, x, 10, 2, 1, adjust(wool, 0.88));
    });
    noiseRect(ctx, r, 1, 3, 15, 8, woolColors, [4, 3, 2, 1]);
    noiseRect(ctx, r, 13, 1, 6, 3, woolColors, [4, 3, 2, 1]);
    rect(ctx, 14, 4, 6, 5, skin);
    rect(ctx, 14, 8, 6, 1, '#c09a7c');
    px(ctx, 17, 5, '#ffffff');
    px(ctx, 18, 5, '#1b1b1b');
    px(ctx, 19, 7, '#b07a6a');
  });
}

function pig(frame) {
  return paint(18, 12, 12, (ctx, r) => {
    const pinks = ['#f0a5a2', '#e8999a', '#f7b2ae', '#df8f8e'];
    [2, 5, 10, 13].forEach((x, i) => {
      const lift = i % 2 === frame ? 1 : 0;
      rect(ctx, x, 9, 2, 3 - lift, '#e08f8f');
    });
    noiseRect(ctx, r, 1, 3, 13, 6, pinks, [4, 3, 2, 1]);
    noiseRect(ctx, r, 12, 1, 6, 8, pinks, [4, 3, 2, 1]);
    rect(ctx, 16, 4, 2, 3, '#e88b8b');
    px(ctx, 17, 4, '#9c4a4a');
    px(ctx, 17, 6, '#9c4a4a');
    px(ctx, 14, 3, '#ffffff');
    px(ctx, 15, 3, '#1b1b1b');
  });
}

function chicken(frame) {
  return paint(12, 13, 13, (ctx, r) => {
    const whites = ['#f4f4f4', '#e6e6e6', '#ffffff'];
    rect(ctx, 4, 10, 1, frame ? 2 : 3, '#f2b33d');
    rect(ctx, 7, 10, 1, frame ? 3 : 2, '#f2b33d');
    noiseRect(ctx, r, 1, 5, 8, 5, whites);
    rect(ctx, 0, 4, 2, 2, '#e3e3e3');
    rect(ctx, 3, 6, 4, 3, frame ? '#d4d4d4' : '#dcdcdc');
    noiseRect(ctx, r, 6, 0, 4, 5, whites);
    rect(ctx, 10, 2, 2, 1, '#f2b33d');
    px(ctx, 10, 3, '#d99a22');
    rect(ctx, 10, 4, 1, 2, '#d6322b');
    px(ctx, 8, 1, '#1b1b1b');
  });
}

function creeper() {
  return paint(8, 16, 14, (ctx, r) => {
    const greens = ['#5fbf52', '#4ea843', '#73d566', '#3c8a33', '#8fe082'];
    noiseRect(ctx, r, 0, 0, 8, 8, greens, [4, 4, 2, 2, 1]);
    noiseRect(ctx, r, 2, 8, 4, 5, greens, [4, 4, 2, 2, 1]);
    noiseRect(ctx, r, 0, 13, 3, 3, greens, [4, 4, 2, 2, 1]);
    noiseRect(ctx, r, 5, 13, 3, 3, greens, [4, 4, 2, 2, 1]);
    const face = '#141414';
    rect(ctx, 1, 2, 2, 2, face);
    rect(ctx, 5, 2, 2, 2, face);
    rect(ctx, 3, 4, 2, 3, face);
    rect(ctx, 2, 5, 1, 3, face);
    rect(ctx, 5, 5, 1, 3, face);
  });
}

// Minero original de Parabellum: casco morado con lámpara. Frente, o de espaldas en la escalera.
function miner(frame, shirt = '#5f35c9') {
  return paint(12, 20, 15, ctx => {
    const skin = '#e0a77c';
    const helmet = '#7f50f2';
    const pants = '#2e3a66';
    const boots = '#3a2a1e';
    if (frame === 'front') {
      rect(ctx, 3, 0, 6, 2, helmet);
      rect(ctx, 5, 0, 2, 1, '#ffe066');
      rect(ctx, 3, 2, 6, 5, skin);
      rect(ctx, 3, 2, 6, 1, '#5b3a22');
      px(ctx, 4, 4, '#ffffff');
      px(ctx, 5, 4, '#2b1a4a');
      px(ctx, 7, 4, '#2b1a4a');
      px(ctx, 8, 4, '#ffffff');
      rect(ctx, 5, 6, 2, 1, '#b97a55');
      rect(ctx, 3, 7, 6, 6, shirt);
      rect(ctx, 5, 8, 2, 2, '#b79bff');
      rect(ctx, 1, 7, 2, 5, shirt);
      rect(ctx, 9, 7, 2, 5, shirt);
      rect(ctx, 1, 12, 2, 1, skin);
      rect(ctx, 9, 12, 2, 1, skin);
      rect(ctx, 3, 13, 6, 5, pants);
      rect(ctx, 5, 15, 2, 3, '#232c50');
      rect(ctx, 3, 18, 3, 2, boots);
      rect(ctx, 6, 18, 3, 2, boots);
      return;
    }
    const a = frame === 'climbA';
    rect(ctx, 3, 0, 6, 2, helmet);
    rect(ctx, 3, 2, 6, 5, '#5b3a22');
    rect(ctx, 3, 7, 6, 6, shirt);
    rect(ctx, 4, 8, 4, 4, adjust(shirt, 0.82));
    const up = a ? 1 : 9;
    const down = a ? 9 : 1;
    rect(ctx, up, 2, 2, 7, shirt);
    rect(ctx, up, 1, 2, 1, skin);
    rect(ctx, down, 7, 2, 6, shirt);
    rect(ctx, down, 13, 2, 1, skin);
    rect(ctx, 3, 13, 3, a ? 5 : 4, pants);
    rect(ctx, 6, 13, 3, a ? 4 : 5, pants);
    rect(ctx, 3, a ? 18 : 17, 3, 2, boots);
    rect(ctx, 6, a ? 17 : 18, 3, 2, boots);
  });
}

function face(hair, skin, eyes) {
  return paint(8, 8, hash(hair + skin), ctx => {
    rect(ctx, 0, 0, 8, 8, skin);
    rect(ctx, 0, 0, 8, 2, hair);
    rect(ctx, 0, 2, 1, 2, hair);
    rect(ctx, 7, 2, 1, 2, hair);
    px(ctx, 1, 4, '#ffffff');
    px(ctx, 2, 4, eyes);
    px(ctx, 5, 4, eyes);
    px(ctx, 6, 4, '#ffffff');
    rect(ctx, 3, 6, 2, 1, adjust(skin, 0.75));
  });
}

function moon() {
  return paint(16, 16, 16, (ctx, r) => {
    rect(ctx, 1, 1, 14, 14, '#ece7c9');
    rect(ctx, 1, 1, 14, 1, '#fffbe6');
    rect(ctx, 1, 14, 14, 1, '#cfc8a0');
    noiseRect(ctx, r, 2, 2, 12, 12, ['#ece7c9', '#e3ddbb', '#f3efd6'], [5, 2, 2]);
    for (const [x, y, s] of [[4, 4, 2], [9, 3, 1], [8, 8, 3], [3, 10, 2], [11, 11, 1]]) rect(ctx, x, y, s, s, '#cdc6a2');
  });
}

function minecart(color = '#7d8390') {
  return paint(16, 11, 17, ctx => {
    rect(ctx, 0, 0, 16, 2, adjust(color, 1.22));
    rect(ctx, 1, 2, 14, 6, color);
    rect(ctx, 2, 2, 12, 3, '#33363e');
    rect(ctx, 1, 7, 14, 1, adjust(color, 0.7));
    rect(ctx, 2, 8, 4, 3, '#2b2e35');
    rect(ctx, 10, 8, 4, 3, '#2b2e35');
    px(ctx, 3, 9, '#5a5e68');
    px(ctx, 11, 9, '#5a5e68');
  });
}

function pc() {
  return paint(16, 15, 18, ctx => {
    rect(ctx, 0, 0, 16, 11, '#2b2733');
    rect(ctx, 1, 1, 14, 8, '#5a33c9');
    rect(ctx, 2, 2, 5, 3, '#9a74ff');
    rect(ctx, 8, 2, 5, 1, '#b79bff');
    rect(ctx, 8, 4, 4, 1, '#b79bff');
    rect(ctx, 2, 6, 11, 1, '#7f50f2');
    px(ctx, 14, 10, '#3dd68c');
    rect(ctx, 6, 11, 4, 2, '#3a3743');
    rect(ctx, 3, 13, 10, 2, '#3a3743');
  });
}

function barrier() {
  return paint(16, 16, 19, ctx => {
    for (let y = 0; y < 16; y++) {
      for (let x = 0; x < 16; x++) {
        const dx = x - 7.5;
        const dy = y - 7.5;
        const d = Math.sqrt(dx * dx + dy * dy);
        const ring = d >= 5.2 && d <= 7.3;
        const slash = Math.abs(dx + dy) <= 1.1 && d < 6;
        if (ring || slash) px(ctx, x, y, (x + y) % 5 === 0 ? '#ff6a6f' : '#e8262d');
      }
    }
  });
}

function tree() {
  const canvas = makeCanvas(TILE * 5, TILE * 7);
  const ctx = canvas.getContext('2d');
  const leaves = [[1, 0], [2, 0], [3, 0], [0, 1], [1, 1], [2, 1], [3, 1], [4, 1], [0, 2], [1, 2], [2, 2], [3, 2], [4, 2], [1, 3], [3, 3]];
  for (let y = 3; y < 7; y++) ctx.drawImage(texture('log', y), 2 * TILE, y * TILE);
  leaves.forEach(([x, y], index) => ctx.drawImage(texture('leaves', index % 4), x * TILE, y * TILE));
  ctx.drawImage(shaded('leaves', 1, 0.8), 2 * TILE, 3 * TILE - 8, TILE, 8);
  return canvas;
}

function portalFrame() {
  const canvas = makeCanvas(TILE * 4, TILE * 5);
  const ctx = canvas.getContext('2d');
  for (let y = 0; y < 5; y++) {
    for (let x = 0; x < 4; x++) {
      if (x === 0 || x === 3 || y === 0 || y === 4) ctx.drawImage(texture('obsidian', x + y), x * TILE, y * TILE);
    }
  }
  return canvas;
}

// ---------------------------------------------------------------- catálogo de sprites

const SPRITES = {
  moon,
  tree,
  pc,
  barrier,
  portalFrame,
  creeper,
  'minecart': () => minecart(),
  'minecart-p2p': () => minecart('#8e6fe6'),
  'minecart-local': () => minecart('#5f8f6a'),
  'minecart-playit': () => minecart('#c98a3c'),
  'miner-front': () => miner('front'),
  'miner-climbA': () => miner('climbA'),
  'miner-climbB': () => miner('climbB'),
  'friend-alex': () => miner('front', '#3c9a62'),
  'face-you': () => face('#5b3a22', '#e0a77c', '#3b2a7a'),
  'face-alex': () => face('#c8682a', '#f1c09a', '#2f7a4a'),
  'face-mar': () => face('#1e1a2e', '#b9805a', '#4a2a10'),
  'face-dani': () => face('#e6c14a', '#f3c7a3', '#2a5ad6'),
  'gem-diamond': () => fromMap(GEM_ROWS, GEMS.diamond),
  'gem-emerald': () => fromMap(GEM_ROWS, GEMS.emerald),
  'gem-lapis': () => fromMap(GEM_ROWS, GEMS.lapis),
  'gem-gold': () => fromMap(GEM_ROWS, GEMS.gold),
  'gem-redstone': () => fromMap(GEM_ROWS, GEMS.redstone),
  'gem-amethyst': () => fromMap(GEM_ROWS, GEMS.amethyst),
  'lump-coal': () => fromMap(LUMP_ROWS, LUMPS.coal),
  'lump-iron': () => fromMap(LUMP_ROWS, LUMPS.iron),
  'lump-copper': () => fromMap(LUMP_ROWS, LUMPS.copper)
};
Object.keys(MAPS).forEach(name => {
  SPRITES[name] = () => fromMap(MAPS[name].rows, MAPS[name].palette);
});

function sprite(name) {
  const key = `sprite|${name}`;
  if (!cache.has(key)) {
    const make = SPRITES[name];
    if (!make) throw new Error(`Sprite desconocido: ${name}`);
    cache.set(key, make());
  }
  return cache.get(key);
}

function mob(kind, frame, color) {
  const key = `mob|${kind}|${frame}|${color || ''}`;
  if (!cache.has(key)) {
    const make = { sheep: () => sheep(frame, color || '#ece8f2'), pig: () => pig(frame), chicken: () => chicken(frame), creeper }[kind];
    cache.set(key, make());
  }
  return cache.get(key);
}

// Lo que cae al romper un mineral.
function dropFor(blockName) {
  const kind = blockName.startsWith('ore:') ? blockName.split(':')[2] : '';
  if (GEMS[kind]) return sprite(`gem-${kind}`);
  if (LUMPS[kind]) return sprite(`lump-${kind}`);
  return sprite('gem-amethyst');
}

function url(canvas) {
  if (!urls.has(canvas)) urls.set(canvas, canvas.toDataURL('image/png'));
  return urls.get(canvas);
}

// ---------------------------------------------------------------- el corte del mundo

const MATERIALS = {
  dirt: { solid: 'dirt', back: 'dirt', patches: [['gravel', 0.14]], ores: [], torches: false },
  cave: { solid: 'stone', back: 'stone', patches: [['gravel', 0.07], ['dirt', 0.05]], ores: [['ore:stone:coal', 0.07], ['ore:stone:iron', 0.04]], torches: true },
  gravel: { solid: 'gravel', back: 'stone', patches: [['stone', 0.3], ['cobble', 0.08]], ores: [['ore:stone:copper', 0.05]], torches: true },
  redstone: { solid: 'stone', back: 'stone', patches: [['gravel', 0.05]], ores: [['ore:stone:redstone', 0.1], ['ore:stone:iron', 0.025]], torches: false, glow: 'redstone' },
  portal: { solid: 'stone', back: 'stone', patches: [['obsidian', 0.1], ['cobble', 0.06]], ores: [['ore:stone:lapis', 0.04], ['ore:stone:gold', 0.015]], torches: true },
  deepslate: { solid: 'deepslate', back: 'deepslate', patches: [['basalt', 0.06]], ores: [['ore:deepslate:gold', 0.04], ['ore:deepslate:lapis', 0.03], ['ore:deepslate:emerald', 0.015], ['ore:deepslate:diamond', 0.015]], torches: true },
  geode: { solid: 'basalt', back: 'amethyst', patches: [], ores: [], torches: false, geode: true },
  diamond: { solid: 'deepslate', back: 'deepslate', patches: [['basalt', 0.05]], ores: [['ore:deepslate:diamond', 0.07], ['ore:deepslate:redstone', 0.04]], torches: true, lava: true },
  bedrock: { solid: 'bedrock', back: 'deepslate', patches: [], ores: [], torches: false, bedrock: true }
};

// Devuelve un canvas con el corte de una capa (bloques sólidos en las orillas y el hueco
// excavado en medio), más la posición de antorchas, lava y minerales brillantes.
function world(options) {
  const { material, width, height, bs, walls, shaft, seed } = options;
  const config = MATERIALS[material];
  const random = mulberry32(seed);
  const cols = Math.ceil(width / bs) + 1;
  const rows = Math.ceil(height / bs) + 1;
  const canvas = makeCanvas(cols * TILE, rows * TILE);
  const ctx = canvas.getContext('2d');

  const ceilWave = wave(cols, 3, random);
  const floorWave = wave(cols, 3, random);
  const leftWave = wave(rows, 3, random);
  const rightWave = wave(rows, 3, random);
  const patchField = wave(cols * rows, 5, random);
  const visibleRows = height / bs;
  const extraFloor = config.bedrock ? 1 : 0;

  const ceil = ceilWave.map(v => 1 + (v > 0.55 ? 1 : 0));
  const floor = floorWave.map(v => 1 + (v > 0.5 ? 1 : 0) + extraFloor);
  const edgeBump = (r, v) => (r < 3 || r > visibleRows - 3) && v > 0.62 ? 1 : 0;
  const left = leftWave.map((v, r) => walls ? Math.max(shaft ? 2 : 1, walls - (v > 0.56 ? 1 : 0)) : edgeBump(r, v));
  const right = rightWave.map((v, r) => walls ? Math.max(1, walls - (v > 0.56 ? 1 : 0)) : edgeBump(r, v));

  const isShaft = c => shaft && c === 0;
  const solidAt = (r, c) => {
    if (c < 0 || c >= cols || r < 0) return true;
    if (isShaft(c)) return false;
    const centerX = c * bs + bs / 2;
    const centerY = r * bs + bs / 2;
    if (r < ceil[c]) return true;
    if (centerY > height - floor[c] * bs) return true;
    if (c < left[Math.min(r, rows - 1)]) return true;
    if (centerX > width - right[Math.min(r, rows - 1)] * bs) return true;
    return false;
  };

  const solid = [];
  for (let r = 0; r < rows; r++) {
    solid.push([]);
    for (let c = 0; c < cols; c++) solid[r].push(solidAt(r, c));
  }

  // Distancia al aire en los bloques sólidos (para los anillos de la geoda).
  const touchesAir = (r, c) => [[1, 0], [-1, 0], [0, 1], [0, -1]].some(([dr, dc]) => solid[r + dr] && solid[r + dr][c + dc] === false);

  const torches = [];
  const glows = [];
  const lava = [];
  const crystals = [];

  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const x = c * TILE;
      const y = r * TILE;
      const variant = randomInt(random, 4);
      if (solid[r][c]) {
        let name = config.solid;
        if (config.geode) {
          if (touchesAir(r, c)) name = 'amethyst';
          else if ([[1, 0], [-1, 0], [0, 1], [0, -1]].some(([dr, dc]) => solid[r + dr] && solid[r + dr][c + dc] && touchesAir(r + dr, c + dc))) name = 'calcite';
        } else if (config.bedrock && r < 3 && random() < 0.5) {
          name = 'deepslate';
        } else {
          const field = patchField[r * cols + c];
          for (const [patch, amount] of config.patches) {
            if (field < amount) name = patch;
          }
          for (const [ore, chance] of config.ores) {
            if (random() < chance) name = ore;
          }
        }
        ctx.drawImage(texture(name, variant), x, y);
        if (config.glow && name.includes(config.glow) && glows.length < 10 && random() < 0.7) glows.push({ x: (c + 0.5) * bs, y: (r + 0.5) * bs });
        continue;
      }

      // Aire: la pared del fondo, oscura, con sombra donde toca roca.
      ctx.drawImage(shaded(config.back, variant, config.geode ? 0.42 : 0.36), x, y);
      if (solid[r - 1] && solid[r - 1][c]) rect(ctx, x, y, TILE, 3, 'rgba(0,0,0,.38)');
      if (solid[r][c - 1]) rect(ctx, x, y, 3, TILE, 'rgba(0,0,0,.32)');
      if (solid[r][c + 1]) rect(ctx, x + 13, y, 3, TILE, 'rgba(0,0,0,.32)');
      if (solid[r + 1] && solid[r + 1][c]) rect(ctx, x, y + 14, TILE, 2, 'rgba(0,0,0,.22)');

      if (isShaft(c)) {
        ctx.drawImage(texture('ladder'), x, y);
        continue;
      }

      const besideLeftWall = solid[r][c - 1] && !isShaft(c - 1);
      const besideRightWall = solid[r][c + 1];
      if (config.torches && walls && r % 6 === 3 && (besideLeftWall || besideRightWall) && torches.length < 6) {
        const torch = sprite('torch');
        const nearLeft = besideLeftWall;
        ctx.drawImage(torch, nearLeft ? x + 1 : x + 10, y + 3);
        torches.push({ x: (c + (nearLeft ? 0.12 : 0.78)) * bs, y: (r + 0.45) * bs });
      }
      if (config.geode && (solid[r + 1] && solid[r + 1][c] || besideLeftWall || besideRightWall) && random() < 0.35) {
        ctx.drawImage(sprite('bud'), x + 4, y + 9);
        if (crystals.length < 12) crystals.push({ x: (c + 0.5) * bs, y: (r + 0.75) * bs });
      }
      const nearFloor = r * bs + bs / 2 > height - (floor[c] + 1) * bs;
      if (config.lava && nearFloor && (c < left[r] + 2 || c * bs > width - (right[r] + 2) * bs) && solid[r + 1] && solid[r + 1][c]) {
        ctx.drawImage(texture('lava', variant), x, y + 5, TILE, TILE - 5);
        lava.push({ x: (c + 0.5) * bs, y: (r + 0.7) * bs });
      }
    }
  }

  // Luz cálida alrededor de antorchas y lava.
  ctx.globalCompositeOperation = 'lighter';
  for (const light of [...torches, ...lava]) {
    const lx = (light.x / bs) * TILE;
    const ly = (light.y / bs) * TILE;
    const gradient = ctx.createRadialGradient(lx, ly, 0, lx, ly, TILE * 3);
    gradient.addColorStop(0, 'rgba(255,170,70,.32)');
    gradient.addColorStop(1, 'rgba(255,170,70,0)');
    ctx.fillStyle = gradient;
    ctx.fillRect(lx - TILE * 3, ly - TILE * 3, TILE * 6, TILE * 6);
  }
  ctx.globalCompositeOperation = 'source-over';

  return { canvas, cssWidth: cols * bs, cssHeight: rows * bs, torches, glows, lava, crystals };
}

// El suelo del héroe: pasto arriba, tierra abajo y el pozo con escalera.
function ground(options) {
  const { width, bs, rows, shaft, seed } = options;
  const random = mulberry32(seed);
  const cols = Math.ceil(width / bs) + 1;
  const canvas = makeCanvas(cols * TILE, rows * TILE);
  const ctx = canvas.getContext('2d');
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const variant = randomInt(random, 4);
      if (shaft && c === 0) {
        ctx.drawImage(shaded('dirt', variant, 0.36), c * TILE, r * TILE);
        rect(ctx, c * TILE + 13, r * TILE, 3, TILE, 'rgba(0,0,0,.3)');
        ctx.drawImage(texture('ladder'), c * TILE, r * TILE);
        continue;
      }
      const name = r === 0 ? 'grass' : r > 2 && random() < 0.25 ? 'gravel' : 'dirt';
      ctx.drawImage(texture(name, variant), c * TILE, r * TILE);
    }
  }
  return { canvas, cssWidth: cols * bs, cssHeight: rows * bs };
}

export const Pixel = { texture, shaded, crack, iso, sprite, mob, dropFor, colorsOf, url, world, ground, mulberry32, hash, makeCanvas };
