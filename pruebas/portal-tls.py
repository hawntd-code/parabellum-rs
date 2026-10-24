"""Verifica el relevo desplegado manteniendo TLS con el host oficial, sin navegador ni cookies."""
import argparse
import json
import ssl
import urllib.error
import urllib.request


def fetch(endpoint, host, path):
    def request(action, data=b'', key=None):
        headers = {'Content-Type': 'application/octet-stream'}
        if key:
            headers['X-Portal-Session'] = key
        if action == '/open':
            headers['Content-Type'] = 'application/json'
        req = urllib.request.Request(endpoint + action, data=data, headers=headers, method='POST')
        try:
            with urllib.request.urlopen(req, timeout=45) as response:
                return response.status, response.read()
        except urllib.error.HTTPError as error:
            return error.code, error.read()
    status, result = request('/open', json.dumps({'host':host}).encode())
    assert status == 200, (status, result)
    key = json.loads(result)['session']
    incoming, outgoing = ssl.MemoryBIO(), ssl.MemoryBIO()
    connection = ssl.create_default_context().wrap_bio(incoming, outgoing, server_side=False, server_hostname=host)
    def exchange():
        while outgoing.pending:
            code, _ = request('/send', outgoing.read(65536), key)
            assert code == 200, code
        code, data = request('/receive', key=key)
        if code == 410:
            incoming.write_eof()
            return False
        assert code in [200, 204], code
        if data:
            incoming.write(data)
        return True
    try:
        while True:
            try:
                connection.do_handshake()
                break
            except ssl.SSLWantReadError:
                assert exchange()
        connection.write(f'GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: Mozilla/5.0\r\nAccept: text/html\r\nConnection: close\r\n\r\n'.encode())
        data = bytearray()
        for _ in range(500):
            try:
                chunk = connection.read(65536)
                if not chunk:
                    break
                data.extend(chunk)
                if len(data) > 4_000_000:
                    raise ValueError('Respuesta demasiado grande')
            except ssl.SSLWantReadError:
                exchange()
            except (ssl.SSLZeroReturnError, ssl.SSLEOFError):
                break
        header, _, body = bytes(data).partition(b'\r\n\r\n')
        return {'host':host, 'tls':connection.version(), 'http':header.decode('latin-1').split('\r\n')[:8],
                'bytes':len(body), 'htmlSample':body[:220].decode('utf-8', errors='replace')}
    finally:
        request('/close', key=key)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--endpoint', required=True)
    parser.add_argument('--host', default='www.curseforge.com')
    parser.add_argument('--path', default='/minecraft/mc-mods/useful-slime/files/6282838')
    args = parser.parse_args()
    print(json.dumps(fetch(args.endpoint, args.host, args.path), ensure_ascii=False, indent=2))
