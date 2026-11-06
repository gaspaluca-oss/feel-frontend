@echo off
cd /d "%~dp0"
echo.
echo  FEEL 2.0 - anteprima 0.1.0
echo  ==========================
echo.
echo  1. Importo la libreria di esempio...
echo.
"%~dp0feel.exe" import "%~dp0demo" --db "%~dp0demo.sqlite"
echo.
echo  ------------------------------------------------------------
echo  2. Cosa contiene il database
echo  ------------------------------------------------------------
echo.
"%~dp0feel.exe" info "%~dp0demo.sqlite"
echo.
echo  ------------------------------------------------------------
echo  3. I giochi della lista
echo  ------------------------------------------------------------
echo.
"%~dp0feel.exe" list "%~dp0demo.sqlite" mame-0
echo.
echo  ------------------------------------------------------------
echo  4. Cosa verrebbe eseguito, senza eseguirlo
echo  ------------------------------------------------------------
echo.
"%~dp0feel.exe" launch "%~dp0demo.sqlite" mame-0 galaga --dry-run
echo.
echo  ------------------------------------------------------------
echo  5. Converte il layout
echo  ------------------------------------------------------------
echo.
"%~dp0feel.exe" layout "%~dp0demo" --dry-run
echo.
echo  Premere un tasto per avviare l'emulatore simulato.
pause >nul
echo.
"%~dp0feel.exe" launch "%~dp0demo.sqlite" mame-0 galaga
echo.
echo  Demo completata.
pause
