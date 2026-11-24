# Le fond sur l'écran de verrouillage

## Ce qui est possible, ce qui ne l'est pas

| Approche | Animé ? | Droits admin | Retenu |
|---|---|---|---|
| Dessiner une fenêtre par-dessus l'écran de verrouillage | — | — | **Impossible** : l'écran de verrouillage est LockApp (application UWP du système) ; la saisie du mot de passe se fait sur le bureau sécurisé de Winlogon. Aucune fenêtre d'une application ordinaire ne s'y affiche. |
| Remplacer LockApp / injecter dans LogonUI | oui | oui + système modifié | Non : fragile, cassé à chaque mise à jour, assimilable à un logiciel malveillant. |
| `HKLM\…\PersonalizationCSP` (`LockScreenImagePath`, `LockScreenImageStatus`) | non | **oui** | Non : demande les droits administrateur, et l'image est alors imposée (l'utilisateur ne peut plus la changer dans Paramètres). |
| Stratégie « Forcer une image d'écran de verrouillage » | non | oui (et éditions Pro/Entreprise) | Non, même raison. |
| API WinRT `Windows.System.UserProfile.LockScreen.SetImageFileAsync` | non | **non** | **Oui.** Fonctionne depuis une application de bureau non empaquetée (vérifié sur Windows 11 25H2, build 26300). |

Wallpaper Engine et Lively font le même constat : ils proposent au mieux une
image fixe pour l'écran de verrouillage.

## Ce que fait Onde

Option du menu « Aussi sur l'écran de verrouillage (image fixe) » (`verrouillage = oui`
dans `onde.ini`) :

1. **Sauvegarde unique** de l'image actuelle de l'écran de verrouillage
   (`LockScreen.GetImageStream`) dans `%APPDATA%\Onde\verrouillage-origine.*`.
2. Rendu **hors écran** d'une image du fond à la résolution de l'écran principal,
   sans souris ni clics, son au repos, heure réelle (`uHeure`) — sur un
   périphérique Direct3D temporaire, dans un fil à part : le rendu animé n'est
   pas touché.
3. Encodage PNG (WIC) dans `%APPDATA%\Onde\verrouillage-a.png` / `-b.png`
   (alternés), puis `LockScreen.SetImageFileAsync`.
4. Rafraîchi **à chaque changement de fond** (et à chaque enregistrement d'un
   fond personnel) **et toutes les heures** — les fonds qui dépendent de l'heure
   suivent ainsi la journée.
5. Décocher l'option **remet l'image d'origine**.

Coût : quelques centaines de millisecondes de CPU/GPU une fois par heure.

## Limites de Windows

- L'image est **fixe** : aucune animation possible sur l'écran de verrouillage.
- « Windows à la une » (Spotlight) : poser une image le remplace par le mode « Image ».
- Une stratégie de groupe d'entreprise qui impose l'image a priorité ; l'appel
  échoue alors (visible dans `onde.log`).
- Windows recopie l'image dans son propre cache ; le changement est visible au
  verrouillage suivant.
- Un seul fond pour tous les écrans : l'écran de verrouillage montre la même
  image partout ; elle est rendue à la taille de l'écran principal.
