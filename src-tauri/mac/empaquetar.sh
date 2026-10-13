#!/bin/bash
# Arma Parabellum.app y su .dmg para Mac con chip Apple, con herramientas que trae macOS (sips, iconutil,
# codesign, hdiutil): no hace falta instalar nada aparte de Rust. Desde cualquier carpeta:
#
#   src-tauri/mac/empaquetar.sh                  compila (cargo build --release) y empaqueta
#   src-tauri/mac/empaquetar.sh --sin-compilar   empaqueta el que ya está en target/release
#
# Deja en src-tauri/target/mac/ la app (Parabellum.app) y el .dmg (Parabellum-<versión>-mac.dmg).
set -euo pipefail

MAC="$(cd "$(dirname "$0")" && pwd)"
TAURI="$(dirname "$MAC")"
RAIZ="$(dirname "$TAURI")"
SALIDA="$TAURI/target/mac"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$TAURI/Cargo.toml" | head -1)"

if [ "${1:-}" != "--sin-compilar" ]; then
  (cd "$TAURI" && cargo build --release)
fi
PROGRAMA="$TAURI/target/release/Parabellum"
[ -x "$PROGRAMA" ] || { echo "Falta $PROGRAMA: compila con cargo build --release." >&2; exit 1; }

rm -rf "$SALIDA"
APP="$SALIDA/Parabellum.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# El programa (la interfaz y el mod del juego van dentro) y su Info.plist con la versión de Cargo.toml.
cp "$PROGRAMA" "$APP/Contents/MacOS/Parabellum"
sed "s/__VERSION__/$VERSION/g" "$MAC/Info.plist" > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" > /dev/null

# El ícono: el logo de la interfaz (512 px) en los tamaños que pide macOS (sin el de 1024).
ICONOS="$SALIDA/Parabellum.iconset"
mkdir -p "$ICONOS"
for lado in 16 32 128 256 512; do
  sips -z "$lado" "$lado" "$RAIZ/ui/logo.png" --out "$ICONOS/icon_${lado}x${lado}.png" > /dev/null
  if [ $((lado * 2)) -le 512 ]; then
    sips -z $((lado * 2)) $((lado * 2)) "$RAIZ/ui/logo.png" --out "$ICONOS/icon_${lado}x${lado}@2x.png" > /dev/null
  fi
done
iconutil -c icns "$ICONOS" -o "$APP/Contents/Resources/Parabellum.icns"
rm -rf "$ICONOS"

# Firma ad hoc: en Mac con chip Apple todo programa necesita al menos esta. Sin cuenta de Apple Developer,
# macOS pide confirmar la primera vez que se abre (tarea 12 de docs/mac.md). codesign no acepta atributos
# extendidos (los que pone el Finder), así que se quitan antes.
xattr -cr "$APP"
codesign --force --sign - "$APP"
codesign --verify --strict "$APP"

# El .dmg: la app y un acceso a Aplicaciones, para arrastrarla ahí.
DMG="$SALIDA/Parabellum-$VERSION-mac.dmg"
CARPETA="$SALIDA/dmg"
mkdir -p "$CARPETA"
ditto "$APP" "$CARPETA/Parabellum.app"
ln -s /Applications "$CARPETA/Aplicaciones"
hdiutil create -volname "Parabellum" -srcfolder "$CARPETA" -ov -format UDZO "$DMG" > /dev/null
rm -rf "$CARPETA"

echo "Listo:"
echo "  $APP"
echo "  $DMG"
