# Place Client

[🇫🇷 Version française](#fr) | [🇬🇧 English version](#en)

---

<a name="fr"></a>
# 🇫🇷 Place Client [FR]

Un client Rust pour placer des pixels sur ftplace.42lwatch.ch selon des patterns prédéfinis. Ce client supporte plusieurs patterns avec un système de priorité.

## Création de Patterns

Un outil en ligne est disponible pour créer facilement vos patterns :
[Pattern Creator](https://steady-concha-59e812.netlify.app/)

### Utilisation de l'outil
1. Visitez le site [Pattern Creator](https://steady-concha-59e812.netlify.app/)
2. Pour changer la taille de la grille :
   - Entrez les nouvelles dimensions souhaitées
   - Cliquez sur le bouton "Effacer" pour appliquer
3. Dessinez votre pattern en utilisant les différentes couleurs
4. Exportez le JSON généré dans un fichier pattern

## Installation

### Windows

1. Installez Rust :
   - Téléchargez et exécutez [rustup-init.exe](https://win.rustup.rs/)
   - Suivez les instructions d'installation
   - Redémarrez votre terminal

2. Installez les dépendances de build :
   - Installez [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   - Sélectionnez "C++ build tools" lors de l'installation

3. Installez Git :
   - Téléchargez et installez [Git pour Windows](https://git-scm.com/download/win)

### Debian/Ubuntu

```bash
# Installez les dépendances système
sudo apt update
sudo apt install -y curl build-essential pkg-config libssl-dev

# Installez Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### macOS

```bash
# Installez Homebrew si nécessaire
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Installez les dépendances
brew install pkg-config openssl@3

# Installez Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Installation du client

```bash
# Clonez le repository
git clone <repository>
cd place_client

# Compilez en mode release
cargo build --release
```

## Structure des fichiers

```
place_client/
├── src/
│   └── main.rs
├── pattern/
│   ├── defensive1.json    # Pattern défensif principal (obligatoire)
│   ├── defensive2.json    # Pattern défensif secondaire (optionnel)
│   ├── build1.json       # Pattern de construction 1 (optionnel)
│   ├── build2.json       # Pattern de construction 2 (optionnel)
│   └── build3.json       # Pattern de construction 3 (optionnel)
├── map/                  # Créé automatiquement
│   ├── board_*.png       # Captures de la board
│   ├── board_*.txt       # État de la board en texte
│   └── colors_*.txt      # Définition des couleurs
└── Cargo.toml
```

## Format des Patterns JSON

Chaque fichier pattern doit suivre le format suivant :

```json
{
  "pattern": [
    {
      "x": 0,
      "y": 0,
      "color": 4
    },
    {
      "x": 1,
      "y": 1,
      "color": 6
    }
  ]
}
```

Où :
- `x`, `y` : Coordonnées relatives au point de départ du pattern
- `color` : ID de la couleur (1-16)

### IDs des Couleurs

| ID | Couleur   | Code Hex |
|----|-----------|----------|
| 1  | white     | #FFFFFF  |
| 2  | lightgray | #D4D4D4  |
| 3  | darkgray  | #888888  |
| 4  | black     | #222222  |
| 5  | pink      | #FFA7D1  |
| 6  | red       | #E50000  |
| 7  | orange    | #E59500  |
| 8  | brown     | #A06A42  |
| 9  | yellow    | #E5D900  |
| 10 | lime      | #94E044  |
| 11 | green     | #02BE01  |
| 12 | cyan      | #00D3DD  |
| 13 | blue      | #0083C7  |
| 14 | indigo    | #0000EA  |
| 15 | magenta   | #CF6EE4  |
| 16 | purple    | #820080  |

## Utilisation

La commande supporte plusieurs patterns avec un système de priorité :

```bash
./target/release/place_client \
  --refresh-token "votre_refresh_token" \
  --token "votre_token" \
  --pattern "/path/to/pattern1.json 12 24 2"
  --pattern "/path/to/pattern2.json 42 21 1"
```

### Paramètres obligatoires
- `refresh-token` : Token de rafraîchissement
- `token` : Token d'authentification

### Autres Paramètres
- `pattern` : ceci indique que le suivant est un pattern à dessiner. Le format est le suivant :
    - Chemin vers un fichier JSON qui contient le pattern.
    - Coordonnées x et y de début du pattern sur la carte.
    - Priorité : indique quel pattern sera dessiné devant les autres.

## Fonctionnalités

- Système de priorité :
Vous pouvez fournir autant de patterns que vous le souhaitez. 
Chaque pattern doit être associé à une priorité. 
La priorité détermine l'ordre dans lequel les patterns seront dessinés :
    - Une priorité de 1 signifie que le pattern sera dessiné en premier.
    - Les patterns avec des priorités plus élevées seront dessinés ensuite, 
    un par un, dans l'ordre croissant des priorités.

> Le système commence par dessiner un pattern. 
> Si le pattern est complet, il passe au suivant, 
> en fonction de la priorité.

- Gestion des erreurs 502 avec retry automatique (10 tentatives, 2 minutes d'attente)
- Place jusqu'à 10 pixels toutes les 31 minutes
- Vérifie l'état actuel avant de placer un pixel
- Gestion automatique du refresh des tokens
- Sauvegarde l'état de la board dans le dossier `map`
- Attend 1 seconde entre chaque placement de pixel

## Logs et Monitoring

Le programme crée trois types de fichiers dans le dossier `map` :
- `board_<timestamp>.png` : Capture visuelle de la board
- `board_<timestamp>.txt` : Matrice des IDs de couleur
- `colors_<timestamp>.txt` : Définition des couleurs utilisées

### Niveaux de log
- DEBUG : Informations détaillées pour le débogage
- INFO : Statut des opérations normales
- ERROR : Erreurs non fatales
- WARN : Avertissements

## Notes Importantes

- Les tokens peuvent être récupérés depuis les cookies du navigateur sur ftplace.42lwatch.ch
- Le programme continue indéfiniment jusqu'à interruption manuelle
- Un délai de 31 minutes est respecté entre chaque batch de pixels
- Maximum de 10 pixels par batch
- Crée automatiquement le dossier `map` si nécessaire
- En cas d'erreur 502, le programme attendra 2 minutes avant de réessayer (10 tentatives maximum)

## Dépannage

### Erreurs Communes

1. Tokens invalides :
   ```
   Error: Request failed with status: 401
   ```
   Solution : Récupérez de nouveaux tokens depuis le navigateur

2. Erreur de connexion :
   ```
   Error: Connection error
   ```
   Solution : Vérifiez votre connexion internet et attendez le retry automatique

3. Erreur de format de pattern :
   ```
   Error: failed to parse pattern file
   ```
   Solution : Vérifiez le format JSON de votre fichier pattern

## Contribution

### Guide de contribution

1. Forkez le repository
2. Créez une branche pour votre fonctionnalité (`git checkout -b feature/maFonctionnalite`)
3. Committez vos changements (`git commit -am 'Ajout de ma fonctionnalité'`)
4. Poussez vers la branche (`git push origin feature/maFonctionnalite`)
5. Ouvrez une Pull Request

### Bonnes pratiques

- Ajoutez des commentaires dans votre code si nécessaire
- Incluez des tests pour les nouvelles fonctionnalités
- Mettez à jour la documentation si nécessaire
- Suivez le style de code existant
- Vérifiez que votre code compile sans warnings
- Testez vos modifications avant de soumettre une PR

### Types de contributions bienvenus

- Correction de bugs
- Nouvelles fonctionnalités
- Améliorations de la documentation
- Optimisations de performances
- Refactoring du code
- Ajout de tests

### Rapport de bugs

Pour signaler un bug, créez une issue en incluant :
- Description détaillée du bug
- Étapes pour reproduire
- Comportement attendu vs obtenu
- Logs d'erreur si disponibles
- Environnement (OS, version de Rust, etc.)

### Suggestions de fonctionnalités

Pour suggérer une nouvelle fonctionnalité :
1. Vérifiez d'abord qu'une suggestion similaire n'existe pas déjà
2. Ouvrez une issue avec le label "enhancement"
3. Décrivez la fonctionnalité et son cas d'utilisation
4. Attendez les retours avant de commencer l'implémentation

---

<a name="en"></a>
# 🇬🇧 Place Client [EN]

A Rust client for placing pixels on ftplace.42lwatch.ch according to predefined patterns. This client supports multiple patterns with a priority system.

## Pattern Creation

An online tool is available to easily create your patterns:
[Pattern Creator](https://steady-concha-59e812.netlify.app/)

### Using the Tool
1. Visit the [Pattern Creator](https://steady-concha-59e812.netlify.app/) website
2. To change the grid size:
   - Enter the desired dimensions
   - Click the "Clear" button to apply
3. Draw your pattern using the different colors
4. Export the generated JSON to a pattern file

## Installation

### Windows

1. Install Rust:
   - Download and run [rustup-init.exe](https://win.rustup.rs/)
   - Follow the installation instructions
   - Restart your terminal

2. Install build dependencies:
   - Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   - Select "C++ build tools" during installation

3. Install Git:
   - Download and install [Git for Windows](https://git-scm.com/download/win)

### Debian/Ubuntu

```bash
# Install system dependencies
sudo apt update
sudo apt install -y curl build-essential pkg-config libssl-dev

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### macOS

```bash
# Install Homebrew if needed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install pkg-config openssl@3

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Client Installation

```bash
# Clone the repository
git clone <repository>
cd place_client

# Build in release mode
cargo build --release
```

## File Structure

```
place_client/
├── src/
│   └── main.rs
├── pattern/
│   ├── defensive1.json    # Main defensive pattern (required)
│   ├── defensive2.json    # Secondary defensive pattern (optional)
│   ├── build1.json       # Build pattern 1 (optional)
│   ├── build2.json       # Build pattern 2 (optional)
│   └── build3.json       # Build pattern 3 (optional)
├── map/                  # Created automatically
│   ├── board_*.png       # Board snapshots
│   ├── board_*.txt       # Board state
│   └── colors_*.txt      # Color definitions
└── Cargo.toml
```

## JSON Pattern Format

Each pattern file must follow this format:

```json
{
  "pattern": [
    {
      "x": 0,
      "y": 0,
      "color": 4
    },
    {
      "x": 1,
      "y": 1,
      "color": 6
    }
  ]
}
```

Where:
- `x`, `y`: Coordinates relative to the pattern's starting point
- `color`: Color ID (1-16)

### Color IDs

| ID | Color     | Hex Code |
|----|-----------|----------|
| 1  | white     | #FFFFFF  |
| 2  | lightgray | #D4D4D4  |
| 3  | darkgray  | #888888  |
| 4  | black     | #222222  |
| 5  | pink      | #FFA7D1  |
| 6  | red       | #E50000  |
| 7  | orange    | #E59500  |
| 8  | brown     | #A06A42  |
| 9  | yellow    | #E5D900  |
| 10 | lime      | #94E044  |
| 11 | green     | #02BE01  |
| 12 | cyan      | #00D3DD  |
| 13 | blue      | #0083C7  |
| 14 | indigo    | #0000EA  |
| 15 | magenta   | #CF6EE4  |
| 16 | purple    | #820080  |

## Usage

The command supports multiple patterns with a priority system:

```bash
./target/release/place_client \
  --refresh-token "your_refresh_token" \
  --token "your_token" \
  --defensive1_x 100 \
  --defensive1_y 100 \
  --defensive1_pattern "pattern/defensive1.json" \
  --defensive2_x 150 \
  --defensive2_y 150 \
  --defensive2_pattern "pattern/defensive2.json" \
  --build1_x 200 \
  --build1_y 200--build1_pattern "pattern/build1.json" \
  --build2_x 250 \
  --build2_y 250 \
  --build2_pattern "pattern/build2.json" \
  --build3_x 300 \
  --build3_y 300 \
  --build3_pattern "pattern/build3.json"
```

### Required Parameters
- `refresh-token`: Refresh token
- `token`: Authentication token

### Other Parameters
- `pattern`: This indicates that the next entry is a pattern to be drawn. The format is as follows:
    - Path to a JSON file containing the pattern.
    - x and y coordinates for the starting position of the pattern on the map.
    - Priority indicates which pattern will be drawn in front of the others.

## Features

- Priority system:
You can provide as many patterns as you like.
Each pattern must be associated with a priority.
The priority determines the order in which the patterns will be drawn:
    - A priority of 1 means the pattern will be drawn first.
    - Patterns with higher priorities will be drawn afterward, 
    one by one, in ascending order of priority.

> The system starts by drawing the first pattern.
> If the pattern is complete, it moves to the next one according to its priority.

- 502 error handling with automatic retry (10 attempts, 2 minutes wait)
- Places up to 10 pixels every 31 minutes
- Checks current state before placing pixels
- Automatic token refresh handling
- Saves board state in the `map` folder
- Waits 1 second between each pixel placement

## Logs and Monitoring

The program creates three types of files in the `map` folder:
- `board_<timestamp>.png`: Visual snapshot of the board
- `board_<timestamp>.txt`: Matrix of color IDs
- `colors_<timestamp>.txt`: Color definitions

### Log Levels
- DEBUG: Detailed debugging information
- INFO: Normal operation status
- ERROR: Non-fatal errors
- WARN: Warnings

## Important Notes

- Tokens can be retrieved from browser cookies on ftplace.42lwatch.ch
- Program runs indefinitely until manually interrupted
- 31-minute delay between each batch of pixels
- Maximum of 10 pixels per batch
- Automatically creates `map` folder if needed
- On 502 error, the program will wait 2 minutes before retrying (maximum 10 attempts)

## Troubleshooting

### Common Errors

1. Invalid tokens:
   ```
   Error: Request failed with status: 401
   ```
   Solution: Get new tokens from the browser

2. Connection error:
   ```
   Error: Connection error
   ```
   Solution: Check your internet connection and wait for automatic retry

3. Pattern format error:
   ```
   Error: failed to parse pattern file
   ```
   Solution: Check the JSON format of your pattern file

## Contributing

### Contribution Guide

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/myFeature`)
3. Commit your changes (`git commit -am 'Add my feature'`)
4. Push to the branch (`git push origin feature/myFeature`)
5. Open a Pull Request

### Best Practices

- Add comments to your code when necessary
- Include tests for new features
- Update documentation if needed
- Follow existing code style
- Ensure your code compiles without warnings
- Test your changes before submitting a PR

### Welcome Contributions

- Bug fixes
- New features
- Documentation improvements
- Performance optimizations
- Code refactoring
- Test additions

### Bug Reports

To report a bug, create an issue including:
- Detailed bug description
- Steps to reproduce
- Expected vs actual behavior
- Error logs if available
- Environment (OS, Rust version, etc.)

### Feature Suggestions

To suggest a new feature:
1. First check if a similar suggestion already exists
2. Open an issue with the "enhancement" label
3. Describe the feature and its use case
4. Wait for feedback before starting implementation
