# Codestral Companion v0.5.0-beta

Application de bureau Ubuntu avec interface GUI et mode CLI/TUI pour interagir avec Mistral AI / Codestral.

## 🚀 Nouveautés v0.5.0-beta

### Mode CLI / TUI (Terminal User Interface)
- **Interface TUI complète** avec ratatui : header, chat scrollable, input, status bar
- **4 modes de travail** : ASK, PLAN, CODE, AUTO (Shift+Tab pour cycler)
- **Mode AUTO** : Travaille en continu jusqu'à [TERMINÉ], sans s'arrêter
- **Auto-compaction** : À 90% du contexte, résume l'historique pour continuer
- **Menu commandes** : Tapez `/` pour ouvrir le menu
- **Commandes disponibles** :
  - `/new` - Nouvelle conversation
  - `/resume` - Reprendre une conversation
  - `/save` - Sauvegarder la conversation
  - `/memory` - Éditer les instructions projet (ouvre vim)
  - `/clear` - Effacer l'historique
  - `/reindex` - Réindexer le projet
  - `/ask`, `/plan`, `/code`, `/auto` - Changer de mode
  - `/quit` - Quitter
- **Mémoire projet** : Fichier `.codestral/memory.md` lu avec chaque prompt
- **Configuration API interactive** : Si pas de clé, assistant de configuration

### Interface GUI
- Bouton copier sur les blocs de code
- Conversations auto-nommées
- Fermeture vers le tray (ne quitte pas)

## Installation

### Depuis le .deb
```bash
sudo dpkg -i "Companion Chat_0.5.0-beta_amd64.deb"
```

### Commandes

```bash
# Mode GUI (fenêtre)
companion-chat

# Mode TUI interactif
companion-chat chat

# Mode TUI dans un projet spécifique
companion-chat chat -c /chemin/projet

# Modes agent (commande unique)
companion-chat plan "Ajoute des tests"
companion-chat interactive "Refactore ce fichier"
companion-chat auto "Corrige tous les bugs"
```

## Configuration API

Au premier lancement CLI sans clé configurée :
1. Choisir l'endpoint (Mistral AI ou Codestral)
2. Entrer la clé API
3. Configuration sauvegardée automatiquement

Ou via l'interface GUI (⚙️ Paramètres).

## Mémoire Projet

Créez un fichier `.codestral/memory.md` à la racine de votre projet pour des instructions persistantes :

```markdown
# Instructions Projet

- Toujours utiliser TypeScript strict
- Préférer les composants fonctionnels React
- Conventions de nommage camelCase
```

Ces instructions sont incluses dans chaque prompt.

## Modes de travail

| Mode | Description |
|------|-------------|
| **ASK** | Questions simples, pas de modifications |
| **PLAN** | Propose un plan, montre les diffs, n'applique pas |
| **CODE** | Propose et demande confirmation avant d'appliquer |
| **AUTO** | Applique automatiquement, continue jusqu'à finir |

## Raccourcis clavier

| Touche | Action |
|--------|--------|
| Shift+Tab | Cycler les modes |
| `/` | Menu commandes |
| ↑↓ | Historique / Scroll |
| ←→ | Naviguer dans l'input |
| PageUp/Down | Scroll rapide |
| Ctrl+C / Esc | Quitter |

## Prérequis

- Ubuntu 20.04 ou supérieur
- Clé API Mistral/Codestral

### Pour le développement
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget \
    libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
npm install
npm run tauri dev
```

## Construction

```bash
npm run tauri build
# Résultat: src-tauri/target/release/bundle/deb/
```

## Licence

MIT

## Support

Issues sur GitHub : [github.com/ronylicha/CodestralCompanion](https://github.com/ronylicha/CodestralCompanion)
