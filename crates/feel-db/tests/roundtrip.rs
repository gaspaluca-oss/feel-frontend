//! Andata e ritorno: una installazione FEEL 1.x -> database -> libreria.
//!
//! È il test che dà valore al database. Se la libreria riletta non corrisponde a
//! quella importata, il frontend mostrerebbe qualcosa di diverso da quello che
//! l'utente ha configurato, ed è esattamente il tipo di errore che si scopre
//! tardi, sul cabinato.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use feel_core::{EmulatorKey, GameKey, GamelistKey, Library, PlatformKey};
use feel_db::{Counts, Database, DbError};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../feel-config/tests/fixtures/feel1")
}

fn libreria_importata() -> Library {
    feel_config::load(fixture()).expect("la fixture deve essere importabile").library
}

/// Tutti i nomi rom per lista, per confrontare due librerie senza dipendere
/// dall'ordine in cui sono state salvate.
fn impronta(library: &Library) -> BTreeSet<(String, String)> {
    library
        .games
        .iter()
        .map(|g| (g.gamelist.to_string(), g.key.to_string()))
        .collect()
}

#[test]
fn salva_e_rilegge_la_libreria() {
    let originale = libreria_importata();

    let mut db = Database::open_in_memory().unwrap();
    db.store_library(&originale).unwrap();
    let riletta = db.load_library().unwrap();

    // 2 piattaforme (arcade, console), 3 emulatori (mame, daphne, snes9x),
    // 4 liste (mame-0, mame-1, daphne-0, snes9x-0), 4 giochi, 3 con statistiche
    // (pacman e puckman da mame.sts, Super_Metroid da snes9x.sts).
    assert_eq!(
        db.counts().unwrap(),
        Counts { platforms: 2, emulators: 3, gamelists: 4, games: 4, played_games: 3 }
    );

    // Le stesse piattaforme, nello stesso ordine.
    let p_orig: Vec<_> = originale.platforms.iter().map(|p| p.key.to_string()).collect();
    let p_letto: Vec<_> = riletta.platforms.iter().map(|p| p.key.to_string()).collect();
    assert_eq!(p_letto, p_orig);

    // Gli stessi giochi.
    assert_eq!(impronta(&riletta), impronta(&originale));

    // Gli stessi emulatori, con l'ordine interno di ogni piattaforma.
    for platform in &originale.platforms {
        let prima = originale.platform(&platform.key).unwrap().emulators.clone();
        let dopo = riletta.platform(&platform.key).unwrap().emulators.clone();
        assert_eq!(dopo, prima, "emulatori di {}", platform.key);
    }
}

#[test]
fn conserva_i_campi_di_un_gioco() {
    let mut db = Database::open_in_memory().unwrap();
    db.store_library(&libreria_importata()).unwrap();

    let rom = db.games_of(&GamelistKey::new("mame-0")).unwrap();
    let pacman = rom.iter().find(|g| g.key.as_str() == "pacman").unwrap();

    assert_eq!(pacman.title, "Pac-Man (Midway)");
    assert_eq!(pacman.year.as_deref(), Some("1980"));
    assert_eq!(pacman.manufacturer.as_deref(), Some("Namco"));
    assert_eq!(pacman.category.as_deref(), Some("Maze / Collect"));
    assert_eq!(pacman.controls.as_deref(), Some("joy4way - 2P - 4Bt"));
    assert_eq!(pacman.status.as_deref(), Some("good"));
    assert_eq!(pacman.clone_of, None);

    // L'orientamento e' un enum: deve tornare quello, non una stringa.
    assert_eq!(
        pacman.rotation,
        Some(feel_core::Rotation::Vertical)
    );

    let puckman = rom.iter().find(|g| g.key.as_str() == "puckman").unwrap();
    assert_eq!(puckman.clone_of.as_ref().unwrap().as_str(), "pacman");

    let s1945 = rom.iter().find(|g| g.key.as_str() == "s1945").unwrap();
    assert_eq!(s1945.relative_path.as_ref().unwrap().to_string_lossy(), "psikyo");
}

#[test]
fn conserva_gli_argomenti_e_i_percorsi_dell_emulatore() {
    let mut db = Database::open_in_memory().unwrap();
    db.store_library(&libreria_importata()).unwrap();
    let riletta = db.load_library().unwrap();

    let mame = riletta
        .emulator(&PlatformKey::new("arcade"), &EmulatorKey::new("mame"))
        .unwrap();

    // Gli argomenti contengono spazi e parentesi: passano dal JSON e tornano uguali.
    assert_eq!(mame.arguments, vec!["[name]"]);
    assert_eq!(mame.executable.to_string_lossy(), "C:/Program Files/mame/mame.exe");
    assert_eq!(mame.rom_path.to_string_lossy(), "C:/Program Files/mame/roms");
    assert_eq!(mame.rom_extensions, vec!["zip"]);
    assert!(mame.recursive_roms);

    let snes = riletta
        .emulator(&PlatformKey::new("console"), &EmulatorKey::new("snes9x"))
        .unwrap();
    assert_eq!(snes.arguments, vec!["[romfile]"]);
    assert_eq!(snes.rom_extensions, vec!["smc", "sfc"]);
}

#[test]
fn conserva_le_statistiche() {
    let mut db = Database::open_in_memory().unwrap();
    db.store_library(&libreria_importata()).unwrap();

    let arcade_mame = (PlatformKey::new("arcade"), EmulatorKey::new("mame"));
    let stats = db.stats(&arcade_mame, &GameKey::new("pacman")).unwrap().unwrap();

    assert_eq!(stats.count, 42);
    assert_eq!(stats.total, Duration::from_secs(5 * 86_400 + 3 * 3_600 + 12 * 60 + 7));
    assert_eq!(stats.last.unwrap().to_string(), "2018-03-12 21:14:00");

    // Un gioco mai avviato non ha statistiche: non deve inventarne di vuote.
    assert!(db.stats(&arcade_mame, &GameKey::new("s1945")).unwrap().is_none());
}

#[test]
fn salvare_due_volte_non_duplica() {
    let library = libreria_importata();
    let mut db = Database::open_in_memory().unwrap();

    db.store_library(&library).unwrap();
    let primo = db.counts().unwrap();

    db.store_library(&library).unwrap();
    let secondo = db.counts().unwrap();

    assert_eq!(primo, secondo);
    assert_eq!(db.games_of(&GamelistKey::new("mame-0")).unwrap().len(), 3);
}

#[test]
fn un_database_nuovo_e_alla_versione_corrente() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.schema_version().unwrap(), feel_db::SCHEMA_VERSION);
    assert_eq!(db.counts().unwrap(), Counts::default());
}

#[test]
fn una_versione_sconosciuta_viene_rifiutata() {
    // Un database scritto da una versione futura non va aperto a meta': meglio
    // un errore chiaro che una libreria interpretata male.
    let path = std::env::temp_dir().join(format!("feel-db-test-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);

    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO meta (key, value) VALUES ('schema_version', '999');",
        )
        .unwrap();
    }

    match Database::open(&path) {
        Err(DbError::SchemaVersion { found, expected }) => {
            assert_eq!(found, 999);
            assert_eq!(expected, feel_db::SCHEMA_VERSION);
        }
        Err(altro) => panic!("atteso un errore di versione, ottenuto: {altro}"),
        Ok(_) => panic!("atteso un errore di versione, ma il database si e' aperto"),
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn una_libreria_vuota_si_salva_e_si_rilegge() {
    let mut db = Database::open_in_memory().unwrap();
    db.store_library(&Library::default()).unwrap();

    let riletta = db.load_library().unwrap();
    assert!(riletta.platforms.is_empty());
    assert!(riletta.games.is_empty());
    assert_eq!(db.counts().unwrap(), Counts::default());
}
