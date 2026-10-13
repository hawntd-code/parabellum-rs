"""Dibuja el panel del juego sin Minecraft y guarda un PNG de cada estado (modo de prueba).

    python mod/vista/vista.py [carpeta de salida]

Compila el núcleo (mod/nucleo) y Vista.java con el JDK 17 de ~/.jdks y la Gson que Gradle ya bajó
(~/.gradle/caches), y usa las fuentes de Parabellum (datos/fuentes de la carpeta de PARABELLUM_ROOT o de
la que indique PARABELLUM_FUENTES). Los PNG quedan en vista/png (o en la carpeta que pases).
"""
import glob
import os
import subprocess
import sys
import tempfile

AQUI = os.path.dirname(os.path.abspath(__file__))
NUCLEO = os.path.join(AQUI, "..", "nucleo", "src", "main", "java", "mx", "parabellum", "juego")


def main():
    salida = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(AQUI, "png"))
    jdks = sorted(glob.glob(os.path.join(os.path.expanduser("~"), ".jdks", "jdk-17*")))
    gson = sorted(glob.glob(os.path.join(os.path.expanduser("~"), ".gradle", "caches", "**", "gson-2.8*.jar"), recursive=True))
    if not jdks or not gson:
        sys.exit("Hacen falta un JDK 17 en ~/.jdks y haber compilado el mod una vez (para la Gson).")
    fuentes = os.environ.get("PARABELLUM_FUENTES") or os.path.join(os.environ.get("PARABELLUM_ROOT", ""), "datos", "fuentes")
    bin_ = os.path.join(jdks[-1], "bin")
    clases = tempfile.mkdtemp(prefix="vista-")
    juego = tempfile.mkdtemp(prefix="vista-juego-")
    fuentes_java = glob.glob(os.path.join(NUCLEO, "*.java")) + [os.path.join(AQUI, "Vista.java")]
    subprocess.run([os.path.join(bin_, "javac"), "--release", "8", "-encoding", "UTF-8", "-nowarn", "-cp", gson[-1], "-d", clases] + fuentes_java, check=True)
    subprocess.run([os.path.join(bin_, "java"), "-Dparabellum.prueba=true", f"-Dparabellum.fuentes={fuentes}", "-Dfile.encoding=UTF-8",
                    "-cp", os.pathsep.join([clases, gson[-1]]), "mx.parabellum.juego.Vista", juego, salida], check=True)
    print(salida)


if __name__ == "__main__":
    main()
