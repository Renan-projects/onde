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
- **Cadence calée sur l'écran** (`Present(n)` + objet d'attente DXGI) : le fil de rendu dort entre deux images, aucune boucle active, aucun minuteur haute résolution. Un minuteur ne complète que les cadences qui ne tombent pas juste (24 i/s sur 60 Hz) ou très basses.
- **Pause automatique, écran par écran** : fenêtre agrandie ou plein écran, jeu en plein écran exclusif, session verrouillée, écran éteint, batterie/économiseur (réglable). En pause, le fil de rendu dort sur un événement et la mémoire du pilote est rendue.
- **Mode efficacité de Windows 11** (EcoQoS) et priorité basse, CPU comme GPU : un jeu passe toujours devant.
- **Réactivité sans crochet système** : la souris est lue une fois par image ; le son (capture « loopback » WASAPI + FFT de 1024 points) n'est ouvert que si le fond affiché s'en sert.

## Plusieurs écrans : un seul fond continu

Par défaut, le fond s'étend sur **tout le bureau virtuel** : les motifs passent
d'un écran à l'autre sans coupure, avec la même horloge partout. Chaque écran
ne dessine que sa part (aucun surcoût). Les écrans de tailles, de positions ou
d'échelles (PPP) différentes sont gérés : le fond est placé en pixels logiques
pour qu'un motif garde la même taille apparente des deux côtés de la jonction.
Menu → « Un seul fond continu sur tous les écrans » pour revenir à un fond
indépendant par écran.

## Écran de verrouillage

Menu → « Aussi sur l'écran de verrouillage (image fixe) » : une image du fond
est rendue à la résolution de l'écran principal et posée comme image de
l'écran de verrouillage, à chaque changement de fond puis toutes les heures.
Sans droits administrateur (API `Windows.System.UserProfile.LockScreen`).
L'image d'origine est sauvegardée et remise quand on décoche l'option.

> **État actuel** : sur Windows 11 26H2 (build 26300), Windows laisse expirer la
> demande quand elle vient d'une application non empaquetée ; l'image n'est donc pas
> encore posée sur ce système. Onde réessaie toutes les heures. Pistes et essais :
> [docs/ECRAN_DE_VERROUILLAGE.md](docs/ECRAN_DE_VERROUILLAGE.md).

Limites de Windows : l'écran de verrouillage appartient à LockApp, sur un
bureau protégé où aucune application ne peut dessiner ; il ne peut donc pas
être **animé**. Si « Windows à la une » (Spotlight) est actif, le choix
d'une image le remplace. Une stratégie de groupe d'entreprise
(« Forcer une image d'écran de verrouillage ») a priorité. Détails :
[docs/ECRAN_DE_VERROUILLAGE.md](docs/ECRAN_DE_VERROUILLAGE.md).

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
(de 5 images/s à la fréquence de l'écran, ou sans limite ; 30 par défaut), la
résolution de rendu (10 à 200 %, 50 par défaut), la vitesse d'animation
(10 à 1000 %), la réaction à la souris et au son, le fond continu sur
plusieurs écrans, l'écran de verrouillage, les pauses, et le lancement au
démarrage de Windows.

Aucune borne arbitraire : `onde.ini` accepte n'importe quelle valeur
raisonnable hors menu (`fps = 144`, `fps = ecran`, `fps = illimite`,
`echelle = 300`, `vitesse = 5000`…) ; le menu l'affiche alors telle quelle.

Réglages : `%APPDATA%\Onde\onde.ini` — journal : `%APPDATA%\Onde\onde.log`
(il indique les images/s et le temps CPU par image).

### Fonds personnels

Tout fichier `.hlsl` placé dans `%APPDATA%\Onde\fonds\` apparaît dans le menu
et est recompilé dès qu'il est enregistré (une erreur s'affiche en
notification). Un `exemple.hlsl` commenté y est créé au premier lancement.

On écrit une fonction `float4 mainImage(float2 fragCoord)`, comme sur
Shadertoy. Variables disponibles : `iResolution`, `iTime`, `iMouse`,
`uSouris`, `uPresence`, `uClics[4]`, `uSon` (graves, médiums, aigus, volume),
`uHeure`, `uEcran`, `uVue`. `vec2/vec3/mix/fract/mod` sont acceptés pour
faciliter le portage depuis GLSL.

En mode continu, `iResolution` est la taille de **tout le bureau** et
`fragCoord`, `iMouse`, `uClics` sont dans ce repère commun : un fond écrit
pour un seul écran devient automatiquement panoramique. `uEcran` y vaut 0
partout (le vrai numéro d'écran est dans `uVue.w`).

### Ligne de commande

```
onde.exe --quitter                          ferme proprement l'instance lancée
onde.exe --verifier [fichier.hlsl…]         compile les fonds et signale les erreurs
onde.exe --capture <fond> <sortie.bmp> [l h t]   rend une image hors écran
onde.exe --capture-bureau <fond> <sortie.bmp> [échelle% t]
                                            rend le bureau entier écran par écran
                                            (vérifie la continuité aux jonctions)
onde.exe --diag                             organisation du bureau et écrans masqués
```

## Compilation

```
cargo build --release
```

`scripts/apercu.py` produit une planche-contact de tous les fonds (Pillow).

## Licence

MIT
