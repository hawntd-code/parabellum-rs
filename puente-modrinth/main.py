"""Puente de Modrinth para Parabellum (una función de Google Cloud en el proyecto de Firebase).

En redes que bloquean la API de Modrinth (api.modrinth.com), como la de la escuela, Parabellum le pide aquí
lo mismo y este puente se lo pregunta a Modrinth. Los archivos (los .jar y .mrpack) NO pasan por aquí: se
bajan directo de cdn.modrinth.com, que esas redes sí dejan pasar.

Solo acepta las rutas que usa Parabellum (src-tauri/src/modrinth.rs); lo demás responde 404. Las
respuestas GET se guardan unos minutos en memoria para no repetirle a Modrinth lo mismo.

Para probarlo en tu PC: python main.py [puerto]  (MODRINTH_API cambia a dónde pregunta, para pruebas).
"""
import gzip
import json
import os
import re
import sys
import threading
import time
import urllib.error
import urllib.request

UPSTREAM = os.environ.get("MODRINTH_API", "https://api.modrinth.com").rstrip("/")
# Modrinth pide un User-Agent que identifique al programa.
AGENT = "juanrojasgarcialin/parabellum-puente/1.0"
ID = r"[A-Za-z0-9._-]{1,64}"
ROUTES = {
    "GET": [
        r"/v2/search",
        rf"/v2/project/{ID}",
        rf"/v2/project/{ID}/version",
        r"/v2/projects",
        r"/v2/tag/category",
        rf"/v2/version/{ID}",
        r"/v2/versions",
        rf"/v2/organization/{ID}",
    ],
    "POST": [r"/v2/version_files", r"/v2/version_files/update"],
}
MAX_BODY = 512 * 1024
# Lo que sale del puente es lo que se cobra (tráfico de salida): el JSON comprimido ocupa ~5 veces menos.
COMPRESS_FROM = 1024
MAX_QUERY = 4096
CACHE_ENTRIES = 600
_cache = {}
_lock = threading.Lock()


def ttl(path):
    """Cuánto se guarda una respuesta: las búsquedas poco, las categorías mucho."""
    if path.startswith("/v2/search"):
        return 120
    if path.startswith("/v2/tag/"):
        return 86400
    return 300


def _json(status, message):
    return status, {"Content-Type": "application/json"}, json.dumps({"error": message}).encode()


def compressed(status, headers, data, accept_encoding):
    """Comprime con gzip si quien pregunta lo acepta (Parabellum sí) y vale la pena."""
    if "gzip" not in (accept_encoding or "").lower() or len(data) < COMPRESS_FROM or "Content-Encoding" in headers:
        return status, headers, data
    return status, {**headers, "Content-Encoding": "gzip", "Vary": "Accept-Encoding"}, gzip.compress(data, 6)


def handle(method, path, query, body, accept_encoding=""):
    """(método, ruta, query sin «?», cuerpo) -> (estado, cabeceras, cuerpo)."""
    return compressed(*_handle(method, path, query, body), accept_encoding)


def _handle(method, path, query, body):
    if path.startswith("/modrinth/"):  # por la URL de cloudfunctions.net llega con el nombre delante
        path = path[len("/modrinth"):]
    if method == "OPTIONS":
        return 204, {"Access-Control-Allow-Origin": "*", "Access-Control-Allow-Methods": "GET, POST", "Access-Control-Allow-Headers": "Content-Type"}, b""
    if method not in ROUTES or not any(re.fullmatch(route, path) for route in ROUTES[method]):
        return _json(404, "Ruta no permitida en el puente de Modrinth.")
    if len(query) > MAX_QUERY or (body and len(body) > MAX_BODY):
        return _json(413, "Petición demasiado grande.")
    key = f"{path}?{query}"
    if method == "GET":
        with _lock:
            hit = _cache.get(key)
        if hit and hit[0] > time.time():
            return 200, {**hit[1], "X-Puente-Cache": "si"}, hit[2]
    url = f"{UPSTREAM}{path}" + (f"?{query}" if query else "")
    request = urllib.request.Request(
        url,
        data=body if method == "POST" else None,
        method=method,
        headers={"User-Agent": AGENT, "Accept": "application/json", "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            status, data, ctype = response.status, response.read(), response.headers.get("Content-Type")
    except urllib.error.HTTPError as error:
        status, data, ctype = error.code, error.read(), error.headers.get("Content-Type")
    except Exception:
        return _json(502, "Modrinth no respondió al puente.")
    headers = {"Content-Type": ctype or "application/json"}
    if method == "GET" and status == 200:
        seconds = ttl(path)
        headers["Cache-Control"] = f"public, max-age={seconds}"
        with _lock:
            if len(_cache) >= CACHE_ENTRIES:
                # Se van primero las que vencen antes.
                for old in sorted(_cache, key=lambda k: _cache[k][0])[: CACHE_ENTRIES // 4]:
                    del _cache[old]
            _cache[key] = (time.time() + seconds, headers, data)
    return status, headers, data


try:
    import functions_framework

    @functions_framework.http
    def modrinth(request):
        query = request.query_string.decode("utf-8", "replace") if request.query_string else ""
        body = request.get_data() if request.method == "POST" else b""
        status, headers, data = handle(request.method, request.path, query, body, request.headers.get("Accept-Encoding", ""))
        return data, status, headers

except ImportError:  # en tu PC, sin functions_framework: python main.py
    pass


if __name__ == "__main__":
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

    class Local(BaseHTTPRequestHandler):
        def _serve(self):
            path, _, query = self.path.partition("?")
            length = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(length) if length else b""
            status, headers, data = handle(self.command, path, query, body, self.headers.get("Accept-Encoding", ""))
            self.send_response(status)
            for name, value in headers.items():
                self.send_header(name, value)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        do_GET = do_POST = do_OPTIONS = _serve

        def log_message(self, *args):
            pass

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8790
    print(f"Puente de Modrinth en http://127.0.0.1:{port}/v2/… (pregunta a {UPSTREAM})")
    ThreadingHTTPServer(("127.0.0.1", port), Local).serve_forever()
