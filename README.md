# FEEL 2.0

Frontend per PC e cabinati da retrogaming. Riscrittura di FEEL con grafica e funzionalità
migliorate, per Windows 7 SP1 → Windows 11.

> **Stato: in costruzione.** Il codice di FEEL 1.x (C# / XNA) è conservato nel tag
> [`feel-1.x-final`](../../tree/feel-1.x-final). Questo ramo è la riscrittura.

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
| Sistema | Windows 7 SP1 → Windows 11 |
| Architettura | x86 (32 bit) primaria, x64 secondaria |
| GPU | Direct3D 9 con pixel shader 2.0 |

Il target GNU usa `msvcrt.dll`, quindi su Windows 7 non serve installare la
Universal CRT (KB2999226).

---

## Documenti

- [`docs/ARCHITETTURA.md`](docs/ARCHITETTURA.md) — vincoli, decisioni tecniche, struttura, roadmap
- [`mockup/frontend.html`](mockup/frontend.html) — prototipo visivo, apribile con un browser
- [`tools/pecheck.py`](tools/pecheck.py) — verifica la compatibilità con Windows 7 di un eseguibile

---

## Compilare

Serve un linker **MinGW-w64 a 32 bit**: Rust non ne include uno. Su questa macchina
è arrivato con MSYS2.

```powershell
winget install --id MSYS2.MSYS2
C:\msys64\usr\bin\bash -lc "pacman -S --noconfirm mingw-w64-i686-gcc"
```

Poi `C:\msys64\mingw32\bin` deve stare nel `PATH`. La toolchain Rust è già fissata da
`rust-toolchain.toml`, non c'è niente da scegliere:

```bash
cargo build --release --target i686-pc-windows-gnu

# controlla che il binario sia davvero caricabile su Windows 7
python tools/pecheck.py target/i686-pc-windows-gnu/release/feel.exe
```

`pecheck.py` non è un extra: è la rete di sicurezza che impedisce a un aggiornamento
di toolchain di rompere silenziosamente il supporto a Windows 7. Vedi
[`docs/ARCHITETTURA.md`](docs/ARCHITETTURA.md), decisione D3.

---

## Licenza

Ereditata da FEEL: GNU LGPL v3 o successiva. Vedi [`__license.txt`](__license.txt).
