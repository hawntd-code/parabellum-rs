"use strict";
// La isla de la portada: un mundo de cubos en vista isométrica, dibujado aquí (sin imágenes ni texturas de
// nadie), con la paleta de Parabellum. Flota, el estanque brilla y suben chispas. Con «reducir
// movimiento» se queda quieta.

(() => {
  const canvas = document.getElementById("isla");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Colores de cada tipo de cubo: arriba, lado izquierdo, lado derecho.
  const COLORS = {
    grass: ["#9fe6bd", "#7a5f97", "#5d4878"],
    dirt: ["#8c6fae", "#7a5f97", "#5d4878"],
    stone: ["#7a6e98", "#655a84", "#52486e"],
    deep: ["#5a4f78", "#4a4066", "#3b3354"],
    sand: ["#efe1c6", "#c9b597", "#a8957a"],
    wood: ["#8a6a5e", "#6e5148", "#553e37"],
    leaves: ["#cbb4ff", "#a98af0", "#8a68d8"],
    leaves2: ["#d9c6ff", "#b89cf5", "#9677e0"],
    water: ["#7d74f6", "#6a5fe0", "#5649c4"],
    crystal: ["#f3eaff", "#d9c0ff", "#b08bf5"],
  };

  // Una semilla fija: la isla es siempre la misma.
  let seed = 7;
  const random = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646;

  const SIZE = 12;
  const blocks = [];
  const add = (x, y, z, type) => blocks.push({ x, y, z, type });
  const center = (SIZE - 1) / 2;
  const height = [];
  for (let x = 0; x < SIZE; x++) {
    height[x] = [];
    for (let y = 0; y < SIZE; y++) {
      const d = Math.hypot(x - center, (y - center) * 1.08);
      const edge = 5.5 + Math.sin(x * 1.3) * 0.35 + Math.cos(y * 1.1) * 0.35;
      if (d > edge) { height[x][y] = -1; continue; }
      // Una loma suave: alta al centro y que baja hacia la orilla.
      const hill = Math.pow(Math.max(0, 1 - d / edge), 1.4) * 3.2;
      height[x][y] = Math.max(1, Math.round(0.6 + hill));
    }
  }
  // El estanque: un hueco poco profundo en un lado.
  const pond = new Set(["7,3", "8,3", "7,4", "8,4", "8,5", "9,4"]);
  for (let x = 0; x < SIZE; x++) {
    for (let y = 0; y < SIZE; y++) {
      const h = height[x][y];
      if (h < 0) continue;
      const d = Math.hypot(x - center, y - center);
      // Por debajo: roca que se afina hacia abajo, como una isla que flota.
      const depth = Math.max(1, Math.round(4.2 - d * 0.7 + random() * 1.2));
      for (let z = -depth; z < 0; z++) add(x, y, z, z < -2 ? "deep" : "stone");
      const top = pond.has(`${x},${y}`) ? Math.min(h, 1) : h;
      for (let z = 0; z < top; z++) {
        const last = z === top - 1;
        const type = pond.has(`${x},${y}`) && last ? "water" : last ? (d > 5.1 ? "sand" : "grass") : "dirt";
        add(x, y, z, type);
      }
    }
  }
  // Árboles de hojas lavanda.
  const tree = (x, y, trunk, kind) => {
    const base = height[x][y];
    for (let z = base; z < base + trunk; z++) add(x, y, z, "wood");
    const top = base + trunk;
    // Una copa en cruz: el centro y sus cuatro vecinos, y uno más arriba.
    for (const [dx, dy] of [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]]) add(x + dx, y + dy, top, kind);
    add(x, y, top + 1, kind);
  };
  tree(3, 7, 2, "leaves");
  tree(8, 8, 2, "leaves2");
  tree(3, 3, 1, "leaves2");
  // Un cristal sobre la colina.
  const peak = { x: 6, y: 5 };
  add(peak.x, peak.y, height[peak.x][peak.y], "crystal");
  add(peak.x, peak.y, height[peak.x][peak.y] + 1, "crystal");

  // De atrás hacia adelante y de abajo hacia arriba.
  blocks.sort((a, b) => a.x + a.y - (b.x + b.y) || a.z - b.z || a.x - b.x);

  // Lo que ocupa la isla (en cubos), para que quepa entera y centrada en cualquier tamaño.
  const bounds = blocks.reduce((box, b) => {
    const x = (b.x - b.y) * 0.866;
    const y = (b.x + b.y) * 0.5 - b.z;
    return { left: Math.min(box.left, x - 0.866), right: Math.max(box.right, x + 0.866), top: Math.min(box.top, y - 0.5), bottom: Math.max(box.bottom, y + 1.5) };
  }, { left: Infinity, right: -Infinity, top: Infinity, bottom: -Infinity });

  const sparks = Array.from({ length: 14 }, () => ({ x: random(), y: random(), speed: 0.15 + random() * 0.25, size: 1 + random() * 2 }));

  function draw(time) {
    const dpr = Math.min(2, devicePixelRatio || 1);
    const width = canvas.clientWidth;
    const heightPx = canvas.clientHeight;
    if (canvas.width !== Math.round(width * dpr)) { canvas.width = Math.round(width * dpr); canvas.height = Math.round(heightPx * dpr); }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, heightPx);
    const s = Math.min((width * 0.94) / (bounds.right - bounds.left), (heightPx * 0.86) / (bounds.bottom - bounds.top));
    const cos = s * 0.866;
    const bob = still ? 0 : Math.sin(time / 1400) * s * 0.18;
    const ox = width / 2 - ((bounds.left + bounds.right) / 2) * s;
    const oy = heightPx / 2 - ((bounds.top + bounds.bottom) / 2) * s + bob;

    // El resplandor debajo: una elipse que se apaga antes de tocar cualquier borde del lienzo (si no, se
    // ve el corte en línea recta).
    const gy = oy + s * 7.5;
    const flat = 0.42;
    const radius = Math.max(1, Math.min(s * 8, ox, width - ox, gy / flat, (heightPx - gy) / flat) * 0.96);
    ctx.save();
    ctx.translate(ox, gy);
    ctx.scale(1, flat);
    const glow = ctx.createRadialGradient(0, 0, 0, 0, 0, radius);
    glow.addColorStop(0, "rgba(152, 106, 242, .30)");
    glow.addColorStop(0.55, "rgba(152, 106, 242, .10)");
    glow.addColorStop(1, "rgba(152, 106, 242, 0)");
    ctx.fillStyle = glow;
    ctx.fillRect(-radius, -radius, radius * 2, radius * 2);
    ctx.restore();

    for (const b of blocks) {
      const x = ox + (b.x - b.y) * cos;
      const y = oy + (b.x + b.y) * s * 0.5 - b.z * s;
      const [top, left, right] = COLORS[b.type];
      const shimmer = b.type === "water" && !still ? 0.82 + Math.sin(time / 500 + b.x + b.y) * 0.12 : 1;
      ctx.globalAlpha = b.type === "water" ? shimmer : 1;
      ctx.fillStyle = left;
      ctx.beginPath(); ctx.moveTo(x - cos, y); ctx.lineTo(x, y + s * 0.5); ctx.lineTo(x, y + s * 1.5); ctx.lineTo(x - cos, y + s); ctx.closePath(); ctx.fill();
      ctx.fillStyle = right;
      ctx.beginPath(); ctx.moveTo(x + cos, y); ctx.lineTo(x, y + s * 0.5); ctx.lineTo(x, y + s * 1.5); ctx.lineTo(x + cos, y + s); ctx.closePath(); ctx.fill();
      ctx.fillStyle = top;
      ctx.beginPath(); ctx.moveTo(x, y - s * 0.5); ctx.lineTo(x + cos, y); ctx.lineTo(x, y + s * 0.5); ctx.lineTo(x - cos, y); ctx.closePath(); ctx.fill();
      // Un borde suave para que se lean los cubos.
      ctx.globalAlpha = 0.16;
      ctx.strokeStyle = "#ffffff";
      ctx.lineWidth = 0.6;
      ctx.beginPath(); ctx.moveTo(x - cos, y); ctx.lineTo(x, y - s * 0.5); ctx.lineTo(x + cos, y); ctx.stroke();
      ctx.globalAlpha = 1;
    }

    // Chispas que suben desde el cristal.
    if (!still) {
      for (const p of sparks) {
        const t = ((time / 1000) * p.speed + p.y) % 1;
        const px = ox + (p.x - 0.5) * s * 7;
        const py = oy - s * 2 - t * s * 6;
        ctx.globalAlpha = Math.sin(t * Math.PI) * 0.8;
        ctx.fillStyle = "#e3d6ff";
        ctx.fillRect(px, py, p.size, p.size);
      }
      ctx.globalAlpha = 1;
    }
  }

  let visible = true;
  let last = 0;
  const loop = (time) => {
    if (visible && time - last > 33) { draw(time); last = time; }
    requestAnimationFrame(loop);
  };
  new IntersectionObserver(([entry]) => (visible = entry.isIntersecting)).observe(canvas);
  draw(0);
  if (!still) requestAnimationFrame(loop);
  addEventListener("resize", () => draw(performance.now()), { passive: true });
})();
