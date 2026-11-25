@echo off
chcp 65001 >nul
rem Onde : le fond sur l'écran de verrouillage, par la stratégie PersonalizationCSP.
rem A lancer UNE fois : clic droit > Exécuter en tant qu'administrateur.
rem Ensuite Onde réécrit lui-même C:\ProgramData\Onde\ecran-verrouillage.png
rem à chaque changement de fond (et toutes les heures), sans plus jamais demander de droits.

net session >nul 2>&1
if errorlevel 1 (
    echo Ce script doit être lancé en administrateur : clic droit ^> Exécuter en tant qu'administrateur.
    pause
    exit /b 1
)

set "DOSSIER=%ProgramData%\Onde"
set "IMAGE=%DOSSIER%\ecran-verrouillage.png"
set "CLE=HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\PersonalizationCSP"

if not exist "%DOSSIER%" mkdir "%DOSSIER%"
rem Les utilisateurs du PC peuvent réécrire l'image (Onde tourne sans droits).
icacls "%DOSSIER%" /grant *S-1-5-32-545:(OI)(CI)M >nul
if errorlevel 1 (
    echo Impossible de régler les droits du dossier %DOSSIER%.
    pause
    exit /b 1
)

rem Une première image tout de suite : la dernière produite par Onde, sinon l'image actuelle de Windows.
if not exist "%IMAGE%" (
    if exist "%APPDATA%\Onde\verrouillage-a.png" (
        copy /y "%APPDATA%\Onde\verrouillage-a.png" "%IMAGE%" >nul
    ) else if exist "%APPDATA%\Onde\verrouillage-origine.png" (
        copy /y "%APPDATA%\Onde\verrouillage-origine.png" "%IMAGE%" >nul
    )
)

reg add "%CLE%" /v LockScreenImagePath /t REG_SZ /d "%IMAGE%" /f >nul || goto :erreur
reg add "%CLE%" /v LockScreenImageUrl /t REG_SZ /d "%IMAGE%" /f >nul || goto :erreur
reg add "%CLE%" /v LockScreenImageStatus /t REG_DWORD /d 1 /f >nul || goto :erreur

echo.
echo C'est fait. L'écran de verrouillage affiche maintenant :
echo   %IMAGE%
echo Onde le met à jour tout seul (changement de fond, puis toutes les heures).
echo Pour annuler : DesactiverEcranVerrouillage.cmd, lui aussi en administrateur.
echo.
pause
exit /b 0

:erreur
echo Écriture dans le registre impossible (%CLE%).
pause
exit /b 1
