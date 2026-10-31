#!/usr/bin/env bash
set -euo pipefail

APP_ID="io.github.analisepc.AnalisePC"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT_DIR/packaging/flatpak/${APP_ID}.yml"
BUILD_DIR="$ROOT_DIR/target/flatpak/build"
REPO_DIR="$ROOT_DIR/target/flatpak/repo"
BUNDLE_OUT="$ROOT_DIR/target/flatpak/analisepc.flatpak"

cd "$ROOT_DIR"

echo "=================================================="
echo "          COMPILANDO PACOTE FLATPAK (GNOME 50)    "
echo "=================================================="

# 1. Checa dependência do flatpak-builder
if ! command -v flatpak-builder >/dev/null 2>&1; then
    echo "Erro: 'flatpak-builder' não está instalado."
    echo "Instale com: sudo apt install flatpak flatpak-builder"
    exit 1
fi

# 2. Garante que o Flathub está configurado
if ! flatpak remotes | grep -q "flathub"; then
    echo "Configurando repositório Flathub..."
    flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
fi

# 3. Detecta se a instalação deve ser --user ou --system para não pedir senha desnecessariamente
INSTALL_SCOPE="--user"
if [ "$EUID" -eq 0 ]; then
    INSTALL_SCOPE="--system"
fi

echo "==> [1/3] Verificando Runtime do GNOME 50 e SDK Rust (escopo: $INSTALL_SCOPE)..."
# Se já estiver instalado no sistema ou no usuário, o Flatpak não baixa novamente
if ! flatpak info org.gnome.Sdk//50 >/dev/null 2>&1 || ! flatpak info org.freedesktop.Sdk.Extension.rust-stable//25.08 >/dev/null 2>&1; then
    flatpak install $INSTALL_SCOPE -y flathub \
        org.gnome.Platform//50 \
        org.gnome.Sdk//50 \
        org.freedesktop.Sdk.Extension.rust-stable//25.08
fi

echo "==> [2/3] Compilando e gerando repositório com flatpak-builder..."
mkdir -p "$ROOT_DIR/target/flatpak"
flatpak-builder --force-clean --repo="$REPO_DIR" "$BUILD_DIR" "$MANIFEST"

echo "==> [3/3] Exportando arquivo bundle distribuível (.flatpak)..."
flatpak build-bundle "$REPO_DIR" "$BUNDLE_OUT" "$APP_ID"

echo ""
echo "=================================================="
echo "  Pacote Flatpak gerado com sucesso!"
echo "  -> $BUNDLE_OUT"
echo "=================================================="
echo ""
echo "Para instalar o bundle gerado:"
echo "  flatpak install --user $BUNDLE_OUT"
echo ""
echo "Para testar rodando:"
echo "  flatpak run $APP_ID"
