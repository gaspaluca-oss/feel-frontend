@echo off
cd /d "%~dp0"
echo.
echo  FEEL 2.0 - anteprima 0.1.0
echo.
echo  Apro il frontend grafico sulla libreria di esempio.
echo.
echo  Frecce su/giu' per scorrere, Esc per uscire.
echo.

REM Importazione e conversione sono idempotenti e veloci: si rifanno sempre,
REM cosi' un pacchetto aggiornato non lascia in giro un layout vecchio senza
REM le immagini.
echo  Preparo la libreria...
"%~dp0feel.exe" import "%~dp0demo" --db "%~dp0demo.sqlite" >nul

echo  Converto il layout...
"%~dp0feel.exe" layout "%~dp0demo" --out "%~dp0layout" >nul

echo  Avvio...
echo.
"%~dp0feel-app.exe" "%~dp0demo.sqlite" mame-0 "%~dp0layout\MAME.json"

if errorlevel 1 (
    echo.
    echo  Il frontend non e' riuscito a partire.
    echo  Se manca OpenGL 3.3, il problema e' la scheda video o il driver.
    pause
)
