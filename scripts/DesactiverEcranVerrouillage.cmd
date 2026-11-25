@echo off
chcp 65001 >nul
rem Onde : annule ActiverEcranVerrouillage.cmd (clic droit > Exécuter en tant qu'administrateur).
rem L'écran de verrouillage redevient réglable dans Paramètres > Personnalisation.

net session >nul 2>&1
if errorlevel 1 (
    echo Ce script doit être lancé en administrateur : clic droit ^> Exécuter en tant qu'administrateur.
    pause
    exit /b 1
)

set "CLE=HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\PersonalizationCSP"
reg delete "%CLE%" /v LockScreenImagePath /f >nul 2>&1
reg delete "%CLE%" /v LockScreenImageUrl /f >nul 2>&1
reg delete "%CLE%" /v LockScreenImageStatus /f >nul 2>&1
rem La clé est supprimée seulement si elle ne contient plus rien d'autre.
reg query "%CLE%" /s 2>nul | findstr /r /c:"REG_" >nul || reg delete "%CLE%" /f >nul 2>&1

echo.
echo C'est annulé : Onde ne touche plus à l'écran de verrouillage par ce moyen.
echo Choisis l'image voulue dans Paramètres ^> Personnalisation ^> Écran de verrouillage.
echo (Dossier laissé en place : %ProgramData%\Onde, supprimable à la main.)
echo.
pause
