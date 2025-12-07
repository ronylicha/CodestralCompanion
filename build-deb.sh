#!/bin/bash

# Script de build pour générer le package .deb Beta
# Usage: ./build-deb.sh

set -e

echo "🔨 Build du package .deb Companion Chat Beta"
echo "=============================================="

# Vérifier si Rust est installé
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo n'est pas installé."
    echo ""
    echo "Pour installer Rust, exécutez:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "  source \$HOME/.cargo/env"
    echo ""
    exit 1
fi

# Vérifier si les dépendances système sont installées
echo "📦 Vérification des dépendances système..."
MISSING_DEPS=()

if ! dpkg -l | grep -q libwebkit2gtk-4.1-dev; then
    MISSING_DEPS+=("libwebkit2gtk-4.1-dev")
fi

if ! dpkg -l | grep -q libayatana-appindicator3-dev; then
    MISSING_DEPS+=("libayatana-appindicator3-dev")
fi

if [ ${#MISSING_DEPS[@]} -ne 0 ]; then
    echo "⚠️  Dépendances manquantes: ${MISSING_DEPS[*]}"
    echo "Installez-les avec:"
    echo "  sudo apt install ${MISSING_DEPS[*]} libgtk-3-dev libssl-dev build-essential librsvg2-dev"
    echo ""
    exit 1
fi

# Vérifier si Node.js et npm sont installés
if ! command -v npm &> /dev/null; then
    echo "❌ npm n'est pas installé."
    echo "Installez Node.js et npm pour continuer."
    exit 1
fi

# Installer les dépendances npm si nécessaire
if [ ! -d "node_modules" ]; then
    echo "📦 Installation des dépendances npm..."
    npm install
fi

# Build du frontend
echo "🏗️  Build du frontend..."
npm run build

# Build de l'application Tauri et génération du .deb
echo "🏗️  Build de l'application Tauri..."
npm run tauri build

# Trouver le fichier .deb généré
DEB_FILE=$(find src-tauri/target/release/bundle/deb -name "*.deb" -type f | head -1)

if [ -z "$DEB_FILE" ]; then
    echo "❌ Aucun fichier .deb trouvé!"
    exit 1
fi

echo ""
echo "✅ Build terminé avec succès!"
echo ""
echo "📦 Fichier .deb généré:"
echo "   $DEB_FILE"
echo ""
echo "📊 Informations du package:"
dpkg-deb -I "$DEB_FILE" 2>/dev/null | head -20 || true
echo ""
echo "Pour installer le package:"
echo "  sudo dpkg -i \"$DEB_FILE\""
echo ""
echo "Pour créer une release GitHub:"
echo "  1. Créez un tag: git tag -a v0.1.0-beta -m 'Version Beta'"
echo "  2. Poussez le tag: git push origin v0.1.0-beta"
echo "  3. Créez une release sur GitHub et uploadez le fichier .deb"
echo ""


