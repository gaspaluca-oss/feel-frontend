//! Importazione di una installazione FEEL 1.x.
//!
//! Legge l'albero di FEEL 1.x e lo trasforma nel modello di [`feel_core`]:
//!
//! ```text
//! config/feel.ini                        -> Feel1Settings
//! config/<platform>/<platform>.ini       -> Platform
//! config/<platform>/<emu>/<emu>.ini      -> Emulator
//! config/<platform>/<emu>/<emu>-N.ini    -> Gamelist
//! data/<gamelist>.lst                    -> Game
//! data/<emu>.sts                         -> PlayStats
//! ```
//!
//! Regola di fondo: **non indovinare**. Quando un dato non e' interpretabile
//! finisce in [`Feel1Report`] e l'importazione va avanti. Un importatore che
//! "aggiusta" in silenzio produce librerie sbagliate che l'utente scopre
//! settimane dopo, sul cabinato.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use feel_core::{
    ArtworkPaths, Emulator, EmulatorKey, Game, GameKey, Gamelist, GamelistKey, LayoutKey, Library,
    Platform, PlatformKey, PlayStats, Rotation, Timestamp,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ini::IniMap;
use crate::paths::PathRules;

/// I campi di un record `.lst`, nell'ordine in cui FEEL 1.x li scrive.
///
/// Vedi `CRomManager.SaveRomList` in FEEL 1.x: sono tredici righe per gioco,
/// e i campi vuoti sono righe vuote. Un record non ha un terminatore, quindi
/// la fine si riconosce solo dalla fine del file.
const LST_FIELDS: usize = 13;

/// Impostazioni globali, da `config/feel.ini`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Feel1Settings {
    /// Ordine in cui le piattaforme compaiono nel frontend.
    pub platform_order: Vec<PlatformKey>,
    /// Cabinato verticale: lo schermo va ruotato.
    pub rotate_screen: bool,
    /// Cambia la risoluzione in base al layout.
    pub switch_resolution: bool,
    pub use_joypad: bool,
    pub screen_saver_enabled: bool,
    pub screen_saver_delay_secs: u32,
    pub screen_saver_slide_secs: u32,
    /// Secondi di attesa prima di riprodurre il video snap.
    pub video_delay_secs: u32,
    pub video_volume: u32,
    pub sound_volume: u32,
    pub music_volume: u32,
    /// Comando del frontend -> tasto, come lo dichiara FEEL 1.x.
    pub keymap: BTreeMap<String, String>,
    /// Layout indicato da `feel.ini`.
    pub default_layout: Option<LayoutKey>,
}

/// Cosa non e' andato liscio, e cosa va controllato a mano.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Feel1Report {
    pub warnings: Vec<String>,
    /// Sottocartelle trovate dentro `rom_path`: l'emulatore deve cercare le rom
    /// anche sotto, altrimenti quei giochi non partono.
    pub nested_rom_paths: BTreeSet<String>,
    /// Estensioni rom incontrate e non dichiarate nella configurazione.
    pub undeclared_extensions: BTreeSet<String>,
}

impl Feel1Report {
    pub fn warn(&mut self, message: impl Into<String>) {
        let message = message.into();
        if !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }

    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
            && self.nested_rom_paths.is_empty()
            && self.undeclared_extensions.is_empty()
    }
}

/// Come interpretare la configurazione trovata.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Riscritture di percorso, per chi ha spostato le rom. Vuoto = nessuna.
    pub path_rules: PathRules,
}

/// Una installazione FEEL 1.x letta.
#[derive(Debug, Clone)]
pub struct Feel1Import {
    /// Cartella da cui e' stato letto tutto.
    pub source: PathBuf,
    pub settings: Feel1Settings,
    pub library: Library,
    pub report: Feel1Report,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("non trovo feel.ini in {0} ne' in {0}/config")]
    MissingFeelIni(PathBuf),
    #[error("errore di lettura di {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Importa una installazione senza riscritture di percorso.
pub fn load(root: impl AsRef<Path>) -> Result<Feel1Import, ImportError> {
    load_with(root, &ImportOptions::default())
}

/// Importa una installazione applicando le opzioni indicate.
pub fn load_with(root: impl AsRef<Path>, options: &ImportOptions) -> Result<Feel1Import, ImportError> {
    let root = root.as_ref();
    let config_dir = resolve_config_dir(root)?;
    let data_dir = resolve_data_dir(root);

    let mut importer = Importer {
        options,
        library: Library::default(),
        report: Feel1Report::default(),
        data_dir,
    };

    let settings = importer.read_settings(&config_dir)?;

    for (order, key) in settings.platform_order.clone().into_iter().enumerate() {
        importer.import_platform(&config_dir, &key, order as u32);
    }

    importer.finish();

    Ok(Feel1Import {
        source: root.to_path_buf(),
        settings,
        library: importer.library,
        report: importer.report,
    })
}

struct Importer<'a> {
    options: &'a ImportOptions,
    library: Library,
    report: Feel1Report,
    data_dir: Option<PathBuf>,
}

impl Importer<'_> {
    // -- percorsi ---------------------------------------------------------------

    fn resolve(&self, raw: &str) -> Option<PathBuf> {
        let translated = self.options.path_rules.translate(raw);
        if translated.is_empty() {
            None
        } else {
            Some(PathBuf::from(translated))
        }
    }

    // -- impostazioni -----------------------------------------------------------

    fn read_settings(&mut self, config_dir: &Path) -> Result<Feel1Settings, ImportError> {
        let path = config_dir.join("feel.ini");
        let ini = IniMap::from_file(&path).map_err(|source| ImportError::Io { path, source })?;

        let keymap = ini_keymap(&ini);

        Ok(Feel1Settings {
            platform_order: ini
                .list("platform_list")
                .into_iter()
                .map(PlatformKey::new)
                .collect(),
            rotate_screen: ini.flag("rotate_screen", false),
            switch_resolution: ini.flag("switch_res", true),
            use_joypad: ini.flag("use_joypad", false),
            screen_saver_enabled: ini.flag("screen_saver_enabled", false),
            screen_saver_delay_secs: ini.int("screen_saver_delay", 30).max(0) as u32,
            screen_saver_slide_secs: ini.int("screen_saver_slide_time", 5).max(0) as u32,
            video_delay_secs: ini.int("video_delay", 2).max(0) as u32,
            video_volume: ini.int("video_volume", 30).clamp(0, 100) as u32,
            sound_volume: ini.int("sound_fx_volume", 80).clamp(0, 100) as u32,
            music_volume: ini.int("music_volume", 60).clamp(0, 100) as u32,
            keymap,
            default_layout: ini.non_empty("current_layout").map(LayoutKey::new),
        })
    }

    // -- piattaforme ------------------------------------------------------------

    fn import_platform(&mut self, config_dir: &Path, key: &PlatformKey, order: u32) {
        let dir = config_dir.join(key.as_str());
        let ini = IniMap::from_file(dir.join(format!("{}.ini", key))).unwrap_or_default();

        let title = ini
            .non_empty("platform_title")
            .unwrap_or_else(|| key.as_str().to_owned());
        let emulator_keys: Vec<EmulatorKey> =
            ini.list("emulator_list").into_iter().map(EmulatorKey::new).collect();

        if emulator_keys.is_empty() {
            self.report.warn(format!(
                "piattaforma '{key}': nessun emulatore dichiarato in {}.ini",
                key
            ));
        }

        self.library.platforms.push(Platform {
            key: key.clone(),
            title,
            order,
            emulators: emulator_keys.clone(),
        });

        for emulator_key in &emulator_keys {
            self.import_emulator(config_dir, key, emulator_key);
        }
    }

    // -- emulatori --------------------------------------------------------------

    fn import_emulator(&mut self, config_dir: &Path, platform: &PlatformKey, key: &EmulatorKey) {
        let dir = config_dir.join(platform.as_str()).join(key.as_str());
        let ini_path = dir.join(format!("{}.ini", key));

        let ini = match IniMap::from_file(&ini_path) {
            Ok(ini) => ini,
            Err(source) => {
                self.report.warn(format!("{key}: non riesco a leggere {ini_path:?} ({source})"));
                return;
            }
        };

        if ini.is_empty() {
            self.report.warn(format!(
                "emulatore '{platform}/{key}': file {key}.ini assente o vuoto"
            ));
        }

        let title = ini.non_empty("emulator_title").unwrap_or_else(|| key.as_str().to_owned());

        // `emulator_commandline` e' l'eseguibile. In FEEL 1.x il segnaposto
        // `[emulator_path]` qui e' il percorso concreto dell'emulatore, quindi si
        // sostituisce subito: l'eseguibile va risolto, non rimandato.
        let raw_commandline = ini.string("emulator_commandline");
        let raw_emulator_path = ini.string("emulator_path");
        let resolved_commandline =
            replace_ignore_case(&raw_commandline, "[emulator_path]", raw_emulator_path.trim());
        let executable = self
            .resolve(&resolved_commandline)
            .unwrap_or_else(|| PathBuf::from(key.as_str()));
        if raw_commandline.trim().is_empty() {
            self.report.warn(format!(
                "emulatore '{platform}/{key}': emulator_commandline vuoto, non so cosa avviare"
            ));
        } else if raw_commandline.split_whitespace().count() > 1 {
            self.report.warn(format!(
                "emulatore '{platform}/{key}': emulator_commandline contiene piu' di un \
                 elemento ({raw_commandline:?}): probabile wrapper, da controllare a mano"
            ));
        }

        let arguments = translate_arguments(&ini.string("emulator_arguments"));
        if arguments.is_empty() {
            self.report.warn(format!(
                "emulatore '{platform}/{key}': emulator_arguments vuoto, non verra' passata \
                 nessuna rom"
            ));
        }

        let rom_path = self
            .resolve(&ini.string("rom_path"))
            .unwrap_or_else(|| PathBuf::from(key.as_str()));

        let mut rom_extensions = ini.list("rom_extension");
        for extension in &mut rom_extensions {
            *extension = extension.trim_start_matches('.').to_ascii_lowercase();
        }
        if rom_extensions.is_empty() {
            rom_extensions.push("zip".to_owned());
        }

        let gamelist_keys: Vec<GamelistKey> = {
            let declared = ini.list("gamelist_list");
            if declared.is_empty() {
                // FEEL 1.x crea sempre una lista "<emu>-0" quando il campo manca.
                vec![GamelistKey::new(format!("{key}-0"))]
            } else {
                declared.into_iter().map(GamelistKey::new).collect()
            }
        };

        let artwork = ArtworkPaths {
            snapshot: self.resolve(&ini.string("snapshot_path")),
            video: self.resolve(&ini.string("video_path")),
            marquee: self.resolve(&ini.string("marquee_path")),
            cabinet: self.resolve(&ini.string("cabinet_path")),
            control_panel: None,
        };

        let layout = ini
            .non_empty("current_layout")
            .map(LayoutKey::new);

        self.library.emulators.push(Emulator {
            key: key.clone(),
            platform: platform.clone(),
            title,
            executable,
            arguments,
            working_directory: self.resolve(&ini.string("emulator_path")),
            rom_path: rom_path.clone(),
            rom_extensions,
            artwork,
            layout,
            gamelists: gamelist_keys.clone(),
            // Dedotto a fine importazione: serve aver visto le rom.
            recursive_roms: false,
        });

        for (order, gamelist_key) in gamelist_keys.iter().enumerate() {
            self.import_gamelist(&dir, platform, key, gamelist_key, order as u32);
        }

        self.import_stats(platform, key);
    }

    // -- liste ------------------------------------------------------------------

    fn import_gamelist(
        &mut self,
        emulator_dir: &Path,
        platform: &PlatformKey,
        emulator: &EmulatorKey,
        key: &GamelistKey,
        order: u32,
    ) {
        let ini = IniMap::from_file(emulator_dir.join(format!("{}.ini", key))).unwrap_or_default();
        let title = ini
            .non_empty("gamelist_title")
            .unwrap_or_else(|| key.as_str().to_owned());

        self.library.gamelists.push(Gamelist {
            key: key.clone(),
            emulator: emulator.clone(),
            platform: platform.clone(),
            title,
            order,
        });

        self.import_games(key);
    }

    fn import_games(&mut self, gamelist: &GamelistKey) {
        let Some(data_dir) = self.data_dir.clone() else {
            self.report.warn(format!(
                "lista '{gamelist}': cartella 'data' non trovata, nessuna rom importata"
            ));
            return;
        };

        let path = data_dir.join(format!("{}.lst", gamelist));
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.report.warn(format!(
                    "lista '{gamelist}': {}.lst assente, la lista resta vuota",
                    gamelist
                ));
                return;
            }
            Err(source) => {
                self.report
                    .warn(format!("lista '{gamelist}': errore di lettura ({source})"));
                return;
            }
        };

        for record in parse_romlist(&text) {
            if record.name.is_empty() {
                continue;
            }

            if let Some(relative) = record.relative_path() {
                self.report.nested_rom_paths.insert(relative.to_owned());
            }

            self.library.games.push(Game {
                key: GameKey::new(&record.name),
                gamelist: gamelist.clone(),
                title: if record.title.is_empty() {
                    record.name.clone()
                } else {
                    record.title.clone()
                },
                year: optional(&record.year),
                manufacturer: optional(&record.manufacturer),
                clone_of: optional(&record.clone_of).map(GameKey::new),
                bios: optional(&record.bios),
                category: optional(&record.category),
                rotation: Rotation::parse(&record.rotation),
                controls: optional(&record.control),
                status: optional(&record.status),
                relative_path: record.relative_path().map(PathBuf::from),
            });
        }
    }

    // -- statistiche ------------------------------------------------------------

    fn import_stats(&mut self, platform: &PlatformKey, emulator: &EmulatorKey) {
        let Some(data_dir) = self.data_dir.clone() else {
            return;
        };

        let path = data_dir.join(format!("{}.sts", emulator));
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(source) => {
                self.report.warn(format!(
                    "statistiche di '{emulator}': errore di lettura ({source})"
                ));
                return;
            }
        };

        let stats = parse_stats(&text);
        if !stats.is_empty() {
            self.library
                .plays
                .insert((platform.clone(), emulator.clone()), stats);
        }
    }

    // -- chiusura ---------------------------------------------------------------

    fn finish(&mut self) {
        // La ricorsione non e' un campo della configurazione di FEEL 1.x: si
        // deduce dal fatto che qualche rom stia in una sottocartella.
        if !self.report.nested_rom_paths.is_empty() {
            for emulator in &mut self.library.emulators {
                emulator.recursive_roms = true;
            }
        }

        // FEEL 1.x nomina le statistiche `data/<emu>.sts`, senza la piattaforma:
        // due piattaforme con un emulatore omonimo condividevano lo stesso file.
        // Qui non succede, ma l'utente deve saperlo.
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut duplicated: BTreeSet<String> = BTreeSet::new();
        for emulator in &self.library.emulators {
            if !seen.insert(emulator.key.as_str()) {
                duplicated.insert(emulator.key.to_string());
            }
        }
        for name in duplicated {
            self.report.warn(format!(
                "emulatore '{name}' compare in piu' di una piattaforma: in FEEL 1.x le loro \
                 statistiche erano lo stesso file, qui restano separate"
            ));
        }

        let total: usize = self.library.games.len();
        if total == 0 {
            self.report.warn(
                "nessun gioco importato: controlla che la cartella 'data' contenga i file .lst"
                    .to_owned(),
            );
        }
    }
}

// ----------------------------------------------------------------------------------
// Parsing dei file di dati di FEEL 1.x
// ----------------------------------------------------------------------------------

/// Un record grezzo di `.lst`, come lo scrive FEEL 1.x.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LstRecord {
    pub name: String,
    pub title: String,
    pub year: String,
    pub manufacturer: String,
    pub clone_of: String,
    pub bios: String,
    /// `VideoType§RomRelativePath`
    pub extra_data: String,
    pub rotation: String,
    pub control: String,
    pub status: String,
    pub color: String,
    pub sound: String,
    pub category: String,
}

impl LstRecord {
    /// La sottocartella dentro `rom_path`, se la rom non sta nella radice.
    fn relative_path(&self) -> Option<&str> {
        let value = self.extra_data.split('§').nth(1)?.trim();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }
}

/// Legge `.lst` nei due formati che FEEL 1.x produce.
///
/// - lista normale: 13 righe per gioco, campi vuoti = righe vuote
/// - `all_emu`: una riga per gioco, campi separati da `|`
pub fn parse_romlist(text: &str) -> Vec<LstRecord> {
    let lines: Vec<&str> = text.lines().map(|line| line.trim_end_matches('\r')).collect();

    if lines.first().map_or(false, |line| line.matches('|').count() >= LST_FIELDS - 1) {
        return lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| record_from_slice(&line.split('|').collect::<Vec<_>>()))
            .collect();
    }

    lines
        .chunks(LST_FIELDS)
        .filter(|chunk| chunk.len() == LST_FIELDS)
        .map(record_from_slice)
        .collect()
}

fn record_from_slice(fields: &[&str]) -> LstRecord {
    let get = |index: usize| {
        fields
            .get(index)
            .map(|value| value.trim().to_owned())
            .unwrap_or_default()
    };
    LstRecord {
        name: get(0),
        title: get(1),
        year: get(2),
        manufacturer: get(3),
        clone_of: get(4),
        bios: get(5),
        extra_data: get(6),
        rotation: get(7),
        control: get(8),
        status: get(9),
        color: get(10),
        sound: get(11),
        category: get(12),
    }
}

/// Legge `.sts`: `nome|partite|tempo|ultima partita`.
///
/// Una riga malformata viene scartata: e' un file di statistiche, non vale la
/// pena far fallire l'importazione per una riga rotta.
pub fn parse_stats(text: &str) -> HashMap<GameKey, PlayStats> {
    let mut out = HashMap::new();

    for line in text.lines() {
        let fields: Vec<&str> = line.trim_end_matches('\r').split('|').collect();
        if fields.len() < 3 || fields[0].trim().is_empty() {
            continue;
        }

        let count = fields[1].trim().parse::<u32>().unwrap_or(0);
        let total = parse_duration(fields[2]).unwrap_or_default();
        let last = fields.get(3).and_then(|value| parse_timestamp(value));

        out.insert(GameKey::new(fields[0].trim()), PlayStats { count, total, last });
    }

    out
}

/// Legge un `TimeSpan` di .NET: `[-][g.]hh:mm:ss[.fffffff]`.
pub fn parse_duration(value: &str) -> Option<Duration> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let (value, negative) = match value.strip_prefix('-') {
        Some(rest) => (rest, true),
        None => (value, false),
    };

    let (days, value) = match value.split_once('.') {
        Some((days, rest)) => (days.parse::<u64>().ok()?, rest),
        None => (0, value),
    };

    let parts: Vec<&str> = value.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [h, m, s] => (h.parse::<u64>().ok()?, m.parse::<u64>().ok()?, s.parse::<f64>().ok()?),
        [m, s] => (0, m.parse::<u64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };

    if negative {
        // I tempi di gioco negativi non esistono: li trattiamo come zero
        // invece di produrre un panico su una sottrazione.
        return Some(Duration::ZERO);
    }

    let whole = days * 86_400 + hours * 3_600 + minutes * 60;
    Some(Duration::from_secs(whole) + Duration::from_secs_f64(seconds))
}

/// Legge `dd/MM/yyyy HH:mm:ss`, il formato delle date in `.sts`.
pub fn parse_timestamp(value: &str) -> Option<Timestamp> {
    let value = value.trim();
    let (date, time) = value.split_once(' ')?;

    let mut date_parts = date.split('/');
    let day = date_parts.next()?.parse::<u8>().ok()?;
    let month = date_parts.next()?.parse::<u8>().ok()?;
    let year = date_parts.next()?.parse::<i32>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u8>().ok()?;
    let minute = time_parts.next()?.parse::<u8>().ok()?;
    let second = time_parts.next().and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60
    {
        return None;
    }

    Some(Timestamp { year, month, day, hour, minute, second })
}

// ----------------------------------------------------------------------------------
// Argomenti
// ----------------------------------------------------------------------------------

/// Segnaposto di FEEL 1.x tradotti nella sintassi di FEEL 2.0.
const ARG_TOKENS: [(&str, &str); 6] = [
    ("[rom_path]", "[rompath]"),
    ("[rom_name]", "[name]"),
    ("[rom_extension]", "[extension]"),
    ("[full_path]", "[romfile]"),
    ("[full_dos_path]", "[romfile]"),
    ("[emulator_path]", "[emulatorpath]"),
];

/// Una mappatura di FEEL comando -> tasto.
fn ini_keymap(ini: &IniMap) -> BTreeMap<String, String> {
    ini.iter()
        .filter(|(key, _)| KEYMAP_COMMANDS.contains(&key.as_str()))
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(key, value)| (key.to_string(), value.trim().to_owned()))
        .collect()
}

/// I comandi di input che FEEL 1.x dichiara in `feel.ini`.
const KEYMAP_COMMANDS: &[&str] = &[
    "action_key",
    "menu_key",
    "menu_ok",
    "menu_cancel",
    "menu_up",
    "menu_down",
    "menu_left",
    "menu_right",
    "previous_game",
    "next_game",
    "previous_letter_game",
    "next_letter_game",
    "previous_page",
    "next_page",
    "find_game",
    "select_platform",
    "select_emulator",
    "select_gamelist",
    "previous_platform",
    "next_platform",
    "previous_emulator",
    "next_emulator",
    "previous_gamelist",
    "next_gamelist",
    "add_game_to_list",
    "remove_game_from_list",
    "show_custom_image",
    "show_game_info",
    "show_exit_menu",
    "screensaver_previous_game",
    "screensaver_next_game",
    "next_sort",
    "reload_config",
];

/// Traduce `emulator_arguments` di FEEL 1.x in una lista di argomenti.
///
/// Il carattere `#` da solo significa "nessun argomento", come in FEEL 1.x.
pub fn translate_arguments(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "#" {
        return Vec::new();
    }

    translate_tokens(trimmed)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// Traduce i segnaposto di FEEL 1.x in quelli di FEEL 2.0, lasciando il resto.
///
/// Negli argomenti `[emulator_path]` **non** viene risolto: li' e' un template
/// che il motore riempie al momento dell'avvio.
pub fn translate_tokens(raw: &str) -> String {
    let mut translated = raw.to_owned();
    for (from, to) in ARG_TOKENS {
        translated = replace_ignore_case(&translated, from, to);
    }
    translated
}

/// Sostituzione che ignora maiuscole e minuscole.
///
/// `str::replace` non ha una variante case-insensitive perche' non puo' esserlo
/// in generale: qui i segnaposto sono ASCII, quindi possiamo farlo noi.
fn replace_ignore_case(haystack: &str, needle: &str, replacement: &str) -> String {
    let lower_haystack = haystack.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut cursor = 0;

    while let Some(offset) = lower_haystack[cursor..].find(&lower_needle) {
        let start = cursor + offset;
        out.push_str(&haystack[cursor..start]);
        out.push_str(replacement);
        cursor = start + needle.len();
    }

    out.push_str(&haystack[cursor..]);
    out
}

fn optional(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

// ----------------------------------------------------------------------------------
// Scoperta delle cartelle
// ----------------------------------------------------------------------------------

/// Trova la cartella che contiene `feel.ini`.
///
/// FEEL 1.x la mette in `config/`, ma in giro si trovano anche installazioni
/// con i file nella radice.
fn resolve_config_dir(root: &Path) -> Result<PathBuf, ImportError> {
    let candidate = root.join("config");
    if candidate.join("feel.ini").is_file() {
        return Ok(candidate);
    }
    if root.join("feel.ini").is_file() {
        return Ok(root.to_path_buf());
    }
    Err(ImportError::MissingFeelIni(root.to_path_buf()))
}

/// Trova la cartella `data`, anche quando sta dentro l'output di build.
fn resolve_data_dir(root: &Path) -> Option<PathBuf> {
    [
        root.join("data"),
        root.join("bin").join("x86").join("Release").join("data"),
        root.join("bin").join("x86").join("Debug").join("data"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_dir())
}
