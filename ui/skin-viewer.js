"use strict";

const SkinViewer = (() => {
  // Cada pieza de Minecraft ocupa la cruz de seis caras en la textura de 64×64.
  function faces(u, v, w, h, d) {
    return [
      ["front", u + d, v + d, w, h],
      ["back", u + 2 * d + w, v + d, w, h],
      ["left", u, v + d, d, h],
      ["right", u + d + w, v + d, d, h],
      ["top", u + d, v, w, d],
      ["bottom", u + d + w, v, w, d],
    ];
  }

  function parts(slim, legacy) {
    const arm = slim ? 3 : 4;
    const armsAt = 4 + arm / 2;
    return [
      { w: 8, h: 8, d: 8, x: 0, y: -12, base: [0, 0], coat: [32, 0], extra: .5 },
      { w: 8, h: 12, d: 4, x: 0, y: -2, base: [16, 16], coat: [16, 32] },
      { w: arm, h: 12, d: 4, x: -armsAt, y: -2, base: [40, 16], coat: [40, 32] },
      { w: arm, h: 12, d: 4, x: armsAt, y: -2, base: legacy ? [40, 16] : [32, 48], coat: [48, 48], mirror: legacy },
      { w: 4, h: 12, d: 4, x: -2, y: 10, base: [0, 16], coat: [0, 32] },
      { w: 4, h: 12, d: 4, x: 2, y: 10, base: legacy ? [0, 16] : [16, 48], coat: [0, 48], mirror: legacy },
    ];
  }

  function plane(part, projection, legacy, extra = 0) {
    let [side, u, v, w, h] = projection;
    const distance = ({ front: part.d, back: part.d, right: part.w, left: part.w, top: part.h, bottom: part.h })[side] / 2 + extra;
    if (part.mirror) {
      if (side === "left") side = "right";
      else if (side === "right") side = "left";
    }
    const width = w + 2 * extra;
    const height = h + 2 * extra;
    const scaleX = width / w;
    const scaleY = height / h;
    return `<span class="sk-face sk-${side}" style="--face-w:${width};--face-h:${height};--depth:${distance}">
      <i style="--tex-x:${-u * scaleX};--tex-y:${-v * scaleY};--tex-w:${64 * scaleX};--tex-h:${(legacy ? 32 : 64) * scaleY};--mirror:${part.mirror ? -1 : 1}"></i>
    </span>`;
  }

  function mesh(slim, legacy) {
    return parts(slim, legacy).map(part => {
      const { w, h, d, x, y, base, coat, extra = .25 } = part;
      const solid = faces(...base, w, h, d).map(face => plane(part, face, legacy)).join("");
      const overlay = !legacy || y === -12 ? faces(...coat, w, h, d).map(face => plane(part, face, legacy, extra)).join("") : "";
      return `<div class="sk-part" style="--x:${x};--y:${y}">${solid}${overlay}</div>`;
    }).join("");
  }

  /** Maniquí local para cuentas sin personalizar y para la demostración del launcher. */
  function mannequin(name, slim = false, tunic = "#51839e") {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 64;
    const ctx = canvas.getContext("2d");
    const head = document.createElement("canvas");
    head.width = head.height = 8;
    drawHead(head, name, { real: false });
    const pixel = (x, y) => `rgb(${[...head.getContext("2d").getImageData(x, y, 1, 1).data].slice(0, 3).join(",")})`;
    const skin = pixel(3, 5);
    const hair = pixel(3, 0);
    const fill = (color, u, v, w, h) => { ctx.fillStyle = color; ctx.fillRect(u, v, w, h); };
    for (const [index, { w, h, d, base }] of parts(slim, false).entries()) {
      for (const [side, u, v, fw, fh] of faces(...base, w, h, d)) {
        const arm = index === 2 || index === 3;
        const leg = index >= 4;
        const color = index === 0 ? (side === "top" || side === "back" ? hair : skin)
          : index === 1 ? tunic : arm ? skin : "#343053";
        fill(color, u, v, fw, fh);
        if ((arm || index === 0) && fh === h && side !== "back") fill(arm ? tunic : hair, u, v, fw, arm ? 4 : 2);
        if (leg && fh === h) fill("#272336", u, v + fh - 3, fw, 3);
      }
    }
    ctx.drawImage(head, 8, 8);
    fill("#ffffff20", 21, 20, 6, 2);
    fill("#00000020", 20, 30, 8, 2);
    fill("#dac8f6", 35, 23, 2, 5);
    fill("#dac8f6", 34, 24, 4, 2);
    return canvas.toDataURL("image/png");
  }

  async function mount(stage, controls, view, name) {
    const model = stage.querySelector(".sk-model");
    const reset = () => { yaw = -27; pitch = -7; paintAngle(); };
    let yaw, pitch, drag = null;
    const paintAngle = () => {
      model.style.setProperty("--yaw", `${yaw}deg`);
      model.style.setProperty("--pitch", `${pitch}deg`);
    };
    const move = (x, y = 0) => {
      yaw = (yaw + x) % 360;
      pitch = Math.max(-55, Math.min(55, pitch + y));
      paintAngle();
    };
    reset();
    stage.onpointerdown = (event) => {
      if (event.button !== 0 || !event.isPrimary) return;
      drag = event;
      stage.setPointerCapture(event.pointerId);
      stage.focus({ preventScroll: true });
    };
    stage.onpointermove = (event) => {
      if (drag?.pointerId !== event.pointerId) return;
      move((event.clientX - drag.clientX) * .9, -(event.clientY - drag.clientY) * .65);
      drag = event;
    };
    stage.onlostpointercapture = () => { drag = null; };
    stage.onkeydown = (event) => {
      const turns = { ArrowLeft: [-24, 0], ArrowRight: [24, 0], ArrowUp: [0, -12], ArrowDown: [0, 12] };
      if (event.key === "Home") reset();
      else if (Object.hasOwn(turns, event.key)) move(...turns[event.key]);
      else return;
      event.preventDefault();
    };
    for (const button of controls.querySelectorAll("[data-turn]")) {
      button.onclick = () => button.dataset.turn === "reset" ? reset() : move(Number(button.dataset.turn));
    }

    const texture = new Image();
    let source = view?.image;
    if (source && /^data:image\/png;base64,[A-Za-z0-9+/]+=*$/.test(source)) {
      texture.src = source;
      await texture.decode().catch(() => { source = null; });
    } else source = null;
    if (!stage.isConnected) return;
    const legacy = texture.naturalHeight === 32;
    model.style.setProperty("--skin-image", `url("${source ?? mannequin(name)}")`);
    model.innerHTML = mesh(!!source && !!view.slim && !legacy, legacy);
  }

  return { mount, mannequin };
})();
