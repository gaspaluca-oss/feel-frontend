//! Costruzione ed esecuzione di un piano di avvio.

use std::path::PathBuf;

use feel_core::{
    ArtworkPaths, Emulator, EmulatorKey, Game, GameKey, GamelistKey, HookCommand, PlatformKey,
};
use feel_launch::{find_rom_file, native_path, LaunchError, LaunchPlan};

// ----------------------------------------------------------------------------------
// Costruzione dei dati di prova
// ----------------------------------------------------------------------------------

fn emulatore(rom_path: PathBuf, rom_extensions: &[&str]) -> Emulator {
    Emulator {
        key: EmulatorKey::new("mame"),
        platform: PlatformKey::new("arcade"),
        title: "M.A.M.E.".into(),
        executable: rom_path.join("mame.exe"),
        arguments: vec!["[name]".into()],
        working_directory: None,
        rom_path,
        rom_extensions: rom_extensions.iter().map(|e| (*e).to_owned()).collect(),
        artwork: ArtworkPaths::default(),
        layout: None,
        gamelists: Vec::new(),
        recursive_roms: false,
        pre_launch: None,
        post_launch: None,
    }
}

fn gioco(key: &str) -> Game {
    Game {
        key: GameKey::new(key),
        gamelist: GamelistKey::new("mame-0"),
        title: key.to_owned(),
        year: None,
        manufacturer: None,
        clone_of: None,
        bios: None,
        category: None,
        rotation: None,
        controls: None,
        status: None,
        relative_path: None,
    }
}

/// Una cartella temporanea che si cancella da sola alla fine del test.
struct Cartella(PathBuf);

impl Cartella {
    fn nuova(nome: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "feel-launch-{}-{nome}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, relativo: &str) -> PathBuf {
        let path = self.0.join(relativo);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, b"finta rom").unwrap();
        path
    }
}

impl Drop for Cartella {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Un comando che esiste di sicuro e termina subito con il codice voluto.
fn comando_di_prova(codice: i32) -> (PathBuf, Vec<String>) {
    if cfg!(windows) {
        let shell = std::env::var("COMSPEC")
            .unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_owned());
        (
            PathBuf::from(shell),
            vec!["/C".into(), format!("exit {codice}")],
        )
    } else {
        (
            PathBuf::from("/bin/sh"),
            vec!["-c".into(), format!("exit {codice}")],
        )
    }
}

/// Il percorso della rom come lo vede l'emulatore: separatori uniformati.
fn rom_file_atteso(cartella: &Cartella) -> PathBuf {
    native_path(&cartella.0.join("roms/pacman.zip"))
}

// ----------------------------------------------------------------------------------
// Sostituzione dei segnaposto
// ----------------------------------------------------------------------------------

#[test]
fn sostituisce_i_segnaposto_negli_argomenti() {
    let cartella = Cartella::nuova("segnaposto");
    let rom = cartella.file("roms/pacman.zip");
    let emulatore = emulatore(cartella.0.join("roms"), &["zip"]);
    let gioco = gioco("pacman");

    let piano = LaunchPlan::build(&emulatore, &gioco, rom.clone());

    // `[name]` diventa il nome della rom, non il titolo mostrato a video.
    assert_eq!(piano.arguments, vec!["pacman"]);
    assert!(piano.rom_found);

    // Tutti i segnaposto, in una volta sola.
    let mut ricco = emulatore.clone();
    ricco.arguments = vec![
        "-rompath".into(),
        "[rompath]".into(),
        "-rom".into(),
        "[romfile]".into(),
        "-ext".into(),
        "[extension]".into(),
    ];
    let piano = LaunchPlan::build(&ricco, &gioco, rom);

    assert_eq!(piano.arguments[1], cartella.0.join("roms").to_string_lossy());
    assert_eq!(
        piano.arguments[3],
        native_path(&rom_file_atteso(&cartella)).to_string_lossy()
    );
    assert_eq!(piano.arguments[5], "zip");
}

#[test]
fn emulatorpath_e_la_cartella_dell_eseguibile() {
    let cartella = Cartella::nuova("emupath");
    let emulatore = emulatore(cartella.0.join("roms"), &["zip"]);

    let mut con_segnaposto = emulatore.clone();
    con_segnaposto.arguments = vec!["--home".into(), "[emulatorpath]".into()];

    let piano = LaunchPlan::build(&con_segnaposto, &gioco("x"), cartella.0.join("x.zip"));

    assert_eq!(
        piano.arguments[1],
        cartella.0.join("roms").to_string_lossy(),
        "in FEEL 2.0 l'eseguibile e' un file, quindi il segnaposto e' la sua cartella"
    );
}

#[test]
fn un_argomento_senza_segnaposto_non_viene_toccato() {
    let cartella = Cartella::nuova("intatto");
    let mut emulatore = emulatore(cartella.0.clone(), &["zip"]);
    emulatore.arguments = vec!["-skip_gameinfo".into(), "--nome=[name]".into()];

    let piano = LaunchPlan::build(&emulatore, &gioco("pacman"), cartella.0.join("x.zip"));

    assert_eq!(piano.arguments[0], "-skip_gameinfo");
    // La sostituzione avviene anche dentro una parola piu' lunga.
    assert_eq!(piano.arguments[1], "--nome=pacman");
}

// ----------------------------------------------------------------------------------
// Ricerca della rom
// ----------------------------------------------------------------------------------

#[test]
fn trova_la_rom_tra_le_estensioni_dichiarate() {
    let cartella = Cartella::nuova("cerca");
    cartella.file("roms/gioco.sfc");
    let emulatore = emulatore(cartella.0.join("roms"), &["smc", "sfc"]);
    let gioco = gioco("gioco");

    let trovata = find_rom_file(&emulatore, &gioco).expect("la rom deve essere trovata");
    assert!(trovata.ends_with("gioco.sfc"), "trovata: {trovata:?}");

    let piano = LaunchPlan::resolve(&emulatore, &gioco);
    assert!(piano.rom_found);
    assert_eq!(piano.arguments, vec!["gioco"]);
}

#[test]
fn cerca_anche_nella_sottocartella_della_rom() {
    let cartella = Cartella::nuova("sottocartella");
    cartella.file("roms/psikyo/s1945.zip");
    let emulatore = emulatore(cartella.0.join("roms"), &["zip"]);

    let mut gioco = gioco("s1945");
    gioco.relative_path = Some(PathBuf::from("psikyo"));

    let piano = LaunchPlan::resolve(&emulatore, &gioco);
    assert!(piano.rom_found);
    assert!(piano.rom_file.ends_with("psikyo/s1945.zip"));
}

#[test]
fn una_rom_introvabile_non_invalida_il_piano() {
    let cartella = Cartella::nuova("assente");
    let emulatore = emulatore(cartella.0.join("roms"), &["zip"]);

    let piano = LaunchPlan::resolve(&emulatore, &gioco("mai_visto"));

    // MAME risolve le rom da solo a partire dal nome: il fatto che il file non
    // sia dove lo cerchiamo noi non impedisce di avviare il gioco.
    assert!(!piano.rom_found);
    assert_eq!(piano.arguments, vec!["mai_visto"]);
    assert!(piano.rom_file.ends_with("mai_visto.zip"));
}

// ----------------------------------------------------------------------------------
// Pre e post avvio
// ----------------------------------------------------------------------------------

#[test]
fn risolve_i_segnaposto_anche_nei_pre_e_post() {
    let cartella = Cartella::nuova("hook");
    let mut emulatore = emulatore(cartella.0.join("roms"), &["zip"]);
    emulatore.pre_launch = Some(HookCommand {
        executable: cartella.0.join("primadi.exe"),
        arguments: vec!["--gioco".into(), "[name]".into()],
        wait_for_exit: true,
    });
    emulatore.post_launch = Some(HookCommand {
        executable: cartella.0.join("dopo.exe"),
        arguments: vec!["[romfile]".into()],
        wait_for_exit: false,
    });

    let rom = cartella.0.join("roms/pacman.zip");
    let piano = LaunchPlan::build(&emulatore, &gioco("pacman"), rom.clone());

    let pre = piano.pre_launch.expect("il pre avvio deve esserci");
    assert_eq!(pre.arguments, vec!["--gioco", "pacman"]);
    assert!(pre.wait_for_exit);

    let post = piano.post_launch.expect("il post avvio deve esserci");
    assert_eq!(post.arguments, vec![native_path(&rom).to_string_lossy().to_string()]);
    assert!(!post.wait_for_exit);
}

// ----------------------------------------------------------------------------------
// Esecuzione
// ----------------------------------------------------------------------------------

#[test]
fn esegue_e_riporta_il_codice_di_uscita() {
    let cartella = Cartella::nuova("esegue");
    let (eseguibile, argomenti) = comando_di_prova(3);

    let mut emulatore = emulatore(cartella.0.clone(), &["zip"]);
    emulatore.executable = eseguibile;
    emulatore.arguments = argomenti;

    let piano = LaunchPlan::build(&emulatore, &gioco("x"), cartella.0.join("x.zip"));
    let esito = piano.run().expect("il comando deve partire");

    assert!(!esito.success(), "un codice diverso da zero non e' un successo");
    assert_eq!(esito.status.code(), Some(3));
    // Il tempo di gioco si misura fra l'avvio e l'uscita.
    assert!(esito.duration().as_secs() < 30);
}

#[test]
fn esegue_un_pre_avvio_aspettandolo() {
    let cartella = Cartella::nuova("pre");
    let (eseguibile, argomenti) = comando_di_prova(0);

    let mut emulatore = emulatore(cartella.0.clone(), &["zip"]);
    emulatore.executable = eseguibile.clone();
    emulatore.arguments = argomenti.clone();
    emulatore.pre_launch = Some(HookCommand {
        executable: eseguibile,
        arguments: argomenti,
        wait_for_exit: true,
    });

    let piano = LaunchPlan::build(&emulatore, &gioco("x"), cartella.0.join("x.zip"));
    let esito = piano.run().expect("pre avvio ed emulatore devono partire");

    assert!(esito.success());
    // L'emulatore e' durato zero: il tempo si misura dopo il pre avvio, non prima.
    assert!(esito.duration().as_secs() < 30);
}

#[test]
fn un_eseguibile_inesistente_e_un_errore_che_si_capisce() {
    let cartella = Cartella::nuova("manca");
    let mut emulatore = emulatore(cartella.0.clone(), &["zip"]);
    emulatore.executable = cartella.0.join("non/esiste/emulatore.exe");

    let piano = LaunchPlan::build(&emulatore, &gioco("x"), cartella.0.join("x.zip"));
    match piano.run() {
        Err(LaunchError::Missing { path }) => {
            assert!(path.ends_with("emulatore.exe"));
        }
        altro => panic!("atteso un errore di eseguibile mancante, ottenuto {altro:?}"),
    }
}

#[test]
fn un_percorso_di_configurazione_non_mescola_i_separatori() {
    let cartella = Cartella::nuova("separatori");

    // I percorsi arrivano dalla configurazione, quindi con `/` anche su Windows.
    let con_slash = cartella.0.to_string_lossy().replace('\\', "/");
    let mut emulatore = emulatore(PathBuf::from(format!("{con_slash}/roms")), &["zip"]);
    emulatore.executable = PathBuf::from(format!("{con_slash}/mame.exe"));

    let rom = cartella.file("roms/pacman.zip");
    let piano = LaunchPlan::build(&emulatore, &gioco("pacman"), rom);

    let testo = piano.rom_file.to_string_lossy().into_owned();
    if cfg!(windows) {
        assert!(!testo.contains('/'), "separatori mescolati: {testo}");
    } else {
        assert!(!testo.contains('\\'), "separatori mescolati: {testo}");
    }

    // Anche quello che finisce negli argomenti dev'essere uniforme.
    let mut con_argomenti = emulatore.clone();
    con_argomenti.arguments = vec!["[rompath]".into(), "[emulatorpath]".into()];
    let piano = LaunchPlan::build(&con_argomenti, &gioco("pacman"), cartella.file("roms/x.zip"));
    for argomento in &piano.arguments {
        if cfg!(windows) {
            assert!(!argomento.contains('/'), "separatori mescolati: {argomento}");
        } else {
            assert!(!argomento.contains('\\'), "separatori mescolati: {argomento}");
        }
    }
}

#[test]
fn la_riga_di_comando_e_quella_che_verrebbe_eseguita() {
    let cartella = Cartella::nuova("riga");
    let emulatore = emulatore(cartella.0.join("roms"), &["zip"]);
    let piano = LaunchPlan::resolve(&emulatore, &gioco("pacman"));

    let riga = piano.command_line();
    assert!(riga[0].ends_with("mame.exe"));
    assert_eq!(riga[1], "pacman");
}
