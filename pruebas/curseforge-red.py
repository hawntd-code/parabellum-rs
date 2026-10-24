"""Fallos controlados de CurseForge, con la API y el CDN bloqueados solo para el proceso de prueba.

No modifica la red de Windows ni usa sesiones. Consulta metadatos públicos, comprueba copias locales
de los dos mods indicados por Juan y prueba los pendientes con el ejecutable de distribución.
"""
import argparse
import copy
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading
import urllib.request

BRIDGE = 'https://curseforge-86460886589.us-central1.run.app'
CASES = [(417702, 6282838), (391382, 6981252)]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', required=True, type=Path)
    parser.add_argument('--root', required=True, type=Path)
    parser.add_argument('--mods', required=True, type=Path)
    args = parser.parse_args()
    root, exe = args.root.resolve(), args.exe.resolve()
    marker = root / '.prueba-curseforge-red'
    if root.exists() and any(root.iterdir()):
        raise SystemExit('Usa una carpeta de prueba nueva y vacía.')
    root.mkdir(parents=True, exist_ok=True)
    marker.touch()
    report = {'scope': 'CLI real y fallos HTTP/proxy simulados; no reproduce el filtro exacto de la escuela ni prueba CSS/JavaScript del WebView', 'cases': [], 'checks': []}
    def passed(name):
        report['checks'].append(name)
        (root/'resultados.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
        print('OK: '+name, flush=True)
    def api(path):
        request = urllib.request.Request(BRIDGE+path, headers={'User-Agent':'Parabellum-pruebas/0.39.0'})
        with urllib.request.urlopen(request, timeout=45) as response:
            return json.load(response)['data']
    infos, files = {}, {}
    for project, file_id in CASES:
        info, file = api(f'/v1/mods/{project}'), api(f'/v1/mods/{project}/files/{file_id}')
        assert file['modId']==project and file['id']==file_id
        original = args.mods/file['fileName']
        expected = next(h['value'] for h in file['hashes'] if h['algo']==1)
        with original.open('rb') as stream:
            actual = hashlib.file_digest(stream, 'sha1').hexdigest()
        assert actual.lower()==expected.lower() and original.stat().st_size==file['fileLength']
        # Solo se copian los dos jars ya presentes, para revisar huellas en el entorno aislado.
        target = root/'archivos-verificados'/file['fileName']
        target.parent.mkdir(exist_ok=True)
        shutil.copy2(original, target)
        report['cases'].append({'project':project,'file':file_id,'name':file['fileName'],'sha1':actual,'bytes':original.stat().st_size,'apiDownloadAvailable':bool(file.get('downloadUrl'))})
        infos[project], files[file_id] = info, file
        passed('Versión y huella oficiales: '+file['fileName'])
    instance = root/'instancias'/'Prueba red NeoForge'
    instance.mkdir(parents=True)
    (instance/'instancia.json').write_text(json.dumps({'name':instance.name,'minecraftVersion':'1.21.1','versionId':'neoforge-21.1.250','loader':'NeoForge','loaderVersion':'21.1.250'}),encoding='utf-8')
    queue_path = instance/'.lanzador'/'curseforge-pendientes.json'
    state = {'mode':'restricted', 'requests':[], 'proxyHosts':[]}
    class Bridge(BaseHTTPRequestHandler):
        def do_POST(self):
            body=json.loads(self.rfile.read(int(self.headers.get('Content-Length',0))))
            self.respond([infos[p] for p in body.get('modIds',[]) if p in infos])
        def do_GET(self):
            parts=self.path.split('?')[0].strip('/').split('/')
            if len(parts)==5 and parts[:2]==['v1','mods'] and parts[3]=='files':
                file=copy.deepcopy(files[int(parts[4])])
                file['downloadUrl']=None if state['mode']!='cdn-blocked' else 'https://edge.forgecdn.net/prueba-controlada/'+file['fileName']
                # Dependencias excluidas para aislar el tratamiento de este archivo pendiente.
                file['dependencies']=[]
                self.respond(file)
            elif len(parts)==3 and parts[:2]==['v1','mods']:
                self.respond(infos[int(parts[2])])
            else:
                self.send_error(404)
        def respond(self, data):
            state['requests'].append(self.path)
            if state['mode'] in ('403','429','503'):
                status=int(state['mode']); payload=b'blocked'; mime='text/plain'
            elif state['mode']=='html':
                status=200; payload=b'<html>Pagina de bloqueo simulada</html>'; mime='text/html'
            else:
                status=200; payload=json.dumps({'data':data}).encode(); mime='application/json'
            self.send_response(status); self.send_header('Content-Type',mime); self.send_header('Content-Length',str(len(payload))); self.end_headers(); self.wfile.write(payload)
        def log_message(self,*args): pass
    class Blocker(BaseHTTPRequestHandler):
        def do_CONNECT(self):
            state['proxyHosts'].append(self.path)
            self.send_error(403,'Bloqueo controlado de prueba')
        def do_GET(self): self.send_error(403)
        def log_message(self,*args): pass
    def queue():
        values=json.loads(queue_path.read_text(encoding='utf-8')) if queue_path.exists() else []
        return sorted(values, key=lambda item:item['fileId'])
    def no_jars(): return not list((instance/'mods').glob('*.jar'))
    def run(project,file_id,bridge,proxy):
        log=root/'datos'/'logs'/'comando.txt'
        log.unlink(missing_ok=True)
        env={**os.environ,'PARABELLUM_ROOT':str(root),'PARABELLUM_CURSEFORGE_PUENTE':bridge,'HTTP_PROXY':proxy,'HTTPS_PROXY':proxy,'ALL_PROXY':proxy,'http_proxy':proxy,'https_proxy':proxy,'all_proxy':proxy,'NO_PROXY':'127.0.0.1,localhost','no_proxy':'127.0.0.1,localhost'}
        result=subprocess.run([str(exe),'--instalar',instance.name,f'cf:{project}',f'cf:{file_id}'],env=env,capture_output=True,timeout=150)
        assert log.exists(), f'No se generó el registro: {result.returncode}'
        return log.read_text(encoding='utf-8-sig')
    with ThreadingHTTPServer(('127.0.0.1',0),Bridge) as server, ThreadingHTTPServer(('127.0.0.1',0),Blocker) as blocker:
        threads=[threading.Thread(target=s.serve_forever,daemon=True) for s in (server,blocker)]
        for thread in threads: thread.start()
        local=f'http://127.0.0.1:{server.server_port}'; proxy=f'http://127.0.0.1:{blocker.server_port}'
        try:
            for project,file_id in CASES:
                output=run(project,file_id,local,proxy)
                assert 'PENDIENTE EN CURSEFORGE' in output and 'ERROR:' not in output, output
            assert len(queue())==2 and no_jars()
            passed('Los dos archivos exactos quedan pendientes y no aparecen como instalados')
            before=queue()
            run(*CASES[0],local,proxy)
            assert queue()==before and no_jars()
            passed('Reabrir el ejecutable y repetir conserva la cola sin duplicados')
            for mode in ['403','429','503','html']:
                state['mode']=mode
                output=run(*CASES[0],local,proxy)
                assert output.startswith('ERROR:') and queue()==before and no_jars(),output
                passed('Respuesta '+mode+': error explícito y pendientes conservados')
            state['mode']='cdn-blocked'
            output=run(*CASES[0],local,proxy)
            assert output.startswith('ERROR:') and queue()==before and no_jars(),output
            assert any(host.startswith('edge.forgecdn.net:') for host in state['proxyHosts'])
            passed('API disponible y CDN bloqueado: falla la descarga sin declarar instalación completa')
            state['mode']='restricted'
            output=run(*CASES[0],BRIDGE,proxy)
            assert output.startswith('ERROR:') and queue()==before and no_jars(),output
            assert any(host.startswith('curseforge-') for host in state['proxyHosts'])
            passed('Puente remoto bloqueado: error de conexión y cola conservada')
        finally:
            for service in (server,blocker): service.shutdown()
            for thread in threads: thread.join()
    report['proxyHosts']=sorted(set(state['proxyHosts']))
    report['result']='correcto'
    (root/'resultados.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(f"Terminadas {len(report['checks'])} comprobaciones.",flush=True)

if __name__=='__main__': main()
