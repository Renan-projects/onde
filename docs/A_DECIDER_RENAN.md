# À décider (Renan)

Décisions prises par défaut pendant la série 0.2.0 ; chacune se change en une ligne.

1. **Écran de verrouillage activé chez toi, désactivé par défaut pour les autres.**
   Il remplace l'image de l'écran de verrouillage (l'originale, `C:\Users\renan\Downloads\fond.png`,
   est sauvegardée dans `%APPDATA%\Onde\verrouillage-origine.png` et remise si tu décoches).
   → L'activer par défaut pour tout le monde ?

2. **Fond continu activé par défaut.** Conséquence : les fonds « centrés » se centrent sur le
   bureau entier. Avec tes deux écrans côte à côte, le soleil d'*Horizon* est pile sur la
   jonction, et le centre des *Ondes* aussi. → Garder, ou décaler ces fonds pour que leur
   sujet tombe au milieu de l'écran principal ?

3. **« Illimité » dans le menu des images/s.** Ça marche (voir mesures dans le README), mais
   c'est un mode de test : sur un fond d'écran, au-delà de la fréquence de l'écran les images
   sont jetées par le compositeur. → Le garder visible, ou seulement via `onde.ini` ?

4. **Rafraîchissement de l'écran de verrouillage toutes les heures.** → Plus souvent, moins
   souvent, ou seulement au changement de fond ?

5. **Ta config actuelle** : `fps = 60`, `echelle = 100`. Avec la levée des bornes, `fps = ecran`
   fait la même chose sur tes écrans 60 Hz et suivrait un écran 144 Hz si tu en changes.

6. **Disque C: plein (≈ 400 Mo libres le 25/11 à minuit) : à libérer en priorité.** Windows
   signale « fichier de pagination insuffisant », et le service qui pose l'image de l'écran
   de verrouillage ne répond plus (délai dépassé à chaque essai). L'option est active chez
   toi ; Onde réessaie toutes les heures et l'image apparaîtra dès que le système respire.
   Si tu veux le confirmer tout de suite après avoir libéré de la place : menu → décocher
   puis recocher « Aussi sur l'écran de verrouillage ».
