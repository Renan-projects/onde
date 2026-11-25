# Reprise (état au 25/11/2026, v0.2.3)

Les trois objectifs de la série 0.2 :

| Objectif | État |
|---|---|
| Lever les limites | **Fait.** i/s : 5 → fréquence de l'écran, au-delà (minuteur haute résolution), ou illimité ; échelle 1-400 %, vitesse 1-100 000 % via `onde.ini`. 1 seul réveil par image même en cadence bridée. |
| Fond sur l'écran de verrouillage | **Prêt, un clic de Renan.** L'API sans droits expire sur ce Windows ; lancer une fois `scripts/ActiverEcranVerrouillage.cmd` en administrateur, ensuite Onde réécrit `C:\ProgramData\Onde\ecran-verrouillage.png` seul. Voir `ECRAN_DE_VERROUILLAGE.md`. |
| Écrans synchronisés | **Fait et vérifié** sur les vrais écrans (contenu des fenêtres d'Onde capturé par `PrintWindow`) : aucune coupure à la jonction. Coordonnées globales en pixels logiques (`src/layout.rs`), même horloge, `uEcran = 0` en mode continu. |

À faire ensuite :

- Après le clic de Renan : verrouiller (Win+L), changer de fond, reverrouiller. Si l'image
  ne suit pas, Windows ne relit le fichier qu'à l'application de la stratégie : parade à
  trouver (ex. alterner deux chemins de fichier, ou forcer `gpupdate`).
- Décisions ouvertes : `A_DECIDER_RENAN.md` (verrouillage par défaut pour les autres, soleil
  d'*Horizon* sur la jonction, « illimité » dans le menu, fréquence de rafraîchissement).

Outils de test : `onde.exe --capture-bureau <fond> <sortie.bmp>` (jonction hors écran),
`APPDATA=<dossier temporaire>` pour ne pas toucher la config de Renan,
`ONDE_VERROU_CHEMIN=<…\Onde\ecran-verrouillage.png>` pour simuler le mode administrateur.
Mesures : un seul PowerShell et `onde.log` (images/s, réveils/s, coût par image), jamais de
boucles bash ; `--quitter` ferme toute instance, relancer celle de Renan ensuite.
