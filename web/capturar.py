"""Rehace las capturas de la web (web/capturas/<es|en>/*.webp): la interfaz de ui/ en modo demostración a
1280×800 con densidad 2 (2560×1600), tema oscuro, en español y en inglés. Usa Edge sin ventana y el paquete
websocket-client. Después: python web/construir.py.

    python web/capturar.py [carpeta]   (por omisión, web/capturas)"""
import base64, functools, http.server, json, os, subprocess, sys, threading, time, urllib.request
import websocket

AQUI = os.path.dirname(os.path.abspath(__file__))
UI = os.path.join(os.path.dirname(AQUI), "ui")
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(AQUI, "capturas")
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(http.server.SimpleHTTPRequestHandler, directory=UI))
server.RequestHandlerClass.log_message = lambda *a: None
threading.Thread(target=server.serve_forever, daemon=True).start()
BASE = f"http://127.0.0.1:{server.server_port}/index.html"
PORT = 9463
profile = os.path.join(os.environ.get("TEMP", "/tmp"), "parabellum-capturas-web")
edge = subprocess.Popen([EDGE, "--headless=new", f"--remote-debugging-port={PORT}", f"--user-data-dir={profile}", "--window-size=1280,800", "--hide-scrollbars", "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
for _ in range(100):
    try:
        page = next(t for t in json.load(urllib.request.urlopen(f"http://127.0.0.1:{PORT}/json")) if t.get("type") == "page")
        break
    except Exception:
        time.sleep(0.2)
ws = websocket.create_connection(page["webSocketDebuggerUrl"], max_size=None)
counter = [0]


def call(method, params=None):
    counter[0] += 1
    ws.send(json.dumps({"id": counter[0], "method": method, "params": params or {}}))
    while True:
        message = json.loads(ws.recv())
        if message.get("id") == counter[0]:
            return message.get("result", {})


def js(expression):
    r = call("Runtime.evaluate", {"expression": expression, "awaitPromise": True, "returnByValue": True})
    if "exceptionDetails" in r:
        print("  error:", str(r["exceptionDetails"])[:300])
    return r.get("result", {}).get("value")


call("Emulation.setDeviceMetricsOverride", {"width": 1280, "height": 800, "deviceScaleFactor": 2, "mobile": False})
call("Emulation.setEmulatedMedia", {"features": [{"name": "prefers-color-scheme", "value": "dark"}, {"name": "prefers-reduced-motion", "value": "reduce"}]})
call("Emulation.setDefaultBackgroundColorOverride", {"color": {"r": 0, "g": 0, "b": 0, "a": 0}})

WAIT = "await new Promise(r => setTimeout(r, 900));"
# Lo que se abre en cada captura (las funciones son las de ui/app.js en modo demostración).
SCENES = {
    "inicio": "navigate('home');",
    "explorar": "navigate('explore');",
    "instancia": "navigate('instance', 'Better MC BMC4');",
    "mensajes": "navigate('chat'); await new Promise(r => setTimeout(r, 700)); const first = (typeof allChats === 'function' ? allChats() : [])[0]; if (first) await openChat(first.id);",
    "comunidades": "navigate('communities');",
    "doctor": "state.settings.crashReports = false; openDoctor('Better MC BMC4', 'crash');",
    "servidor": "navigate('servers'); await new Promise(r => setTimeout(r, 700)); document.querySelector('[data-hosted], .host-card, .server-card')?.click();",
}
for lang in ["es", "en"]:
    os.makedirs(os.path.join(OUT, lang), exist_ok=True)
    # La demostración elige el inglés si la dirección lleva «en».
    query = "?en=1&toma=" if lang == "en" else "?toma="
    for name, scene in SCENES.items():
        call("Page.navigate", {"url": BASE + query + "0"})
        time.sleep(1.5)
        js(f"localStorage.setItem('parabellum-idioma', '{lang}'); true")
        call("Page.navigate", {"url": BASE + query + "1"})
        time.sleep(2.5)
        js(f"(async () => {{ document.documentElement.style.background = 'transparent'; document.body.style.borderRadius = '12px'; document.body.style.overflow = 'hidden'; {scene} {WAIT} }})()")
        time.sleep(1.2)
        data = call("Page.captureScreenshot", {"format": "webp", "quality": 88})["data"]
        path = os.path.join(OUT, lang, name + ".webp")
        open(path, "wb").write(base64.b64decode(data))
        print("listo", lang, name, js("state.page"), os.path.getsize(path) // 1024, "KB")
ws.close()
edge.terminate()
server.shutdown()
