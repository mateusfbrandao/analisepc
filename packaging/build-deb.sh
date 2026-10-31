#!/usr/bin/env bash
set -euo pipefail

APP_NAME="analisepc"
APP_ID="io.github.analisepc.AnalisePC"
VERSION="0.1.0"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "==> [1/4] Compilando binário de release com Cargo..."
if command -v cargo >/dev/null 2>&1; then
    cargo build --release
elif [ -x "$HOME/.cargo/bin/cargo" ]; then
    "$HOME/.cargo/bin/cargo" build --release
else
    echo "Erro: 'cargo' não encontrado no PATH nem em ~/.cargo/bin/cargo" >&2
    exit 1
fi

STAGE_DIR="$ROOT_DIR/target/debian/${APP_NAME}_${VERSION}_${ARCH}"
OUT_DEB="$ROOT_DIR/target/debian/${APP_NAME}_${VERSION}_${ARCH}.deb"

echo "==> [2/4] Preparando estrutura do pacote em $STAGE_DIR..."
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/DEBIAN"
mkdir -p "$STAGE_DIR/usr/bin"
mkdir -p "$STAGE_DIR/usr/share/applications"
mkdir -p "$STAGE_DIR/usr/share/metainfo"
mkdir -p "$STAGE_DIR/usr/share/icons/hicolor"

# Binário principal (strip para reduzir tamanho)
install -Dm755 "$ROOT_DIR/target/release/$APP_NAME" "$STAGE_DIR/usr/bin/$APP_NAME"
strip --strip-unneeded "$STAGE_DIR/usr/bin/$APP_NAME" 2>/dev/null || true

# Desktop Entry & AppStream Metainfo
install -Dm644 "$ROOT_DIR/data/${APP_ID}.desktop" "$STAGE_DIR/usr/share/applications/${APP_ID}.desktop"
install -Dm644 "$ROOT_DIR/data/${APP_ID}.metainfo.xml" "$STAGE_DIR/usr/share/metainfo/${APP_ID}.metainfo.xml"

# Ícones (App Icon + Symbolic Icons)
cp -r "$ROOT_DIR/assets/icons/hicolor/"* "$STAGE_DIR/usr/share/icons/hicolor/"

# Calcula tamanho instalado em KB
INSTALLED_SIZE="$(du -sk "$STAGE_DIR/usr" | cut -f1)"

echo "==> [3/4] Gerando DEBIAN/control e scripts pós-instalação..."
cat > "$STAGE_DIR/DEBIAN/control" <<EOF
Package: ${APP_NAME}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Depends: libc6 (>= 2.34), libgtk-4-1 (>= 4.12), libadwaita-1-0 (>= 1.5), libcairo2 (>= 1.16)
Recommends: smartmontools, pciutils, usbutils, iputils-ping, power-profiles-daemon
Maintainer: Mateus <mateus@localhost>
Homepage: https://github.com/analisepc/AnalisePC
Description: Monitoramento de hardware, diagnóstico S.M.A.R.T. e otimização para GNOME
 Aplicativo nativo em Rust, GTK 4 e Libadwaita projetado segundo as diretrizes
 GNOME Human Interface Guidelines (HIG) para monitorar CPU, memória RAM,
 discos NVMe/SATA, temperatura, rede, barramentos PCIe/USB e saúde do sistema.
EOF

cat > "$STAGE_DIR/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
EOF
chmod 0755 "$STAGE_DIR/DEBIAN/postinst"

echo "==> [4/4] Empacotando .deb com dpkg-deb..."
dpkg-deb --root-owner-group --build "$STAGE_DIR" "$OUT_DEB"

echo ""
echo "Pacote .deb gerado com sucesso:"
echo "  -> $OUT_DEB"
echo ""
echo "Para instalar:"
echo "  sudo apt install $OUT_DEB"
