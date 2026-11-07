#!/bin/bash
# ----------------------------------------------------------------------------------
# Costruisce il pacchetto scaricabile per Windows.
#
# Non e' un aiuto per lo sviluppatore: e' la ricetta della release. Chiunque abbia
# il repository e la toolchain puo' rilanciare questo script e ottenere esattamente
# lo stesso pacchetto, senza passaggi fatti a mano che nessuno ricorda piu'.
#
# Uso:
#   ./scripts/package-windows.sh [versione]
#
# Senza argomenti prende la versione da Cargo.toml.
# ----------------------------------------------------------------------------------
set -euo pipefail

cd "$(dirname "$0")/.."
PROGETTO="$(pwd)"

VERSIONE="${1:-$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)}"
NOME="feel2-${VERSIONE}-win32"
DIST="$PROGETTO/dist"
PACCHETTO="$DIST/$NOME"

# Su Windows la toolchain e' a 32 bit: vedi docs/ARCHITETTURA.md, decisione D8.
TOOLCHAIN="${FEEL_TOOLCHAIN:-}"
CARGO="cargo"
if [ -n "$TOOLCHAIN" ]; then
    CARGO="cargo +$TOOLCHAIN"
fi

echo "==> Compilo la release ($CARGO, versione $VERSIONE)"
$CARGO build --release -p feel-cli -p feel-app

BINARIO="$PROGETTO/target/release/feel.exe"
GRAFICO="$PROGETTO/target/release/feel-app.exe"
[ -f "$BINARIO" ] || { echo "ERRORE: $BINARIO non esiste" >&2; exit 1; }
[ -f "$GRAFICO" ] || { echo "ERRORE: $GRAFICO non esiste" >&2; exit 1; }

echo "==> Verifico che partano su Windows 7"
python "$PROGETTO/tools/pecheck.py" "$BINARIO" "$GRAFICO"

echo "==> Compongo il pacchetto"
rm -rf "$PACCHETTO"
mkdir -p "$PACCHETTO"
cp "$BINARIO" "$PACCHETTO/feel.exe"
cp "$GRAFICO" "$PACCHETTO/feel-app.exe"
cp "$PROGETTO/packaging/LEGGIMI.txt" "$PACCHETTO/LEGGIMI.txt"
cp "$PROGETTO/packaging/AVVIA-DEMO.bat" "$PACCHETTO/AVVIA-DEMO.bat"
cp "$PROGETTO/packaging/AVVIA-GUI.bat" "$PACCHETTO/AVVIA-GUI.bat"
cp "$PROGETTO/__license.txt" "$PACCHETTO/LICENZA.txt"
cp -r "$PROGETTO/packaging/demo" "$PACCHETTO/demo"

# Un database di prova non deve finire in una release.
rm -f "$PACCHETTO/demo.sqlite"

echo "==> Comprimo"
rm -f "$DIST/$NOME.zip"

# PowerShell vuole percorsi Windows: se passiamo "/c/Users/..." risponde
# "il percorso non esiste" invece di comprimere.
if command -v cygpath >/dev/null 2>&1; then
    ORIGINE_PS="$(cygpath -w "$PACCHETTO")"
    DESTINAZIONE_PS="$(cygpath -w "$DIST/$NOME.zip")"
else
    ORIGINE_PS="$PACCHETTO"
    DESTINAZIONE_PS="$DIST/$NOME.zip"
fi

powershell.exe -NoProfile -Command \
    "Compress-Archive -Path '$ORIGINE_PS' -DestinationPath '$DESTINAZIONE_PS' -Force"

echo
echo "Fatto:"
echo "  cartella : $PACCHETTO"
echo "  archivio : $DIST/$NOME.zip  ($(du -h "$DIST/$NOME.zip" | cut -f1))"
