//! I tipi del dominio.
//!
//! La gerarchia riprende quella di FEEL 1.x, che era corretta:
//! piattaforma -> emulatore -> lista (gamelist) -> gioco.
//!
//! Le chiavi sono tipi distinti e non `String` sciolte: `GameKey` e `EmulatorKey`
//! non sono intercambiabili, quindi il compilatore impedisce di passare l'una
//! dove serve l'altra. In FEEL 1.x questa confusione era possibile e costava bug.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Coppia che identifica un emulatore in tutta la libreria.
pub type EmulatorRef = (PlatformKey, EmulatorKey);

macro_rules! key_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
        )]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

key_type!(
    /// Chiave di una piattaforma, come `arcade` o `console`.
    PlatformKey
);
key_type!(
    /// Chiave di un emulatore. Non e' unica da sola: lo e' insieme alla piattaforma.
    EmulatorKey
);
key_type!(
    /// Chiave di una lista di giochi, come `mame-0`.
    GamelistKey
);
key_type!(
    /// Nome della rom, cioe' il nome del file senza estensione.
    GameKey
);
key_type!(
    /// Nome di una cartella dentro `layouts/`.
    LayoutKey
);

/// Identifica un emulatore in modo univoco: la sua chiave **e** la piattaforma.
///
/// Serve perche' `mame` puo' esistere sia sotto `arcade` sia sotto `computer`,
/// e sono due emulatori diversi con liste diverse.

/// Orientamento richiesto dal gioco.
///
/// Serve a decidere quale variante del layout usare su un cabinato verticale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rotation {
    Horizontal,
    Vertical,
}

impl Rotation {
    /// Riconosce i valori del campo `ScreenOrientation` di FEEL 1.x.
    ///
    /// Ritorna `None` per i valori vuoti o non riconosciuti, invece di
    /// indovinare: un orientamento sbagliato mostra il gioco ruotato.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "horizontal" | "h" | "0" => Some(Rotation::Horizontal),
            "vertical" | "v" | "90" | "270" => Some(Rotation::Vertical),
            _ => None,
        }
    }
}

/// Istante senza fuso orario, nel formato che usa FEEL 1.x nel file `.sts`
/// (`dd/MM/yyyy HH:mm:ss`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

impl Timestamp {
    /// Legge il formato con cui questo tipo viene scritto: `AAAA-MM-GG hh:mm:ss`.
    ///
    /// È il formato che finisce nel database, quindi serve sia a scrivere sia a
    /// rileggere senza perdere l'istante.
    pub fn parse_iso(value: &str) -> Option<Self> {
        let (date, time) = value.trim().split_once(' ')?;

        let mut date_parts = date.split('-');
        let year = date_parts.next()?.parse::<i32>().ok()?;
        let month = date_parts.next()?.parse::<u8>().ok()?;
        let day = date_parts.next()?.parse::<u8>().ok()?;
        if date_parts.next().is_some() {
            return None;
        }

        let mut time_parts = time.split(':');
        let hour = time_parts.next()?.parse::<u8>().ok()?;
        let minute = time_parts.next()?.parse::<u8>().ok()?;
        let second = time_parts
            .next()
            .and_then(|part| part.parse::<u8>().ok())
            .unwrap_or(0);
        if time_parts.next().is_some() {
            return None;
        }

        if !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || hour > 23
            || minute > 59
            || second > 60
        {
            return None;
        }

        Some(Timestamp { year, month, day, hour, minute, second })
    }
}

/// Statistiche di gioco, dal file `.sts` di FEEL 1.x.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayStats {
    /// Quante volte il gioco e' stato avviato.
    pub count: u32,
    /// Tempo di gioco accumulato.
    pub total: Duration,
    /// Ultima partita. `None` se il file non lo riporta.
    pub last: Option<Timestamp>,
}

/// Dove stanno le immagini e i video di un emulatore.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtworkPaths {
    /// Immagini di gioco. Sono anche la sorgente dello screensaver.
    pub snapshot: Option<PathBuf>,
    /// Video snap.
    pub video: Option<PathBuf>,
    /// Immagini della plancia sopra lo schermo.
    pub marquee: Option<PathBuf>,
    /// Immagini della sagoma del cabinato.
    pub cabinet: Option<PathBuf>,
    /// Immagini dei comandi, mostrate accanto alle informazioni del gioco.
    pub control_panel: Option<PathBuf>,
}

/// Un raggruppamento di liste, come `arcade`, `console` o `computer`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platform {
    pub key: PlatformKey,
    /// Nome mostrato a video.
    pub title: String,
    /// Ordine di comparsa nell'elenco delle piattaforme.
    pub order: u32,
    /// Emulatori della piattaforma, nell'ordine dichiarato.
    pub emulators: Vec<EmulatorKey>,
}

/// Un emulatore e il modo in cui avviarlo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Emulator {
    pub key: EmulatorKey,
    pub platform: PlatformKey,
    pub title: String,
    /// Binario da eseguire.
    pub executable: PathBuf,
    /// Argomenti, con i segnaposto gia' tradotti in sintassi FEEL 2.0.
    pub arguments: Vec<String>,
    /// Cartella di lavoro. Su Windows serve a certi emulatori per trovare
    /// i propri file di supporto.
    pub working_directory: Option<PathBuf>,
    pub rom_path: PathBuf,
    /// Estensioni accettate, senza punto.
    pub rom_extensions: Vec<String>,
    pub artwork: ArtworkPaths,
    /// Layout da usare per questo emulatore.
    pub layout: Option<LayoutKey>,
    /// Liste dichiarate, nell'ordine.
    pub gamelists: Vec<GamelistKey>,
    /// Esegui le rom nelle sottocartelle di `rom_path`.
    pub recursive_roms: bool,
}

/// Una lista di giochi, come `All Games` o `Best Games`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gamelist {
    pub key: GamelistKey,
    pub emulator: EmulatorKey,
    pub platform: PlatformKey,
    pub title: String,
    pub order: u32,
}

/// Un gioco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    /// Nome della rom: identifica il gioco dentro la sua lista.
    pub key: GameKey,
    pub gamelist: GamelistKey,
    pub title: String,
    pub year: Option<String>,
    pub manufacturer: Option<String>,
    /// Rom da cui questa deriva, se e' un clone.
    pub clone_of: Option<GameKey>,
    pub bios: Option<String>,
    pub category: Option<String>,
    pub rotation: Option<Rotation>,
    /// Descrizione dei comandi, come `joy4way - 1P - 4Bt`.
    pub controls: Option<String>,
    /// Stato della rom secondo il listxml di MAME.
    pub status: Option<String>,
    /// Sottocartella della rom dentro `rom_path`, per gli emulatori che le
    /// organizzano in cartelle.
    pub relative_path: Option<PathBuf>,
}

impl Game {
    /// Il titolo "pulito": senza la parte tra parentesi.
    ///
    /// FEEL 1.x lo calcolava a runtime; qui e' disponibile quando serve.
    pub fn cleaned_title(&self) -> String {
        crate::text::cleanup_name(&self.title)
    }
}

/// Tutta la libreria, con gli indici per ritrovare le cose per chiave.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Library {
    pub platforms: Vec<Platform>,
    pub emulators: Vec<Emulator>,
    pub gamelists: Vec<Gamelist>,
    pub games: Vec<Game>,
    /// Statistiche, raggruppate per emulatore e poi per nome della rom.
    ///
    /// La chiave e' la coppia piattaforma/emulatore, non il solo emulatore:
    /// FEEL 1.x scrive un unico `data/<emu>.sts` per nome di emulatore, quindi
    /// due piattaforme con un emulatore omonimo si scambierebbero le partite.
    #[serde(default)]
    pub plays: HashMap<EmulatorRef, HashMap<GameKey, PlayStats>>,
}

impl Library {
    pub fn platform(&self, key: &PlatformKey) -> Option<&Platform> {
        self.platforms.iter().find(|p| &p.key == key)
    }

    pub fn emulator(&self, platform: &PlatformKey, key: &EmulatorKey) -> Option<&Emulator> {
        self.emulators
            .iter()
            .find(|e| &e.key == key && &e.platform == platform)
    }

    pub fn gamelist(&self, key: &GamelistKey) -> Option<&Gamelist> {
        self.gamelists.iter().find(|g| &g.key == key)
    }

    /// I giochi di una lista.
    ///
    /// La chiave viene copiata nell'iteratore invece di essere prestata: cosi'
    /// si puo' scrivere `lib.games_of(&GamelistKey::new("mame-0"))` senza dover
    /// tenere in vita una variabile solo per l'iterazione.
    pub fn games_of(&self, gamelist: &GamelistKey) -> impl Iterator<Item = &Game> + '_ {
        let gamelist = gamelist.clone();
        self.games.iter().filter(move |g| g.gamelist == gamelist)
    }

    pub fn stats(
        &self,
        platform: &PlatformKey,
        emulator: &EmulatorKey,
        game: &GameKey,
    ) -> Option<&PlayStats> {
        self.plays
            .get(&(platform.clone(), emulator.clone()))
            .and_then(|by_game| by_game.get(game))
    }

    /// Le statistiche di un gioco, risalendo a piattaforma ed emulatore dalla sua lista.
    pub fn stats_of(&self, game: &Game) -> Option<&PlayStats> {
        let gamelist = self.gamelist(&game.gamelist)?;
        self.stats(&gamelist.platform, &gamelist.emulator, &game.key)
    }

    /// I giochi, in ordine alfabetico per titolo.
    ///
    /// L'ordinamento vero sara' configurabile (anno, produttore, categoria);
    /// questo e' il default di FEEL 1.x.
    pub fn sorted_by_title(&self) -> Vec<&Game> {
        let mut out: Vec<&Game> = self.games.iter().collect();
        out.sort_by(|a, b| {
            a.title
                .to_ascii_lowercase()
                .cmp(&b.title.to_ascii_lowercase())
                .then_with(|| a.key.cmp(&b.key))
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_chiavi_non_sono_intercambiabili() {
        // Non compila se si prova a passare una EmulatorKey dove serve una
        // PlatformKey: e' il motivo per cui questi tipi esistono.
        let p = PlatformKey::new("arcade");
        let e = EmulatorKey::from("mame");
        assert_eq!(p.as_str(), "arcade");
        assert_eq!(e.to_string(), "mame");
        assert_ne!(p, PlatformKey::new("console"));
    }

    #[test]
    fn riconosce_l_orientamento() {
        assert_eq!(Rotation::parse("Horizontal"), Some(Rotation::Horizontal));
        assert_eq!(Rotation::parse(" vertical "), Some(Rotation::Vertical));
        assert_eq!(Rotation::parse("270"), Some(Rotation::Vertical));
        // Sconosciuto o vuoto: nessuna ipotesi.
        assert_eq!(Rotation::parse(""), None);
        assert_eq!(Rotation::parse("cocktail"), None);
    }

    #[test]
    fn formatta_il_timestamp() {
        let t = Timestamp {
            year: 2018,
            month: 3,
            day: 12,
            hour: 21,
            minute: 14,
            second: 0,
        };
        assert_eq!(t.to_string(), "2018-03-12 21:14:00");
    }

    #[test]
    fn rilegge_il_timestamp_che_scrive() {
        let originale = Timestamp {
            year: 2018,
            month: 3,
            day: 12,
            hour: 21,
            minute: 14,
            second: 7,
        };
        assert_eq!(Timestamp::parse_iso(&originale.to_string()), Some(originale));

        // Valori fuori intervallo non producono un istante inventato.
        assert_eq!(Timestamp::parse_iso("2018-13-01 00:00:00"), None);
        assert_eq!(Timestamp::parse_iso("2018-03-12 25:00:00"), None);
        assert_eq!(Timestamp::parse_iso("non una data"), None);
        assert_eq!(Timestamp::parse_iso("2018-03-12"), None);
    }

    fn libreria_di_prova() -> Library {
        let gamelist = GamelistKey::new("mame-0");
        Library {
            platforms: vec![Platform {
                key: PlatformKey::new("arcade"),
                title: "Arcade".into(),
                order: 0,
                emulators: vec![EmulatorKey::new("mame")],
            }],
            emulators: vec![],
            gamelists: vec![Gamelist {
                key: gamelist.clone(),
                emulator: EmulatorKey::new("mame"),
                platform: PlatformKey::new("arcade"),
                title: "All Games".into(),
                order: 0,
            }],
            games: vec![
                Game {
                    key: GameKey::new("pacman"),
                    gamelist: gamelist.clone(),
                    title: "Pac-Man (Midway)".into(),
                    year: Some("1980".into()),
                    manufacturer: Some("Namco".into()),
                    clone_of: None,
                    bios: None,
                    category: Some("Maze / Collect".into()),
                    rotation: Some(Rotation::Horizontal),
                    controls: None,
                    status: None,
                    relative_path: None,
                },
                Game {
                    key: GameKey::new("puckman"),
                    gamelist,
                    title: "Puck Man (Japan set 1)".into(),
                    year: None,
                    manufacturer: None,
                    clone_of: Some(GameKey::new("pacman")),
                    bios: None,
                    category: None,
                    rotation: None,
                    controls: None,
                    status: None,
                    relative_path: None,
                },
            ],
            plays: HashMap::new(),
        }
    }

    #[test]
    fn trova_per_chiave() {
        let lib = libreria_di_prova();
        assert_eq!(lib.platform(&PlatformKey::new("arcade")).unwrap().title, "Arcade");
        assert!(lib.platform(&PlatformKey::new("nope")).is_none());
        assert_eq!(lib.games_of(&GamelistKey::new("mame-0")).count(), 2);
    }

    #[test]
    fn ordina_per_titolo_ignorando_le_maiuscole() {
        let lib = libreria_di_prova();
        let ordinati: Vec<&str> = lib
            .sorted_by_title()
            .iter()
            .map(|g| g.key.as_str())
            .collect();
        assert_eq!(ordinati, vec!["pacman", "puckman"]);
    }

    #[test]
    fn pulisce_il_titolo() {
        let lib = libreria_di_prova();
        assert_eq!(lib.games[0].cleaned_title(), "Pac-Man");
        assert_eq!(lib.games[1].cleaned_title(), "Puck Man");
    }
}
