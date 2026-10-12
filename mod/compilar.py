"""Compila el mod de Parabellum para cada versión y lo deja listo para Parabellum.

    python compilar.py                 todas las versiones
    python compilar.py fabric-26.2     solo esas

Cada versión vive en versiones/<carpeta> y comparte el núcleo (nucleo/). Los .jar van a
src-tauri/juego/mods/ y se genera src-tauri/juego/mods/lista.rs, la tabla con la que Parabellum elige el
mod según el loader y la versión de Minecraft de la instancia (ver src-tauri/src/juego.rs).

Hace falta un JDK (no va dentro de Parabellum): por defecto se buscan en ~/.jdks (Temurin 21 y 25).
"""
import glob
import os
import shutil
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
DESTINO = os.path.join(AQUI, "..", "src-tauri", "juego", "mods")

# (carpeta, loaders, desde (incluida), hasta (sin incluir), JDK con el que corre Gradle). Con «+» el mismo
# .jar sirve en varios loaders (NeoForge 1.20.1 todavía carga mods de Forge).
VERSIONES = [
    ("fabric-1.21.1", "Fabric", "1.21", "1.21.2", 25),
    ("fabric-26.1.2", "Fabric", "26.1", "26.2", 25),
    ("fabric-26.2", "Fabric", "26.2", "26.3", 25),
    ("fabric-26.3", "Fabric", "26.3", "26.4", 25),
    ("neoforge-1.21.1", "NeoForge", "1.21", "1.21.2", 25),
    ("forge-1.20.1", "Forge+NeoForge", "1.20", "1.20.2", 25),
    ("fabric-1.20.1", "Fabric", "1.20", "1.20.2", 25),
    ("fabric-1.21.4", "Fabric", "1.21.2", "1.21.5", 25),
    ("fabric-1.21.5", "Fabric", "1.21.5", "1.21.6", 25),
    ("fabric-1.21.8", "Fabric", "1.21.6", "1.21.9", 25),
    ("fabric-1.21.10", "Fabric", "1.21.9", "1.21.11", 25),
    ("fabric-1.21.11", "Fabric", "1.21.11", "1.21.12", 25),
    ("forge-1.19.2", "Forge", "1.19", "1.19.3", 25),
    ("fabric-1.19.2", "Fabric", "1.19", "1.19.3", 25),
]


def jdk(version):
    candidatos = sorted(glob.glob(os.path.join(os.path.expanduser("~"), ".jdks", f"jdk-{version}*")))
    if not candidatos:
        sys.exit(f"No encontré un JDK {version} en ~/.jdks")
    return candidatos[-1]


def compilar(carpeta, version_jdk):
    proyecto = os.path.join(AQUI, "versiones", carpeta)
    envoltorio = os.path.join(proyecto, "gradlew.bat") if os.path.exists(os.path.join(proyecto, "gradlew.bat")) else os.path.join(AQUI, "gradlew.bat")
    entorno = dict(os.environ, JAVA_HOME=jdk(version_jdk))
    # Los JDK de ~/.jdks para las herramientas que piden otra versión (Forge 1.20.1 usa Java 17).
    instalados = ",".join(sorted(glob.glob(os.path.join(os.path.expanduser("~"), ".jdks", "jdk-*"))))
    print(f"== {carpeta}", flush=True)
    resultado = subprocess.run([envoltorio, "-p", proyecto, "build", "--console=plain", "-q", f"-Porg.gradle.java.installations.paths={instalados}"], env=entorno, cwd=AQUI)
    if resultado.returncode != 0:
        sys.exit(f"No compiló {carpeta}")
    jars = [j for j in glob.glob(os.path.join(proyecto, "build", "libs", "*.jar")) if not j.endswith("-sources.jar")]
    if len(jars) != 1:
        sys.exit(f"{carpeta}: esperaba un .jar y hay {jars}")
    shutil.copyfile(jars[0], os.path.join(DESTINO, f"parabellum-{carpeta}.jar"))


def lista():
    lineas = ["// Generado por mod/compilar.py: no editar a mano.", "&["]
    for carpeta, loaders, desde, hasta, _ in VERSIONES:
        if os.path.exists(os.path.join(DESTINO, f"parabellum-{carpeta}.jar")):
            for loader in loaders.split("+"):
                lineas.append(f'    Build {{ loader: "{loader}", from: "{desde}", until: "{hasta}", jar: include_bytes!("parabellum-{carpeta}.jar") }},')
    lineas.append("]")
    open(os.path.join(DESTINO, "lista.rs"), "w", encoding="utf-8", newline="\n").write("\n".join(lineas) + "\n")


def main():
    os.makedirs(DESTINO, exist_ok=True)
    pedidas = sys.argv[1:]
    for carpeta, _, _, _, version_jdk in VERSIONES:
        if not pedidas or carpeta in pedidas:
            compilar(carpeta, version_jdk)
    lista()
    print("listo:", len([v for v in VERSIONES if os.path.exists(os.path.join(DESTINO, f"parabellum-{v[0]}.jar"))]), "versiones")


if __name__ == "__main__":
    main()
