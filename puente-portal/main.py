"""Relevo TCP limitado al portal oficial. Los bytes TLS nunca se descifran aquí.

Cloud Run: un proceso, máximo una instancia, 20 hilos, concurrencia 20, timeout 60 s.
No comparte la clave ni el servicio de la API. No acepta destinos elegidos por URL.
"""
import secrets
import json
import socket
import threading
import time

import functions_framework

HOSTS = frozenset({
    "curseforge.com", "www.curseforge.com", "static.curseforge.com", "static-beta.curseforge.com",
    "media.forgecdn.net", "mediafilez.forgecdn.net", "edge.forgecdn.net",
    "static.forgecdn.net", "static-assets.overwolf.com", "challenges.cloudflare.com",
})
MAX_SESSIONS = 8
MAX_CHUNK = 65536
MAX_BYTES = 128 * 1024 * 1024
TTL = 900
LOCK = threading.Lock()
SESSIONS = {}


class Tunnel:
    def __init__(self, stream):
        self.stream = stream
        self.created = time.monotonic()
        self.touched = self.created
        self.bytes = 0
        self.sending = threading.Lock()
        self.receiving = threading.Lock()

    def close(self):
        try:
            self.stream.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.stream.close()


def discard(key):
    with LOCK:
        tunnel = SESSIONS.pop(key, None)
    if tunnel:
        tunnel.close()


def cleanup():
    now = time.monotonic()
    with LOCK:
        expired = [key for key, tunnel in SESSIONS.items() if now - tunnel.created > TTL or now - tunnel.touched > 60]
    for key in expired:
        discard(key)


@functions_framework.http
def portal_curseforge(request):
    headers = {"Cache-Control": "no-store", "X-Content-Type-Options": "nosniff"}
    if request.path == "/health" and request.method == "GET":
        return ({"service": "portal-curseforge", "protocol": 1}, 200, headers)
    if request.method != "POST":
        return ("Método no permitido", 405, headers)
    # HTTP/2 puede omitir Content-Length incluso con cuerpo. La lectura acotada mantiene
    # el mismo límite para peticiones con longitud conocida o transferencia por fragmentos.
    if (request.content_length or 0) > MAX_CHUNK:
        return ("Tamaño no permitido", 413, headers)
    cleanup()
    action = request.path
    data = b""
    if action in {"/open", "/send"}:
        data = request.stream.read(MAX_CHUNK + 1)
        if len(data) > MAX_CHUNK:
            return ("Tamaño no permitido", 413, headers)
    if action == "/open":
        try:
            body = json.loads(data)
        except (ValueError, UnicodeDecodeError):
            return ("Solicitud inválida", 400, headers)
        if not isinstance(body, dict):
            return ("Solicitud inválida", 400, headers)
        host = body.get("host")
        if not isinstance(host, str) or host not in HOSTS:
            return ("Destino no permitido", 403, headers)
        # Se reserva el cupo antes de conectar; no se admiten carreras para exceder el límite.
        key = secrets.token_urlsafe(32)
        with LOCK:
            if len(SESSIONS) >= MAX_SESSIONS:
                return ("Reintenta en unos segundos", 429, headers)
            SESSIONS[key] = Tunnel(socket.socket())
        try:
            stream = socket.create_connection((host, 443), timeout=15)
            stream.settimeout(20)
            with LOCK:
                old = SESSIONS[key]
                SESSIONS[key] = Tunnel(stream)
            old.close()
            return ({"session": key}, 200, headers)
        except OSError:
            discard(key)
            return ("No se pudo conectar con el portal", 502, headers)
    key = request.headers.get("X-Portal-Session", "")
    with LOCK:
        tunnel = SESSIONS.get(key)
        if tunnel is not None:
            tunnel.touched = time.monotonic()
    if tunnel is None:
        return ("Conexión terminada", 410, headers)
    try:
        if action == "/close":
            discard(key)
            return (b"", 200, headers)
        if action == "/send":
            with tunnel.sending:
                tunnel.stream.sendall(data)
            count = len(data)
            output = b""
        elif action == "/receive":
            # Un único lector por conexión; no bloquea el envío simultáneo del TLS.
            if not tunnel.receiving.acquire(blocking=False):
                return ("Lectura en curso", 409, headers)
            try:
                output = tunnel.stream.recv(MAX_CHUNK)
            except socket.timeout:
                return (b"", 204, headers)
            finally:
                tunnel.receiving.release()
            if not output:
                discard(key)
                return (b"", 410, headers)
            count = len(output)
        else:
            return ("Ruta no disponible", 404, headers)
        with LOCK:
            tunnel.bytes += count
            exhausted = tunnel.bytes > MAX_BYTES
        if exhausted:
            discard(key)
            return ("Límite de conexión alcanzado", 429, headers)
        return (output, 200, {**headers, "Content-Type": "application/octet-stream"})
    except OSError:
        discard(key)
        return ("Conexión interrumpida", 502, headers)
