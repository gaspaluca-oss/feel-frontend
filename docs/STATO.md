# Stato del lavoro e come riprendere

Ultimo aggiornamento: 6 novembre 2026.
Ramo di lavoro: **`feel2`** su `gaspaluca-oss/feel-frontend`.

Questo file esiste per una ragione sola: far ripartire il lavoro fra una settimana
senza dover ricostruire il contesto a memoria. Va aggiornato quando cambia qualcosa
di sostanziale, non a ogni commit.

---

## 1. Come si riprende

### Ambiente

Serve la toolchain a 32 bit e MinGW. Su questa macchina ci sono già:

```bash
export PATH="$HOME/.cargo/bin:/c/msys64/mingw32/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.77.2-i686-pc-windows-gnu
cd <cartella>/gaspaluca-feel
```

**`RUSTUP_TOOLCHAIN` non è opzionale.** `rust-toolchain.toml` fissa solo la versione,
non l'host, perché un host Windows non si installerebbe su Linux. Senza quella
variabile cargo usa l'host MSVC, che cerca `link.exe` e fallisce con
`link: extra operand` — un errore che sembra un problema di codice e non lo è.

### Verifiche

```bash
cargo test                       # 84 test, devono passare tutti
cargo build --release -p feel-cli
python tools/pecheck.py target/release/feel.exe
```

`pecheck.py` deve dire **COMPATIBILE con Windows 7**. Se dice altro, qualcosa ha
introdotto una dipendenza da Windows 8 o successivo.

### Costruire il pacchetto scaricabile

```bash
FEEL_TOOLCHAIN=1.77.2-i686-pc-windows-gnu ./scripts/package-windows.sh
```

Produce `dist/feel2-<versione>-win32/` e il relativo `.zip`.

---

## 2. Cosa è fatto e funziona

Il **motore**, cioè tutto quello che non disegna. 84 test, zero avvisi.

| Crate | Cosa fa |
|---|---|
| `feel-core` | modello dati (piattaforme, emulatori, liste, giochi, statistiche) e formato dei layout |
| `feel-config` | lettura di una installazione FEEL 1.x e conversione dei layout |
| `feel-db` | libreria su SQLite, con statistiche e migrazioni |
| `feel-launch` | costruzione del piano di avvio ed esecuzione dell'emulatore |
| `feel-cli` | il binario `feel`: import, info, list, launch, layout |

Verificato end-to-end su una installazione FEEL 1.x di esempio: importa 3 giochi,
converte il layout, avvia un emulatore e registra il tempo di gioco.

Il pacchetto scaricabile è già confezionato e include una demo funzionante con un
emulatore simulato, così si vede il ciclo completo senza avere MAME installato.

---

## 3. Cosa manca

**Tutto il rendering.** È l'unico pezzo grosso rimasto, ed è quello che trasforma
questo motore in un frontend che si vede.

Crate da scrivere, secondo `docs/ARCHITETTURA.md` §3:

- `feel-render` — OpenGL 3.3, finestra, shader, testo, schermate
- `feel-input` — winit + gilrs: tastiera, gamepad, mappatura controlli
- `feel-media` — FFmpeg: video snap e immagini
- `feel-platform` — le differenze fra sistemi: cambio risoluzione, LED
- `feel-app` — il binario grafico che mette insieme tutto

Ordine consigliato: **`feel-render` + `feel-app` per primi**, con una finestra che
mostra la lista dei giochi presa dal database. È la fetta verticale: se quella
regge, il resto è lavoro; se non regge, si scopre subito.

---

## 4. Trappole già scoperte (non riscoprirle)

**Rust recente non parte su Windows 7.** Da 1.78 la std importa
`bcryptprimitives.dll` e `api-ms-win-core-synch-l1-2-0.dll`, che su Windows 7 non
esistono. La 1.77.2 è l'ultima utilizzabile. `tools/pecheck.py` lo verifica.

**L'header PE mente.** Sia la 1.99 sia la 1.77 dichiarano "versione minima OS 4.0".
L'unico segnale affidabile è la tabella degli import.

**I build script si compilano per l'host, non per il target.** Per questo l'host è
`i686-pc-windows-gnu` e non MSVC: altrimenti servirebbe Visual Studio, e in questa
shell `/usr/bin/link` di MSYS shadowa comunque `link.exe`.

**I percorsi in configurazione usano sempre `/`**, anche su Windows. Vanno
uniformati una volta sola, quando escono dal frontend, altrimenti si ottengono
percorsi come `.../roms\pacman.zip`: funzionanti ma illeggibili nei log.

**Le statistiche sono per `(piattaforma, emulatore)`**, non per solo emulatore.
FEEL 1.x le scrive in un unico `data/<emu>.sts`, quindi due piattaforme con un
emulatore omonimo si scambiavano le partite.

**Il formato `.lst` di FEEL 1.x ha 13 campi**, non 13 righe: nome, titolo, anno,
produttore, clone di, bios, dati extra, orientamento, comandi, stato, colore,
suono, categoria. Un errore qui fa importare zero giochi senza dire perché.

---

## 5. Decisioni aperte

**Dove gira.** Deciso: Windows 7 SP1 → 11 e Linux (Debian/Ubuntu). Non ancora
provato su una macchina Windows 7 vera: è la prima cosa da fare quando si ha
sottomano il cabinato.

**Il rendering su quale macchina si prova.** Il cabinato vero o una VM. Una VM
spesso non ha OpenGL accelerato, e si rischia di inseguire problemi che sul ferro
non esistono.

**Il video snap.** FFmpeg è una dipendenza C da compilare staticamente. Va deciso
se tenerla o se partire senza video e aggiungerli dopo.

**La licenza del codice nuovo.** Il progetto è LGPL v3 ereditata da FEEL 1.x. Se
si vuole un'altra licenza, va deciso adesso, non a codice scritto.

---

## 6. Dove sta FEEL 1.x

Nel tag `feel-1.x-final`, 196 file. Non è nel ramo di lavoro:

```bash
git worktree add ../feel-1.x feel-1.x-final
```

Serve come riferimento quando bisogna capire come il vecchio frontend faceva una
cosa — ed è la sorgente per chi vuole continuare a usarlo così com'è.
