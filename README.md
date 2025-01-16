# Place Client

Un client Rust pour placer des pixels sur ftplace.42lwatch.ch selon un pattern prédéfini.

## Installation

1. Installez Rust si ce n'est pas déjà fait :
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Clonez le repository et compilez :
```bash
git clone <repository>
cd place_client
cargo build --release
```

## Structure des fichiers

```
place_client/
├── src/
│   └── main.rs
├── pattern/
│   └── votre_pattern.json
├── map/          # Créé automatiquement
│   ├── board_*.png
│   ├── board_*.txt
│   └── colors_*.txt
└── Cargo.toml
```

## Format du Pattern JSON

Le fichier pattern doit être au format suivant :
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
- `x`, `y` : Coordonnées relatives au point de départ
- `color` : ID de la couleur (1-16)

IDs des couleurs :
- 1: white
- 2: lightgray
- 3: darkgray
- 4: black
- 5: pink
- 6: red
- 7: orange
- 8: brown
- 9: yellow
- 10: lime
- 11: green
- 12: cyan
- 13: blue
- 14: indigo
- 15: magenta
- 16: purple

## Utilisation

```bash
cargo run --release -- \
    --refresh-token "votre_refresh_token" \
    --token "votre_token" \
    --start-x <x_depart> \
    --start-y <y_depart> \
    --pattern-file "pattern/votre_pattern.json"
```

## Fonctionnalités

- Place jusqu'à 10 pixels toutes les 31 minutes
- Vérifie l'état actuel avant de placer un pixel
- Gère automatiquement le refresh des tokens
- Sauvegarde l'état de la board dans le dossier `map`
- Attend 1 seconde entre chaque placement de pixel

## Logs

Le programme crée trois types de fichiers dans le dossier `map` :
- `board_<timestamp>.png` : Image de la board
- `board_<timestamp>.txt` : Matrice des IDs de couleur
- `colors_<timestamp>.txt` : Définition des couleurs

## Notes

- Le programme continuera indéfiniment jusqu'à ce que le pattern soit complet ou qu'il soit interrompu
- Les tokens peuvent être récupérés depuis les cookies du navigateur sur ftplace.42lwatch.ch
- Un délai de 31 minutes est respecté entre chaque batch de pixels