"""Recorrido real de CurseForge, sin tocar las instancias del jugador.

python pruebas/curseforge.py --exe src-tauri/target/release/Parabellum.exe --root ../_verificacion-curseforge --base ../Parabellum/base
Usa el puente público, sin clave de API en este PC. --base solo copia archivos compartidos ya descargados.
"""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading
import urllib.parse
import urllib.request
import zipfile

BRIDGE = "https://curseforge-86460886589.us-central1.run.app"


def api(path, **query):
    url = BRIDGE + path + ("?" + urllib.parse.urlencode(query) if query else "")
    request = urllib.request.Request(url, headers={"User-Agent": "Parabellum-pruebas/0.36.0"})
    with urllib.request.urlopen(request, timeout=45) as response:
        return json.load(response)["data"]


def copy_file(source, target):
    if source.is_file() and not target.exists():
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)


def seed_base(source, target):
    """Copia únicamente Minecraft 1.21.1, su Fabric, bibliotecas, sonidos y Java."""
    for version_id in ["1.21.1", "fabric-loader-0.19.5-1.21.1"]:
        folder = source / "versions" / version_id
        for file in folder.rglob("*"):
            if file.is_file():
                copy_file(file, target / file.relative_to(source))
        config = folder / (version_id + ".json")
        if not config.exists():
            continue
        data = json.loads(config.read_text(encoding="utf-8-sig"))
        for library in data.get("libraries", []):
            downloads = library.get("downloads", {})
            artifacts = [downloads.get("artifact", {}), *downloads.get("classifiers", {}).values()]
            for artifact in artifacts:
                if artifact.get("path"):
                    relative = Path("libraries") / artifact["path"]
                    copy_file(source / relative, target / relative)
            coordinate = library.get("name", "").split(":")
            if len(coordinate) >= 3:
                group, name, version = coordinate[:3]
                suffix = "-" + coordinate[3] if len(coordinate) > 3 else ""
                relative = Path("libraries") / group.replace(".", "/") / name / version / f"{name}-{version}{suffix}.jar"
                copy_file(source / relative, target / relative)
        asset_id = data.get("assetIndex", {}).get("id")
        if asset_id:
            relative = Path("assets/indexes") / (asset_id + ".json")
            copy_file(source / relative, target / relative)
            if (source / relative).exists():
                index = json.loads((source / relative).read_text(encoding="utf-8-sig"))
                for asset in index.get("objects", {}).values():
                    digest = asset["hash"]
                    relative = Path("assets/objects") / digest[:2] / digest
                    copy_file(source / relative, target / relative)
        component = data.get("javaVersion", {}).get("component")
        if component and (source / "java" / component).is_dir():
            shutil.copytree(source / "java" / component, target / "java" / component, dirs_exist_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--base", type=Path)
    args = parser.parse_args()
    exe, root = args.exe.resolve(), args.root.resolve()
    marker = root / ".prueba-curseforge"
    if root.exists() and any(root.iterdir()) and not marker.exists():
        raise SystemExit("La carpeta de prueba debe estar vacía o tener .prueba-curseforge. Nunca uses tu carpeta real.")
    root.mkdir(parents=True, exist_ok=True)
    marker.touch()
    report = []

    def passed(message):
        report.append(message)
        print("OK: " + message, flush=True)
        (root / "resultado.txt").write_text("\n".join(report) + "\n", encoding="utf-8")

    def run(*command, bridge=BRIDGE):
        log = root / "datos/logs/comando.txt"
        log.unlink(missing_ok=True)
        environment = {**os.environ, "PARABELLUM_ROOT": str(root), "PARABELLUM_CURSEFORGE_PUENTE": bridge}
        subprocess.run([str(exe), *command], env=environment, timeout=600, check=True)
        result = log.read_text(encoding="utf-8-sig")
        if result.startswith("ERROR:"):
            raise AssertionError(result)
        return result

    def instance(name, minecraft):
        folder = root / "instancias" / name
        folder.mkdir(parents=True, exist_ok=True)
        (folder / "instancia.json").write_text(json.dumps({"name": name, "minecraftVersion": minecraft, "versionId": "fabric-loader-0.19.5-" + minecraft,
                                                       "loader": "Fabric", "loaderVersion": "0.19.5"}), encoding="utf-8")
        return folder

    categories = api("/v1/categories", gameId=432, classId=6)
    assert len(categories) > 10
    passed(f"Puente público: {len(categories)} categorías sin clave local")
    assert "CurseForge por su puente" in run("--buscar", "Architectury", "mod", "fabric", "1.21.1", "curseforge")
    passed("El ejecutable busca en CurseForge")
    projects = [419699, 306612]  # Architectury y Fabric API: mod principal y dependencia.
    files = []
    for project in projects:
        available = api(f"/v1/mods/{project}/files", gameVersion="1.21.1", modLoaderType=4, pageSize=50)
        files.append(next(file for file in available if file.get("downloadUrl") and file.get("releaseType") == 1))
    folder = instance("CurseForge Fabric", "1.21.1")
    result = run("--instalar", folder.name, "cf:419699", f"cf:{files[0]['id']}")
    assert "INSTALADO" in result
    index = json.loads((folder / ".lanzador/curseforge.json").read_text())
    assert 419699 in [entry["projectId"] for entry in index["files"].values()]
    assert 306612 in [entry["projectId"] for entry in index["files"].values()], "Falta Fabric API, la dependencia obligatoria"
    for path, entry in index["files"].items():
        assert hashlib.sha1((folder / path).read_bytes()).hexdigest() == entry["sha1"]
    passed(f"Instalación real y huellas SHA-1: {len(index['files'])} archivos")
    assert "YA INSTALADA" in run("--instalar", folder.name, "cf:419699", f"cf:{files[0]['id']}")
    passed("Volver a instalar reconoce la versión existente")
    wrong = instance("Incompatible", "1.20.1")
    assert "NO SE PUEDE" in run("--instalar", wrong.name, "cf:419699", f"cf:{files[0]['id']}")
    assert not list(wrong.glob("mods/*.jar"))
    passed("Una versión elegida a mano no se instala en otro Minecraft")
    # Mismo proyecto y archivo reales, pero con el enlace retirado por el autor.
    info = api("/v1/mods/419699")
    blocked_file = {**files[0], "downloadUrl": None}

    class Restricted(BaseHTTPRequestHandler):
        def do_POST(self):
            payload = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
            if "fileIds" in payload:
                self.reply([blocked_file if file["id"] == blocked_file["id"] else file for file in files if file["id"] in payload["fileIds"]])
            else:
                self.reply([info if project == 419699 else api(f"/v1/mods/{project}") for project in payload.get("modIds", [])])

        def do_GET(self):
            path = self.path.split("?")[0]
            if path == f"/v1/mods/419699/files/{blocked_file['id']}":
                self.reply(blocked_file)
            elif path == "/v1/mods/419699/files":
                self.reply([blocked_file])
            else:
                self.reply(api(self.path))

        def reply(self, data):
            body = json.dumps({"data": data}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):
            pass

    if args.base:
        seed_base(args.base.resolve(), root / "base")
        passed("Base de prueba preparada mediante copias, sin enlaces a datos reales")
    with ThreadingHTTPServer(("127.0.0.1", 0), Restricted) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        restricted = instance("Descarga restringida", "1.21.1")
        try:
            result = run("--instalar", restricted.name, "cf:419699", f"cf:{files[0]['id']}", bridge=f"http://127.0.0.1:{server.server_port}")
            assert "PENDIENTE EN CURSEFORGE" in result and "curseforge.com" in result
            assert not (restricted / "mods" / files[0]["fileName"]).exists()
            pending = json.loads((restricted / ".lanzador" / "curseforge-pendientes.json").read_text(encoding="utf-8"))
            assert pending[0]["fileId"] == files[0]["id"] and len(pending[0]["sha1"]) == 40
            archive = root / "recorrido-restringido.zip"
            manifest = {"manifestType": "minecraftModpack", "manifestVersion": 1, "name": "Modpack restringido", "version": "1.0",
                        "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "fabric-0.19.5", "primary": True}]},
                        "files": [{"projectID": 419699, "fileID": blocked_file["id"], "required": True}], "overrides": "overrides"}
            with zipfile.ZipFile(archive, "w") as output:
                output.writestr("manifest.json", json.dumps(manifest))
            run("--importar", str(archive), "Modpack restringido", bridge=f"http://127.0.0.1:{server.server_port}")
            imported = root / "instancias/Modpack restringido"
            queue = json.loads((imported / ".lanzador/curseforge-pendientes.json").read_text(encoding="utf-8"))
            assert queue[0]["fileId"] == blocked_file["id"] and queue[0]["sha1"] == pending[0]["sha1"]
            tracked = json.loads((imported / ".lanzador/archivos.json").read_text(encoding="utf-8"))
            assert any(item["sha1"] == pending[0]["sha1"] for item in tracked)
            assert not (imported / "mods" / blocked_file["fileName"]).exists()
            passed("Modpack restringido: conserva el archivo exacto pendiente y su huella sin descargarlo")
        finally:
            server.shutdown()
            thread.join()
    passed("Autor sin descarga externa: ofrece su página y no descarga el archivo")
    archive = root / "recorrido-curseforge.zip"
    manifest = {"manifestType": "minecraftModpack", "manifestVersion": 1, "name": "Recorrido CurseForge", "version": "1.0",
                "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "fabric-0.19.5", "primary": True}]},
                "files": [{"projectID": project, "fileID": file["id"], "required": True} for project, file in zip(projects, files)],
                "overrides": "overrides"}
    with zipfile.ZipFile(archive, "w") as output:
        output.writestr("manifest.json", json.dumps(manifest))
        output.writestr("overrides/config/prueba-curseforge.txt", "Configuración del modpack\n")
    imported = "Recorrido CurseForge"
    result = run("--importar", str(archive), imported)
    assert imported in result
    pack = root / "instancias" / imported
    assert (pack / "config/prueba-curseforge.txt").read_text(encoding="utf-8").startswith("Configuración")
    assert len(json.loads((pack / ".lanzador/curseforge.json").read_text())["files"]) == 2
    assert len(json.loads((pack / ".lanzador/archivos.json").read_text())) >= 2
    passed("Importación de .zip: instancia, loader, dos mods, overrides e índices de integridad")
    for file in files:
        downloaded = pack / "mods" / file["fileName"]
        expected = next(value["value"] for value in file["hashes"] if value["algo"] == 1)
        assert hashlib.sha1(downloaded.read_bytes()).hexdigest() == expected
    passed("Los archivos importados coinciden con las huellas de CurseForge")
    print(f"Recorrido completo: {len(report)} comprobaciones. Informe: {root / 'resultado.txt'}", flush=True)


if __name__ == "__main__":
    main()
