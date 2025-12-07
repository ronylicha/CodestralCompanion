# Instructions d'Installation - Companion Chat

## Installation des Dépendances Système

### Ubuntu/Debian

```bash
sudo apt update
sudo apt install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libasound2-dev
```

### Installation de Rust

Si Rust n'est pas installé :

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Installation de Node.js

Si Node.js n'est pas installé (version 18+) :

```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
```

## Installation du Projet

### 1. Installer les dépendances Node.js

```bash
npm install
```

### 2. Vérifier la configuration

Assurez-vous que `src-tauri/tauri.conf.json` est correctement configuré et que toutes les icônes sont présentes dans `src-tauri/icons/`.

### 3. Mode Développement

Pour lancer l'application en mode développement :

```bash
npm run tauri dev
```

Cette commande :
- Lance le serveur de développement Vite
- Compile le backend Rust
- Ouvre l'application dans une fenêtre

### 4. Construction du Package .deb

Pour créer le package d'installation :

```bash
npm run tauri build
```

Le fichier `.deb` sera généré dans :
```
src-tauri/target/release/bundle/deb/companion-chat_0.1.0_amd64.deb
```

### 5. Installation du Package .deb

```bash
cd src-tauri/target/release/bundle/deb/
sudo dpkg -i companion-chat_0.1.0_amd64.deb
```

Si des dépendances manquent :

```bash
sudo apt-get install -f
```

## Dépannage

### Erreur : "libwebkit2gtk not found"
```bash
sudo apt install libwebkit2gtk-4.1-dev
```

### Erreur : "System tray not working"
```bash
sudo apt install libayatana-appindicator3-dev
```

### Erreur de compilation Rust
```bash
rustup update
cargo clean
npm run tauri build
```

### L'application ne démarre pas après installation
Vérifiez les logs :
```bash
journalctl -u companion-chat
# ou
~/.local/share/applications/companion-chat.desktop
```

## Désinstallation

```bash
sudo apt remove companion-chat
sudo apt autoremove
```

## Notes Importantes

1. **Première utilisation** : L'application nécessite une configuration API avant utilisation
2. **System Tray** : Assurez-vous que votre environnement de bureau supporte les applets de la barre système
3. **Permissions** : L'application a besoin d'accès au réseau pour communiquer avec l'API Mistral


