# Le fond sur l'écran de verrouillage

## Ce qui est possible, ce qui ne l'est pas

| Approche | Animé ? | Droits admin | Retenu |
|---|---|---|---|
| Dessiner une fenêtre par-dessus l'écran de verrouillage | — | — | **Impossible** : l'écran de verrouillage est LockApp (application UWP du système) ; la saisie du mot de passe se fait sur le bureau sécurisé de Winlogon. Aucune fenêtre d'une application ordinaire ne s'y affiche. |
| Remplacer LockApp / injecter dans LogonUI | oui | oui + système modifié | Non : fragile, cassé à chaque mise à jour, assimilable à un logiciel malveillant. |
| `HKLM\…\PersonalizationCSP` (`LockScreenImagePath`, `LockScreenImageStatus`) | non | **oui** | Non : demande les droits administrateur, et l'image est alors imposée (l'utilisateur ne peut plus la changer dans Paramètres). |
| Stratégie « Forcer une image d'écran de verrouillage » | non | oui (et éditions Pro/Entreprise) | Non, même raison. |
| API WinRT `Windows.System.UserProfile.LockScreen.SetImageFileAsync` | non | **non** | **Oui.** Accessible depuis une application de bureau non empaquetée, sans droits (appel accepté sur Windows 11 build 26300 ; voir l'état des essais ci-dessous). |

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

## État des essais (24/11/2026, PC de Renan)

- Sauvegarde de l'image d'origine, rendu et encodage PNG : **OK**.
- `SetImageFileAsync` avec une image **différente** de l'actuelle : **délai dépassé
  (0x800705B4) après 30 s**, depuis Onde comme depuis PowerShell (STA ou MTA, PNG ou JPEG).
  Avec l'image déjà en place, l'appel répond en 43 ms (aucun travail à faire).
- Le disque C: était alors plein (pagination insuffisante), mais **le même échec persiste
  avec 85 Go libres** (25/11, 00:20) : ce n'était pas la cause.
- Autres essais, tous en échec : `SetImageStreamAsync` (même délai dépassé) ; appel
  depuis une fenêtre au premier plan qui pompe les messages (même délai) ;
  `UserProfilePersonalizationSettings.TrySetLockScreenImageAsync` → `False` en 10 ms
  (attendu : cette API exige une application empaquetée). Aucune stratégie de groupe,
  aucun service désactivé en cause ; Windows 11 26H2 build 26300.9974 (canal normal).
- **Conclusion : sur ce Windows, poser l'image de l'écran de verrouillage depuis une
  application de bureau non empaquetée n'aboutit pas.** Onde garde la tentative toutes les
  heures (coût négligeable : un thread qui attend 30 s), au cas où une mise à jour de
  Windows corrige le service.

## Pistes qui marcheraient (décision de Renan)

1. **PersonalizationCSP avec droits admin, une seule fois** : une commande élevée (UAC)
   écrit `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\PersonalizationCSP`
   (`LockScreenImagePath`, `LockScreenImageUrl` → un PNG qu'Onde met à jour,
   `LockScreenImageStatus = 1`). Inconvénient : l'image devient imposée (plus modifiable
   dans Paramètres tant que la clé existe) ; à confirmer si Windows relit le fichier à
   chaque verrouillage ou seulement à l'application de la stratégie.
2. **Empaqueter Onde (MSIX avec identité de paquet, même « sparse package »)** : rend
   `TrySetLockScreenImageAsync` utilisable sans droits. Demande un certificat de signature
   et une installation différente.
3. **Manuel** : Paramètres → Personnalisation → Écran de verrouillage → Parcourir →
   `%APPDATA%\Onde\verrouillage-a.png` (image fixe, mise à jour seulement si on recommence).
