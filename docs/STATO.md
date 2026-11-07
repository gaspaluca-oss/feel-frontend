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
| `feel-render` | finestra, contesto OpenGL 3.3, disegno di rettangoli |
| `feel-app` | binario grafico: mostra la lista dei giochi dal database |

Verificato end-to-end su una installazione FEEL 1.x di esempio: importa 3 giochi,
converte il layout, avvia un emulatore e registra il tempo di gioco.

Il pacchetto scaricabile è già confezionato e include una demo funzionante con un
emulatore simulato, così si vede il ciclo completo senza avere MAME installato.

---

## 3. Cosa manca

La **finestra** c'è: `feel-app` apre OpenGL 3.3, legge la libreria dal database e
disegna la lista secondo il layout convertito. Frecce per scorrere, Esc per uscire.

Manca, in ordine di importanza:

- **il testo** — adesso le righe sono barre colorate, non titoli. Serve un
  rasterizzatore di font (`ab_glyph`) e un atlante di glifi;
- **le immagini** — sfondo del layout, snapshot, marquee;
- **gli shader CRT** — è la ragione per cui il renderer è OpenGL;
- `feel-input` — gilrs per i gamepad, la mappatura controlli;
- `feel-media` — FFmpeg per i video snap;
- `feel-platform` — cambio risoluzione, LED.

Prova rapida:

```bash
feel import packaging/demo --db demo.sqlite
feel layout packaging/demo --out layout
feel-app demo.sqlite mame-0 layout/MAME.json
```

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

**Le dipendenze recenti non si lasciano compilare da Cargo 1.77.** `jobserver`
0.1.35 tira dentro `getrandom` 0.4.3, che usa `edition2024`: Cargo 1.77 non sa
nemmeno leggere il suo manifest, e l'errore parla di "failed to download
replaced source registry", che non c'entra niente. Va fissato:

```bash
cargo update -p jobserver --precise 0.1.32
cargo update -p unicode-segmentation --precise 1.10.1
```

Le versioni sono già nel `Cargo.lock` committato: servono solo se si rigenera.

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
