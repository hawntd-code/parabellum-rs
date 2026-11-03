"""Puente de CurseForge para Parabellum (una función de Google Cloud en el proyecto de Firebase).

Parabellum no lleva la clave de la API de CurseForge: el exe se reparte y cualquiera podría sacarla. La
tiene este puente, en la variable CURSEFORGE_API_KEY, y Parabellum le pide aquí lo que necesita. También
sirve en redes que bloquean api.curseforge.com, como la de la escuela. Los archivos (.jar y .zip) NO pasan
por aquí: se bajan directo del CDN de CurseForge (forgecdn.net), que esas redes sí dejan pasar.

Solo acepta las rutas que usa Parabellum (src-tauri/src/curseforge.rs); lo demás responde 404. Las
respuestas GET se guardan unos minutos en memoria para no repetirle a CurseForge lo mismo.

También sube a Mojang, por MineSkin, las skins en PNG de los jugadores (skins.rs de Parabellum), con la
clave MINESKIN_API_KEY: Minecraft solo acepta skins de textures.minecraft.net firmadas por Mojang. Sin esa
clave, /skins/estado dice que no está disponible y Parabellum no ofrece subir un PNG.

Para probarlo en tu PC: python main.py [puerto]  (con CURSEFORGE_API_KEY; CURSEFORGE_API cambia a dónde
pregunta, para pruebas).
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

UPSTREAM = os.environ.get("CURSEFORGE_API", "https://api.curseforge.com").rstrip("/")
AGENT = "parabellum-puente/1.0 (github.com/juanrojasgarcialin/parabellum)"
ID = r"[0-9]{1,10}"
ROUTES = {
    "GET": [
        r"/v1/mods/search",
        rf"/v1/mods/{ID}",
        rf"/v1/mods/{ID}/description",
        rf"/v1/mods/{ID}/files",
        rf"/v1/mods/{ID}/files/{ID}",
        r"/v1/categories",
    ],
    "POST": [r"/v1/mods", r"/v1/mods/files", r"/v1/fingerprints/432"],
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
    if path.startswith("/v1/mods/search"):
        return 120
    if path.startswith("/v1/categories"):
        return 86400
    return 300


def _json(status, message):
    return status, {"Content-Type": "application/json"}, json.dumps({"error": message}).encode()


def compressed(status, headers, data, accept_encoding):
    """Comprime con gzip si quien pregunta lo acepta (Parabellum sí) y vale la pena."""
    if "gzip" not in (accept_encoding or "").lower() or len(data) < COMPRESS_FROM or "Content-Encoding" in headers:
        return status, headers, data
    return status, {**headers, "Content-Encoding": "gzip", "Vary": "Accept-Encoding"}, gzip.compress(data, 6)


def handle(method, path, query, body, accept_encoding="", client=""):
    """(método, ruta, query sin «?», cuerpo) -> (estado, cabeceras, cuerpo)."""
    if path.startswith("/curseforge/"):
        path = path[len("/curseforge"):]
    if path.startswith("/skins/"):
        return skins(method, path, query, body, client)
    return compressed(*_handle(method, path, query, body), accept_encoding)


# ================= Skins en PNG, por MineSkin

MINESKIN = os.environ.get("MINESKIN_API", "https://api.mineskin.org").rstrip("/")
SKIN_MAX = 24 * 1024
SKIN_EVERY = 30  # segundos entre dos de la misma dirección (los créditos de MineSkin son pocos)
_last_skin = {}


def skin_size(png):
    """(ancho, alto) si es un PNG; None si no."""
    if len(png) < 24 or png[:8] != b"\x89PNG\r\n\x1a\n" or png[12:16] != b"IHDR":
        return None
    return int.from_bytes(png[16:20], "big"), int.from_bytes(png[20:24], "big")


def multipart(fields, file_field, file_name, file_bytes):
    boundary = "parabellum" + os.urandom(12).hex()
    out = b""
    for name, value in fields.items():
        out += f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n'.encode()
    out += f'--{boundary}\r\nContent-Disposition: form-data; name="{file_field}"; filename="{file_name}"\r\nContent-Type: image/png\r\n\r\n'.encode()
    out += file_bytes + f"\r\n--{boundary}--\r\n".encode()
    return out, f"multipart/form-data; boundary={boundary}"


def skin_texture(answer):
    """La propiedad «textures» firmada de la respuesta de MineSkin (la de la v2 o la de la v1)."""
    for data in (answer.get("skin", {}).get("texture", {}).get("data"), answer.get("data", {}).get("texture")):
        if isinstance(data, dict) and data.get("value") and data.get("signature"):
            return {"value": data["value"], "signature": data["signature"]}
    return None


def skins(method, path, query, body, client):
    key_value = os.environ.get("MINESKIN_API_KEY", "").strip()
    if method == "GET" and path == "/skins/estado":
        return 200, {"Content-Type": "application/json", "Cache-Control": "public, max-age=300"}, json.dumps({"enabled": bool(key_value)}).encode()
    if method != "POST" or path != "/skins/generar":
        return _json(404, "Ruta no permitida en el puente.")
    if not key_value:
        return _json(503, "Subir skins todavía no está disponible.")
    variant = "slim" if "variant=slim" in query else "classic"
    if len(body) > SKIN_MAX or skin_size(body) not in [(64, 64), (64, 32)]:
        return _json(400, "La skin tiene que ser un PNG de 64×64 (o 64×32) de menos de 24 KB.")
    now = time.time()
    with _lock:
        if now - _last_skin.get(client, 0) < SKIN_EVERY:
            return _json(429, "Espera unos segundos antes de subir otra skin.")
        _last_skin[client] = now
        if len(_last_skin) > 5000:
            for old in [c for c, at in _last_skin.items() if now - at > SKIN_EVERY]:
                del _last_skin[old]
    data, content_type = multipart({"variant": variant, "visibility": "unlisted", "name": "parabellum"}, "file", "skin.png", body)
    request = urllib.request.Request(
        f"{MINESKIN}/v2/generate",
        data=data,
        method="POST",
        headers={"User-Agent": AGENT, "Accept": "application/json", "Content-Type": content_type, "Authorization": f"Bearer {key_value}"},
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            answer = json.loads(response.read())
    except urllib.error.HTTPError as error:
        if error.code == 429:
            return _json(429, "Se subieron muchas skins seguidas. Prueba en un minuto.")
        return _json(502, "MineSkin no aceptó la skin.")
    except Exception:
        return _json(502, "MineSkin no respondió.")
    texture = skin_texture(answer)
    if not texture:
        return _json(502, "MineSkin no devolvió la skin firmada.")
    return 200, {"Content-Type": "application/json", "Cache-Control": "no-store"}, json.dumps(texture).encode()


def _handle(method, path, query, body):
    if method == "OPTIONS":
        return 204, {"Access-Control-Allow-Origin": "*", "Access-Control-Allow-Methods": "GET, POST", "Access-Control-Allow-Headers": "Content-Type"}, b""
    if method not in ROUTES or not any(re.fullmatch(route, path) for route in ROUTES[method]):
        return _json(404, "Ruta no permitida en el puente de CurseForge.")
    if len(query) > MAX_QUERY or (body and len(body) > MAX_BODY):
        return _json(413, "Petición demasiado grande.")
    key_value = os.environ.get("CURSEFORGE_API_KEY", "").strip()
    if not key_value:
        return _json(503, "Al puente le falta la clave de CurseForge (CURSEFORGE_API_KEY).")
    key = f"{method} {path}?{query}"
    if method == "POST":
        key += " " + body.decode("utf-8", "replace")
    with _lock:
        hit = _cache.get(key)
    if hit and hit[0] > time.time():
        return 200, {**hit[1], "X-Puente-Cache": "si"}, hit[2]
    url = f"{UPSTREAM}{path}" + (f"?{query}" if query else "")
    request = urllib.request.Request(
        url,
        data=body if method == "POST" else None,
        method=method,
        headers={"User-Agent": AGENT, "Accept": "application/json", "Content-Type": "application/json", "x-api-key": key_value},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            status, data, ctype = response.status, response.read(), response.headers.get("Content-Type")
    except urllib.error.HTTPError as error:
        status, data, ctype = error.code, error.read(), error.headers.get("Content-Type")
    except Exception:
        return _json(502, "CurseForge no respondió al puente.")
    headers = {"Content-Type": ctype or "application/json"}
    # Las POST de Parabellum solo leen (archivos y mods por id): también se guardan.
    if status == 200:
        seconds = ttl(path)
        headers["Cache-Control"] = f"public, max-age={seconds}" if method == "GET" else "no-store"
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
    def curseforge(request):
        query = request.query_string.decode("utf-8", "replace") if request.query_string else ""
        body = request.get_data() if request.method == "POST" else b""
        # Cloud Run pone la dirección de quien pregunta al final de X-Forwarded-For.
        client = (request.headers.get("X-Forwarded-For", "").split(",")[-1].strip()) or request.remote_addr or ""
        status, headers, data = handle(request.method, request.path, query, body, request.headers.get("Accept-Encoding", ""), client)
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
            status, headers, data = handle(self.command, path, query, body, self.headers.get("Accept-Encoding", ""), self.client_address[0])
            self.send_response(status)
            for name, value in headers.items():
                self.send_header(name, value)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        do_GET = do_POST = do_OPTIONS = _serve

        def log_message(self, *args):
            pass

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8792
    print(f"Puente de CurseForge en http://127.0.0.1:{port}/v1/… (pregunta a {UPSTREAM})")
    ThreadingHTTPServer(("127.0.0.1", port), Local).serve_forever()
