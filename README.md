# FEEL 2.0

Frontend per PC e cabinati da retrogaming. Riscrittura di FEEL con grafica e funzionalità
migliorate, per Windows 7 SP1 → Windows 11.

> **Stato: in costruzione.** Il codice di FEEL 1.x (C# / XNA) è conservato nel tag
> [`feel-1.x-final`](../../tree/feel-1.x-final). Questo ramo è la riscrittura.
>
> Fatto finora, con 77 test: il modello dati (`feel-core`), **l'importatore di una
> installazione FEEL 1.x** (`feel-config`), la libreria su SQLite (`feel-db`),
> **l'avvio dell'emulatore** (`feel-launch`) e la riga di comando (`feel-cli`).
> Il ciclo completo — importa la tua configurazione, avvia un gioco, registra il
> tempo giocato — funziona già; manca tutto quello che si vede a schermo.

---

## Cosa cambia rispetto a FEEL 1.x

**Grafica**

- pipeline a shader: CRT (scanline, maschera RGB, curvatura), LCD, bloom, aberrazione cromatica
- layout con ancoraggi e unità relative invece di coordinate assolute in pixel:
  la stessa schermata va bene da 640×480 a 4K
- animazioni con timeline ed easing invece di sequenze di PNG
- marquee e DMD su monitor secondario
- caricamento asincrono con cache: niente stutter passando da un gioco all'altro

**Funzionalità**

- libreria in SQLite: filtri, playlist, preferiti, statistiche, ricerca full-text
- scraper integrato con coda riprendibile
- mappatura dei controlli dentro al frontend, con verifica dal vivo
- editor di layout visuale
- profili per singolo gioco di primo livello
- LED dei pulsanti con API a moduli (LED-Wiz, PacDrive, Ultimarc)
- importazione diretta della configurazione FEEL 1.x esistente

---

## Requisiti

| | |
|---|---|
| Sistema | Windows 7 SP1 → Windows 11, oppure Linux |
| Architettura | x86 (32 bit) primaria su Windows, nativa su Linux |
| GPU | OpenGL 3.3 |

Su Windows il target GNU usa `msvcrt.dll`, quindi su Windows 7 non serve installare
la Universal CRT (KB2999226).

---

## Documenti

- [`docs/ARCHITETTURA.md`](docs/ARCHITETTURA.md) — vincoli, decisioni tecniche, struttura, roadmap
- [`mockup/frontend.html`](mockup/frontend.html) — prototipo visivo, apribile con un browser
- [`tools/pecheck.py`](tools/pecheck.py) — verifica la compatibilità con Windows 7 di un eseguibile

---

## Uso

La prima cosa che sa fare è importare una installazione FEEL 1.x esistente — rom,
liste, statistiche e mappatura dei tasti — in un database.

```bash
# guarda cosa verrebbe importato, senza scrivere niente
feel import "C:\Program Files\Feel" --dry-run

# importa davvero
feel import "C:\Program Files\Feel" --db feel.sqlite

# cosa c'è dentro
feel info feel.sqlite
feel list feel.sqlite mame-0

# vedi esattamente cosa verrebbe eseguito, senza avviare niente
feel launch feel.sqlite mame-0 pacman --dry-run

# avvia, e registra il tempo giocato
feel launch feel.sqlite mame-0 pacman
```

`--dry-run` è la cosa da usare quando un gioco non parte: mostra l'eseguibile, il
percorso della rom, la riga di comando e le eventuali applicazioni di pre e post
avvio, esattamente come sarebbero eseguite.

Se hai spostato le rom: `--remap "C:\Program Files\mame=D:\MAME"`, ripetibile.

`import` stampa **quello che non è riuscito a interpretare** invece di inventarlo.
Un avviso come "`emulator_commandline` vuoto, non so cosa avviare" è l'unico modo per
accorgersi di un problema prima di trovarlo sul cabinato.

---

## Compilare

### Linux

Il codice non ha parti specifiche di Windows: finestra, OpenGL e input passano da
`winit`, `glutin` e `glow`, che sono multipiattaforma.

**Non è mai stato compilato né provato su Linux.** È la prima cosa da verificare, e
finché non è fatto va considerato non funzionante là.

Una cosa aiuta: su Linux si usa **Rust stable**, non la 1.77.2. Quel pin esiste solo
per Windows 7, e le dipendenze che qui vanno fissate a mano (`jobserver`,
`unicode-segmentation`, `quick-xml`) lì si risolvono da sole. Il build è più semplice,
non più difficile.

Pacchetti necessari (Debian/Ubuntu):

```bash
sudo apt install build-essential pkg-config \
    libx11-dev libxi-dev libxkbcommon-dev libwayland-dev \
    libgl1-mesa-dev libegl1-mesa-dev fonts-dejavu-core
```

Poi:

```bash
cargo test
cargo build --release -p feel-cli -p feel-app
```

### Windows

Serve un linker **MinGW-w64 a 32 bit**, perché Rust non ne include uno.

```powershell
winget install --id MSYS2.MSYS2
C:\msys64\usr\bin\bash -lc "pacman -S --noconfirm mingw-w64-i686-gcc"

# rustup rifiuta gli host non nativi senza --force-non-host: e' voluto, l'host
# a 32 bit serve perche' i build script si compilano per l'host.
rustup toolchain install 1.77.2-i686-pc-windows-gnu --profile minimal --force-non-host
```

Poi `C:\msys64\mingw32\bin` deve stare nel `PATH`, e si compila indicando la toolchain:

```bash
cargo +1.77.2-i686-pc-windows-gnu test
cargo +1.77.2-i686-pc-windows-gnu build --release

# controlla che il binario sia davvero caricabile su Windows 7
python tools/pecheck.py target/release/feel.exe
```

`pecheck.py` non è un extra: è la rete di sicurezza che impedisce a un aggiornamento
di toolchain di rompere silenziosamente il supporto a Windows 7. Vedi
[`docs/ARCHITETTURA.md`](docs/ARCHITETTURA.md), decisione D3 e D8.

---

## Licenza

Ereditata da FEEL: GNU LGPL v3 o successiva. Vedi [`__license.txt`](__license.txt).
FEEL 1.x è di Maurizio Montel e dei contributori del progetto originale.

---

## Dove sta FEEL 1.x

Il codice C# / XNA non è in questo ramo. È conservato nel tag
[`feel-1.x-final`](../../tree/feel-1.x-final), che contiene i 196 file originali:
è il riferimento per capire come il vecchio frontend faceva una cosa, e la sorgente
per chi vuole continuare a usarlo così com'è.

```bash
git worktree add ../feel-1.x feel-1.x-final
```

Tenerlo fuori dal ramo di lavoro non è pignoleria: con dentro due progetti, ogni
`cargo build` e ogni ricerca nel codice si portavano dietro file che non c'entravano.
