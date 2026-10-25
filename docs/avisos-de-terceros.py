# -*- coding: utf-8 -*-
"""Genera AVISOS-DE-TERCEROS.md (en la raíz del repo) con los componentes de terceros de Parabellum.

Uso, desde la raíz del repo (hace falta cargo; para los textos de licencia, conexión a internet):

    python docs/avisos-de-terceros.py

Qué hace:
1. Dependencias de Rust: corre `cargo metadata --format-version 1 --locked` en src-tauri y recorre
   resolve.nodes desde el paquete «parabellum», siguiendo solo las dependencias normales (dep_kinds con
   kind null: no las de desarrollo ni las de compilación). Las condiciones cfg(...) se evalúan para los
   dos objetivos que compila la CI: x86_64-pc-windows-msvc (windows-latest) y aarch64-apple-darwin
   (macos-14). Los paquetes que resultan son los que entran en el ejecutable de esas plataformas.
2. Lo que no es de Rust: la lista de abajo se revisó leyendo los ficheros. El script comprueba que existan
   y avisa si aparece una fuente o un .jar nuevo sin revisar.
3. Textos de las licencias: los oficiales (SPDX; Apache-2.0 de apache.org; MPL-2.0 de mozilla.org), bajados
   al generar y sin cambios. Los avisos de copyright se leen de los ficheros LICENSE de cada paquete.

Solo usa la biblioteca estándar de Python.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import urllib.request
import zipfile
from pathlib import Path

try:
    sys.stdout.reconfigure(encoding="utf-8")
except AttributeError:
    pass

RAIZ = Path(__file__).resolve().parent.parent
TAURI = RAIZ / "src-tauri"
SALIDA = RAIZ / "AVISOS-DE-TERCEROS.md"

# ---------------------------------------------------------------------------
# Plataformas: valores de cfg(...) que Cargo evalúa para cada objetivo.
# Cualquier cfg que no esté aquí (windows_raw_dylib, loom, fuzzing...) se toma como inactivo: por defecto
# Cargo no los activa (solo con RUSTFLAGS). Los que se desconocen se muestran al final del proceso.
# ---------------------------------------------------------------------------


def _plataforma(sistema, familia, entorno, arquitectura, proveedor, banderas):
    return {
        "valores": {
            "target_os": {sistema},
            "target_family": {familia},
            "target_env": {entorno},
            "target_arch": {arquitectura},
            "target_vendor": {proveedor},
            "target_pointer_width": {"64"},
            "target_endian": {"little"},
            "target_abi": {""},
            "target_has_atomic": {"8", "16", "32", "64", "ptr"},
            "target_feature": set(),
        },
        "banderas": banderas,
    }


PLATAFORMAS = {
    "x86_64-pc-windows-msvc": _plataforma("windows", "windows", "msvc", "x86_64", "pc", {"windows"}),
    "aarch64-apple-darwin": _plataforma("macos", "unix", "", "aarch64", "apple", {"unix"}),
}
NOMBRES_CFG_CONOCIDOS = set(PLATAFORMAS["x86_64-pc-windows-msvc"]["valores"]) | {"unix", "windows"}
DESCONOCIDOS = set()

TOKEN_CFG = re.compile(r'"[^"]*"|[(),=]|[A-Za-z_][A-Za-z0-9_]*')


def evaluar_cfg(expresion, triple):
    """Evalúa un destino de dependencia: «cfg(...)» o un triple exacto."""
    if not expresion.startswith("cfg("):
        return expresion == triple
    plataforma = PLATAFORMAS[triple]
    tokens = TOKEN_CFG.findall(expresion[4:-1])
    pos = 0

    def leer():
        nonlocal pos
        token = tokens[pos]
        pos += 1
        return token

    def mirar():
        return tokens[pos] if pos < len(tokens) else None

    def expr():
        nombre = leer()
        if nombre in ("all", "any", "not") and mirar() == "(":
            leer()
            valores = []
            while mirar() != ")":
                valores.append(expr())
                if mirar() == ",":
                    leer()
            leer()
            if nombre == "all":
                return all(valores)
            if nombre == "any":
                return any(valores)
            return not valores[0]
        if mirar() == "=":
            leer()
            valor = leer().strip('"')
            return valor in plataforma["valores"].get(nombre, set())
        if nombre not in NOMBRES_CFG_CONOCIDOS:
            DESCONOCIDOS.add(nombre)
        return nombre in plataforma["banderas"]

    return expr()


def cargo_ejecutable():
    encontrado = shutil.which("cargo")
    if encontrado:
        return encontrado
    for candidato in (Path.home() / ".cargo" / "bin" / "cargo.exe", Path.home() / ".cargo" / "bin" / "cargo"):
        if candidato.exists():
            return str(candidato)
    sys.exit("No encuentro cargo. Instálalo con rustup o añade %USERPROFILE%\\.cargo\\bin al PATH.")


def metadatos_cargo():
    orden = [cargo_ejecutable(), "metadata", "--format-version", "1", "--locked"]
    proceso = subprocess.run(orden, cwd=TAURI, capture_output=True)
    if proceso.returncode != 0:
        sys.exit("cargo metadata falló:\n" + proceso.stderr.decode("utf-8", "replace"))
    return json.loads(proceso.stdout.decode("utf-8"))


def paquetes_que_entran(datos):
    """Ids de los paquetes que llegan desde «parabellum» por dependencias normales, en cada plataforma."""
    nodos = {n["id"]: n for n in datos["resolve"]["nodes"]}
    raiz = datos["resolve"]["root"]
    entran = set()
    for triple in PLATAFORMAS:
        vistos = {raiz}
        pila = [raiz]
        while pila:
            actual = pila.pop()
            for dep in nodos[actual]["deps"]:
                normal = any(
                    dk["kind"] is None and (dk["target"] is None or evaluar_cfg(dk["target"], triple))
                    for dk in dep["dep_kinds"]
                )
                if normal and dep["pkg"] not in vistos:
                    vistos.add(dep["pkg"])
                    pila.append(dep["pkg"])
        vistos.discard(raiz)
        entran |= vistos
    return entran


# ---------------------------------------------------------------------------
# Lo que no es de Rust (revisado leyendo los ficheros; ver docs/avisos-de-terceros.py y el md).
# ---------------------------------------------------------------------------

NO_RUST = [
    {
        "componente": "Inter (fuente)",
        "que": (
            "Fuente de la interfaz (versión web variable de 100 a 900) y las del panel del juego (versión 4.001, "
            "que `src-tauri/src/juego.rs` incluye en el .exe)."
        ),
        "ficheros": [
            "ui/fonts/inter-latin.woff2",
            "ui/fonts/inter-latin-ext.woff2",
            "ui/fonts/OFL.txt",
            "src-tauri/juego/fuentes/Inter-Regular.ttf",
            "src-tauri/juego/fuentes/Inter-SemiBold.ttf",
            "src-tauri/juego/fuentes/OFL.txt",
        ],
        "licencia": "SIL Open Font License 1.1",
        "id_licencia": "OFL-1.1",
        "autor": "Copyright 2016 The Inter Project Authors (https://github.com/rsms/inter)",
    },
]

PREFIJOS_JAR_PROPIOS = ("mx/parabellum/", "assets/parabellum/", "META-INF/", "fabric.mod.json",
                        "parabellum.mixins.json", "pack.mcmeta")
EXTENSIONES_FUENTE = (".ttf", ".otf", ".woff", ".woff2")


def revisar_no_rust():
    """Devuelve (avisos, numero_de_jars, problemas_jar). Avisa si falta un fichero listado, si aparece una
    fuente sin revisar o si un .jar de juego/mods trae algo que no sea código o recursos de Parabellum."""
    avisos = []
    conocidos = set()
    for componente in NO_RUST:
        for fichero in componente["ficheros"]:
            conocidos.add(fichero)
            if not (RAIZ / fichero).exists():
                avisos.append(f"No existe {fichero} (listado como {componente['componente']}).")
    for carpeta in (RAIZ / "ui", TAURI / "juego"):
        for ruta in carpeta.rglob("*"):
            if ruta.is_file() and ruta.suffix.lower() in EXTENSIONES_FUENTE:
                relativo = ruta.relative_to(RAIZ).as_posix()
                if relativo not in conocidos:
                    avisos.append(f"Fuente sin revisar: {relativo} (añádela a NO_RUST).")

    problemas = []
    jars = sorted((TAURI / "juego" / "mods").glob("*.jar"))
    for jar in jars:
        with zipfile.ZipFile(jar) as zf:
            for nombre in zf.namelist():
                if nombre.endswith("/"):
                    continue
                sospechoso = (
                    not nombre.startswith(PREFIJOS_JAR_PROPIOS)
                    or nombre.lower().endswith(".jar")
                    or "/jars/" in nombre
                    or re.search(r"LICEN[CS]E|NOTICE", nombre, re.I) is not None
                )
                if sospechoso:
                    problemas.append(f"{jar.name}: {nombre}")
    if problemas:
        avisos.append("Revisar los .jar (puede haber código de terceros): " + "; ".join(problemas[:10]))
    return avisos, len(jars), problemas


# ---------------------------------------------------------------------------
# Textos de licencias y avisos de copyright.
# ---------------------------------------------------------------------------

TEXTOS_OFICIALES = {
    "Apache-2.0": "https://www.apache.org/licenses/LICENSE-2.0.txt",
    "MPL-2.0": "https://www.mozilla.org/media/MPL/2.0/index.txt",
}
URL_SPDX = "https://raw.githubusercontent.com/spdx/license-list-data/main/text/{}.txt"


def texto_licencia(id_licencia):
    url = TEXTOS_OFICIALES.get(id_licencia, URL_SPDX.format(id_licencia))
    try:
        with urllib.request.urlopen(url, timeout=60) as respuesta:
            texto = respuesta.read().decode("utf-8-sig")
    except Exception as error:
        sys.exit(f"No pude bajar el texto de {id_licencia} ({url}): {error}")
    texto = texto.replace("\r\n", "\n").replace("\r", "\n").strip()
    if len(texto) < 200 or texto.lstrip().startswith("<"):
        sys.exit(f"El texto de {id_licencia} ({url}) no parece una licencia.")
    return texto


def licencias_separadas(expresion):
    """«MIT OR Apache-2.0» → ['MIT', 'Apache-2.0']; «Apache-2.0 WITH LLVM-exception» → [..., 'LLVM-exception']."""
    return [t for t in re.findall(r"[A-Za-z0-9][A-Za-z0-9.+-]*", expresion or "") if t not in ("OR", "AND", "WITH")]


PATRON_FICHERO_LICENCIA = re.compile(r"^(licen[cs]e|licence|copying|unlicense|notice)", re.I)
# Solo avisos de verdad: «Copyright ...» con mayúscula, «(c)» o «©» al principio de línea. La prosa de la
# licencia Apache dice «copyright notice...» en minúscula y se descarta entera (es el texto estándar).
PATRON_LINEA_COPYRIGHT = re.compile(r"^(Copyright\b|\(c\)|\(C\)|©)")
# «(c)» o «©» sin año es casi siempre texto de la licencia (p. ej. «(c) under Patent Claims...» de MPL-2.0).
PATRON_CON_ANO = re.compile(r"\b(19|20)\d{2}\b")
PATRON_PLANTILLA = re.compile(r"\[(yyyy|name of)|<year>|<copyright", re.I)
PATRON_APACHE_ESTANDAR = re.compile(r"Apache License\s+Version 2\.0, January 2004")


def avisos_copyright(carpeta):
    """Líneas de copyright de los ficheros de licencia del paquete (sin las plantillas tipo [yyyy])."""
    lineas = []
    tiene_fichero = False
    for nombre in sorted(os.listdir(carpeta)):
        ruta = os.path.join(carpeta, nombre)
        if not PATRON_FICHERO_LICENCIA.match(nombre) or not os.path.isfile(ruta):
            continue
        if os.path.splitext(nombre)[1].lower() in (".rs", ".toml", ".json", ".yml", ".yaml"):
            continue
        tiene_fichero = True
        with open(ruta, encoding="utf-8", errors="replace") as f:
            texto = f.read()
        if PATRON_APACHE_ESTANDAR.search(re.sub(r"\s+", " ", texto)):
            continue
        for linea in texto.splitlines():
            linea = linea.strip()
            if not PATRON_LINEA_COPYRIGHT.match(linea) or PATRON_PLANTILLA.search(linea):
                continue
            if not linea.startswith("Copyright") and not PATRON_CON_ANO.search(linea):
                continue
            if linea not in lineas:
                lineas.append(linea)
    return lineas, tiene_fichero


def clave_version(version):
    partes = re.split(r"[.+-]", version)
    return tuple((0, int(p)) if p.isdigit() else (1, p) for p in partes)


def celda(texto):
    return str(texto).replace("|", "\\|").replace("\n", " ")


def generar():
    datos = metadatos_cargo()
    paquetes = {p["id"]: p for p in datos["packages"]}
    raiz_id = datos["resolve"]["root"]
    version_parabellum = paquetes[raiz_id]["version"]

    ids = paquetes_que_entran(datos)
    filas = []
    for pid in ids:
        p = paquetes[pid]
        licencia = p.get("license") or ""
        if not licencia and p.get("license_file"):
            licencia = f"(ver fichero {p['license_file']})"
        filas.append({
            "nombre": p["name"],
            "version": p["version"],
            "licencia": licencia or "(sin licencia declarada)",
            "repositorio": p.get("repository") or "(sin repositorio en Cargo.toml)",
            "carpeta": os.path.dirname(p["manifest_path"]),
        })
    filas.sort(key=lambda f: (f["nombre"].lower(), clave_version(f["version"])))

    # Licencias distintas (cada una por separado, también las excepciones tipo LLVM-exception).
    ids_rust = {}
    for fila in filas:
        for id_lic in licencias_separadas(fila["licencia"]):
            ids_rust.setdefault(id_lic, 0)
            ids_rust[id_lic] += 1

    ids_rust_sin_fuente = set(ids_rust)
    avisos_no_rust, n_jars, problemas_jar = revisar_no_rust()
    for componente in NO_RUST:
        ids_rust.setdefault(componente["id_licencia"], 0)

    copyright_por_paquete = []
    sin_fichero = []
    for fila in filas:
        lineas, tiene_fichero = avisos_copyright(fila["carpeta"])
        if lineas:
            copyright_por_paquete.append((fila, lineas))
        if not tiene_fichero:
            sin_fichero.append(fila)

    # Textos oficiales, bajados una vez por licencia.
    textos = {}
    for id_lic in sorted(ids_rust):
        textos[id_lic] = texto_licencia(id_lic)

    n_nombres = len({f["nombre"] for f in filas})
    licencias_no_spdx = sorted({f["licencia"] for f in filas if "/" in f["licencia"]})

    L = []
    L.append("# Avisos de terceros")
    L.append("")
    L.append(
        "Parabellum incluye componentes de terceros: la fuente Inter y las librerías de Rust que se compilan dentro "
        "del programa. Aquí aparecen con su origen, su licencia y su aviso de copyright."
    )
    L.append("")
    L.append(
        "Cada componente de terceros conserva su propia licencia, y el [LICENSE](LICENSE) del proyecto no cambia "
        "sus condiciones."
    )
    L.append("")
    L.append(f"Versión de Parabellum: {version_parabellum} (`src-tauri/Cargo.toml`).")
    L.append("")
    L.append(
        "Este documento lo genera `docs/avisos-de-terceros.py` (`python docs/avisos-de-terceros.py`, desde la raíz "
        "del repo). No se edita a mano: para actualizarlo tras cambiar dependencias, vuelve a correrlo."
    )
    L.append("")

    L.append("## 1. Componentes que no son de Rust")
    L.append("")
    L.append("| Componente | Qué es y dónde está | Licencia | Autor y copyright |")
    L.append("|---|---|---|---|")
    for componente in NO_RUST:
        ficheros = ", ".join(f"`{f}`" for f in componente["ficheros"])
        L.append(
            f"| {celda(componente['componente'])} | {celda(componente['que'])} Ficheros: {ficheros}. | "
            f"{celda(componente['licencia'])} (texto en la sección 4) | {celda(componente['autor'])} |"
        )
    L.append("")
    if problemas_jar:
        L.append(
            "Los .jar de los mods (`src-tauri/juego/mods/`) tienen entradas que hay que revisar; ver la lista que "
            "mostró el script."
        )
    else:
        L.append(
            f"Los {n_jars} .jar de los mods (`src-tauri/juego/mods/`) solo contienen código, recursos y metadatos "
            "propios de Parabellum (revisado al generar): no llevan librerías de terceros. Minecraft, Fabric y Forge "
            "no van incluidos: los pone el propio juego."
        )
    L.append("")
    if avisos_no_rust:
        L.append("Avisos al generar:")
        for aviso in avisos_no_rust:
            L.append(f"- {aviso}")
        L.append("")

    L.append("## 2. Dependencias de Rust")
    L.append("")
    L.append(
        "Son los paquetes que entran en el ejecutable: las dependencias normales (no las de desarrollo ni las de "
        "compilación) del paquete `parabellum`, para Windows (`x86_64-pc-windows-msvc`) y macOS "
        "(`aarch64-apple-darwin`), los dos objetivos que compila la CI. Se sacaron de `cargo metadata --locked`."
    )
    L.append("")
    L.append(
        f"Total: {len(filas)} paquetes ({n_nombres} nombres distintos). Identificadores de licencia distintos en "
        f"Rust: {len(ids_rust_sin_fuente)} (licencias y una excepción, LLVM-exception); a ellos se suma la de la "
        "fuente Inter (sección 1)."
    )
    L.append("")
    if licencias_no_spdx:
        L.append(
            "La columna de licencia es la expresión que declara cada paquete tal cual. Algunas no son SPDX estricta "
            "(por ejemplo " + ", ".join(f"`{x}`" for x in licencias_no_spdx[:4]) + ")."
        )
        L.append("")
    L.append("| Nombre | Versión | Licencia | Repositorio |")
    L.append("|---|---|---|---|")
    for fila in filas:
        L.append(f"| {celda(fila['nombre'])} | {celda(fila['version'])} | {celda(fila['licencia'])} | "
                 f"{celda(fila['repositorio'])} |")
    L.append("")

    L.append("## 3. Avisos de copyright")
    L.append("")
    L.append(
        "Las licencias MIT, BSD, ISC y similares obligan a conservar el aviso de copyright junto al texto de la "
        "licencia. Aquí están los avisos que aparecen en los ficheros de licencia de cada paquete, por nombre."
    )
    L.append("")
    for componente in NO_RUST:
        L.append(f"- {componente['componente']}: {componente['autor']}")
    for fila, lineas in copyright_por_paquete:
        for linea in lineas:
            L.append(f"- {fila['nombre']} {fila['version']}: {linea}")
    L.append("")
    if sin_fichero:
        L.append(
            f"Paquetes sin fichero de licencia en el paquete descargado ({len(sin_fichero)}). Su aviso de copyright no "
            "se pudo leer en local: hay que mirarlo en su repositorio antes de distribuir:"
        )
        L.append("")
        for fila in sin_fichero:
            L.append(f"- {fila['nombre']} {fila['version']} ({fila['licencia']})")
        L.append("")

    L.append("## 4. Textos de las licencias")
    L.append("")
    L.append(
        "Textos oficiales, sin cambios: SPDX (spdx.org), salvo Apache-2.0 (apache.org) y MPL-2.0 (mozilla.org). "
        "Los campos entre < > de las plantillas (año, titular) se rellenan con los avisos de la sección 3."
    )
    L.append("")
    for id_lic in sorted(textos):
        usos = []
        if id_lic in ids_rust and ids_rust[id_lic] > 0:
            n = ids_rust[id_lic]
            usos.append(f"{n} paquete{'s' if n != 1 else ''} de Rust")
        for componente in NO_RUST:
            if componente["id_licencia"] == id_lic:
                usos.append(componente["componente"])
        L.append(f"### {id_lic}")
        L.append("")
        L.append("Usada por: " + "; ".join(usos) + ".")
        L.append("")
        L.append("````text")
        L.append(textos[id_lic])
        L.append("````")
        L.append("")

    SALIDA.write_text("\n".join(L).rstrip() + "\n", encoding="utf-8", newline="\n")

    print(f"Escrito: {SALIDA}")
    print(f"Dependencias de Rust: {len(filas)} paquetes ({n_nombres} nombres distintos)")
    print("Licencias distintas (Rust): " + ", ".join(sorted(ids_rust_sin_fuente)))
    if sin_fichero:
        print(f"AVISO: {len(sin_fichero)} paquetes sin fichero de licencia: " +
              ", ".join(f"{f['nombre']} {f['version']}" for f in sin_fichero))
    for aviso in avisos_no_rust:
        print("AVISO: " + aviso)
    if DESCONOCIDOS:
        print("cfg no previstos (tomados como inactivos): " + ", ".join(sorted(DESCONOCIDOS)))


if __name__ == "__main__":
    generar()
