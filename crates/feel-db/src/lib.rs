//! Persistenza della libreria su SQLite.
//!
//! Un solo database per libreria, statistiche, preferiti e playlist: sostituisce
//! i file di testo `data/*.lst` e `data/*.sts` di FEEL 1.x.
//!
//! Due scelte:
//!
//! - **SQLite compilato dai sorgenti** (feature `bundled`): nessuna DLL da
//!   installare sul cabinato, e la stessa versione su Windows e Linux.
//! - **`store_library` sostituisce tutto in una transazione**: l'importazione e'
//!   un'operazione atomica. Se si interrompe a meta', il database resta quello
//!   di prima invece di diventare una libreria parziale che sembra valida.

pub mod schema;

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use feel_core::{
    ArtworkPaths, Emulator, EmulatorKey, EmulatorRef, Game, GameKey, Gamelist, GamelistKey,
    HookCommand, LayoutKey, Library, Platform, PlatformKey, PlayStats, Rotation, Timestamp,
};
use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

pub use schema::SCHEMA_VERSION;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("errore SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("una colonna non contiene il JSON atteso: {0}")]
    Json(#[from] serde_json::Error),

    #[error("versione dello schema {found} non riconosciuta (questa build usa la {expected})")]
    SchemaVersion { found: i64, expected: i64 },
}

/// Permette di usare `?` dentro le closure di `rusqlite::query_map`, che
/// pretendono un `rusqlite::Result`.
///
/// Il caso concreto e' decodificare una colonna JSON mentre si costruisce una
/// riga: se il contenuto non e' valido, l'errore giusto da riportare e' proprio
/// "questa colonna non si puo' convertire", che e' cosa significa
/// `FromSqlConversionFailure`.
impl From<DbError> for rusqlite::Error {
    fn from(err: DbError) -> Self {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(err),
        )
    }
}

/// Quanti elementi contiene il database. Costa una query, non carica niente.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    pub platforms: i64,
    pub emulators: i64,
    pub gamelists: i64,
    pub games: i64,
    /// Giochi con almeno una partita registrata.
    pub played_games: i64,
}

/// Il database della libreria.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Apre, o crea, il database indicato.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Un database in memoria: comodo per i test e per le anteprime.
    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self, DbError> {
        // Va fatto per ogni connessione: SQLite non lo ricorda fra una e l'altra.
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let db = Self { conn };
        db.prepare_schema()?;
        Ok(db)
    }

    fn prepare_schema(&self) -> Result<(), DbError> {
        // Su un database nuovo la tabella `meta` non esiste ancora: interrogarla
        // direttamente darebbe "no such table" invece di "nessuna versione".
        let has_meta: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'meta'",
            [],
            |row| row.get(0),
        )?;

        if has_meta == 0 {
            self.conn.execute_batch(schema::DDL)?;
            // La tabella dei preferiti ha un DDL suo perche' serve anche alla
            // migrazione dalla versione 2: qui si crea insieme al resto.
            self.conn.execute_batch(schema::FAVOURITE_DDL)?;
            self.conn.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                params![schema::VERSION_KEY, SCHEMA_VERSION.to_string()],
            )?;
            return Ok(());
        }

        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![schema::VERSION_KEY],
                |row| row.get(0),
            )
            .optional()?;

        match raw.and_then(|value| value.parse::<i64>().ok()) {
            Some(found) if found == SCHEMA_VERSION => Ok(()),

            // Migrazione dalla 2 alla 3: si aggiunge la tabella dei preferiti
            // senza toccare niente di quello che c'e' gia'. Un utente che ha
            // importato la libreria non deve rifarlo.
            Some(2) => {
                self.conn.execute_batch(schema::FAVOURITE_DDL)?;
                self.conn.execute(
                    "UPDATE meta SET value = ?1 WHERE key = ?2",
                    params![SCHEMA_VERSION.to_string(), schema::VERSION_KEY],
                )?;
                Ok(())
            }

            Some(found) => Err(DbError::SchemaVersion { found, expected: SCHEMA_VERSION }),
            // Tabelle presenti ma versione assente: schema non riconosciuto.
            // Meglio un errore chiaro che interpretare a caso.
            None => Err(DbError::SchemaVersion { found: 0, expected: SCHEMA_VERSION }),
        }
    }

    pub fn schema_version(&self) -> Result<i64, DbError> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![schema::VERSION_KEY],
                |row| row.get(0),
            )
            .optional()?;
        Ok(raw.and_then(|value| value.parse().ok()).unwrap_or(0))
    }

    // -- scrittura --------------------------------------------------------------

    /// Sostituisce l'intera libreria.
    ///
    /// Tutto o niente: se qualcosa fallisce a meta', la transazione viene
    /// annullata e il database resta come era prima.
    pub fn store_library(&mut self, library: &Library) -> Result<(), DbError> {
        let tx = self.conn.transaction()?;

        for table in schema::TABLES {
            // `table` viene da una costante del codice, non da input esterno.
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }

        for platform in &library.platforms {
            tx.execute(
                "INSERT INTO platform (id, title, position) VALUES (?1, ?2, ?3)",
                params![platform.key.as_str(), platform.title, platform.order as i64],
            )?;
        }

        // Gli emulatori non hanno un ordine proprio nel modello: si usa l'ordine
        // in cui compaiono, contato per piattaforma.
        let mut emulator_position: HashMap<&PlatformKey, i64> = HashMap::new();
        for emulator in &library.emulators {
            let position = emulator_position.entry(&emulator.platform).or_insert(0);
            let current = *position;
            *position += 1;

            tx.execute(
                "INSERT INTO emulator (
                     platform, id, title, executable, arguments, working_dir, rom_path,
                     rom_extensions, layout, recursive_roms, position,
                     snapshot_path, video_path, marquee_path, cabinet_path, controls_path,
                     pre_launch, post_launch
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                           ?17, ?18)",
                params![
                    emulator.platform.as_str(),
                    emulator.key.as_str(),
                    emulator.title,
                    emulator.executable.to_string_lossy(),
                    encode_list(&emulator.arguments)?,
                    emulator.working_directory.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    emulator.rom_path.to_string_lossy(),
                    encode_list(&emulator.rom_extensions)?,
                    emulator.layout.as_ref().map(|l| l.as_str().to_owned()),
                    emulator.recursive_roms as i64,
                    current,
                    emulator.artwork.snapshot.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    emulator.artwork.video.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    emulator.artwork.marquee.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    emulator.artwork.cabinet.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    emulator.artwork.control_panel.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    encode_hook(emulator.pre_launch.as_ref())?,
                    encode_hook(emulator.post_launch.as_ref())?,
                ],
            )?;
        }

        for gamelist in &library.gamelists {
            tx.execute(
                "INSERT INTO gamelist (id, platform, emulator, title, position)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    gamelist.key.as_str(),
                    gamelist.platform.as_str(),
                    gamelist.emulator.as_str(),
                    gamelist.title,
                    gamelist.order as i64,
                ],
            )?;
        }

        for game in &library.games {
            tx.execute(
                "INSERT INTO game (
                     gamelist, id, title, year, manufacturer, clone_of, bios,
                     category, rotation, controls, status, relative_path
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    game.gamelist.as_str(),
                    game.key.as_str(),
                    game.title,
                    game.year,
                    game.manufacturer,
                    game.clone_of.as_ref().map(|k| k.as_str().to_owned()),
                    game.bios,
                    game.category,
                    game.rotation.map(rotation_to_text),
                    game.controls,
                    game.status,
                    game.relative_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                ],
            )?;
        }

        for ((platform, emulator), by_game) in &library.plays {
            for (game, stats) in by_game {
                tx.execute(
                    "INSERT INTO play (platform, emulator, game, count, total_secs, last_played)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        platform.as_str(),
                        emulator.as_str(),
                        game.as_str(),
                        stats.count as i64,
                        stats.total.as_secs() as i64,
                        stats.last.map(|t| t.to_string()),
                    ],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    // -- lettura ----------------------------------------------------------------

    pub fn counts(&self) -> Result<Counts, DbError> {
        let one = |sql: &str| -> Result<i64, DbError> {
            Ok(self.conn.query_row(sql, [], |row| row.get(0))?)
        };

        Ok(Counts {
            platforms: one("SELECT COUNT(*) FROM platform")?,
            emulators: one("SELECT COUNT(*) FROM emulator")?,
            gamelists: one("SELECT COUNT(*) FROM gamelist")?,
            games: one("SELECT COUNT(*) FROM game")?,
            played_games: one("SELECT COUNT(*) FROM play WHERE count > 0")?,
        })
    }

    /// Rilegge l'intera libreria.
    ///
    /// Serve ai test e alle librerie piccole. Per una lista di migliaia di giochi
    /// il frontend userebbe query mirate, non un caricamento totale.
    pub fn load_library(&self) -> Result<Library, DbError> {
        let mut library = Library::default();

        // Piattaforme
        {
            let mut stmt = self
                .conn
                .prepare("SELECT id, title, position FROM platform ORDER BY position, id")?;
            let rows = stmt.query_map([], |row| {
                Ok(Platform {
                    key: PlatformKey::new(row.get::<_, String>(0)?),
                    title: row.get(1)?,
                    order: row.get::<_, i64>(2)? as u32,
                    emulators: Vec::new(),
                })
            })?;
            for row in rows {
                library.platforms.push(row?);
            }
        }

        // Emulatori
        {
            let mut stmt = self.conn.prepare(
                "SELECT platform, id, title, executable, arguments, working_dir, rom_path,
                        rom_extensions, layout, recursive_roms,
                        snapshot_path, video_path, marquee_path, cabinet_path, controls_path,
                        pre_launch, post_launch
                 FROM emulator ORDER BY platform, position",
            )?;
            let rows = stmt.query_map([], |row| {
                let platform = PlatformKey::new(row.get::<_, String>(0)?);
                let key = EmulatorKey::new(row.get::<_, String>(1)?);
                let arguments: String = row.get(4)?;
                let rom_extensions: String = row.get(7)?;
                let layout: Option<String> = row.get(8)?;
                let pre_launch: Option<String> = row.get(15)?;
                let post_launch: Option<String> = row.get(16)?;
                Ok((
                    platform,
                    Emulator {
                        key,
                        platform: PlatformKey::default(),
                        title: row.get(2)?,
                        executable: path_of(row.get::<_, String>(3)?),
                        arguments: decode_list(&arguments)?,
                        working_directory: row.get::<_, Option<String>>(5)?.map(path_of),
                        rom_path: path_of(row.get::<_, String>(6)?),
                        rom_extensions: decode_list(&rom_extensions)?,
                        artwork: ArtworkPaths {
                            snapshot: row.get::<_, Option<String>>(10)?.map(path_of),
                            video: row.get::<_, Option<String>>(11)?.map(path_of),
                            marquee: row.get::<_, Option<String>>(12)?.map(path_of),
                            cabinet: row.get::<_, Option<String>>(13)?.map(path_of),
                            control_panel: row.get::<_, Option<String>>(14)?.map(path_of),
                        },
                        layout: layout.map(LayoutKey::new),
                        gamelists: Vec::new(),
                        recursive_roms: row.get::<_, i64>(9)? != 0,
                        pre_launch: decode_hook(pre_launch.as_deref())?,
                        post_launch: decode_hook(post_launch.as_deref())?,
                    },
                ))
            })?;
            for row in rows {
                let (platform, mut emulator) = row?;
                emulator.platform = platform.clone();
                if let Some(entry) = library.platforms.iter_mut().find(|p| p.key == platform) {
                    entry.emulators.push(emulator.key.clone());
                }
                library.emulators.push(emulator);
            }
        }

        // Liste
        {
            let mut stmt = self.conn.prepare(
                "SELECT id, platform, emulator, title, position
                 FROM gamelist ORDER BY platform, emulator, position, id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(Gamelist {
                    key: GamelistKey::new(row.get::<_, String>(0)?),
                    platform: PlatformKey::new(row.get::<_, String>(1)?),
                    emulator: EmulatorKey::new(row.get::<_, String>(2)?),
                    title: row.get(3)?,
                    order: row.get::<_, i64>(4)? as u32,
                })
            })?;
            for row in rows {
                let gamelist = row?;
                if let Some(emulator) = library.emulators.iter_mut().find(|e| {
                    e.platform == gamelist.platform && e.key == gamelist.emulator
                }) {
                    emulator.gamelists.push(gamelist.key.clone());
                }
                library.gamelists.push(gamelist);
            }
        }

        // Giochi
        {
            let mut stmt = self.conn.prepare(
                "SELECT gamelist, id, title, year, manufacturer, clone_of, bios,
                        category, rotation, controls, status, relative_path
                 FROM game ORDER BY gamelist, title, id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(Game {
                    gamelist: GamelistKey::new(row.get::<_, String>(0)?),
                    key: GameKey::new(row.get::<_, String>(1)?),
                    title: row.get(2)?,
                    year: row.get(3)?,
                    manufacturer: row.get(4)?,
                    clone_of: row.get::<_, Option<String>>(5)?.map(GameKey::new),
                    bios: row.get(6)?,
                    category: row.get(7)?,
                    rotation: row
                        .get::<_, Option<String>>(8)?
                        .and_then(|value| Rotation::parse(&value)),
                    controls: row.get(9)?,
                    status: row.get(10)?,
                    relative_path: row.get::<_, Option<String>>(11)?.map(path_of),
                })
            })?;
            for row in rows {
                library.games.push(row?);
            }
        }

        // Statistiche
        {
            let mut stmt = self.conn.prepare(
                "SELECT platform, emulator, game, count, total_secs, last_played FROM play",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    PlatformKey::new(row.get::<_, String>(0)?),
                    EmulatorKey::new(row.get::<_, String>(1)?),
                    GameKey::new(row.get::<_, String>(2)?),
                    PlayStats {
                        count: row.get::<_, i64>(3)? as u32,
                        total: Duration::from_secs(row.get::<_, i64>(4)? as u64),
                        last: row
                            .get::<_, Option<String>>(5)?
                            .and_then(|value| Timestamp::parse_iso(&value)),
                    },
                ))
            })?;
            for row in rows {
                let (platform, emulator, game, stats) = row?;
                let entry: &mut HashMap<GameKey, PlayStats> = library
                    .plays
                    .entry((platform, emulator))
                    .or_default();
                entry.insert(game, stats);
            }
        }

        Ok(library)
    }

    /// I giochi di una lista, in ordine di titolo.
    pub fn games_of(&self, gamelist: &GamelistKey) -> Result<Vec<Game>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT gamelist, id, title, year, manufacturer, clone_of, bios,
                    category, rotation, controls, status, relative_path
             FROM game WHERE gamelist = ?1 ORDER BY title, id",
        )?;
        let rows = stmt.query_map(params![gamelist.as_str()], |row| {
            Ok(Game {
                gamelist: GamelistKey::new(row.get::<_, String>(0)?),
                key: GameKey::new(row.get::<_, String>(1)?),
                title: row.get(2)?,
                year: row.get(3)?,
                manufacturer: row.get(4)?,
                clone_of: row.get::<_, Option<String>>(5)?.map(GameKey::new),
                bios: row.get(6)?,
                category: row.get(7)?,
                rotation: row
                    .get::<_, Option<String>>(8)?
                    .and_then(|value| Rotation::parse(&value)),
                controls: row.get(9)?,
                status: row.get(10)?,
                relative_path: row.get::<_, Option<String>>(11)?.map(path_of),
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Registra una partita: incrementa il conteggio, somma il tempo e aggiorna
    /// l'istante dell'ultima volta.
    ///
    /// Un solo `INSERT ... ON CONFLICT`, non una lettura piu' una scrittura:
    /// cosi' non c'e' finestra fra il controllo e l'aggiornamento.
    pub fn record_play(
        &self,
        emulator: &EmulatorRef,
        game: &GameKey,
        played: Duration,
        when: Timestamp,
    ) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO play (platform, emulator, game, count, total_secs, last_played)
             VALUES (?1, ?2, ?3, 1, ?4, ?5)
             ON CONFLICT (platform, emulator, game) DO UPDATE SET
                 count      = count + 1,
                 total_secs = total_secs + excluded.total_secs,
                 last_played = excluded.last_played",
            params![
                emulator.0.as_str(),
                emulator.1.as_str(),
                game.as_str(),
                played.as_secs() as i64,
                when.to_string(),
            ],
        )?;
        Ok(())
    }

    // -- preferiti ---------------------------------------------------------------

    /// Segna o toglie un gioco dai preferiti.
    pub fn set_favourite(
        &self,
        gamelist: &GamelistKey,
        game: &GameKey,
        preferito: bool,
    ) -> Result<(), DbError> {
        if preferito {
            self.conn.execute(
                "INSERT OR IGNORE INTO favourite (gamelist, game, added) VALUES (?1, ?2, ?3)",
                params![gamelist.as_str(), game.as_str(), Timestamp::now().to_string()],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM favourite WHERE gamelist = ?1 AND game = ?2",
                params![gamelist.as_str(), game.as_str()],
            )?;
        }
        Ok(())
    }

    pub fn is_favourite(
        &self,
        gamelist: &GamelistKey,
        game: &GameKey,
    ) -> Result<bool, DbError> {
        let quanti: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM favourite WHERE gamelist = ?1 AND game = ?2",
            params![gamelist.as_str(), game.as_str()],
            |row| row.get(0),
        )?;
        Ok(quanti > 0)
    }

    /// I preferiti di una lista, in ordine alfabetico.
    pub fn favourites_of(&self, gamelist: &GamelistKey) -> Result<Vec<GameKey>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT f.game FROM favourite f
             JOIN game g ON g.gamelist = f.gamelist AND g.id = f.game
             WHERE f.gamelist = ?1 ORDER BY g.title",
        )?;
        let righe = stmt.query_map(params![gamelist.as_str()], |row| {
            Ok(GameKey::new(row.get::<_, String>(0)?))
        })?;

        let mut fuori = Vec::new();
        for riga in righe {
            fuori.push(riga?);
        }
        Ok(fuori)
    }

    /// Le statistiche di un gioco.
    pub fn stats(&self, emulator: &EmulatorRef, game: &GameKey) -> Result<Option<PlayStats>, DbError> {
        let found: Option<(i64, i64, Option<String>)> = self
            .conn
            .query_row(
                "SELECT count, total_secs, last_played FROM play
                 WHERE platform = ?1 AND emulator = ?2 AND game = ?3",
                params![emulator.0.as_str(), emulator.1.as_str(), game.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        Ok(found.map(|(count, seconds, last)| PlayStats {
            count: count as u32,
            total: Duration::from_secs(seconds as u64),
            last: last.and_then(|value| Timestamp::parse_iso(&value)),
        }))
    }
}

fn path_of(value: String) -> std::path::PathBuf {
    std::path::PathBuf::from(value)
}

fn encode_list(values: &[String]) -> Result<String, DbError> {
    Ok(serde_json::to_string(values)?)
}

fn decode_list(raw: &str) -> Result<Vec<String>, DbError> {
    Ok(serde_json::from_str(raw)?)
}

/// Le applicazioni pre/post emulatore hanno campi opzionali di tipo diverso
/// dagli altri, quindi non entrano in `encode_list`.
fn encode_hook(hook: Option<&HookCommand>) -> Result<Option<String>, DbError> {
    match hook {
        Some(hook) => Ok(Some(serde_json::to_string(hook)?)),
        None => Ok(None),
    }
}

fn decode_hook(raw: Option<&str>) -> Result<Option<HookCommand>, DbError> {
    match raw {
        Some(text) => Ok(Some(serde_json::from_str(text)?)),
        None => Ok(None),
    }
}

fn rotation_to_text(rotation: Rotation) -> String {
    match rotation {
        Rotation::Horizontal => "horizontal",
        Rotation::Vertical => "vertical",
    }
    .to_owned()
}
