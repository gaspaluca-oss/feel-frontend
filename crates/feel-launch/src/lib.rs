//! Avvio dell'emulatore.
//!
//! Due fasi separate di proposito:
//!
//! - **costruire il piano** ([`LaunchPlan`]) e' puro: sostituisce i segnaposto,
//!   decide il file della rom, valida i percorsi. Si testa senza avviare niente.
//! - **eseguirlo** ([`LaunchPlan::run`]) tocca il sistema operativo.
//!
//! La separazione conta perche' quasi tutti i problemi di un frontend stanno
//! nella prima fase — un segnaposto sbagliato, una rom che non si trova, un
//! percorso con uno spazio dentro — e sono anche quelli che si possono
//! verificare con `--dry-run` prima di andare sul cabinato.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, SystemTime};

use feel_core::{Emulator, Game};
use thiserror::Error;

/// Segnaposto che FEEL 2.0 sostituisce negli argomenti.
///
/// Sono gli stessi di FEEL 1.x con i nomi allineati: `[rom_name]` non esiste
/// piu' perche' si chiama `[name]`, e `[full_path]` si chiama `[romfile]`.
pub const TOKENS: [&str; 5] = [
    "[name]",
    "[rompath]",
    "[romfile]",
    "[extension]",
    "[emulatorpath]",
];

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("l'eseguibile {path} non esiste")]
    Missing { path: PathBuf },

    #[error("non riesco ad avviare {path}: {source}")]
    Spawn {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("non riesco ad aspettare {path}: {source}")]
    Wait {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Un'applicazione ausiliaria, con i segnaposto gia' risolti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedHook {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub wait_for_exit: bool,
}

/// Cosa eseguire, esattamente, per avviare un gioco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// La rom da passare all'emulatore.
    pub rom_file: PathBuf,
    /// Se il file della rom esiste davvero.
    ///
    /// Non e' un errore quando e' falso: MAME risolve le rom da solo a partire
    /// dal nome, quindi il file puo' benissimo non essere dove lo cerchiamo noi.
    /// Ma per gli emulatori che vogliono un percorso vero e' la differenza fra
    /// partire e non partire.
    pub rom_found: bool,
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub pre_launch: Option<PlannedHook>,
    pub post_launch: Option<PlannedHook>,
}

/// Cosa e' successo eseguendo un piano.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub started_at: SystemTime,
    pub finished_at: SystemTime,
    pub status: ExitStatus,
}

impl Outcome {
    /// Quanto e' rimasto aperto l'emulatore, cioe' il tempo di gioco.
    pub fn duration(&self) -> Duration {
        self.finished_at
            .duration_since(self.started_at)
            .unwrap_or_default()
    }

    /// Vero se l'emulatore e' uscito con codice zero.
    pub fn success(&self) -> bool {
        self.status.success()
    }
}

impl LaunchPlan {
    /// Costruisce il piano per un gioco, sapendo gia' qual e' il file della rom.
    ///
    /// Puro: non tocca il disco e non avvia niente.
    pub fn build(emulator: &Emulator, game: &Game, rom_file: PathBuf) -> Self {
        // Uniforma i separatori una volta sola, dove il percorso esce dal
        // frontend. Vedi `native_path` per il perche'.
        let rom_file = native_path(&rom_file);
        let context = Context::new(emulator, game, &rom_file);
        let rom_found = rom_file.is_file();

        Self {
            rom_file: rom_file.clone(),
            rom_found,
            executable: native_path(&emulator.executable),
            arguments: resolve(&emulator.arguments, &context),
            working_directory: emulator.working_directory.as_ref().map(|dir| native_path(dir)),
            pre_launch: emulator.pre_launch.as_ref().map(|hook| PlannedHook {
                executable: native_path(&hook.executable),
                arguments: resolve(&hook.arguments, &context),
                wait_for_exit: hook.wait_for_exit,
            }),
            post_launch: emulator.post_launch.as_ref().map(|hook| PlannedHook {
                executable: native_path(&hook.executable),
                arguments: resolve(&hook.arguments, &context),
                wait_for_exit: hook.wait_for_exit,
            }),
        }
    }

    /// Costruisce il piano cercando la rom fra le estensioni dichiarate.
    ///
    /// Se non la trova usa comunque la prima estensione dichiarata: il piano
    /// resta valido, e `rom_found` dice che quel percorso non e' verificato.
    pub fn resolve(emulator: &Emulator, game: &Game) -> Self {
        let rom_file = find_rom_file(emulator, game)
            .unwrap_or_else(|| expected_rom_file(emulator, game));

        Self::build(emulator, game, rom_file)
    }

    /// La riga di comando come verrebbe eseguita. Serve a mostrarla senza eseguirla.
    pub fn command_line(&self) -> Vec<String> {
        let mut out = vec![self.executable.to_string_lossy().into_owned()];
        out.extend(self.arguments.iter().cloned());
        out
    }

    /// Esegue il piano e aspetta la fine dell'emulatore.
    ///
    /// I pre/post avvio vengono eseguiti solo se il piano li prevede, e
    /// aspettati solo se dichiarano `wait_for_exit`.
    pub fn run(&self) -> Result<Outcome, LaunchError> {
        ensure_exists(&self.executable)?;
        if let Some(hook) = &self.pre_launch {
            ensure_exists(&hook.executable)?;
        }
        if let Some(hook) = &self.post_launch {
            ensure_exists(&hook.executable)?;
        }

        // Prima dell'emulatore.
        if let Some(hook) = &self.pre_launch {
            let mut child = spawn(&hook.executable, &hook.arguments, None)?;
            if hook.wait_for_exit {
                wait(&mut child, &hook.executable)?;
            }
        }

        let started_at = SystemTime::now();
        let mut child = spawn(
            &self.executable,
            &self.arguments,
            self.working_directory.as_deref(),
        )?;
        let status = wait(&mut child, &self.executable)?;
        let finished_at = SystemTime::now();

        // Dopo che l'emulatore e' uscito.
        if let Some(hook) = &self.post_launch {
            let mut child = spawn(&hook.executable, &hook.arguments, None)?;
            if hook.wait_for_exit {
                wait(&mut child, &hook.executable)?;
            }
        }

        Ok(Outcome { started_at, finished_at, status })
    }
}

/// Cerca la rom fra le estensioni dichiarate dall'emulatore.
///
/// Non viene usata la ricorsione anche quando `recursive_roms` e' vero: il
/// percorso relativo e' gia' registrato nella rom al momento dell'importazione,
/// quindi non serve girare per le cartelle.
pub fn find_rom_file(emulator: &Emulator, game: &Game) -> Option<PathBuf> {
    let base = rom_directory(emulator, game);
    emulator
        .rom_extensions
        .iter()
        .map(|extension| base.join(format!("{}.{}", game.key, extension)))
        .find(|candidate| candidate.is_file())
}

/// Il percorso che la rom *dovrebbe* avere, con la prima estensione dichiarata.
pub fn expected_rom_file(emulator: &Emulator, game: &Game) -> PathBuf {
    let base = rom_directory(emulator, game);
    match emulator.rom_extensions.first() {
        Some(extension) => base.join(format!("{}.{}", game.key, extension)),
        None => base.join(game.key.as_str()),
    }
}

fn rom_directory(emulator: &Emulator, game: &Game) -> PathBuf {
    let mut base = native_path(&emulator.rom_path);
    if let Some(relative) = &game.relative_path {
        base.push(native_path(relative));
    }
    base
}

/// Porta i separatori a quelli del sistema corrente.
///
/// In configurazione i percorsi usano sempre `/`, anche su Windows (vedi
/// `feel_config::paths`). Le API di Windows accettano entrambi, ma mescolarli
/// produce percorsi illeggibili — `.../roms\pacman.zip` — nei log e nei
/// messaggi d'errore, cioe' proprio dove servono per capire un problema.
/// Qui si uniformano una volta sola, quando il percorso esce dal frontend.
pub fn native_path(path: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().replace('/', "\\"))
    } else {
        path.to_path_buf()
    }
}

// ----------------------------------------------------------------------------------
// Sostituzione dei segnaposto
// ----------------------------------------------------------------------------------

struct Context {
    name: String,
    rom_path: String,
    rom_file: String,
    extension: String,
    emulator_path: String,
}

impl Context {
    fn new(emulator: &Emulator, game: &Game, rom_file: &Path) -> Self {
        Self {
            name: game.key.to_string(),
            rom_path: native_path(&emulator.rom_path).to_string_lossy().into_owned(),
            rom_file: rom_file.to_string_lossy().into_owned(),
            extension: rom_file
                .extension()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default(),
            // Il segnaposto si chiama `path` come in FEEL 1.x, ma in FEEL 2.0
            // l'emulatore e' un file: quello che serve agli argomenti e' la
            // cartella che lo contiene.
            emulator_path: emulator
                .executable
                .parent()
                .map(|parent| native_path(parent).to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    fn value(&self, token: &str) -> &str {
        match token {
            "[name]" => &self.name,
            "[rompath]" => &self.rom_path,
            "[romfile]" => &self.rom_file,
            "[extension]" => &self.extension,
            "[emulatorpath]" => &self.emulator_path,
            _ => "",
        }
    }
}

fn resolve(arguments: &[String], context: &Context) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| {
            let mut resolved = argument.clone();
            for token in TOKENS {
                if resolved.contains(token) {
                    resolved = resolved.replace(token, context.value(token));
                }
            }
            resolved
        })
        .collect()
}

// ----------------------------------------------------------------------------------
// Esecuzione
// ----------------------------------------------------------------------------------

fn ensure_exists(path: &Path) -> Result<(), LaunchError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(LaunchError::Missing { path: path.to_path_buf() })
    }
}

fn spawn(
    executable: &Path,
    arguments: &[String],
    working_directory: Option<&Path>,
) -> Result<Child, LaunchError> {
    let mut command = Command::new(executable);
    command.args(arguments);
    if let Some(directory) = working_directory {
        command.current_dir(directory);
    }

    command.spawn().map_err(|source| LaunchError::Spawn {
        path: executable.to_path_buf(),
        source,
    })
}

fn wait(child: &mut Child, executable: &Path) -> Result<ExitStatus, LaunchError> {
    child.wait().map_err(|source| LaunchError::Wait {
        path: executable.to_path_buf(),
        source,
    })
}
