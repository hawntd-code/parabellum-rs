"""La web de Parabellum (https://parabellum-roan.vercel.app): inicio, descargar y preguntas, en español (/)
y en inglés (/en/), escrita a mano y sin compilador. El marco (barra y pie) es plantilla.html y cada página
es una pieza de paginas/.

    python web/construir.py            arma web/publico/ (va al repo) y web/vista/ (para verla en tu PC)
    python web/construir.py despliegue  arma web/despliegue/ para Vercel, fijado al último commit de main

Para publicar: construir, subir el commit a main y después «despliegue»; en Vercel van solo
despliegue/preparar.mjs y despliegue/vercel.json. Al compilar, preparar.mjs baja de GitHub (el repo es
público) las páginas, las imágenes, las fuentes y la demostración (ui/, que en el navegador usa datos de
prueba), todo de ese commit.

Las capturas de la web están en web/capturas/<es|en>/ (las rehace web/capturar.py): la interfaz a 2560×1600
(1280×800 con densidad 2), en WebP y en cada idioma. No son las de docs/capturas (esas son del README).

Las descargas salen de la release más nueva de GitHub: aquí se escriben las de hoy para que la página
sirva sin JavaScript, y sitio.js las pone al día al abrirla. Los textos están en textos.py: todo lo que
dicen tiene que existir en el launcher."""
import html
import json
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path

from textos import EN, ES, MEMORIA, MEMORIA_CURVA

AQUI = Path(__file__).resolve().parent
REPO = AQUI.parent
PUBLICO = AQUI / "publico"
VISTA = AQUI / "vista"
CAPTURAS = ["inicio", "explorar", "instancia", "mensajes", "comunidades", "doctor", "servidor"]
ESTATICOS = ["estilos.css", "sitio.js", "isla.js"]

ICONOS = {
    "search": "M10.5 3a7.5 7.5 0 0 1 5.96 12.05l4.25 4.24-1.42 1.42-4.24-4.25A7.5 7.5 0 1 1 10.5 3Zm0 2a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z",
    "update": "M12 4a8 8 0 0 1 7.75 6H22l-3.5 4-3.5-4h2.68A6 6 0 1 0 18 14.9l1.74 1A8 8 0 1 1 12 4Z",
    "versions": "M4 6h16v2H4zm0 5h16v2H4zm0 5h10v2H4z",
    "chat": "M4 4h16a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H9l-5 4v-4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z",
    "communities": "M12 2 3 6v6c0 5 3.8 9.3 9 10 5.2-.7 9-5 9-10V6Zm0 2.2 7 3.1V12c0 3.9-2.9 7.3-7 8-4.1-.7-7-4.1-7-8V7.3Z",
    "ingame": "M3 5h18v14H3Zm2 2v10h14V7Zm2 2h4v6H7Z",
    "skins": "M8 2h8l1 3h3v6h-3v11H7V11H4V5h3Zm1.5 2-.5 2H6v3h3v11h6V9h3V6h-3l-.5-2Z",
    "worlds": "M6.5 19A4.5 4.5 0 0 1 6 10.03 6 6 0 0 1 17.65 9 5 5 0 0 1 17 19Z",
    "doctor": "M10 3h4v7h7v4h-7v7h-4v-7H3v-4h7Z",
    "import": "M11 3h2v9.6l3.3-3.3 1.4 1.4L12 16.4l-5.7-5.7 1.4-1.4 3.3 3.3Zm-7 15h16v2H4Z",
    "windows": "M3 3h8.4v8.4H3zm9.6 0H21v8.4h-8.4zM3 12.6h8.4V21H3zm9.6 0H21V21h-8.4z",
    "mac": "M16.37 12.6c.02 2.5 2.2 3.33 2.22 3.34-.02.06-.35 1.18-1.14 2.33-.69 1-1.4 2-2.52 2.02-1.1.02-1.46-.65-2.72-.65-1.27 0-1.66.63-2.7.67-1.08.04-1.9-1.08-2.6-2.08-1.42-2.05-2.5-5.8-1.05-8.32.72-1.25 2-2.05 3.4-2.07 1.06-.02 2.06.71 2.71.71.65 0 1.87-.88 3.15-.75.54.02 2.04.22 3 1.63-.08.05-1.79 1.04-1.77 3.12M14.3 4.36c.58-.7.97-1.67.86-2.64-.83.03-1.84.55-2.44 1.25-.53.62-1 1.61-.88 2.56.93.07 1.88-.47 2.46-1.17",
}
FUNCIONES = [("search", "f_search"), ("update", "f_update"), ("versions", "f_versions"), ("chat", "f_chat"),
             ("communities", "f_communities"), ("ingame", "f_ingame"), ("skins", "f_skins"), ("worlds", "f_worlds"),
             ("doctor", "f_doctor"), ("import", "f_import")]


def releases():
    """La versión más nueva de Windows (instalador y zip) y la de Mac, como sitio.js."""
    request = urllib.request.Request("https://api.github.com/repos/hawntd-code/parabellum-rs/releases?per_page=40", headers={"User-Agent": "parabellum-web"})
    data = json.load(urllib.request.urlopen(request, timeout=30))
    stable = [r for r in data if not r["draft"] and not r["prerelease"]]
    win = next(r for r in stable if r["tag_name"].startswith("v") and any(a["name"].endswith("-setup.exe") for a in r["assets"]))
    mac = next(r for r in stable if r["tag_name"].startswith("mac-v"))
    setup = next(a for a in win["assets"] if a["name"].endswith("-setup.exe"))
    portable = next(a for a in win["assets"] if a["name"].endswith(".zip"))
    mac_zip = next(a for a in mac["assets"] if a["name"].endswith(".zip"))
    mb = lambda a: f"{a['size'] / 1e6:.1f}".removesuffix(".0")
    return {
        "win_url": setup["browser_download_url"], "win_version": win["tag_name"][1:], "win_size": mb(setup),
        "portable_url": portable["browser_download_url"],
        "mac_url": mac_zip["browser_download_url"], "mac_version": mac["tag_name"][5:], "mac_size": mb(mac_zip),
        "version": win["tag_name"][1:],
    }


def bloques(t):
    e = html.escape
    funciones = "\n".join(
        f'          <article class="feature reveal"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="{ICONOS[icono]}"/></svg>'
        f'<h3>{e(t[clave + "_title"])}</h3><p>{e(t[clave + "_text"])}</p></article>'
        for icono, clave in FUNCIONES
    )
    # El recorrido: el índice de pasos (con su línea de avance) y las capturas, una encima de otra.
    pasos = "\n".join(
        f'              <li class="tour-step{" is-active" if i == 0 else ""}" data-step="{i}">'
        f'<span class="tour-fill" aria-hidden="true"></span>'
        f'<button class="tour-tab" type="button"{" aria-current=\"step\"" if i == 0 else ""}>{e(nombre)}</button>'
        f'<div class="tour-text"><p>{e(texto)}</p></div></li>'
        for i, (_, nombre, texto, _) in enumerate(t["tour"])
    )
    capturas = "\n".join(
        f'                <figure class="tour-shot" data-shot="{i}"><img src="/capturas/{t["lang"]}/{archivo}.webp" alt="{e(alt)}" '
        f'width="2560" height="1600" decoding="async"{"" if i == 0 else " loading=\"lazy\""}></figure>'
        for i, (archivo, _, _, alt) in enumerate(t["tour"])
    )
    barras = "".join("<i></i>" for _ in t["tour"])
    faq = "\n".join(
        f'          <details class="qa"><summary>{e(pregunta)}</summary><div class="qa-body"><p>{e(respuesta)}</p></div></details>'
        for pregunta, respuesta in t["faq"]
    )
    # Los dos botones de descarga: arriba en el inicio y al final de él.
    cta = (
        '          <div class="cta">\n'
        f'            <a class="btn primary" data-dl="windows" href="{{{{win_url}}}}"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="{ICONOS["windows"]}"/></svg><span>{e(t["download_windows"])}</span></a>\n'
        f'            <a class="btn" data-dl="mac" href="{{{{mac_url}}}}"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="{ICONOS["mac"]}"/></svg><span>{e(t["download_mac"])}</span></a>\n'
        '          </div>'
    )
    return {"features": funciones, "tour_steps": pasos, "tour_shots": capturas, "tour_bars": barras,
            "tour_count": str(len(t["tour"])), "faq": faq, "cta": cta, **memoria(t),
            "rust_path": (AQUI / "marca" / "rust.svg").read_text(encoding="utf-8").split(' d="')[1].split('"')[0],
            "icon_windows": ICONOS["windows"], "icon_mac": ICONOS["mac"]}


def memoria(t):
    """La sección de memoria: la curva de Parabellum (SVG) y las barras de la comparación, de MEMORIA."""
    en = t is EN
    nuestro = next(mb for nombre, _, mb, _ in MEMORIA if nombre == "Parabellum")
    # La curva: x de 15 s a 2 min, y de 0 a 300 MB, en un lienzo de 400×150 con la base en y = 120.
    x = lambda s: 8 + (s - MEMORIA_CURVA[0][0]) / (MEMORIA_CURVA[-1][0] - MEMORIA_CURVA[0][0]) * 384
    y = lambda mb: 120 - mb / 300 * 108
    puntos = [(round(x(s), 1), round(y(mb), 1), s, mb) for s, mb in MEMORIA_CURVA]
    linea = "M" + " L".join(f"{px} {py}" for px, py, _, _ in puntos)
    area = f"{linea} L{puntos[-1][0]} 120 L{puntos[0][0]} 120 Z"
    (x0, y0, _, mb0), (x1, y1, _, mb1) = puntos[0], puntos[-1]
    aria = (f"From {mb0} MB at 15 seconds to {mb1} MB at 2 minutes" if en else f"De {mb0} MB a los 15 segundos a {mb1} MB a los 2 minutos")
    datos = ",".join(f"{s}:{mb}" for _, _, s, mb in puntos)
    marcas = "".join(f'<circle class="mem-dot" cx="{px}" cy="{py}" r="4"/>' for px, py, _, _ in (puntos[0], puntos[-1]))
    grafica = (
        f'              <svg class="mem-svg" viewBox="0 0 400 150" role="img" aria-label="{aria}" data-curve="{datos}">\n'
        '                <defs><linearGradient id="mem-area" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="currentColor" stop-opacity=".22"/><stop offset="1" stop-color="currentColor" stop-opacity="0"/></linearGradient></defs>\n'
        '                <line class="mem-base" x1="8" y1="120" x2="392" y2="120"/>\n'
        f'                <path class="mem-area" d="{area}"/>\n'
        f'                <path class="mem-line" d="{linea}" pathLength="1"/>\n'
        f'                {marcas}\n'
        f'                <text class="mem-tag" x="{x0 + 8}" y="{y0 - 10}">{mb0} MB</text>\n'
        f'                <text class="mem-tag" x="{x1}" y="{y1 - 12}" text-anchor="end">{mb1} MB</text>\n'
        f'                <text class="mem-axis" x="8" y="142">15 s</text><text class="mem-axis" x="392" y="142" text-anchor="end">2 min</text>\n'
        '                <g class="mem-hover" aria-hidden="true"><line y1="8" y2="120"/><circle r="4.5"/><text y="4"></text></g>\n'
        '              </svg>'
    )
    mayor = max(mb for _, _, mb, _ in MEMORIA)
    filas = []
    for i, (nombre, version, mb, procesos) in enumerate(MEMORIA):
        if en:
            version = version.replace("sin Overwolf", "without Overwolf")
        unidad = t["mem_process"] if procesos == 1 else t["mem_processes"]
        filas.append(
            f'              <li class="mem-row{" is-us" if nombre == "Parabellum" else ""}" style="--v: {mb / mayor:.4f}; --i: {i}">'
            f'<span class="mem-name">{html.escape(nombre)} <small>{html.escape(version)}</small></span>'
            f'<span class="mem-value"><b data-count="{mb}">{mb}</b> MB</span>'
            f'<span class="mem-track"><i></i></span>'
            f'<span class="mem-procs">{procesos} {unidad}</span></li>'
        )
    return {"mem_us": str(nuestro), "mem_chart": grafica, "mem_rows": "\n".join(filas)}


def render(t, pagina, datos):
    """Una página (inicio, descargar o preguntas) en el idioma de t."""
    otro = EN if t is ES else ES
    rutas = t["pages"]
    valores = {
        **{k: v for k, v in t.items() if isinstance(v, str)}, **datos, **bloques(t),
        "title": t["titles"][pagina], "description": t["descriptions"][pagina],
        "path": rutas[pagina], "other_path": otro["pages"][pagina],
        "es_path": ES["pages"][pagina], "en_path": EN["pages"][pagina],
        "home_path": rutas["inicio"], "download_path": rutas["descargar"], "faq_path": rutas["preguntas"],
        "current_descargar": ' aria-current="page"' if pagina == "descargar" else "",
        "current_preguntas": ' aria-current="page"' if pagina == "preguntas" else "",
    }
    page = (AQUI / "plantilla.html").read_text(encoding="utf-8")
    page = page.replace("{{main}}", (AQUI / "paginas" / f"{pagina}.html").read_text(encoding="utf-8").rstrip("\n"))
    # Dos vueltas: los bloques (los botones) traen sus propios {{…}}. Los textos con <br> o <b> son de
    # textos.py (no de afuera): van tal cual.
    for _ in range(2):
        for clave, valor in valores.items():
            page = page.replace("{{" + clave + "}}", valor)
    assert "{{" not in page, page[page.index("{{"):page.index("{{") + 40]
    return page


def paginas():
    """(archivo dentro de publico/, idioma, página) de las seis páginas."""
    return [("index.html" if ruta == "/" else ruta.strip("/") + "/index.html", t, pagina)
            for t in (ES, EN) for pagina, ruta in t["pages"].items()]


def vaciar(carpeta):
    """Vacía la carpeta sin borrarla (puede estar abierta)."""
    for viejo in carpeta.glob("*") if carpeta.exists() else []:
        shutil.rmtree(viejo) if viejo.is_dir() else viejo.unlink()


def construir():
    datos = releases()
    vaciar(PUBLICO)
    for archivo, t, pagina in paginas():
        (PUBLICO / archivo).parent.mkdir(parents=True, exist_ok=True)
        (PUBLICO / archivo).write_text(render(t, pagina, datos), encoding="utf-8")
    for nombre in ESTATICOS:
        shutil.copy(AQUI / nombre, PUBLICO / nombre)
    # La vista local: el sitio completo, como queda en Vercel.
    vaciar(VISTA)
    shutil.copytree(PUBLICO, VISTA, dirs_exist_ok=True)
    shutil.copytree(AQUI / "marca", VISTA, dirs_exist_ok=True)
    (VISTA / "fuentes").mkdir()
    for nombre in ["inter-latin.woff2", "inter-latin-ext.woff2", "OFL.txt"]:
        shutil.copy(REPO / "ui" / "fonts" / nombre, VISTA / "fuentes" / nombre)
    shutil.copytree(AQUI / "capturas", VISTA / "capturas")
    shutil.copytree(REPO / "ui", VISTA / "demo", ignore=shutil.ignore_patterns("*.md"))
    print("Listo: web/publico y web/vista · Windows", datos["win_version"], "· Mac", datos["mac_version"])


def despliegue():
    """web/despliegue/: preparar.mjs (baja todo de GitHub, de ese commit) y vercel.json."""
    commit = subprocess.run(["git", "-C", str(REPO), "rev-parse", "origin/main"], capture_output=True, text=True, check=True).stdout.strip()
    pendiente = subprocess.run(["git", "-C", str(REPO), "status", "--porcelain", "web/publico"], capture_output=True, text=True).stdout.strip()
    if pendiente:
        sys.exit("web/publico tiene cambios sin subir a main: súbelos antes de desplegar.")
    remotos = [f"web/publico/{p}:{p}" for p in [archivo for archivo, _, _ in paginas()] + ESTATICOS]
    remotos += [f"web/marca/{m.name}:{m.name}" for m in sorted((AQUI / "marca").iterdir())]
    remotos += [f"web/capturas/{l}/{c}.webp:capturas/{l}/{c}.webp" for l in ("es", "en") for c in CAPTURAS]
    remotos += [f"ui/fonts/{f}:fuentes/{f}" for f in ["inter-latin.woff2", "inter-latin-ext.woff2", "OFL.txt"]]
    for archivo in sorted(p for p in (REPO / "ui").rglob("*") if p.is_file() and p.suffix != ".md"):
        relativo = archivo.relative_to(REPO / "ui").as_posix()
        remotos.append(f"ui/{relativo}:demo/{relativo}")
    destino = AQUI / "despliegue"
    vaciar(destino)
    destino.mkdir(exist_ok=True)
    script = f"""// Arma salida/ para Vercel con la web de parabellum-rs @ {commit[:7]}: las páginas (web/publico), la marca,
// las capturas, las fuentes y la demostración del launcher (ui/). Lo genera web/construir.py.
import {{ mkdir, writeFile }} from "node:fs/promises";
import {{ dirname }} from "node:path";
const COMMIT = "{commit}";
const ARCHIVOS = {json.dumps(remotos, ensure_ascii=False)};
await Promise.all(ARCHIVOS.map(async (par) => {{
  const [origen, destino] = par.split(":");
  const respuesta = await fetch(`https://raw.githubusercontent.com/hawntd-code/parabellum-rs/${{COMMIT}}/${{origen}}`);
  if (!respuesta.ok) throw new Error(`${{origen}}: ${{respuesta.status}}`);
  await mkdir(dirname(`salida/${{destino}}`), {{ recursive: true }});
  await writeFile(`salida/${{destino}}`, Buffer.from(await respuesta.arrayBuffer()));
}}));
console.log(`salida/ lista: ${{ARCHIVOS.length}} archivos de ${{COMMIT.slice(0, 7)}}`);
"""
    (destino / "preparar.mjs").write_text(script, encoding="utf-8")
    config = json.loads((AQUI / "vercel.json").read_text(encoding="utf-8"))
    config.update({"installCommand": "", "buildCommand": "node preparar.mjs", "outputDirectory": "salida"})
    (destino / "vercel.json").write_text(json.dumps(config, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print("Listo: web/despliegue para", commit[:7])


if __name__ == "__main__":
    despliegue() if sys.argv[1:] == ["despliegue"] else construir()
