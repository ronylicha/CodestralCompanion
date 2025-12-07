# Codestral Companion v0.7.0-beta

Application de bureau Ubuntu avec interface GUI et mode CLI/TUI pour interagir avec Mistral AI / Codestral.

## 🚀 Nouveautés v0.7.0-beta

### Formulaire Tabbé
- **Formulaire multi-questions** : Quand plusieurs questions sont posées, affiche un formulaire tabbé
- **Navigation** : Tab / Shift+Tab entre les champs
- **Curseur** : ← → pour naviguer dans le champ actuel
- **Validation** : Enter pour soumettre toutes les réponses
- **Test** : `/questions` pour démontrer le formulaire

### Raccourcis mis à jour
- **Alt+Shift** : Changer de mode (visible dans la barre de status)
- **Barre de status** : `-- CODE [Alt+⇧] │ 1234 tok │ ~96% │ /: menu`

### Nouvelles commandes
- `/exit` : Sauvegarder et quitter
- `/questions` : Tester le formulaire tabbé

### Mode CLI / TUI
- **Interface TUI complète** avec ratatui
- **4 modes** : ASK, PLAN, CODE, AUTO
- **Mode AUTO** : Continue jusqu'à [TERMINÉ]
- **Auto-compaction** : À 90% du contexte
- **Menu `/`** : Toutes les commandes
- **Mémoire projet** : `.codestral/memory.md` (créé automatiquement avec template)

### Interface GUI
- Bouton copier 📋 sur les blocs de code
- Zone de texte auto-expansible
- Gros collages affichés en résumé

## Installation

```bash
sudo dpkg -i "Companion Chat_0.7.0-beta_amd64.deb"
```

## Commandes

```bash
companion-chat          # Mode GUI
companion-chat chat     # Mode TUI
companion-chat chat -c /projet
```

## Raccourcis TUI

| Touche | Action |
|--------|--------|
| Alt+Shift+Tab | Cycler les modes |
| `/` | Menu commandes |
| Tab / Shift+Tab | Navigation formulaire |
| ↑↓ | Historique / Scroll |
| Enter | Envoyer / Valider |
| Ctrl+C / Esc | Quitter |

## Commandes disponibles

| Commande | Description |
|----------|-------------|
| `/new` | Nouvelle conversation |
| `/resume` | Reprendre une conversation |
| `/save` | Sauvegarder |
| `/memory` | Éditer instructions projet |
| `/questions` | Test formulaire tabbé |
| `/exit` | Sauvegarder et quitter |
| `/quit` | Quitter sans sauvegarder |
| `/ask`, `/plan`, `/code`, `/auto` | Changer de mode |

## Licence

MIT
