"""Prueba el exe final mediante un proxy local: no cambia la red de Windows ni usa sesiones.

Requiere internet para comprobar orígenes y puentes reales. --base permite verificar además la
reposición de un recurso de Minecraft desde Mojang y desde el espejo, copiando solo archivos base.
"""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import select
import shutil
import socket
import subprocess
import threading


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--base', type=Path)
    args = parser.parse_args()
    root, exe = args.root.resolve(), args.exe.resolve()
    if root.exists() and any(root.iterdir()):
        raise SystemExit('Usa una carpeta de prueba nueva y vacía.')
    root.mkdir(parents=True, exist_ok=True)
    (root/'.prueba-red-automatica').touch()
    blocked, hosts = set(), []
    checks = []
    result = {'scope': 'Restricciones controladas solo para el proceso, no sustituye la red física de la escuela.', 'checks': checks}

    def passed(name, output):
        checks.append({'check': name, 'output': output, 'hosts': sorted(set(hosts))})
        (root/'resultado.json').write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
        print('OK: '+name, flush=True)

    class Proxy(BaseHTTPRequestHandler):
        def do_CONNECT(self):
            host, port = self.path.rsplit(':', 1)
            hosts.append(host)
            if host in blocked or '*' in blocked or port != '443':
                self.send_error(403, 'Restriccion de prueba')
                return
            try:
                with socket.create_connection((host, 443), timeout=8) as remote:
                    self.send_response(200)
                    self.end_headers()
                    peers = [self.connection, remote]
                    while True:
                        readable, _, _ = select.select(peers, [], [], 25)
                        if not readable:
                            return
                        for source in readable:
                            data = source.recv(65536)
                            if not data:
                                return
                            (remote if source is self.connection else self.connection).sendall(data)
            except (OSError, TimeoutError):
                return

        def log_message(self, *_):
            pass

    class Unavailable(BaseHTTPRequestHandler):
        def do_GET(self):
            hosts.append('fixture-curseforge')
            self.send_response(503)
            self.send_header('Content-Length', '0')
            self.end_headers()

        def log_message(self, *_):
            pass

    with ThreadingHTTPServer(('127.0.0.1', 0), Proxy) as proxy, ThreadingHTTPServer(('127.0.0.1', 0), Unavailable) as cf:
        for server in (proxy, cf):
            threading.Thread(target=server.serve_forever, daemon=True).start()
        address = f'http://127.0.0.1:{proxy.server_port}'

        def run(folder, *commands, fallback=False):
            folder.mkdir(parents=True, exist_ok=True)
            settings = folder/'datos/ajustes.json'
            if not settings.exists():
                settings.parent.mkdir(parents=True, exist_ok=True)
                settings.write_text('{"language":"es","username":"PruebaRed","closeToTray":false}', encoding='utf-8')
            env = {**os.environ, 'PARABELLUM_ROOT': str(folder), 'USERPROFILE': str(root/'usuario')}
            for key in ('HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'http_proxy', 'https_proxy', 'all_proxy'):
                env[key] = address
            env.update(NO_PROXY='127.0.0.1,localhost', no_proxy='127.0.0.1,localhost')
            if fallback:
                env['PARABELLUM_CURSEFORGE_PUENTE'] = f'http://127.0.0.1:{cf.server_port}'
            log = folder/'datos/logs/comando.txt'
            log.unlink(missing_ok=True)
            hosts.clear()
            subprocess.run([str(exe), *commands], env=env, capture_output=True, timeout=300, check=True)
            output = log.read_text(encoding='utf-8-sig')
            assert not output.startswith('ERROR:'), output
            return output

        normal = root/'normal'
        (normal/'datos').mkdir(parents=True)
        (normal/'datos/ajustes.json').write_text('{"language":"es","schoolMode":true,"username":"PruebaRed","closeToTray":false}', encoding='utf-8')
        output = run(normal, '--probar-red')
        for service in ('Mojang', 'Modrinth', 'Portal'):
            assert f'{service}: Direct' in output, output
        assert not any(host.endswith('.run.app') for host in hosts)
        passed('Red normal y ajuste antiguo: acceso directo sin puentes', output)

        output = run(root/'curseforge-cloud', '--buscar', 'Architectury', 'mod', 'fabric', '1.21.1', 'curseforge')
        assert 'CurseForge por su puente' in output and any(host.startswith('curseforge-') for host in hosts), output
        assert 'api.modrinth.com' not in hosts
        passed('CurseForge conserva su API en Cloud', output)

        output = run(root/'respaldo-mr', '--buscar', 'Architectury', 'mod', 'fabric', '1.21.1', 'curseforge', fallback=True)
        assert 'Mostramos los resultados de Modrinth' in output and 'directo' in output, output
        assert 'fixture-curseforge' in hosts and 'api.modrinth.com' in hosts
        assert not any(host.startswith('modrinth-') for host in hosts)
        passed('CurseForge caído: respaldo en Modrinth directo', output)

        blocked.add('api.modrinth.com')
        output = run(root/'puente-mr', '--buscar', 'Architectury', 'mod', 'fabric', '1.21.1')
        assert 'por el puente' in output and any(host.startswith('modrinth-') for host in hosts), output
        passed('Modrinth restringido con internet: usa su puente', output)

        blocked.add('modrinth-86460886589.us-central1.run.app')
        output = run(root/'respaldo-drive', '--buscar', 'Architectury', 'mod', 'fabric', '1.21.1', 'curseforge', fallback=True)
        assert 'catálogo Drive' in output, output
        passed('CurseForge y Modrinth indisponibles: ofrece Drive', output)

        blocked.clear()
        blocked.update(['piston-meta.mojang.com', 'meta.fabricmc.net', 'meta.quiltmc.org', 'maven.minecraftforge.net', 'maven.neoforged.net', 'api.modrinth.com', 'static-beta.curseforge.com'])
        output = run(root/'restringida', '--probar-red')
        for service in ('Mojang', 'Fabric', 'Quilt', 'Forge', 'NeoForge', 'Modrinth', 'Portal'):
            assert f'{service}: Restricted' in output, output
        passed('Restricción por servicio, incluido portal sin estilos', output)

        blocked.clear()
        blocked.add('*')
        output = run(root/'sin-internet', '--probar-red')
        assert output.count(': Offline') == 7, output
        assert not any(host.endswith('.run.app') for host in hosts)
        passed('Sin internet: no activa puentes', output)

        if args.base:
            from curseforge import seed_base
            for label, restricted in [('recurso-directo', False), ('recurso-espejo', True)]:
                folder = root/label
                seed_base(args.base.resolve(), folder/'base')
                index = json.loads((folder/'base/assets/indexes/17.json').read_text(encoding='utf-8'))
                entry = min((item for item in index['objects'].values() if item['size'] > 0), key=lambda item: item['size'])
                digest = entry['hash']
                resource = folder/'base/assets/objects'/digest[:2]/digest
                assert resource.is_file() and entry['size'] > 0
                resource.unlink()  # Solo una copia en la carpeta nueva marcada arriba.
                blocked.clear()
                if restricted:
                    blocked.update(['piston-meta.mojang.com', 'piston-data.mojang.com', 'resources.download.minecraft.net', 'libraries.minecraft.net', 'launchermeta.mojang.com'])
                output = run(folder, '--crear', 'Prueba recursos', '1.21.1')
                assert resource.is_file() and hashlib.sha1(resource.read_bytes()).hexdigest() == digest
                if restricted:
                    assert any('google' in host or 'drive.' in host for host in hosts), hosts
                else:
                    assert 'resources.download.minecraft.net' in hosts, hosts
                passed(label+': repone el archivo exacto', output)
        for server in (proxy, cf):
            server.shutdown()
    result['result'] = 'correcto'
    (root/'resultado.json').write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')


if __name__ == '__main__':
    main()
