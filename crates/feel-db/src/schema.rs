//! Lo schema SQLite.
//!
//! Le chiavi sono colonne `id`, non `key`: `KEY` e' una parola riservata di
//! SQLite e usarla come nome di colonna funziona finche' non serve citarla,
//! e a quel punto il codice diventa pieno di virgolette.
//!
//! Le date sono testo ISO (`AAAA-MM-GG hh:mm:ss`) e i tempi di gioco secondi
//! interi: cosi' il database resta leggibile con qualunque strumento, senza
//! dipendere dal fuso orario del sistema che l'ha scritto.

/// Versione dello schema. Va alzata a ogni modifica incompatibile.
///
/// 1 -> 2: aggiunte le applicazioni pre/post emulatore.
pub const SCHEMA_VERSION: i64 = 2;

/// Chiave sotto cui la versione e' salvata nella tabella `meta`.
pub const VERSION_KEY: &str = "schema_version";

pub const DDL: &str = r#"
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE platform (
    id       TEXT PRIMARY KEY,
    title    TEXT NOT NULL,
    position INTEGER NOT NULL
);

CREATE TABLE emulator (
    platform       TEXT NOT NULL REFERENCES platform(id) ON DELETE CASCADE,
    id             TEXT NOT NULL,
    title          TEXT NOT NULL,
    executable     TEXT NOT NULL,
    arguments      TEXT NOT NULL,
    working_dir    TEXT,
    rom_path       TEXT NOT NULL,
    rom_extensions TEXT NOT NULL,
    layout         TEXT,
    recursive_roms INTEGER NOT NULL,
    position       INTEGER NOT NULL,
    snapshot_path  TEXT,
    video_path     TEXT,
    marquee_path   TEXT,
    cabinet_path   TEXT,
    controls_path  TEXT,
    pre_launch     TEXT,
    post_launch    TEXT,
    PRIMARY KEY (platform, id)
);

CREATE TABLE gamelist (
    id        TEXT PRIMARY KEY,
    platform  TEXT NOT NULL,
    emulator  TEXT NOT NULL,
    title     TEXT NOT NULL,
    position  INTEGER NOT NULL,
    FOREIGN KEY (platform, emulator) REFERENCES emulator(platform, id) ON DELETE CASCADE
);

CREATE TABLE game (
    gamelist      TEXT NOT NULL REFERENCES gamelist(id) ON DELETE CASCADE,
    id            TEXT NOT NULL,
    title         TEXT NOT NULL,
    year          TEXT,
    manufacturer  TEXT,
    clone_of      TEXT,
    bios          TEXT,
    category      TEXT,
    rotation      TEXT,
    controls      TEXT,
    status        TEXT,
    relative_path TEXT,
    PRIMARY KEY (gamelist, id)
);

CREATE TABLE play (
    platform    TEXT NOT NULL,
    emulator    TEXT NOT NULL,
    game        TEXT NOT NULL,
    count       INTEGER NOT NULL,
    total_secs  INTEGER NOT NULL,
    last_played TEXT,
    PRIMARY KEY (platform, emulator, game),
    FOREIGN KEY (platform, emulator) REFERENCES emulator(platform, id) ON DELETE CASCADE
);

CREATE INDEX game_title_idx ON game (title);
CREATE INDEX game_gamelist_idx ON game (gamelist);
CREATE INDEX play_emulator_idx ON play (platform, emulator);
"#;

/// Le tabelle da svuotare per ripartire da zero, in ordine di dipendenza.
pub const TABLES: [&str; 5] = ["play", "game", "gamelist", "emulator", "platform"];
