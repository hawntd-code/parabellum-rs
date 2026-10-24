"""Receta original y reinstalación real de ATM10SKY en una raíz separada.

La base de Minecraft se copia como caché; los mods y overrides se instalan con el launcher.
No usa sesiones, mundos ni configuraciones personales de la instancia de origen.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--base', type=Path, required=True)
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--complete', action='store_true')
    args = parser.parse_args()
    root = args.root.resolve()
    marker = root / '.prueba-0391'
    origin = root / 'instancias' / 'Origen ATM10SKY'
    if args.prepare:
        if root.exists() and any(root.iterdir()):
            raise SystemExit('La preparación requiere una carpeta vacía.')
        if shutil.disk_usage(root.parent).free < 8 * 1024**3:
            raise SystemExit('Se requieren 8 GiB libres para esta prueba.')
        origin.mkdir(parents=True)
        marker.touch()
        original = json.loads((args.source / 'instancia.json').read_text(encoding='utf-8-sig'))
        # Solo el contrato del pack; nada de opciones, rutas, nombres o argumentos personales.
        metadata = {k: original[k] for k in ['loader', 'loaderVersion', 'minecraftVersion', 'versionId', 'modpack']}
        metadata['name'] = 'ATM10SKY compartido'
        (origin / 'instancia.json').write_text(json.dumps(metadata, ensure_ascii=False), encoding='utf-8')
        for directory in ['assets', 'java', 'libraries', 'natives', 'versions']:
            shutil.copytree(args.base / directory, root / 'base' / directory)
        print('Base copiada; destino sin mods ni overrides.', flush=True)
        return
    if not marker.exists():
        raise SystemExit('Falta preparar la raíz de prueba.')
    environment = {**os.environ, 'PARABELLUM_ROOT': str(root)}
    environment['USERPROFILE'] = str(root / 'usuario')
    environment['TEMP'] = environment['TMP'] = str(root / 'temporales')
    (root / 'temporales').mkdir(exist_ok=True)
    if args.complete:
        # Introduce únicamente copias públicas que coinciden con los pendientes reales del instalador.
        # La consulta del asistente revalida estos bytes antes de recibirlos en la instancia nueva.
        cache = root / 'datos' / 'curseforge-verificados'
        cache.mkdir(parents=True, exist_ok=True)
        queues = list((root / 'instancias').glob('*/.lanzador/curseforge-pendientes.json'))
        count = 0
        for queue in queues:
            for file in json.loads(queue.read_text(encoding='utf-8')):
                source = args.source / file['folder'] / file['fileName']
                if not source.exists():
                    raise SystemExit('El origen no tiene: ' + file['fileName'])
                with source.open('rb') as stream:
                    sha1 = hashlib.file_digest(stream, 'sha1').hexdigest()
                assert sha1.lower() == file['sha1'].lower() and source.stat().st_size == file['size']
                shutil.copy2(source, cache / sha1)
                count += 1
        print(f'{count} archivos recibidos previamente, con huella y tamaño oficiales, preparados en caché.', flush=True)
        return
    log = root / 'compartida.txt'
    before = set((root / 'instancias').iterdir())
    with log.open('w', encoding='utf-8') as output:
        result = subprocess.run([str(args.exe.resolve()), '--probar-receta', origin.name, 'instalar'],
                                env=environment, stdout=output, stderr=subprocess.STDOUT, timeout=1800)
    command = (root / 'datos/logs/comando.txt').read_text(encoding='utf-8-sig')
    with log.open('a', encoding='utf-8') as output:
        output.write('\n' + command)
    print(log.read_text(encoding='utf-8', errors='replace')[-7000:])
    if result.returncode or command.startswith('ERROR:'):
        print(command)
        if not result.returncode:
            raise SystemExit(1)
        raise SystemExit(result.returncode)
    text = log.read_text(encoding='utf-8')
    assert 'parabellum:instancia2:' in text and '"s":"curseforge"' in text
    assert '"p":"cf:1298402"' in text and '"v":"cf:9261952"' in text
    assert '"manual"' in text and '"warning": null' not in text
    installed = [p for p in (root/'instancias').iterdir() if p.is_dir() and p not in before]
    assert len(installed) == 1
    metadata = json.loads((installed[0]/'instancia.json').read_text(encoding='utf-8'))
    assert metadata['modpack']['source'] == 'curseforge'
    assert metadata['modpack']['projectId'] == 'cf:1298402' and metadata['modpack']['versionId'] == 'cf:9261952'
    assert (installed[0]/'config').is_dir()
    pending = json.loads((installed[0]/'.lanzador/curseforge-pendientes.json').read_text(encoding='utf-8'))
    assert {6282838, 6981252}.issubset({f['fileId'] for f in pending})
    for file in pending:
        assert not (installed[0]/file['folder']/file['fileName']).exists()
    report = {'project':'cf:1298402', 'version':'cf:9261952', 'instance':installed[0].name,
              'pending':[f['fileId'] for f in pending], 'mods':len(list((installed[0]/'mods').glob('*.jar'))),
              'configFiles':len([f for f in (installed[0]/'config').rglob('*') if f.is_file()]),
              'scope':'Receta y el instalador real, con base copiada como caché. No envía mensajes en Firebase.'}
    (root/'resultado-compartir.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)


if __name__ == '__main__':
    main()
