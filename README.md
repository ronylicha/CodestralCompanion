# Codestral Companion v0.9.0-beta

Application de bureau Ubuntu avec interface GUI et mode CLI/TUI pour interagir avec Mistral AI / Codestral.

## 🚀 Nouveautés v0.9.0-beta

### 🤖 Outils AI (Agent Mode)
- **read_file** : L'AI peut lire les fichiers du projet
- **write_file** : L'AI peut créer/modifier des fichiers
- **execute_bash** : L'AI exécute des commandes shell
- **list_directory** : L'AI liste les répertoires
- **search_in_files** : L'AI recherche dans les fichiers
- **Sécurité** : Commandes dangereuses (`rm`, `sudo`) demandent confirmation

### 🔌 Support MCP (Model Context Protocol)
- **Serveurs MCP** : Intégration de serveurs externes (Context7, WebSearch, etc.)
- **Config standard** : `.codestral/mcp_servers.json`
- **Outils dynamiques** : Les outils MCP sont découverts automatiquement

### ⚡ Améliorations
- **Retry API** : 4 tentatives avec backoff exponentiel (1s, 2s, 4s)
- **Spinner animé** : Indicateur de réflexion qui pulse

## Installation

```bash
sudo dpkg -i "Companion Chat_0.9.0-beta_amd64.deb"
sudo ln -sf /usr/bin/companion-chat /usr/bin/companion-chat-cli
```

## Configuration MCP

Créer `.codestral/mcp_servers.json` dans votre projet :
```json
{
  "mcpServers": {
    "context7": {
      "command": "npx",
      "args": ["-y", "@upstash/context7-mcp"]
    }
  }
}
```

## Commandes

```bash
companion-chat          # Mode GUI
companion-chat-cli      # Mode TUI (auto-détecté)
companion-chat-cli chat -c /projet
```

## Raccourcis TUI

| Touche | Action |
|--------|--------|
| Shift+Tab ou Alt+M | Cycler les modes |
| `/` | Menu commandes |
| PageUp/PageDown | Scroll chat |
| ↑↓ | Historique input |
| Enter | Envoyer |
| Ctrl+C / Esc | Quitter |

## Commandes disponibles

| Commande | Description |
|----------|-------------|
| `/new` | Nouvelle conversation |
| `/resume` | Reprendre une conversation |
| `/reindex` | Réindexer le projet (SQLite) |
| `/memory` | Éditer instructions projet |
| `/exit` | Sauvegarder et quitter |

## Historique des versions

### v0.8.0-beta
- Affichage instantané des messages
- Index SQLite avec barre de progression
- Navigation PageUp/PageDown
- Formulaire tabbé avec choix prédéfinis

### v0.7.0-beta
- Interface TUI complète avec ratatui
- 4 modes : ASK, PLAN, CODE, AUTO
- Auto-compaction à 90% du contexte

### v0.6.0-beta
- Interface GUI avec system tray
- Support Mistral AI / Codestral

## Licence

MIT
