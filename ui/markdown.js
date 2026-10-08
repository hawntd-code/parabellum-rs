"use strict";
// Descripciones de Modrinth: Markdown (con algo de HTML) convertido a HTML y después LIMPIADO.
// Lo escriben terceros, así que solo pasan etiquetas y atributos de una lista blanca: nada de
// scripts, estilos, formularios ni eventos; los enlaces se abren en el navegador y las imágenes
// solo por https.

const Markdown = (() => {
  const escape = (text) => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  const attribute = (url) => url.replace(/"/g, "%22");

  function inline(text) {
    const codes = [];
    text = text.replace(/`([^`\n]+)`/g, (_, code) => {
      codes.push(code);
      return `\u0000${codes.length - 1}\u0000`;
    });
    text = text
      // Imagen con enlace: [![alt](img)](url)
      .replace(/!\[([^\]]*)\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g, (_, alt, url) => `<img src="${attribute(url)}" alt="${escape(alt)}">`)
      .replace(/\[([^\]]+)\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g, (_, label, url) => `<a href="${attribute(url)}">${label}</a>`)
      .replace(/<(https?:\/\/[^>\s]+)>/g, (_, url) => `<a href="${attribute(url)}">${escape(url)}</a>`)
      .replace(/\*\*\*(?!\s)(.+?)\*\*\*/g, "<strong><em>$1</em></strong>")
      .replace(/\*\*(?!\s)(.+?)\*\*/g, "<strong>$1</strong>")
      .replace(/__(?!\s)(.+?)__/g, "<strong>$1</strong>")
      .replace(/(^|[^*\w])\*(?![\s*])(.+?)\*(?!\w)/g, "$1<em>$2</em>")
      .replace(/(^|[^_\w])_(?![\s_])(.+?)_(?!\w)/g, "$1<em>$2</em>")
      .replace(/~~(.+?)~~/g, "<del>$1</del>");
    return text.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${escape(codes[i])}</code>`);
  }

  const blank = (line) => !line.trim();
  const listItem = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/;
  const blockStart = /^\s{0,3}(#{1,6}\s|>|```|~~~)|^(\s*)([-*+]|\d+[.)])\s+/;

  function dedent(lines) {
    const indent = Math.min(...lines.slice(1).filter((l) => !blank(l)).map((l) => l.match(/^\s*/)[0].length));
    return [lines[0], ...lines.slice(1).map((l) => l.slice(Number.isFinite(indent) ? indent : 0))].join("\n");
  }

  function render(source) {
    const lines = source.replace(/\r\n?/g, "\n").split("\n");
    const out = [];
    let i = 0;
    while (i < lines.length) {
      const line = lines[i];
      let match;
      if ((match = line.match(/^\s*(```|~~~)/))) {
        const fence = match[1];
        const code = [];
        i++;
        while (i < lines.length && !lines[i].trim().startsWith(fence)) code.push(lines[i++]);
        i++;
        out.push(`<pre><code>${escape(code.join("\n"))}</code></pre>`);
      } else if (blank(line)) {
        i++;
      } else if ((match = line.match(/^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$/))) {
        const level = match[1].length;
        out.push(`<h${level}>${inline(match[2])}</h${level}>`);
        i++;
      } else if (/^\s{0,3}([-*_])(\s*\1){2,}\s*$/.test(line)) {
        out.push("<hr>");
        i++;
      } else if (/^\s{0,3}>/.test(line)) {
        const quote = [];
        while (i < lines.length && /^\s{0,3}>/.test(lines[i])) quote.push(lines[i++].replace(/^\s{0,3}>\s?/, ""));
        out.push(`<blockquote>${render(quote.join("\n"))}</blockquote>`);
      } else if (line.includes("|") && i + 1 < lines.length && /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/.test(lines[i + 1])) {
        const cells = (row) => row.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((c) => inline(c.trim()));
        const head = cells(line);
        const rows = [];
        i += 2;
        while (i < lines.length && lines[i].includes("|") && !blank(lines[i])) rows.push(cells(lines[i++]));
        out.push(
          `<table><thead><tr>${head.map((c) => `<th>${c}</th>`).join("")}</tr></thead>` +
            `<tbody>${rows.map((r) => `<tr>${r.map((c) => `<td>${c}</td>`).join("")}</tr>`).join("")}</tbody></table>`,
        );
      } else if ((match = line.match(listItem))) {
        const ordered = /\d/.test(match[2]);
        const items = [];
        while (i < lines.length) {
          const item = lines[i].match(listItem);
          if (item && item[1].length <= match[1].length + 1) {
            items.push([item[3]]);
            i++;
          } else if (items.length && !blank(lines[i]) && /^\s+/.test(lines[i])) {
            items.at(-1).push(lines[i++]);
          } else {
            break;
          }
        }
        const tag = ordered ? "ol" : "ul";
        out.push(`<${tag}>${items.map((item) => `<li>${item.length > 1 ? render(dedent(item)) : inline(item[0])}</li>`).join("")}</${tag}>`);
      } else if (/^\s*<\/?[a-zA-Z][^>]*>/.test(line)) {
        const block = [];
        while (i < lines.length && !blank(lines[i])) block.push(lines[i++]);
        out.push(inline(block.join("\n")));
      } else {
        const paragraph = [lines[i++]];
        while (i < lines.length && !blank(lines[i]) && !blockStart.test(lines[i])) paragraph.push(lines[i++]);
        out.push(`<p>${inline(paragraph.map((l) => (/ {2,}$/.test(l) ? `${l.trimEnd()}<br>` : l)).join("\n"))}</p>`);
      }
    }
    return out.join("\n");
  }

  // ---------- Limpieza ----------
  const common = ["title", "align"];
  const allowed = {
    a: ["href", "title"],
    img: ["src", "alt", "title", "width", "height", "align"],
    p: common, div: common, span: [], center: [], br: [], hr: [],
    h1: common, h2: common, h3: common, h4: common, h5: common, h6: common,
    strong: [], b: [], em: [], i: [], u: [], s: [], del: [], ins: [], mark: [], small: [], sub: [], sup: [], kbd: [],
    code: [], pre: [], blockquote: [], ul: [], ol: ["start"], li: [],
    table: [], thead: [], tbody: [], tr: [], th: ["align", "colspan", "rowspan"], td: ["align", "colspan", "rowspan"],
    details: [], summary: [],
  };
  const dropped = new Set(["script", "style", "object", "embed", "form", "input", "button", "textarea", "select", "option", "noscript", "template", "svg", "math", "link", "meta", "base", "head", "title", "frame", "frameset"]);

  function safeUrl(url, image = false) {
    try {
      const parsed = new URL(url, "https://modrinth.com/");
      if (parsed.protocol === "https:" || (!image && parsed.protocol === "http:")) return parsed.href;
    } catch {
      // no es una URL
    }
    return null;
  }

  function clean(source, target) {
    for (const node of [...source.childNodes]) {
      if (node.nodeType === Node.TEXT_NODE) {
        target.appendChild(document.createTextNode(node.textContent));
        continue;
      }
      if (node.nodeType !== Node.ELEMENT_NODE) continue;
      const tag = node.tagName.toLowerCase();
      if (tag === "iframe") {
        // Videos de YouTube y similares: un enlace en lugar del reproductor.
        const src = safeUrl(node.getAttribute("src") ?? "");
        if (src) {
          const link = document.createElement("a");
          link.href = src.replace("/embed/", "/watch?v=");
          link.textContent = "▶ Ver el video";
          link.className = "video-link";
          target.appendChild(link);
        }
        continue;
      }
      if (dropped.has(tag)) continue;
      if (!(tag in allowed)) {
        clean(node, target);
        continue;
      }
      const element = document.createElement(tag);
      for (const name of allowed[tag]) {
        let value = node.getAttribute(name);
        if (value == null) continue;
        if (name === "href" && !(value = safeUrl(value))) continue;
        if (name === "src" && !(value = safeUrl(value, true))) continue;
        if ((name === "width" || name === "height") && !/^\d{1,4}%?$/.test(value)) continue;
        if (name === "align" && !/^(left|right|center|justify)$/i.test(value)) continue;
        if (name === "start" && !/^\d{1,6}$/.test(value)) continue;
        element.setAttribute(name, value);
      }
      if (tag === "img") {
        if (!element.hasAttribute("src")) continue;
        element.loading = "lazy";
        element.referrerPolicy = "no-referrer";
      }
      clean(node, element);
      target.appendChild(element);
    }
  }

  /** Devuelve un fragmento listo para insertar (nunca HTML sin limpiar). */
  function toFragment(markdown) {
    // DOMParser crea un documento inerte: no ejecuta nada ni carga imágenes.
    const parsed = new DOMParser().parseFromString(`<body>${render(markdown ?? "")}</body>`, "text/html");
    const fragment = document.createDocumentFragment();
    clean(parsed.body, fragment);
    return fragment;
  }

  return { toFragment, render, safeUrl };
})();
