"""Contrato y límites del relevo, sin Google Cloud ni tráfico externo."""
import importlib.util
import json
import io
import socket
import sys
import types
import unittest
from unittest.mock import patch
from pathlib import Path

sys.modules.setdefault("functions_framework", types.SimpleNamespace(http=lambda f: f))
spec = importlib.util.spec_from_file_location("portal", Path(__file__).with_name("main.py"))
portal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(portal)


class Request:
    def __init__(self, path, body=b"", key="", method="POST"):
        self.path, self.body, self.method = path, body, method
        self.headers = {"X-Portal-Session": key}
        self.content_length = len(body)
        self.stream = io.BytesIO(body)

    def get_json(self, silent=False):
        return json.loads(self.body)

    def get_data(self, cache=False):
        return self.body


class PortalTest(unittest.TestCase):
    def tearDown(self):
        for key in list(portal.SESSIONS):
            portal.discard(key)

    def test_rechaza_destinos_arbitrarios_y_api(self):
        for host in ["127.0.0.1", "api.curseforge.com", "www.curseforge.com.ejemplo.com", [], "edge.forgecdn.net:80"]:
            response = portal.portal_curseforge(Request("/open", json.dumps({"host": host}).encode()))
            self.assertEqual(response[1], 403)
        self.assertEqual(portal.portal_curseforge(Request("/send", b"a" * 65537))[1], 413)

    def test_transferencia_binaria_cierre_y_sesion_ajena(self):
        local, remote = socket.socketpair()
        remote.settimeout(2)
        with patch.object(portal.socket, "create_connection", return_value=local):
            response = portal.portal_curseforge(Request("/open", b'{"host":"www.curseforge.com"}'))
        key = response[0]["session"]
        self.assertEqual(portal.portal_curseforge(Request("/send", b"\x00\xffTLS", key))[1], 200)
        self.assertEqual(remote.recv(1024), b"\x00\xffTLS")
        remote.sendall(b"\xff\x00respuesta")
        self.assertEqual(portal.portal_curseforge(Request("/receive", key=key))[0], b"\xff\x00respuesta")
        self.assertEqual(portal.portal_curseforge(Request("/receive", key="ajena"))[1], 410)
        self.assertEqual(portal.portal_curseforge(Request("/close", key=key))[1], 200)
        self.assertEqual(remote.recv(1), b"")
        remote.close()
        self.assertEqual(portal.portal_curseforge(Request("/receive", key=key))[1], 410)

    def test_limite_y_caducidad(self):
        for n in range(portal.MAX_SESSIONS):
            portal.SESSIONS[str(n)] = portal.Tunnel(socket.socket())
        self.assertEqual(portal.portal_curseforge(Request("/open", b'{"host":"curseforge.com"}'))[1], 429)
        for tunnel in portal.SESSIONS.values():
            tunnel.created -= portal.TTL + 1
        portal.cleanup()
        self.assertFalse(portal.SESSIONS)

    def test_recibir_timeout_no_cierra_sesion(self):
        stream = unittest.mock.Mock()
        stream.recv.side_effect = socket.timeout()
        portal.SESSIONS["test"] = portal.Tunnel(stream)
        self.assertEqual(portal.portal_curseforge(Request("/receive", key="test"))[1], 204)
        self.assertIn("test", portal.SESSIONS)

    def test_libera_conexiones_abandonadas(self):
        tunnel = portal.Tunnel(socket.socket())
        tunnel.touched -= 61
        portal.SESSIONS["abandonada"] = tunnel
        portal.cleanup()
        self.assertFalse(portal.SESSIONS)

    def test_http2_sin_content_length_en_peticiones_vacias(self):
        stream = unittest.mock.Mock()
        stream.recv.return_value = b"TLS"
        portal.SESSIONS["test"] = portal.Tunnel(stream)
        request = Request("/receive", key="test")
        request.content_length = None
        self.assertEqual(portal.portal_curseforge(request)[:2], (b"TLS", 200))
        request = Request("/send", b"TLS", key="test")
        request.content_length = None
        self.assertEqual(portal.portal_curseforge(request)[1], 200)
        request = Request("/send", b"x" * 65537, key="test")
        request.content_length = None
        self.assertEqual(portal.portal_curseforge(request)[1], 413)


if __name__ == "__main__":
    unittest.main()
