# Guide de Build - Génération du Package .deb Beta

Ce guide explique comment générer le package `.deb` pour distribuer la version Beta de Companion Chat.

## Prérequis

### 1. Installation de Rust

Si Rust n'est pas installé :

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Vérifiez l'installation :
```bash
cargo --version
rustc --version
```

### 2. Installation des Dépendances Système

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

### 3. Installation de Node.js (si nécessaire)

```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
```

Vérifiez l'installation :
```bash
node --version
npm --version
```

## Méthode 1 : Script Automatique (Recommandé)

Utilisez le script de build fourni :

```bash
./build-deb.sh
```

Ce script :
- ✅ Vérifie les prérequis (Rust, dépendances système)
- ✅ Installe les dépendances npm si nécessaire
- ✅ Build le frontend React
- ✅ Build l'application Tauri
- ✅ Génère le package .deb
- ✅ Affiche les informations du package

## Méthode 2 : Build Manuel

### Étape 1 : Installer les dépendances npm

```bash
npm install
```

### Étape 2 : Build du frontend

```bash
npm run build
```

### Étape 3 : Build de l'application Tauri

```bash
npm run tauri build
```

### Étape 4 : Localiser le fichier .deb

Le fichier `.deb` sera généré dans :
```
src-tauri/target/release/bundle/deb/companion-chat_0.1.0-beta_amd64.deb
```

## Informations du Package

Le package généré contient :
- Version : `0.1.0-beta`
- Architecture : `amd64`
- Nom du package : `companion-chat`
- Identifiant : `com.rony.companion-chat`

## Test du Package

Avant de distribuer, testez l'installation :

```bash
# Installer
sudo dpkg -i src-tauri/target/release/bundle/deb/companion-chat_0.1.0-beta_amd64.deb

# Si des dépendances manquent
sudo apt-get install -f

# Vérifier l'installation
dpkg -l | grep companion-chat

# Tester l'application
companion-chat
```

## Désinstallation

```bash
sudo apt remove companion-chat
sudo apt autoremove
```

## Distribution sur GitHub

### 1. Créer un Tag Git

```bash
git tag -a v0.1.0-beta -m "Version Beta - Première release"
git push origin v0.1.0-beta
```

### 2. Créer une Release GitHub

1. Allez sur votre dépôt GitHub
2. Cliquez sur "Releases" → "Draft a new release"
3. Sélectionnez le tag `v0.1.0-beta`
4. Titre : `v0.1.0 Beta`
5. Description :
   ```
   ## Version Beta - Companion Chat
   
   Première version Beta de Companion Chat - Application de chat Ubuntu avec intégration Mistral Codestral.
   
   ### Installation
   
   ```bash
   sudo dpkg -i companion-chat_0.1.0-beta_amd64.deb
   sudo apt-get install -f  # Si des dépendances manquent
   ```
   
   ### Fonctionnalités
   
   - ✅ System tray intégration
   - ✅ Support dual API (api.mistral.ai / codestral.mistral.ai)
   - ✅ Interface de chat moderne
   - ✅ Rendu Markdown complet
   - ✅ Gestion de multiples conversations
   - ✅ Persistance des données
   
   ### Notes
   
   - Version Beta - Des bugs peuvent être présents
   - N'hésitez pas à rapporter les problèmes sur les Issues GitHub
   ```
6. Uploadez le fichier `.deb` dans "Attach binaries"
7. Publiez la release

## Dépannage

### Erreur : "cargo: command not found"
- Installer Rust avec rustup (voir prérequis)

### Erreur : "libwebkit2gtk not found"
- Installer les dépendances système (voir prérequis)

### Erreur : "Failed to build"
- Vérifiez que toutes les dépendances sont installées
- Essayez `cargo clean` puis rebuildez
- Vérifiez les logs dans `build.log`

### Le .deb n'est pas généré
- Vérifiez que `tauri.conf.json` a `"targets": ["deb"]` dans `bundle`
- Vérifiez les logs de build pour les erreurs
- Assurez-vous que le build frontend a réussi

## Prochaines Versions

Pour créer de nouvelles versions :

1. Mettre à jour la version dans `src-tauri/tauri.conf.json` :
   ```json
   "version": "0.1.1-beta",
   ```

2. Mettre à jour la version dans `package.json` :
   ```json
   "version": "0.1.1",
   ```

3. Rebuild et créer un nouveau tag Git


