# Onde

Fonds d'écran animés et réactifs pour Windows 10/11, conçus pour ne consommer
quasiment rien.

| Mesure (aurore + réaction au son, 2 écrans 1080p, RTX 5060 Ti) | Onde |
|---|---|
| CPU en régime établi | ~0,3 % d'un cœur |
| GPU | ~1,5 % |
| CPU/GPU quand le bureau est caché (jeu, fenêtre agrandie, écran verrouillé…) | 0 % |
| Exécutable | ~0,5 Mo, aucune dépendance |

## Pourquoi c'est léger

- **Natif** : Rust + Direct3D 11, pas de navigateur embarqué, pas de moteur de jeu, pas de vidéo à décoder.
- **Un triangle par image** : chaque fond est un petit programme GPU (shader HLSL).
- **Rendu à résolution réduite** (50 % par défaut) puis étiré gratuitement par le compositeur de Windows ; les fonds sont doux, la différence ne se voit pas.
- **Cadence calée sur l'écran** (`Present(n)` + objet d'attente DXGI) : le fil de rendu dort entre deux images, aucune boucle active, aucun minuteur haute résolution.
- **Pause automatique, écran par écran** : fenêtre agrandie ou plein écran, jeu en plein écran exclusif, session verrouillée, écran éteint, batterie/économiseur (réglable). En pause, le fil de rendu dort sur un événement et la mémoire du pilote est rendue.
- **Mode efficacité de Windows 11** (EcoQoS) et priorité basse, CPU comme GPU : un jeu passe toujours devant.
- **Réactivité sans crochet système** : la souris est lue une fois par image ; le son (capture « loopback » WASAPI + FFT de 1024 points) n'est ouvert que si le fond affiché s'en sert.

## Fonds intégrés

| Fond | Réagit à |
|---|---|
| Aurore boréale | souris (éclaire le ciel), graves |
| Encre | souris (tourbillon), graves |
| Ondes | clics sur le bureau (ronds dans l'eau), sillage de la souris, graves |
| Horizon | soleil qui suit la souris, grille au rythme des basses |
| Constellation | liens vers la souris, scintillement sur les aigus |
| Lampe à lave | bulle attirée par la souris, basses |
| Brume (le plus léger) | lueur sous la souris, volume |

## Utilisation

Lancer `onde.exe` : le fond démarre sous les icônes et une icône apparaît
dans la zone de notification. Son menu permet de choisir le fond, la fluidité
(10 à 60 images/s, 30 par défaut), la résolution de rendu, la vitesse
d'animation, la réaction à la souris et au son, les pauses, et le lancement
au démarrage de Windows.

Réglages : `%APPDATA%\Onde\onde.ini` — journal : `%APPDATA%\Onde\onde.log`
(il indique les images/s et le temps CPU par image).

### Fonds personnels

Tout fichier `.hlsl` placé dans `%APPDATA%\Onde\fonds\` apparaît dans le menu
et est recompilé dès qu'il est enregistré (une erreur s'affiche en
notification). Un `exemple.hlsl` commenté y est créé au premier lancement.

On écrit une fonction `float4 mainImage(float2 fragCoord)`, comme sur
Shadertoy. Variables disponibles : `iResolution`, `iTime`, `iMouse`,
`uSouris`, `uPresence`, `uClics[4]`, `uSon` (graves, médiums, aigus, volume),
`uHeure`, `uEcran`. `vec2/vec3/mix/fract/mod` sont acceptés pour faciliter le
portage depuis GLSL.

### Ligne de commande

```
onde.exe --quitter                          ferme proprement l'instance lancée
onde.exe --verifier [fichier.hlsl…]         compile les fonds et signale les erreurs
onde.exe --capture <fond> <sortie.bmp> [l h t]   rend une image hors écran
onde.exe --diag                             organisation du bureau et écrans masqués
```

## Compilation

```
cargo build --release
```

`scripts/apercu.py` produit une planche-contact de tous les fonds (Pillow).

## Licence

MIT
