# FEEL 2.0 — Architettura e decisioni

Riscrittura pulita di FEEL. Stessa esperienza d'uso, grafica e funzionalità migliorate.
Questo documento fissa i vincoli e le scelte tecniche **prima** di scrivere codice: ogni
decisione qui dentro è stata presa perché una delle alternative non reggeva un vincolo.

---

## 1. Vincoli

| Vincolo | Valore |
|---|---|
| Sistemi operativi | **Windows 7 SP1 → Windows 11** e **Linux** |
| Architettura | **x86 (32-bit) primaria**, x64 secondaria |
| Schermi | CRT 15/31 kHz e LCD, orizzontali e verticali |
| Hardware | PC da cabinato, anche molto datati |
| Input | encoder tastiera (HID), gamepad, mouse (spinner/trackball) |
| Rete | opzionale (scraper), mai richiesta per avviare un gioco |
| Formati video | video snap in mp4/h264 e formati legacy |

**Windows XP è escluso.** Non è una preferenza: XP imporrebbe C++ e Direct3D 9 puro,
eliminerebbe Rust e ogni libreria moderna, e — soprattutto — non farebbe girare gli
emulatori attuali (MAME richiede Windows 10). Il cabinato XP resta su FEEL 1.x, che già
funziona. Il tag `feel-1.x-final` è lì per questo.

**Windows e Linux contano allo stesso modo.** Non è un porting: è un solo programma che
gira su due sistemi. La conseguenza pratica è che nessuna scelta tecnica può appoggiarsi
solo a Windows — vedi D2, D5 e D6, che sono state riscritte proprio per questo.

---

## 2. Decisioni

### D1 — Linguaggio: Rust

Rust è sicuro, ha `cargo` (build, test, dipendenze) e non ha runtime da installare.
Su Windows 7 è utilizzabile perché la soglia minima dei target MSVC è Windows 7 SP1
fino alla 1.77.

*Alternative scartate:* C++ (nessun guadagno di sicurezza, toolchain molto più fragile),
C#/.NET Framework 4.0 (è la tecnologia di FEEL 1.x: riscrivere con lo stesso stack non
migliora nulla).

### D2 — Renderer: OpenGL 3.3 core, condiviso fra Windows e Linux

Era Direct3D 9Ex finché il target era solo Windows. Con Linux nel target D3D9 non
serve più, e l'unica API grafica che copre **tutta** la fascia Windows 7 → 11 *e*
Linux è OpenGL:

| | Windows 7 → 11 | Linux |
|---|---|---|
| OpenGL 3.3 | driver GPU dal 2010 in poi | Mesa, sempre |
| Direct3D 9 | sì | no |
| Direct3D 12 | solo da Windows 10 | no |
| Vulkan | no | sì |

**Un renderer solo, non due.** Un backend per sistema operativo raddoppierebbe il
codice che disegna e la superficie da testare, e il rendering è la parte che costa più
di tutte scrivere e verificare. GLSL 3.30 copre tutto quello che serve: scanline,
maschera RGB, curvatura, bloom, aberrazione cromatica, scaling bicubico.

Su Linux c'è un vantaggio che su Windows non esiste: Mesa accetta modeline arbitrarie,
quindi il cambio di risoluzione per gioco su un CRT a 15 kHz funziona senza driver
modificati. È lo stesso motivo per cui GroovyMAME gira su Linux e non su Windows.

*Alternative scartate:* wgpu (il backend DX12 richiede Windows 10, Vulkan non esiste su
Windows 7, il backend DX11 è stato rimosso); due backend separati; Godot (4 richiede
Windows 10, 3.x è fermo).

### D3 — Toolchain: Rust 1.77.2, target `i686-pc-windows-gnu`

Il target a 32 bit è la scelta giusta per questa fascia: gira ovunque, consuma meno
memoria e copre i cabinati con Windows a 32 bit. Il target **GNU** usa `msvcrt.dll`
invece della Universal CRT, quindi su Windows 7 non serve installare la KB2999226.

**La versione di Rust qui è un vincolo, non una preferenza — ed è misurata.**
Da 1.78 in poi la std importa `bcryptprimitives.dll` e
`api-ms-win-core-synch-l1-2-0.dll`, che su Windows 7 non esistono: il sistema si
ferma in avvio con un errore che non spiega nulla.

| DLL importata | Rust 1.99.1 | Rust 1.77.2 |
|---|---|---|
| `bcryptprimitives.dll` (Windows 8+) | **sì** | no |
| `api-ms-win-core-synch-l1-2-0.dll` (Windows 8+) | **sì** | no |
| `bcrypt.dll` (Windows 7) | no | sì |
| `msvcrt.dll` | sì | sì |

`rust-toolchain.toml` fissa la 1.77.2 e `tools/pecheck.py` verifica a ogni build che il
binario non importi DLL posteriori a Windows 7. Il controllo serve perché l'header PE
**non è affidabile**: MinGW lascia la versione minima dichiarata a 4.0 in entrambi i
casi, quindi l'unico segnale vero è la tabella degli import.

Il prezzo è una toolchain di aprile 2024. È il costo dichiarato di Windows 7: se il
floor sale a Windows 10, il pin si rimuove e si torna su stable.

**Anche l'host è `i686-pc-windows-gnu`.** I build script e le proc-macro si compilano
per l'host, non per il target: con l'host MSVC servirebbe `link.exe` di Visual Studio,
che non è installato, e che in questa shell verrebbe per giunta shadowato da
`/usr/bin/link` di MSYS. Tenendo host e target entrambi a 32 bit serve solo MinGW i686,
e l'ambiente di build coincide con l'ambiente di esecuzione.

Verificato end-to-end: build script eseguiti, proc-macro caricate (`serde_derive`),
link con MinGW, test eseguito e superato.

Vale per Windows. Su Linux l'host è quello nativo e non serve nulla di tutto questo:
vedi D8.

### D4 — Dati: SQLite embedded

Al posto dei file di testo `data/*.lst` e `data/*.sts` di FEEL 1.x. Un solo database
per: libreria giochi, statistiche, preferiti, playlist, filtri, profili per gioco.
La feature `bundled` di `rusqlite` compila SQLite dai sorgenti, quindi nessuna DLL da
installare sul cabinato.

Il formato di FEEL 1.x è documentato e viene **importato**, non abbandonato: vedi §6.

### D5 — Video: FFmpeg

Media Foundation è solo Windows, quindi con Linux nel target non regge più. FFmpeg
decodifica mp4/h264 e i formati legacy su entrambi i sistemi con una sola strada di
codice.

Costo dichiarato: è una dipendenza C. Va compilata staticamente per non dover
distribuire DLL accanto al frontend, e va aggiornata a mano perché non la gestisce cargo.

### D6 — Finestra, input e audio: winit + gilrs + cpal

- **winit** per la finestra e la tastiera: un'API sola per Windows e Linux.
- **gilrs** per i gamepad: astrae XInput su Windows ed evdev su Linux.
- **cpal** per l'audio: WASAPI su Windows, ALSA o PulseAudio su Linux.
- Encoder arcade, spinner e trackball sono dispositivi HID tastiera o mouse: li legge
  la finestra. La mappatura dei controlli si configura **dentro al frontend**, con
  verifica dal vivo.

Sono librerie pure Rust, quindi non c'è nessuna DLL da spedire insieme al frontend:
è il motivo per cui sono state preferite a SDL2.

### D7 — Cosa resta specifico di un sistema operativo

Poco, e va isolato dietro un'interfaccia unica. Questi sono i punti in cui Windows e
Linux divergono davvero:

| | Windows | Linux |
|---|---|---|
| Cambio risoluzione | API di sistema | `xrandr` con modeline |
| LED dei pulsanti | LED-Wiz, PacDrive via DLL | LEDSpicer, seriale |
| Avvio emulatore | `CreateProcess` | `fork`/`exec` |

`feel-core`, `feel-config` e `feel-db` **non compaiono** in questa tabella: non toccano
il sistema operativo. È la ragione per cui questo cambio di rotta costa poco — l'unica
parte da rifare è quella che disegna, e non era ancora stata scritta.

### D8 — La toolchain cambia per sistema operativo

`rust-toolchain.toml` fissa **solo la versione**, non l'host: un file con un host
Windows non si puo' nemmeno installare su Linux. Le differenze stanno qui:

| | Windows (per Windows 7) | Linux |
|---|---|---|
| Host | `i686-pc-windows-gnu` | nativo (`x86_64-unknown-linux-gnu`) |
| Target | `i686-pc-windows-gnu` | nativo, o `i686` per cabinati a 32 bit |
| Comando | `cargo +1.77.2-i686-pc-windows-gnu build` | `cargo build` |
| Linker | MinGW i686 (MSYS2) | quello di sistema |

La versione 1.77.2 resta per tutti e due: **su Linux non sarebbe necessaria**, ma
un solo MSRV in tutto il repository evita che Windows e Linux divergono senza che
nessuno se ne accorga.

---

## 3. Struttura del workspace

```
crates/
  feel-core/      modello dati, libreria, filtri, statistiche          portabile
  feel-config/    configurazione, schema, importatore FEEL 1.x          portabile
  feel-db/        SQLite: schema, migrazioni, query                     portabile
  feel-media/     FFmpeg: video snap, immagini, audio
  feel-render/    OpenGL 3.3: finestra, shader, schermate, testo
  feel-input/     winit + gilrs: tastiera, gamepad, mappatura tasti
  feel-launch/    avvio emulatori, attesa, ritorno al frontend
  feel-platform/  le differenze fra sistemi: risoluzione, LED
  feel-scrape/    scraper (ScreenScraper), coda e ripresa
  feel-cli/       binario: importazione e diagnostica da riga di comando
  feel-ui/        schermate del frontend e dell'editor di layout
  feel-app/       binario: ciclo principale, orchestrazione
mockup/           prototipo visivo (HTML, apribile nel browser)
```

`feel-core`, `feel-config` e `feel-db` non toccano Windows: si testano con `cargo test`
normale e restano portabili, anche se il target è solo Windows.

---

## 4. Modello dati

```
platform   (id, nome, titolo, ordine)
emulator   (id, platform_id, nome, titolo, eseguibile, argomenti, rom_path, rom_ext,
            snapshot_path, video_path, marquee_path, cabinet_path, layout_id, ...)
gamelist   (id, emulator_id, nome, titolo, ordine)
game       (id, gamelist_id, rom_name, titolo, anno, produttore, cloneof, bios,
            categoria, rotazione, controlli, stato, extra)
play       (game_id, iniziato_il, durata_sec)
favourite  (game_id, aggiunto_il)
profile    (game_id, eseguibile, argomenti, note)   -- i "profili per gioco"
layout     (id, nome, origine, definizione_json)
```

Le liste di FEEL 1.x (`.lst` a 13 righe, `.sts` pipe-separated) si mappano 1:1 su questa
struttura: nessuna informazione va persa, comprese le statistiche di gioco.

---

## 5. Formato dei layout

FEEL 1.x descriveva un layout con un `.ini` di coordinate assolute in pixel su una
risoluzione fissa (640×480). È il difetto peggiore del vecchio frontend: cambiare
risoluzione o passare da CRT a LCD significa rifare il layout.

FEEL 2.0 usa un formato **dichiarativo**:

- posizioni espresse con **ancoraggi** (`left`/`center`/`right`, `top`/`middle`/`bottom`)
  più un offset, non coordinate assolute
- dimensioni in unità relative allo schermo, con minimi in pixel dove serve
- **un solo file** che descrive sia l'orientamento orizzontale sia quello verticale
- gli stili ereditano da un tema base, così si sovrascrive solo ciò che cambia

Il prototipo in `mockup/frontend.html` è il riferimento visivo di questo formato.

---

## 6. Migrazione da FEEL 1.x

L'importatore legge un'installazione FEEL 1.x e produce il database di FEEL 2.0:

| FEEL 1.x | FEEL 2.0 |
|---|---|
| `config/feel.ini` | impostazioni globali + mappatura tasti |
| `config/<platform>/<platform>.ini` | `platform` + assegnazione emulatori |
| `config/<platform>/<emu>/<emu>.ini` | `emulator` (percorsi, comando, argomenti) |
| `config/<platform>/<emu>/<emu>-N.ini` | `gamelist` |
| `data/<gamelist>.lst` | `game` (13 campi per rom) |
| `data/<emu>.sts` | `play` (conteggio, tempo, ultima partita) |
| `layouts/<nome>/layout.ini` | `layout` (convertito in ancoraggi) |
| `[rom_name]`, `[full_path]`, … | argomenti con i token del nuovo motore |

I percorsi Windows vengono riscritti secondo regole esplicite, e tutto ciò che non è
mappabile finisce in un report invece di essere indovinato.

---

## 7. Roadmap

**Fetta verticale** — la prima cosa da fare, perché è dove si scoprono i problemi veri:

1. finestra fullscreen su D3D9Ex, vsync, chiusura pulita
2. lista giochi letta da SQLite, navigazione da tastiera ed encoder
3. video snap che parte e si ferma con la selezione
4. lancio dell'emulatore, attesa, ritorno al frontend
5. importatore della configurazione FEEL 1.x

Tutto il resto (scraper, editor di layout, LED, temi) viene dopo, e solo se la fetta
verticale regge su una macchina Windows 7 reale.
