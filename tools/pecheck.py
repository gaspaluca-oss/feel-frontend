#!/usr/bin/env python3
"""Verifica che un eseguibile Windows sia caricabile su Windows 7.

Perche' serve
-------------
Il campo "versione minima del sistema operativo" nell'header PE **non e' affidabile**:
MinGW lo lascia a 4.0 anche quando il binario importa API che su Windows 7 non
esistono. E' stato verificato: sia la Rust 1.99.1 sia la 1.77.2 dichiarano 4.0,
ma solo la prima importa bcryptprimitives.dll.

L'unico segnale vero e' la tabella degli import. Se il binario importa una DLL
introdotta dopo Windows 7, il sistema operativo si ferma con un errore che non
spiega nulla ("impossibile avviare il programma", senza dettagli).

Uso
---
    python tools/pecheck.py target/i686-pc-windows-gnu/release/feel.exe

Esce con 0 se il binario e' compatibile con Windows 7, 1 altrimenti.
Nessuna dipendenza esterna: l'header PE viene letto direttamente.
"""

import struct
import sys

# DLL che su Windows 7 non esistono. Se compaiono, il binario non parte.
MISSING_ON_WIN7 = {
    "bcryptprimitives.dll": "Windows 8",
    "api-ms-win-core-synch-l1-2-0.dll": "Windows 8",
    "api-ms-win-core-synch-l1-1-0.dll": "Windows 8",
    "api-ms-win-core-file-l1-2-0.dll": "Windows 8",
    "api-ms-win-core-processthreads-l1-1-1.dll": "Windows 8",
    "api-ms-win-core-rtlsupport-l1-1-0.dll": "Windows 8",
    "api-ms-win-core-libraryloader-l1-2-0.dll": "Windows 8",
    "api-ms-win-core-heap-l1-2-0.dll": "Windows 8",
    "d3d11.dll": "Windows 8 (su 7 solo con Platform Update)",
    "d3d12.dll": "Windows 10",
    "shcore.dll": "Windows 8.1",
    "ucrtbase.dll": "se non presente la Universal CRT (KB2999226)",
}

# DLL note per essere presenti su Windows 7. Tutto il resto viene segnalato
# come "da verificare" invece che dato per buono.
OK_ON_WIN7 = {
    "kernel32.dll", "ntdll.dll", "user32.dll", "gdi32.dll", "advapi32.dll",
    "shell32.dll", "shlwapi.dll", "comctl32.dll", "comdlg32.dll", "ole32.dll",
    "oleaut32.dll", "userenv.dll", "ws2_32.dll", "winmm.dll", "msvcrt.dll",
    "bcrypt.dll", "crypt32.dll", "dbghelp.dll", "imm32.dll", "version.dll",
    "opengl32.dll", "dsound.dll", "uxtheme.dll", "dinput8.dll", "xinput1_3.dll", "d3d9.dll",
    "dxgi.dll", "dwmapi.dll", "setupapi.dll", "wininet.dll", "iphlpapi.dll",
}

PE32, PE32_PLUS = 0x10B, 0x20B


def read_imports(path):
    """Ritorna (magic, minima_os, minima_subsystem, [dll importate])."""
    with open(path, "rb") as fh:
        data = fh.read()

    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("%s non e' un eseguibile Windows valido" % path)

    opt = pe + 24
    magic = struct.unpack_from("<H", data, opt)[0]
    min_os = struct.unpack_from("<HH", data, opt + 40)
    min_sub = struct.unpack_from("<HH", data, opt + 48)

    n_sections = struct.unpack_from("<H", data, pe + 6)[0]
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    section_table = pe + 24 + opt_size

    sections = []
    for i in range(n_sections):
        off = section_table + i * 40
        vsize, vaddr, rawsize, rawptr = struct.unpack_from("<IIII", data, off + 8)
        sections.append((vaddr, vsize, rawptr, rawsize))

    def rva_to_offset(rva):
        for vaddr, vsize, rawptr, rawsize in sections:
            if vaddr <= rva < vaddr + max(vsize, rawsize):
                return rawptr + (rva - vaddr)
        return None

    # data directory 1 = import table
    import_rva = struct.unpack_from("<I", data, opt + 96 + 8)[0]
    off = rva_to_offset(import_rva)

    names = []
    while off:
        descriptor = struct.unpack_from("<IIIII", data, off)
        if not any(descriptor):
            break
        name_off = rva_to_offset(descriptor[3])
        if name_off is None:
            break
        end = data.index(b"\0", name_off)
        names.append(data[name_off:end].decode("ascii", "replace"))
        off += 20

    return magic, min_os, min_sub, sorted(set(names))


def main(argv):
    if len(argv) < 2:
        print(__doc__.strip())
        return 2

    failed = False
    for path in argv[1:]:
        magic, min_os, min_sub, imports = read_imports(path)
        kind = {PE32: "PE32 (32 bit)", PE32_PLUS: "PE32+ (64 bit)"}.get(magic, "?")

        print("%s" % path)
        print("  formato            : %s" % kind)
        print("  versioni header    : OS %d.%d, subsystem %d.%d"
              % (min_os[0], min_os[1], min_sub[0], min_sub[1]))
        print("  (non affidabili: MinGW le lascia a 4.0 anche quando le API non esistono)")

        bad = [(n, MISSING_ON_WIN7[n.lower()]) for n in imports if n.lower() in MISSING_ON_WIN7]
        unknown = [n for n in imports if n.lower() not in OK_ON_WIN7
                   and n.lower() not in MISSING_ON_WIN7]

        print("  dipendenze         : %d" % len(imports))
        if bad:
            failed = True
            print("  INCOMPATIBILE con Windows 7:")
            for name, since in bad:
                print("      %s  (disponibile da %s)" % (name, since))
        if unknown:
            print("  da verificare a mano:")
            for name in unknown:
                print("      %s" % name)
        if not bad and not unknown:
            print("  COMPATIBILE con Windows 7")
        elif not bad:
            print("  nessuna dipendenza nota come incompatibile")

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
