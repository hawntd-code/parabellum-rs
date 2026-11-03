"""Banco de pruebas de los modpacks más populares (plan de noviembre, 2026-11-02).

Instala y revisa los N modpacks más descargados de Modrinth y de CurseForge con el Parabellum
compilado, en una carpeta de prueba aparte (nunca la de nadie). Con --abrir, además abre cada uno
hasta el menú principal. Al final deja un informe en Markdown y sale con código 1 si alguno falló.

Uso, desde la raíz del repo (después de `cargo build --release` en src-tauri):

    python docs/banco-modpacks.py [--n 10] [--abrir] [--raiz CARPETA] [--exe RUTA] [--informe ARCHIVO]

En GitHub Actions (.github/workflows/banco.yml, cada lunes) corre sin --abrir: allá no hay tarjeta
gráfica para abrir Minecraft. Para la prueba completa, córrelo en un PC con --abrir.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import time
from datetime import date
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")
REPO = Path(__file__).resolve().parent.parent


def run(exe, root, *args, timeout):
    """Corre un comando --… de Parabellum en `root` y devuelve lo que dejó en datos/logs/comando.txt."""
    out = root / "datos" / "logs" / "comando.txt"
    out.unlink(missing_ok=True)
    env = dict(os.environ, PARABELLUM_ROOT=str(root))
    try:
        subprocess.run([str(exe), *args], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=timeout)
    except subprocess.TimeoutExpired:
        return f"ERROR: no terminó en {timeout // 60} minutos"
    return out.read_text(encoding="utf-8", errors="replace") if out.exists() else "ERROR: no dejó resultado"


def test(exe, root, project, name, open_game):
    result = {"name": name, "project": project, "ok": True, "notes": []}
    started = time.time()
    text = run(exe, root, "--instalar-modpack", project, timeout=40 * 60)
    result["install"] = round(time.time() - started)
    folder = re.search(r'carpeta Some\("(.+?)"\)', text)
    if text.startswith("ERROR") or not folder:
        result["ok"] = False
        result["notes"].append("No se instaló: " + text.strip().splitlines()[0][:300] if text.strip() else "No se instaló.")
        return result
    folder = folder.group(1)
    manual = [line[len("MANUAL: "):] for line in text.splitlines() if line.startswith("MANUAL: ")]
    if manual:
        result["notes"].append(f"{len(manual)} archivo(s) para bajar a mano: " + ", ".join(m.split(" — ")[0] for m in manual[:5]))
    for line in text.splitlines():
        if line.startswith("AVISO: "):
            result["notes"].append(line)
    review = run(exe, root, "--revisar", folder, timeout=10 * 60)
    errors = [line for line in review.splitlines() if line.lower().startswith(("error", "grave"))]
    if review.startswith("ERROR") or errors:
        result["ok"] = False
        result["notes"].extend((errors or [review.strip()])[:5])
    if open_game and result["ok"] and not manual:
        game = run(exe, root, "--probar-juego", folder, "420", timeout=15 * 60)
        menu = re.search(r"MENÚ PRINCIPAL en (\d+) s", game)
        if menu:
            result["menu"] = int(menu.group(1))
        else:
            result["ok"] = False
            result["notes"].extend(line.strip() for line in game.splitlines()[1:4] if line.strip())
    # Cada modpack ocupa mucho: se borra al terminar (Minecraft, Java y las bibliotecas se reutilizan).
    shutil.rmtree(root / "instancias" / folder, ignore_errors=True)
    shutil.rmtree(root / "datos" / "temporal", ignore_errors=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--n", type=int, default=10, help="cuántos de cada sitio (10)")
    parser.add_argument("--abrir", action="store_true", help="abrir cada uno hasta el menú principal")
    parser.add_argument("--raiz", type=Path, default=REPO.parent / "_banco-modpacks", help="carpeta de prueba")
    parser.add_argument("--exe", type=Path, default=REPO / "src-tauri" / "target" / "release" / "Parabellum.exe")
    parser.add_argument("--informe", type=Path, default=None, help="dónde dejar el informe (por defecto, en la carpeta de prueba)")
    args = parser.parse_args()
    root = args.raiz.resolve()
    root.mkdir(parents=True, exist_ok=True)
    results = []
    for source, label in [("", "Modrinth"), ("curseforge", "CurseForge")]:
        listing = run(args.exe, root, "--populares", str(args.n), *([source] if source else []), timeout=120)
        if listing.startswith("ERROR"):
            results.append({"name": f"(lista de {label})", "project": "", "ok": False, "notes": [listing.strip()], "install": 0})
            continue
        for line in listing.splitlines():
            if "\t" not in line:
                continue
            project, name = line.split("\t", 1)
            print(f"[{label}] {name}…", flush=True)
            result = test(args.exe, root, project, name, args.abrir)
            result["source"] = label
            results.append(result)
            print(("  bien" if result["ok"] else "  FALLÓ") + "".join(f"\n  · {n}" for n in result["notes"]), flush=True)
    failed = [r for r in results if not r["ok"]]
    lines = [
        f"# Banco de modpacks · {date.today().isoformat()}",
        "",
        f"{len(results) - len(failed)} de {len(results)} bien" + (" (con prueba de abrir hasta el menú)." if args.abrir else " (instalar y revisar; sin abrir el juego)."),
        "",
        "| Sitio | Modpack | Instalación | Menú | Resultado | Notas |",
        "|---|---|---|---|---|---|",
    ]
    for r in results:
        notes = " · ".join(n.replace("|", "/") for n in r["notes"]) or "—"
        menu = f"{r['menu']} s" if "menu" in r else "—"
        lines.append(f"| {r.get('source', '')} | {r['name']} (`{r['project']}`) | {r['install']} s | {menu} | {'bien' if r['ok'] else '**falló**'} | {notes} |")
    report = "\n".join(lines) + "\n"
    target = args.informe or root / f"informe-{date.today().isoformat()}.md"
    target.write_text(report, encoding="utf-8")
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as f:
            f.write(report)
    print(f"\nInforme: {target}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
