@echo off
cd /d "%~dp0"
echo.
echo  FEEL 2.0 - anteprima 0.1.0
echo  ==========================
echo.

REM Controllo che ci sia tutto. Se il pacchetto e' stato aperto dentro al file
REM zip invece di essere scompattato, qui non c'e' niente e vale la pena dirlo
REM subito invece di far fallire i passi uno per uno.
if not exist "%~dp0feel.exe" goto mancante
if not exist "%~dp0feel-app.exe" goto mancante
if not exist "%~dp0demo" goto mancante

echo  Preparo la libreria...
echo.
"%~dp0feel.exe" import "%~dp0demo" --db "%~dp0demo.sqlite"
if errorlevel 1 goto fallito

echo.
echo  Converto il layout...
echo.
"%~dp0feel.exe" layout "%~dp0demo" --out "%~dp0layout"
if errorlevel 1 goto fallito

echo.
echo  Frecce o joystick per scorrere, Invio per giocare,
echo  lettera per saltare, Tab per i filtri, F1 per le impostazioni.
echo.
echo  Avvio...
echo.
"%~dp0feel-app.exe" "%~dp0demo.sqlite" mame-0 "%~dp0layout\MAME.json"
if errorlevel 1 goto frontend_fallito
goto fine

:mancante
echo  Mancano dei file accanto a questo .bat: feel.exe, feel-app.exe o demo.
echo.
echo  Se hai aperto il pacchetto direttamente dentro al file .zip, Windows
echo  non lo esegue: scompattalo prima (tasto destro sul .zip, "Estrai tutto")
echo  e poi lancia questo file dalla cartella estratta.
goto fine

:fallito
echo.
echo  Un passo e' fallito: leggi il messaggio qui sopra.
echo.
echo  Se dice "Accesso negato", quasi sempre e' l'antivirus che blocca
echo  feel.exe mentre lo analizza. Apri Windows Security, "Protezione da virus
echo  e minacce", "Cronologia protezione", e consenti il file. Poi rilancia.
echo.
echo  Se dice "Impossibile trovare il percorso", il pacchetto e' stato
echo  scompattato in un posto dove non si puo' scrivere: spostalo in una
echo  cartella tua, per esempio sul Desktop.
goto fine

:frontend_fallito
echo.
echo  Il frontend non e' riuscito a partire.
echo.
echo  Se il messaggio parla di OpenGL, il problema e' la scheda video o il
echo  driver: serve OpenGL 3.3, cioe' una scheda dal 2010 in poi con i driver
echo  aggiornati.
echo  Se parla di un percorso mancante, la conversione del layout non e'
echo  andata a buon fine: rileggi il messaggio del passo precedente.

:fine
echo.
pause
