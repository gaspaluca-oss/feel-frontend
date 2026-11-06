//! Importazione di una installazione FEEL 1.x, sulle fixture in `tests/fixtures`.
//!
//! Le fixture riproducono fedelmente il formato: i file `.lst` hanno tredici
//! righe per gioco (comprese le righe vuote dei campi assenti), e `feel.ini`
//! separa chiave e valore con spazi.

use std::path::PathBuf;
use std::time::Duration;

use feel_config::{load, load_with, ImportOptions, PathRules};
use feel_core::{EmulatorKey, GameKey, GamelistKey, PlatformKey, Rotation};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/feel1")
}

fn importa() -> feel_config::Feel1Import {
    load(fixture()).expect("la fixture deve essere importabile")
}

#[test]
fn importa_piattaforme_ed_emulatori() {
    let import = importa();
    let lib = &import.library;

    let piattaforme: Vec<&str> = lib.platforms.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(piattaforme, vec!["arcade", "console"]);

    let arcade = lib.platform(&PlatformKey::new("arcade")).unwrap();
    assert_eq!(arcade.title, "Arcade");
    assert_eq!(arcade.order, 0);
    assert_eq!(
        arcade.emulators.iter().map(|e| e.as_str()).collect::<Vec<_>>(),
        vec!["mame", "daphne"]
    );

    let console = lib.platform(&PlatformKey::new("console")).unwrap();
    assert_eq!(console.title, "Console");
    assert_eq!(console.order, 1);
}

#[test]
fn importa_i_dettagli_dell_emulatore() {
    let import = importa();
    let mame = import
        .library
        .emulator(&PlatformKey::new("arcade"), &EmulatorKey::new("mame"))
        .unwrap();

    assert_eq!(mame.title, "M.A.M.E.");
    // `[emulator_path]` nella commandline e' il percorso concreto, e viene risolto.
    assert_eq!(mame.executable.to_string_lossy(), "C:/Program Files/mame/mame.exe");
    // `[emulator_path]` negli argomenti invece resta un template.
    assert_eq!(mame.arguments, vec!["[name]"]);
    assert_eq!(mame.rom_path.to_string_lossy(), "C:/Program Files/mame/roms");
    assert_eq!(mame.rom_extensions, vec!["zip"]);
    assert_eq!(mame.layout.as_ref().unwrap().as_str(), "MAME");
    assert_eq!(
        mame.gamelists.iter().map(|g| g.as_str()).collect::<Vec<_>>(),
        vec!["mame-0", "mame-1"]
    );
    assert_eq!(
        mame.artwork.snapshot.as_ref().unwrap().to_string_lossy(),
        "C:/Program Files/mame/snap"
    );
    assert_eq!(
        mame.artwork.video.as_ref().unwrap().to_string_lossy(),
        "C:/Program Files/mame/video"
    );
}

#[test]
fn importa_le_liste_con_i_loro_titoli() {
    let import = importa();
    let lib = &import.library;

    let all = lib.gamelist(&GamelistKey::new("mame-0")).unwrap();
    assert_eq!(all.title, "All Games");
    assert_eq!(all.order, 0);

    let best = lib.gamelist(&GamelistKey::new("mame-1")).unwrap();
    assert_eq!(best.title, "Best Games");
    assert_eq!(best.order, 1);
}

#[test]
fn importa_tutti_i_campi_di_una_rom() {
    let import = importa();
    let lib = &import.library;

    let rom: Vec<_> = lib.games_of(&GamelistKey::new("mame-0")).collect();
    assert_eq!(rom.len(), 3, "le rom della lista mame-0");

    let pacman = lib
        .games_of(&GamelistKey::new("mame-0"))
        .find(|g| g.key.as_str() == "pacman")
        .unwrap();
    assert_eq!(pacman.title, "Pac-Man (Midway)");
    assert_eq!(pacman.year.as_deref(), Some("1980"));
    assert_eq!(pacman.manufacturer.as_deref(), Some("Namco"));
    assert_eq!(pacman.category.as_deref(), Some("Maze / Collect"));
    assert_eq!(pacman.rotation, Some(Rotation::Vertical));
    assert_eq!(pacman.controls.as_deref(), Some("joy4way - 2P - 4Bt"));
    assert_eq!(pacman.status.as_deref(), Some("good"));
    // Senza questo campo la rom e' un originale, non un clone.
    assert_eq!(pacman.clone_of, None);
    assert_eq!(pacman.cleaned_title(), "Pac-Man");

    let puckman = lib
        .games_of(&GamelistKey::new("mame-0"))
        .find(|g| g.key.as_str() == "puckman")
        .unwrap();
    assert_eq!(puckman.clone_of.as_ref().unwrap().as_str(), "pacman");

    // Il campo extra porta la sottocartella della rom, separata da `§`.
    let s1945 = lib
        .games_of(&GamelistKey::new("mame-0"))
        .find(|g| g.key.as_str() == "s1945")
        .unwrap();
    assert_eq!(s1945.relative_path.as_ref().unwrap().to_string_lossy(), "psikyo");
    assert_eq!(s1945.rotation, Some(Rotation::Horizontal));
}

#[test]
fn importa_le_statistiche_di_gioco() {
    let import = importa();
    let lib = &import.library;

    let pacman = GameKey::new("pacman");
    let stats = lib.stats(&EmulatorKey::new("mame"), &pacman).unwrap();

    assert_eq!(stats.count, 42);
    // 5 giorni, 3 ore, 12 minuti e 7 secondi.
    assert_eq!(stats.total, Duration::from_secs(5 * 86_400 + 3 * 3_600 + 12 * 60 + 7));
    let last = stats.last.unwrap();
    assert_eq!(last.year, 2018);
    assert_eq!(last.month, 3);
    assert_eq!(last.day, 12);
    assert_eq!(last.to_string(), "2018-03-12 21:14:00");
}

#[test]
fn deduce_la_ricorsione_dalle_sottocartelle() {
    let import = importa();
    let mame = import
        .library
        .emulator(&PlatformKey::new("arcade"), &EmulatorKey::new("mame"))
        .unwrap();

    // La fixture ha una rom in `psikyo/`, quindi l'emulatore deve cercare
    // anche sotto `rom_path`, altrimenti quel gioco non parte.
    assert!(mame.recursive_roms);
    assert!(import.report.nested_rom_paths.contains("psikyo"));
}

#[test]
fn riscrive_i_percorsi_quando_richiesto() {
    let options = ImportOptions {
        path_rules: PathRules::new().remap(r"C:\Program Files\mame", r"D:\MAME"),
    };
    let import = load_with(fixture(), &options).unwrap();
    let mame = import
        .library
        .emulator(&PlatformKey::new("arcade"), &EmulatorKey::new("mame"))
        .unwrap();

    assert_eq!(mame.rom_path.to_string_lossy(), "D:/MAME/roms");
    assert_eq!(mame.executable.to_string_lossy(), "D:/MAME/mame.exe");
    assert_eq!(
        mame.artwork.video.as_ref().unwrap().to_string_lossy(),
        "D:/MAME/video"
    );
    // Fuori dal prefisso non si tocca niente.
    let snes = import
        .library
        .emulator(&PlatformKey::new("console"), &EmulatorKey::new("snes9x"))
        .unwrap();
    assert_eq!(snes.rom_path.to_string_lossy(), "C:/Roms/snes");
}

#[test]
fn importa_anche_una_piattaforma_non_mame() {
    let import = importa();
    let snes = import
        .library
        .emulator(&PlatformKey::new("console"), &EmulatorKey::new("snes9x"))
        .unwrap();

    assert_eq!(snes.title, "SNES");
    assert_eq!(snes.rom_extensions, vec!["smc", "sfc"]);
    // `[full_path]` diventa `[romfile]`: la rom si passa con il percorso completo.
    assert_eq!(snes.arguments, vec!["[romfile]"]);
    // I percorsi immagini vuoti restano vuoti, non diventano stringhe casuali.
    assert_eq!(snes.artwork.video, None);
    assert_eq!(snes.artwork.marquee, None);

    let rom: Vec<_> = import.library.games_of(&GamelistKey::new("snes9x-0")).collect();
    assert_eq!(rom.len(), 1);
    assert_eq!(rom[0].title, "Super Metroid (Europe)");
    assert_eq!(rom[0].cleaned_title(), "Super Metroid");
}

#[test]
fn segnala_cio_che_manca_invece_di_inventarlo() {
    let import = importa();

    // `daphne` e' dichiarato in arcade.ini ma non ha configurazione propria.
    assert!(
        import
            .report
            .warnings
            .iter()
            .any(|w| w.contains("daphne") && w.contains("assente")),
        "atteso un avviso su daphne, trovati: {:?}",
        import.report.warnings
    );

    // La lista mame-1 non ha un `.lst`: deve restare vuota, non riempirsi di ipotesi.
    assert_eq!(import.library.games_of(&GamelistKey::new("mame-1")).count(), 0);
    assert!(!import.report.is_clean());
}

#[test]
fn un_installazione_senza_feel_ini_e_un_errore() {
    let vuota = std::env::temp_dir().join("feel-config-cartella-inesistente");
    let err = load(&vuota).unwrap_err();
    assert!(matches!(err, feel_config::ImportError::MissingFeelIni(_)));
}

#[test]
fn le_impostazioni_globali_vengono_lette() {
    let import = importa();
    let s = &import.settings;

    assert_eq!(
        s.platform_order.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        vec!["arcade", "console"]
    );
    assert!(s.switch_resolution);
    assert!(!s.rotate_screen);
    assert!(s.screen_saver_enabled);
    assert_eq!(s.screen_saver_delay_secs, 30);
    assert_eq!(s.video_delay_secs, 2);
    assert_eq!(s.video_volume, 30);
    assert_eq!(s.default_layout.as_ref().unwrap().as_str(), "sheet");

    // La mappatura dei tasti: solo i comandi dichiarati e non vuoti.
    assert_eq!(s.keymap.get("action_key").map(String::as_str), Some("1"));
    assert_eq!(s.keymap.get("menu_cancel").map(String::as_str), Some("2"));
    assert_eq!(s.keymap.get("show_exit_menu").map(String::as_str), Some("ESC"));
    assert!(!s.keymap.contains_key("select_gamelist"), "vuoto, non mappato");
}
