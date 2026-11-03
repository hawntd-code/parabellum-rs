"""La web de Parabellum (https://parabellum-roan.vercel.app): una sola página en español (/) y en inglés
(/en/), escrita a mano y sin compilador.

    python web/construir.py            arma web/publico/ (va al repo) y web/vista/ (para verla en tu PC)
    python web/construir.py despliegue  arma web/despliegue/ para Vercel, fijado al último commit de main

Para publicar: construir, subir el commit a main y después «despliegue»; en Vercel van solo
despliegue/preparar.mjs y despliegue/vercel.json. Al compilar, preparar.mjs baja de GitHub (el repo es
público) la página, las imágenes, las fuentes y la demostración (ui/, que en el navegador usa datos de
prueba), todo de ese commit.

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

from textos import EN, ES

AQUI = Path(__file__).resolve().parent
REPO = AQUI.parent
PUBLICO = AQUI / "publico"
VISTA = AQUI / "vista"
CAPTURAS = ["inicio", "explorar", "instancia", "mensajes", "comunidades", "doctor", "servidor"]
PAGINA = ["index.html", "en/index.html", "estilos.css", "sitio.js", "isla.js"]

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
}
FUNCIONES = [("search", "f_search"), ("update", "f_update"), ("versions", "f_versions"), ("chat", "f_chat"),
             ("communities", "f_communities"), ("ingame", "f_ingame"), ("skins", "f_skins"), ("worlds", "f_worlds"),
             ("doctor", "f_doctor"), ("import", "f_import")]
DETALLES = ["d_update", "d_verify", "d_downloads", "d_backup", "d_all"]


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
    pestanas = "\n".join(
        f'            <button role="tab" type="button" data-shot="{i}" aria-selected="{"true" if i == 0 else "false"}">{e(nombre)}</button>'
        for i, (_, nombre, _) in enumerate(t["shots"])
    )
    imagenes = "\n".join(
        f'            <img data-shot-img="{i}" src="/capturas/{archivo}" alt="{e(alt)}" width="1280" height="800" loading="{"eager" if i == 0 else "lazy"}"{"" if i == 0 else " hidden"}>'
        for i, (archivo, _, alt) in enumerate(t["shots"])
    )
    detalles = "\n".join(
        f'          <div class="detail reveal"><h3>{e(t[clave + "_title"])}</h3><p>{e(t[clave + "_text"])}</p></div>'
        for clave in DETALLES
    )
    faq = "\n".join(
        f'          <details class="qa reveal"><summary>{e(pregunta)}</summary><p>{e(respuesta)}</p></details>'
        for pregunta, respuesta in t["faq"]
    )
    return {"features": funciones, "shot_tabs": pestanas, "shot_images": imagenes, "details": detalles, "faq": faq}


def render(t, datos):
    page = (AQUI / "plantilla.html").read_text(encoding="utf-8")
    valores = {**{k: v for k, v in t.items() if isinstance(v, str)}, **datos, **bloques(t)}
    # Los textos con <br> o <b> son de textos.py (no de afuera): van tal cual.
    for clave, valor in valores.items():
        page = page.replace("{{" + clave + "}}", valor)
    assert "{{" not in page, page[page.index("{{"):page.index("{{") + 40]
    return page


def vaciar(carpeta):
    """Vacía la carpeta sin borrarla (puede estar abierta)."""
    for viejo in carpeta.glob("*") if carpeta.exists() else []:
        shutil.rmtree(viejo) if viejo.is_dir() else viejo.unlink()


def construir():
    datos = releases()
    vaciar(PUBLICO)
    (PUBLICO / "en").mkdir(parents=True, exist_ok=True)
    (PUBLICO / "index.html").write_text(render(ES, datos), encoding="utf-8")
    (PUBLICO / "en" / "index.html").write_text(render(EN, datos), encoding="utf-8")
    for nombre in ["estilos.css", "sitio.js", "isla.js"]:
        shutil.copy(AQUI / nombre, PUBLICO / nombre)
    # La vista local: la página completa, como queda en Vercel.
    vaciar(VISTA)
    shutil.copytree(PUBLICO, VISTA, dirs_exist_ok=True)
    shutil.copytree(AQUI / "marca", VISTA, dirs_exist_ok=True)
    (VISTA / "fuentes").mkdir()
    for nombre in ["inter-latin.woff2", "inter-latin-ext.woff2", "OFL.txt"]:
        shutil.copy(REPO / "ui" / "fonts" / nombre, VISTA / "fuentes" / nombre)
    (VISTA / "capturas").mkdir()
    for nombre in CAPTURAS:
        shutil.copy(REPO / "docs" / "capturas" / f"{nombre}.png", VISTA / "capturas" / f"{nombre}.png")
    shutil.copytree(REPO / "ui", VISTA / "demo", ignore=shutil.ignore_patterns("*.md"))
    print("Listo: web/publico y web/vista · Windows", datos["win_version"], "· Mac", datos["mac_version"])


def despliegue():
    """web/despliegue/: preparar.mjs (baja todo de GitHub, de ese commit) y vercel.json."""
    commit = subprocess.run(["git", "-C", str(REPO), "rev-parse", "origin/main"], capture_output=True, text=True, check=True).stdout.strip()
    pendiente = subprocess.run(["git", "-C", str(REPO), "status", "--porcelain", "web/publico"], capture_output=True, text=True).stdout.strip()
    if pendiente:
        sys.exit("web/publico tiene cambios sin subir a main: súbelos antes de desplegar.")
    remotos = [f"web/publico/{p}:{p}" for p in PAGINA]
    remotos += [f"web/marca/{m.name}:{m.name}" for m in sorted((AQUI / "marca").iterdir())]
    remotos += [f"docs/capturas/{c}.png:capturas/{c}.png" for c in CAPTURAS]
    remotos += [f"ui/fonts/{f}:fuentes/{f}" for f in ["inter-latin.woff2", "inter-latin-ext.woff2", "OFL.txt"]]
    for archivo in sorted(p for p in (REPO / "ui").rglob("*") if p.is_file() and p.suffix != ".md"):
        relativo = archivo.relative_to(REPO / "ui").as_posix()
        remotos.append(f"ui/{relativo}:demo/{relativo}")
    destino = AQUI / "despliegue"
    vaciar(destino)
    destino.mkdir(exist_ok=True)
    script = f"""// Arma salida/ para Vercel con la web de parabellum-rs @ {commit[:7]}: la página (web/publico), la marca,
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
