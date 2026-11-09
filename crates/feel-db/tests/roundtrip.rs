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
fn registrare_una_partita_accumula() {
    let mut db = Database::open_in_memory().unwrap();
    db.store_library(&libreria_importata()).unwrap();

    let arcade_mame = (PlatformKey::new("arcade"), EmulatorKey::new("mame"));
    let pacman = GameKey::new("pacman");

    let prima = db.stats(&arcade_mame, &pacman).unwrap().unwrap();
    assert_eq!(prima.count, 42);

    // Due partite: il conteggio cresce e i tempi si sommano.
    let quando = feel_core::Timestamp::from_unix_seconds(1_709_208_896);
    db.record_play(&arcade_mame, &pacman, Duration::from_secs(90), quando).unwrap();
    db.record_play(&arcade_mame, &pacman, Duration::from_secs(30), quando).unwrap();

    let dopo = db.stats(&arcade_mame, &pacman).unwrap().unwrap();
    assert_eq!(dopo.count, 44);
    assert_eq!(dopo.total, prima.total + Duration::from_secs(120));
    assert_eq!(dopo.last.unwrap().to_string(), "2024-02-29 12:14:56");

    // Un gioco mai giocato riceve la sua prima riga, non un errore.
    let s1945 = GameKey::new("s1945");
    db.record_play(&arcade_mame, &s1945, Duration::from_secs(10), quando).unwrap();
    let nuova = db.stats(&arcade_mame, &s1945).unwrap().unwrap();
    assert_eq!(nuova.count, 1);
    assert_eq!(nuova.total, Duration::from_secs(10));
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

#[test]
fn i_preferiti_si_mettono_e_si_tolgono() {
    let mut db = Database::open_in_memory().unwrap();
    db.store_library(&libreria_importata()).unwrap();

    let lista = GamelistKey::new("mame-0");
    let pacman = GameKey::new("pacman");
    let galaga = GameKey::new("s1945");

    assert!(!db.is_favourite(&lista, &pacman).unwrap());
    assert!(db.favourites_of(&lista).unwrap().is_empty());

    // In ordine alfabetico per titolo, non per nome rom: s1945 e' "Strikers
    // 1945", pacman e' "Pac-Man (Midway)".
    db.set_favourite(&lista, &pacman, true).unwrap();
    db.set_favourite(&lista, &galaga, true).unwrap();

    assert!(db.is_favourite(&lista, &pacman).unwrap());
    assert_eq!(
        db.favourites_of(&lista).unwrap(),
        vec![pacman.clone(), galaga.clone()]
    );

    // Rimettarlo non lo duplica.
    db.set_favourite(&lista, &pacman, true).unwrap();
    assert_eq!(db.favourites_of(&lista).unwrap().len(), 2);

    db.set_favourite(&lista, &pacman, false).unwrap();
    assert!(!db.is_favourite(&lista, &pacman).unwrap());
    assert_eq!(db.favourites_of(&lista).unwrap(), vec![galaga]);
}

#[test]
fn i_preferiti_sopravvivono_a_una_reimportazione() {
    let library = libreria_importata();
    let mut db = Database::open_in_memory().unwrap();
    db.store_library(&library).unwrap();

    let lista = GamelistKey::new("mame-0");
    let pacman = GameKey::new("pacman");
    db.set_favourite(&lista, &pacman, true).unwrap();

    // Reimportare la stessa libreria non deve cancellare le scelte dell'utente.
    db.store_library(&library).unwrap();
    assert!(db.is_favourite(&lista, &pacman).unwrap());
}

#[test]
fn un_database_della_versione_2_viene_aggiornato_senza_perdere_niente() {
    let path = std::env::temp_dir().join(format!("feel-db-v2-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);

    {
        // Ricostruisco un database come lo lasciava la versione 2: stesse
        // tabelle, senza quella dei preferiti.
        // Stesse tabelle della versione 3 tranne quella dei preferiti: e'
        // esattamente cio' che lasciava la versione 2.
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(feel_db::schema::DDL).unwrap();
        conn.execute_batch(
            "INSERT INTO meta (key, value) VALUES ('schema_version', '2');
             INSERT INTO platform (id, title, position) VALUES ('arcade', 'Arcade', 0);",
        )
        .unwrap();
    }

    let db = Database::open(&path).expect("la migrazione deve riuscire");
    assert_eq!(db.schema_version().unwrap(), feel_db::SCHEMA_VERSION);
    // I dati che c'erano sono ancora li'.
    assert_eq!(db.counts().unwrap().platforms, 1);
    // E la tabella nuova funziona.
    assert!(!db
        .is_favourite(&GamelistKey::new("mame-0"), &GameKey::new("pacman"))
        .unwrap());

    let _ = std::fs::remove_file(&path);
}
