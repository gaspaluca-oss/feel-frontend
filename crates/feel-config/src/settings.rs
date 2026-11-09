//! Le impostazioni di FEEL 2.0.
//!
//! FEEL 1.x configurava tutto in una ventina di file `.ini` sparsi, con chiavi
//! in colonna e valori separati da spazi. Funzionava, ma per cambiare una cosa
//! bisognava sapere in quale dei venti file stava.
//!
//! Qui c'e' **un solo file**, con una struttura leggibile, e i valori
//! predefiniti sono quelli giusti per un cabinato: chi non lo tocca ha
//! comunque un frontend configurato.
//!
//! Il file non e' pensato per essere scritto a mano tutte le volte: i comandi
//! `feel config get/set` e, in futuro, la schermata di impostazioni nel
//! frontend servono a questo. Ma resta leggibile per chi vuole guardarci
//! dentro, ed e' JSON e non un formato binario proprio per questo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use feel_core::Shader;
use serde::{Deserialize, Serialize};

/// Versione del formato delle impostazioni.
pub const SETTINGS_VERSION: u32 = 1;

/// Nome del file, accanto al database.
pub const FILE_NAME: &str = "feel.json";

/// Come e' ruotato lo schermo del cabinato.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Rotate {
    /// Schermo orizzontale, il caso normale.
    #[default]
    None,
    /// Cabinato verticale, schermo ruotato a sinistra.
    Left,
    /// Cabinato verticale, schermo ruotato a destra.
    Right,
}

impl Rotate {
    pub fn ruota(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Left => Some("left"),
            Self::Right => Some("right"),
        }
    }
}

/// Impostazioni dell'immagine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoSettings {
    /// Finestra intera invece che finestra normale.
    pub fullscreen: bool,
    /// Effetto applicato all'immagine finale.
    pub shader: Shader,
    /// Rotazione per i cabinati verticali.
    pub rotate: Rotate,
    /// Sincronizza con il refresh del monitor.
    pub vsync: bool,
    /// Mostra il fotogramma al secondo, per capire se il cabinato ce la fa.
    pub show_fps: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            fullscreen: true,
            // Su un cabinato il CRT e' la scelta giusta; su un LCD il valore
            // predefinito "nessuno" e' piu' sicuro, ma il cabinato e' il caso
            // per cui questo programma esiste.
            shader: Shader::Crt,
            rotate: Rotate::None,
            vsync: true,
            show_fps: false,
        }
    }
}

/// Impostazioni dell'audio, in percentuale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Volume generale.
    pub master: u8,
    /// Musica di sottofondo del frontend.
    pub music: u8,
    /// Effetti dei menu.
    pub effects: u8,
    /// Volume dei video snap.
    pub video: u8,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { master: 80, music: 50, effects: 100, video: 30 }
    }
}

/// Lo screensaver, cioe' la modalita' attrattiva del cabinato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenSaverSettings {
    pub enabled: bool,
    /// Secondi di inattivita' prima di partire.
    pub delay_secs: u32,
    /// Secondi che ogni gioco resta a video.
    pub slide_secs: u32,
    /// Passa ai video invece che alle immagini.
    pub use_videos: bool,
}

impl Default for ScreenSaverSettings {
    fn default() -> Self {
        Self { enabled: true, delay_secs: 60, slide_secs: 8, use_videos: true }
    }
}

/// I comandi del frontend e il tasto che li attiva.
///
/// I nomi dei tasti sono quelli di FEEL 1.x (`Up`, `LAlt`, `ESC`), cosi' chi
/// arriva dal vecchio frontend non deve imparare un'altra nomenclatura.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputSettings {
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
    /// Avvia il gioco selezionato.
    pub select: String,
    /// Torna indietro / apre il menu.
    pub back: String,
    /// Pagina successiva della lista.
    pub page_up: String,
    pub page_down: String,
    /// Cambia il filtro: tutti, preferiti, categorie.
    #[serde(default = "default_filter")]
    pub filter: String,
    /// Segna o toglie il gioco dai preferiti.
    #[serde(default = "default_favourite")]
    pub favourite: String,
}

fn default_filter() -> String {
    "Tab".into()
}

fn default_favourite() -> String {
    "F2".into()
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            up: "Up".into(),
            down: "Down".into(),
            left: "Left".into(),
            right: "Right".into(),
            select: "Return".into(),
            back: "Escape".into(),
            page_up: "PageUp".into(),
            page_down: "PageDown".into(),
            filter: default_filter(),
            favourite: default_favourite(),
        }
    }
}

impl InputSettings {
    /// Tutte le associazioni, per mostrarle o confrontarle.
    pub fn tutte(&self) -> BTreeMap<&'static str, &str> {
        let mut mappa = BTreeMap::new();
        mappa.insert("su", self.up.as_str());
        mappa.insert("giu", self.down.as_str());
        mappa.insert("sinistra", self.left.as_str());
        mappa.insert("destra", self.right.as_str());
        mappa.insert("avvia", self.select.as_str());
        mappa.insert("indietro", self.back.as_str());
        mappa.insert("pagina_su", self.page_up.as_str());
        mappa.insert("pagina_giu", self.page_down.as_str());
        mappa.insert("filtro", self.filter.as_str());
        mappa.insert("preferito", self.favourite.as_str());
        mappa
    }
}

/// Tutte le impostazioni di FEEL 2.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    #[serde(default)]
    pub video: VideoSettings,
    #[serde(default)]
    pub audio: AudioSettings,
    #[serde(default)]
    pub screensaver: ScreenSaverSettings,
    #[serde(default)]
    pub input: InputSettings,
    /// Percorso di un font specifico. `None` = cerca fra quelli di sistema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<PathBuf>,
    /// Layout predefinito, se non si vuole passarlo da riga di comando.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            screensaver: ScreenSaverSettings::default(),
            input: InputSettings::default(),
            font: None,
            layout: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("non riesco a leggere {path}: {source}")]
    Lettura {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} non e' un file di impostazioni valido: {source}")]
    Formato {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

impl Settings {
    /// Legge le impostazioni, oppure crea il file con i valori predefiniti.
    ///
    /// Crearlo invece di limitarsi a usare i valori in memoria e' voluto: la
    /// prima cosa che fa un utente e' cercare il file per cambiare qualcosa, e
    /// deve trovarlo.
    pub fn load_or_create(percorso: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let percorso = percorso.as_ref();

        match std::fs::read_to_string(percorso) {
            Ok(testo) => serde_json::from_str(&testo).map_err(|source| SettingsError::Formato {
                path: percorso.to_path_buf(),
                source,
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let impostazioni = Self::default();
                impostazioni.save(percorso)?;
                Ok(impostazioni)
            }
            Err(source) => Err(SettingsError::Lettura {
                path: percorso.to_path_buf(),
                source,
            }),
        }
    }

    /// Legge le impostazioni, o i valori predefiniti se il file non c'e'.
    ///
    /// Non scrive niente: serve ai comandi che devono solo mostrarle.
    pub fn load(percorso: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let percorso = percorso.as_ref();
        match std::fs::read_to_string(percorso) {
            Ok(testo) => serde_json::from_str(&testo).map_err(|source| SettingsError::Formato {
                path: percorso.to_path_buf(),
                source,
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(SettingsError::Lettura {
                path: percorso.to_path_buf(),
                source,
            }),
        }
    }

    pub fn save(&self, percorso: impl AsRef<Path>) -> Result<(), SettingsError> {
        let percorso = percorso.as_ref();
        let testo = serde_json::to_string_pretty(self).map_err(|source| SettingsError::Formato {
            path: percorso.to_path_buf(),
            source,
        })?;

        if let Some(cartella) = percorso.parent() {
            if !cartella.as_os_str().is_empty() {
                std::fs::create_dir_all(cartella).map_err(|source| SettingsError::Lettura {
                    path: percorso.to_path_buf(),
                    source,
                })?;
            }
        }

        std::fs::write(percorso, testo + "\n").map_err(|source| SettingsError::Lettura {
            path: percorso.to_path_buf(),
            source,
        })
    }

    /// Cambia un valore per nome, come `video.shader = crt`.
    ///
    /// E' il motore dietro `feel config set`: evita di dover aprire il file e
    /// di dover ricordare la struttura.
    pub fn set(&mut self, chiave: &str, valore: &str) -> Result<(), String> {
        let chiave = chiave.trim().to_ascii_lowercase();
        match chiave.as_str() {
            "video.fullscreen" => self.video.fullscreen = leggi_bool(valore)?,
            "video.vsync" => self.video.vsync = leggi_bool(valore)?,
            "video.show_fps" | "video.mostra_fps" => self.video.show_fps = leggi_bool(valore)?,
            "video.shader" => {
                self.video.shader = match valore.trim().to_ascii_lowercase().as_str() {
                    "none" | "nessuno" => Shader::None,
                    "crt" => Shader::Crt,
                    "lcd" => Shader::Lcd,
                    "scanlines" => Shader::Scanlines,
                    altro => return Err(format!("shader sconosciuto: {altro} (none, crt, lcd, scanlines)")),
                }
            }
            "video.rotate" | "video.rotazione" => {
                self.video.rotate = match valore.trim().to_ascii_lowercase().as_str() {
                    "none" | "no" => Rotate::None,
                    "left" | "sinistra" => Rotate::Left,
                    "right" | "destra" => Rotate::Right,
                    altro => return Err(format!("rotazione sconosciuta: {altro} (none, left, right)")),
                }
            }
            "audio.master" => self.audio.master = leggi_percento(valore)?,
            "audio.music" => self.audio.music = leggi_percento(valore)?,
            "audio.effects" => self.audio.effects = leggi_percento(valore)?,
            "audio.video" => self.audio.video = leggi_percento(valore)?,
            "screensaver.enabled" => self.screensaver.enabled = leggi_bool(valore)?,
            "screensaver.delay_secs" => self.screensaver.delay_secs = leggi_u32(valore)?,
            "screensaver.slide_secs" => self.screensaver.slide_secs = leggi_u32(valore)?,
            "screensaver.use_videos" => self.screensaver.use_videos = leggi_bool(valore)?,
            "input.up" => self.input.up = valore.trim().to_owned(),
            "input.down" => self.input.down = valore.trim().to_owned(),
            "input.left" => self.input.left = valore.trim().to_owned(),
            "input.right" => self.input.right = valore.trim().to_owned(),
            "input.select" => self.input.select = valore.trim().to_owned(),
            "input.back" => self.input.back = valore.trim().to_owned(),
            "input.page_up" => self.input.page_up = valore.trim().to_owned(),
            "input.page_down" => self.input.page_down = valore.trim().to_owned(),
            "input.filter" => self.input.filter = valore.trim().to_owned(),
            "input.favourite" => self.input.favourite = valore.trim().to_owned(),
            "font" => {
                let v = valore.trim();
                self.font = if v.is_empty() || v == "nessuno" || v == "none" {
                    None
                } else {
                    Some(PathBuf::from(v))
                }
            }
            "layout" => {
                let v = valore.trim();
                self.layout = if v.is_empty() { None } else { Some(v.to_owned()) };
            }
            altro => return Err(format!("chiave sconosciuta: {altro}")),
        }
        Ok(())
    }

    /// Tutte le chiavi modificabili, per l'aiuto.
    pub fn chiavi() -> &'static [&'static str] {
        &[
            "video.fullscreen",
            "video.shader",
            "video.rotate",
            "video.vsync",
            "video.show_fps",
            "audio.master",
            "audio.music",
            "audio.effects",
            "audio.video",
            "screensaver.enabled",
            "screensaver.delay_secs",
            "screensaver.slide_secs",
            "screensaver.use_videos",
            "input.up",
            "input.down",
            "input.left",
            "input.right",
            "input.select",
            "input.back",
            "input.page_up",
            "input.page_down",
            "input.filter",
            "input.favourite",
            "font",
            "layout",
        ]
    }
}

fn leggi_bool(valore: &str) -> Result<bool, String> {
    match valore.trim().to_ascii_lowercase().as_str() {
        "1" | "si" | "sì" | "true" | "on" | "yes" => Ok(true),
        "0" | "no" | "false" | "off" => Ok(false),
        altro => Err(format!("atteso si/no, trovato {altro:?}")),
    }
}

fn leggi_percento(valore: &str) -> Result<u8, String> {
    valore
        .trim()
        .trim_end_matches('%')
        .parse::<u8>()
        .map(|v| v.min(100))
        .map_err(|_| format!("atteso un numero da 0 a 100, trovato {valore:?}"))
}

fn leggi_u32(valore: &str) -> Result<u32, String> {
    valore
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("atteso un numero, trovato {valore:?}"))
}
