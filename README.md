# Companion Chat - Application de Chat Ubuntu avec Intégration Mistral Codestral

Application de bureau Ubuntu qui s'exécute dans la barre système (system tray) et fournit une interface de chat pour interagir avec le modèle Codestral de Mistral AI à travers deux options d'API différentes.

## Fonctionnalités

### Système Tray
- Icône dans la barre système Ubuntu
- Clic gauche : Afficher/masquer la fenêtre de chat
- Clic droit : Menu contextuel (Paramètres, Effacer l'historique, Quitter)
- Minimise dans le tray au lieu de quitter lors de la fermeture de la fenêtre

### Intégration API Dual
- **codestral.mistral.ai** : Abonnement mensuel (actuellement gratuit), nécessite une inscription avec numéro de téléphone sur le site pour obtenir la clé API
- **api.mistral.ai** : Utilise une clé API obtenue sur console.mistral.ai, paiement à l'usage pour Codestral

**Note :** Les deux fournisseurs nécessitent une clé API. Le numéro de téléphone sert uniquement à l'inscription sur le site web de Mistral pour obtenir la clé API, il n'est pas stocké dans l'application.

### Interface de Chat
- Thème clair avec design minimal et moderne
- Support de multiples conversations avec sélecteur en dropdown
- Design épuré avec alignement de texte simple
- Champ de saisie avec bouton d'envoi
- Historique de messages défilable
- Indicateurs de chargement pendant les appels API
- Gestion des erreurs pour les échecs API/limites de taux

### Gestion des Conversations
- Sélecteur dropdown pour basculer entre conversations
- Créer de nouvelles conversations
- Titres de conversations (auto-générés ou éditables)
- Supprimer des conversations avec confirmation
- Renommer des conversations
- Persistance de la liste des conversations entre les sessions

### Rendu Markdown Complet
- Support complet du Markdown standard avec tables, code inline, blocs de code avec coloration syntaxique
- Headers (H1-H6)
- Texte en gras, italique, barré
- Listes ordonnées et non ordonnées (support imbriqué)
- Liens (cliquables avec gestion d'URL appropriée)
- Citations
- Règles horizontales
- Images (si supportées par l'API)
- Boutons de copie pour les blocs de code
- Échappement et assainissement appropriés

### Persistance des Données
- Stockage de l'historique de chat dans des fichiers JSON locaux (un par conversation)
- Sauvegarde sécurisée des paramètres, clés API, choix du fournisseur et métadonnées de conversation
- Sauvegarde automatique lors de nouveaux messages
- Option "Effacer l'historique" dans le menu du system tray
- Restauration des données de conversation entre les sessions

## Prérequis

### Pour le développement
- Node.js (v18 ou supérieur)
- Rust (dernière version stable)
- Dépendances système Ubuntu :
  ```bash
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev \
      build-essential \
      curl \
      wget \
      libssl-dev \
      libgtk-3-dev \
      libayatana-appindicator3-dev \
      librsvg2-dev
  ```

### Pour l'utilisation
- Ubuntu 20.04 ou supérieur
- Clé API Mistral (requise pour les deux fournisseurs)

## Installation

### Depuis le code source

1. Cloner le dépôt :
```bash
git clone <repository-url>
cd companion-chat
```

2. Installer les dépendances :
```bash
npm install
```

3. Lancer en mode développement :
```bash
npm run tauri dev
```

### Construire le paquet .deb

```bash
npm run tauri build
```

Le fichier `.deb` sera généré dans `src-tauri/target/release/bundle/deb/`

### Installer le .deb

```bash
sudo dpkg -i companion-chat_0.1.0_amd64.deb
```

Si des dépendances manquent :
```bash
sudo apt-get install -f
```

## Configuration

### Premier lancement

1. L'application démarre dans le system tray (barre système)
2. Clic gauche sur l'icône pour ouvrir la fenêtre
3. Cliquer sur l'icône ⚙️ pour ouvrir les paramètres

### Configuration API

**Important :** Les deux fournisseurs nécessitent une clé API. Le numéro de téléphone sert uniquement à l'inscription sur le site web de Mistral pour obtenir votre clé API.

#### Option 1 : api.mistral.ai (Recommandé)
1. Obtenez votre clé API sur [console.mistral.ai](https://console.mistral.ai/)
2. Dans les paramètres, sélectionnez "api.mistral.ai"
3. Entrez votre clé API
4. Cliquez sur "Tester la connexion" pour vérifier
5. Enregistrez les paramètres

#### Option 2 : codestral.mistral.ai
1. Inscrivez-vous sur [codestral.mistral.ai](https://codestral.mistral.ai/) avec votre numéro de téléphone pour obtenir votre clé API
2. Dans les paramètres de l'application, sélectionnez "codestral.mistral.ai"
3. Entrez la clé API que vous avez obtenue lors de l'inscription
4. Cliquez sur "Tester la connexion" pour vérifier
5. Enregistrez les paramètres

### Créer une conversation

1. Utilisez le dropdown "Sélectionner une conversation..."
2. Cliquez sur "+ Nouvelle" pour créer une nouvelle conversation
3. Commencez à chatter immédiatement

## Structure du Projet

```
companion-chat/
├── src/                          # Frontend React
│   ├── components/
│   │   ├── ChatWindow.tsx       # Interface de chat principale
│   │   ├── ConversationSelector.tsx  # Sélecteur de conversations
│   │   ├── SettingsModal.tsx    # Modal de paramètres
│   │   └── MarkdownRenderer.tsx # Rendu Markdown
│   ├── App.tsx                   # Composant principal
│   ├── types.ts                  # Types TypeScript
│   └── App.css                   # Styles
├── src-tauri/                    # Backend Rust
│   ├── src/
│   │   ├── main.rs              # Point d'entrée
│   │   ├── lib.rs               # Configuration Tauri + System Tray
│   │   ├── commands.rs          # Commandes Tauri (API calls, file I/O)
│   │   ├── auth.rs              # Gestion de l'authentification dual
│   │   ├── conversations.rs     # Gestion des conversations
│   │   └── models.rs            # Structures de données
│   ├── Cargo.toml               # Dépendances Rust
│   └── tauri.conf.json          # Configuration Tauri
├── package.json                  # Dépendances Node.js
└── README.md                     # Ce fichier
```

## Utilisation

### Raccourcis clavier
- `Enter` : Envoyer le message
- `Shift + Enter` : Nouvelle ligne dans le champ de saisie
- `Ctrl + C` : Copier (dans les blocs de code)
- `Ctrl + N` : Nouvelle conversation (via le bouton)

### Fonctionnalités Markdown

L'application supporte le Markdown complet. Exemples :

- **Gras** : `**texte**`
- *Italique* : `*texte*`
- Code inline : `` `code` ``
- Blocs de code : ` ```language\ncode\n``` `
- Tables : Utilisez la syntaxe Markdown standard
- Listes : `- item` ou `1. item`
- Liens : `[texte](url)`

### Gestion des conversations

- **Renommer** : Cliquez sur "Renommer" et entrez le nouveau titre
- **Supprimer** : Cliquez sur "Supprimer" (confirmation requise)
- **Créer** : Cliquez sur "+ Nouvelle"

## Sécurité

- Toutes les clés API sont stockées de manière sécurisée via le système de stockage de Tauri
- Aucune donnée sensible n'est loggée
- Communication sécurisée entre frontend et backend
- Assainissement HTML pour le Markdown rendu

## Dépannage

### L'application ne démarre pas
- Vérifiez que toutes les dépendances système sont installées
- Vérifiez les logs dans la console

### Erreurs de connexion API
- Vérifiez que votre clé API est correcte
- Testez la connexion dans les paramètres
- Vérifiez votre connexion internet
- Vérifiez les limites de taux de votre compte Mistral

### L'icône du system tray n'apparaît pas
- Certaines distributions nécessitent des packages supplémentaires :
  ```bash
  sudo apt install libayatana-appindicator3-dev
  ```

## Contribution

Les contributions sont les bienvenues ! Veuillez :
1. Fork le projet
2. Créer une branche pour votre fonctionnalité
3. Commiter vos changements
4. Pousser vers la branche
5. Ouvrir une Pull Request

## Licence

[À définir]

## Support

Pour signaler des bugs ou demander des fonctionnalités, veuillez ouvrir une issue sur le dépôt GitHub.

## Notes

- L'inscription par téléphone pour codestral.mistral.ai nécessite l'implémentation de l'API d'inscription de Mistral (actuellement un placeholder)
- Pour l'instant, il est recommandé d'utiliser api.mistral.ai avec une clé API existante
