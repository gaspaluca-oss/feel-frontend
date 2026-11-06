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

Su Windows 7 serve SP1 con l'Universal CRT (KB2999226), presente su qualsiasi sistema aggiornato.

---

## Documenti

- [`docs/ARCHITETTURA.md`](docs/ARCHITETTURA.md) — vincoli, decisioni tecniche, struttura, roadmap
- [`mockup/frontend.html`](mockup/frontend.html) — prototipo visivo, apribile con un browser

---

## Compilare

Il progetto è agli inizi: la toolchain Rust è fissata in `rust-toolchain.toml`.

```bash
cargo build --release
```

---

## Licenza

Ereditata da FEEL: GNU LGPL v3 o successiva. Vedi [`__license.txt`](__license.txt).
