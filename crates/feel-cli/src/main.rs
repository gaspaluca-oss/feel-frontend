//! Riga di comando di FEEL 2.0.
//!
//! Serve a due cose, e nessuna delle due e' un ripiego:
//!
//! - **importare** una installazione FEEL 1.x in un database, e dire cosa non
//!   e' andato liscio invece di far finta di niente;
//! - **diagnosticare**: contare e mostrare cosa c'e' dentro un database senza
//!   aprire il frontend. Quando un cabinato si comporta male, questa e' la prima
//!   cosa da eseguire.
//!
//! L'analisi degli argomenti e' fatta a mano: `clap` non aggiunge niente qui e
//! porterebbe con se' un albero di dipendenze da tenere compatibile con la
//! toolchain fissata per Windows 7.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use feel_config::ImportOptions;
use feel_core::{
    ArtworkSlot, Dimension, Element, ElementKind, GamelistKey, LabelKind, Library, Timestamp,
};
use feel_db::Database;
use feel_launch::LaunchPlan;

const USO: &str = "\
FEEL 2.0 - riga di comando

Uso:
  feel import <cartella-feel> --db <file> [--remap VECCHIO=NUOVO]...
  feel info   <file.db>
  feel list   <file.db> <lista>
  feel launch <file.db> <lista> <rom> [--dry-run]
  feel layout <cartella-feel> [--out <cartella>] [--dry-run]

Comandi:
  import   legge una installazione FEEL 1.x e scrive la libreria in un database
  info     mostra cosa contiene un database
  list     elenca i giochi di una lista
  launch   avvia un gioco e registra il tempo di gioco
  layout   converte i layout dal formato di FEEL 1.x a quello di FEEL 2.0

Opzioni:
  --db <file>         database su cui lavorare (obbligatorio per import)
  --out <cartella>    dove scrivere i layout convertiti
  --remap A=B         riscrive un percorso durante l'importazione (ripetibile)
  --dry-run           import: importa senza scrivere; launch: mostra senza avviare;
                      layout: mostra la conversione senza scrivere i file
  -h, --help          questo aiuto
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        print!("{USO}");
        return ExitCode::SUCCESS;
    }

    let esito = match args[0].as_str() {
        "import" => comando_import(&args[1..]),
        "info" => comando_info(&args[1..]),
        "list" => comando_list(&args[1..]),
        "launch" => comando_launch(&args[1..]),
        "layout" => comando_layout(&args[1..]),
        altro => Err(format!("comando sconosciuto: {altro} (prova --help)").into()),
    };

    match esito {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("\nerrore: {err}");
            ExitCode::FAILURE
        }
    }
}

// ----------------------------------------------------------------------------------
// import
// ----------------------------------------------------------------------------------

fn comando_import(args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut feel_root: Option<PathBuf> = None;
    let mut db_path: Option<PathBuf> = None;
    let mut remaps: Vec<(String, String)> = Vec::new();
    let mut dry_run = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--db" => {
                db_path = Some(PathBuf::from(prossimo(args, &mut index, "--db")?));
            }
            "--remap" => {
                let raw = prossimo(args, &mut index, "--remap")?;
                let (from, to) = raw
                    .split_once('=')
                    .ok_or_else(|| format!("--remap vuole VECCHIO=NUOVO, non {raw:?}"))?;
                remaps.push((from.trim().to_owned(), to.trim().to_owned()));
            }
            "--dry-run" => dry_run = true,
            altro if altro.starts_with("--") => {
                return Err(format!("opzione sconosciuta: {altro}").into())
            }
            altro => {
                if feel_root.is_some() {
                    return Err(format!("troppa roba sulla riga di comando: {altro}").into());
                }
                feel_root = Some(PathBuf::from(altro));
            }
        }
        index += 1;
    }

    let feel_root = feel_root.ok_or("manca la cartella di FEEL 1.x da importare")?;
    if !dry_run && db_path.is_none() {
        return Err("manca --db: senza database non c'e' niente da salvare".into());
    }

    println!("Leggo FEEL 1.x da: {}", feel_root.display());

    let mut options = ImportOptions::default();
    for (from, to) in &remaps {
        options.path_rules = std::mem::take(&mut options.path_rules).remap(from, to);
    }

    let import = feel_config::load_with(&feel_root, &options)?;
    stampa_riepilogo(&import.library);

    if !import.report.warnings.is_empty() {
        println!("\nDa controllare a mano ({}):", import.report.warnings.len());
        for avviso in &import.report.warnings {
            println!("  - {avviso}");
        }
    }
    if !import.report.nested_rom_paths.is_empty() {
        println!("\nRom in sottocartelle di rom_path:");
        for percorso in &import.report.nested_rom_paths {
            println!("  - {percorso}");
        }
    }

    if dry_run {
        println!("\n--dry-run: nessun database scritto.");
        return Ok(());
    }

    let db_path = db_path.expect("controllato sopra");
    let mut db = Database::open(&db_path)?;
    db.store_library(&import.library)?;

    println!("\nDatabase scritto in: {}", db_path.display());
    let conta = db.counts()?;
    println!(
        "Verifica: {} piattaforme, {} emulatori, {} liste, {} giochi.",
        conta.platforms, conta.emulators, conta.gamelists, conta.games
    );

    Ok(())
}

// ----------------------------------------------------------------------------------
// info
// ----------------------------------------------------------------------------------

fn comando_info(args: &[String]) -> Result<(), Box<dyn Error>> {
    let path = args
        .first()
        .ok_or("manca il file del database")?;

    let db = Database::open(path)?;
    println!("Database: {}", Path::new(path).display());
    println!("Versione dello schema: {}", db.schema_version()?);

    let conta = db.counts()?;
    println!();
    println!("  piattaforme   {:>8}", conta.platforms);
    println!("  emulatori     {:>8}", conta.emulators);
    println!("  liste         {:>8}", conta.gamelists);
    println!("  giochi        {:>8}", conta.games);
    println!("  giocati       {:>8}", conta.played_games);

    if conta.games == 0 {
        println!("\nIl database e' vuoto: importa una installazione con 'feel import'.");
        return Ok(());
    }

    // Le liste, con quanti giochi contengono: e' la prima cosa che si vuole
    // sapere quando qualcosa non torna.
    let library = db.load_library()?;
    println!("\nListe:");
    for gamelist in &library.gamelists {
        let quanti = library.games_of(&gamelist.key).count();
        println!(
            "  {:<28} {:>7} giochi   ({}/{})",
            gamelist.title,
            quanti,
            gamelist.platform,
            gamelist.emulator
        );
    }

    Ok(())
}

// ----------------------------------------------------------------------------------
// list
// ----------------------------------------------------------------------------------

fn comando_list(args: &[String]) -> Result<(), Box<dyn Error>> {
    let path = args.first().ok_or("manca il file del database")?;
    let gamelist = args.get(1).ok_or("manca il nome della lista")?;

    let db = Database::open(path)?;
    let giochi = db.games_of(&feel_core::GamelistKey::new(gamelist.clone()))?;

    if giochi.is_empty() {
        println!("Nessun gioco nella lista '{gamelist}'.");
        return Ok(());
    }

    for gioco in &giochi {
        let anno = gioco.year.as_deref().unwrap_or("----");
        let categoria = gioco.category.as_deref().unwrap_or("");
        println!(
            "  {:<12} {:<40} {}  {}",
            gioco.key,
            tronca(&gioco.title, 40),
            anno,
            categoria
        );
    }
    println!("\n{} giochi.", giochi.len());

    Ok(())
}

// ----------------------------------------------------------------------------------
// launch
// ----------------------------------------------------------------------------------

fn comando_launch(args: &[String]) -> Result<(), Box<dyn Error>> {
    let path = args.first().ok_or("manca il file del database")?;
    let gamelist = args.get(1).ok_or("manca il nome della lista")?;
    let rom = args.get(2).ok_or("manca il nome della rom")?;
    let dry_run = args.iter().any(|argomento| argomento == "--dry-run");

    let db = Database::open(path)?;
    let library = db.load_library()?;

    let chiave_lista = GamelistKey::new(gamelist.clone());
    let lista = library
        .gamelist(&chiave_lista)
        .ok_or_else(|| format!("lista '{gamelist}' non presente nel database"))?;
    let emulatore = library
        .emulator(&lista.platform, &lista.emulator)
        .ok_or_else(|| format!("emulatore '{}/{}' non trovato", lista.platform, lista.emulator))?;
    let gioco = library
        .games_of(&chiave_lista)
        .find(|g| g.key.as_str() == rom)
        .ok_or_else(|| format!("'rom' non e' nella lista '{gamelist}'"))?;

    let piano = LaunchPlan::resolve(emulatore, gioco);

    println!("Emulatore : {}", piano.executable.display());
    println!(
        "Rom       : {}{}",
        piano.rom_file.display(),
        if piano.rom_found {
            ""
        } else {
            "   (non trovata dove la cerchiamo)"
        }
    );
    println!("Comando   : {}", piano.command_line().join(" "));
    if let Some(hook) = &piano.pre_launch {
        println!("Prima     : {} {}", hook.executable.display(), hook.arguments.join(" "));
    }
    if let Some(hook) = &piano.post_launch {
        println!("Dopo      : {} {}", hook.executable.display(), hook.arguments.join(" "));
    }

    if dry_run {
        println!("\n--dry-run: niente e' stato avviato.");
        return Ok(());
    }

    let esito = piano.run()?;
    let durata = esito.duration();

    println!(
        "\nUscito dopo {:.0} s con codice {:?}.",
        durata.as_secs_f64(),
        esito.status.code()
    );
    if !esito.success() {
        println!("Codice diverso da zero: l'emulatore potrebbe aver avuto un problema.");
    }

    db.record_play(
        &(lista.platform.clone(), lista.emulator.clone()),
        &gioco.key,
        durata,
        Timestamp::now(),
    )?;
    println!("Partita registrata.");

    Ok(())
}

// ----------------------------------------------------------------------------------
// layout
// ----------------------------------------------------------------------------------

fn comando_layout(args: &[String]) -> Result<(), Box<dyn Error>> {
    let root = args.first().ok_or("manca la cartella di FEEL 1.x da convertire")?;
    let dry_run = args.iter().any(|argomento| argomento == "--dry-run");
    let out_dir = args
        .iter()
        .position(|argomento| argomento == "--out")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from);

    let layouts = feel_config::layout1::find_layouts(Path::new(root));
    if layouts.is_empty() {
        println!("Nessun layout trovato in {root}.");
        return Ok(());
    }

    if let Some(dir) = &out_dir {
        if !dry_run {
            std::fs::create_dir_all(dir)?;
        }
    }

    for (name, percorso) in &layouts {
        let layout = feel_config::load_layout(percorso)?;

        println!("\n{name}");
        if let Some(schermo) = layout.design_size {
            println!("  pensato per   {:.0}x{:.0}", schermo.width, schermo.height);
        }
        println!("  elementi      {}", layout.elements.len());

        for elemento in layout.visible_elements() {
            println!(
                "    {:<26} {:<30} {}",
                descrivi_kind(&elemento.kind),
                descrivi_ancoraggio(elemento),
                descrivi_area(&layout, elemento),
            );
        }

        if let Some(sfondo) = &layout.background {
            match (&sfondo.image, sfondo.frames.len()) {
                (Some(immagine), _) => println!("  sfondo        {immagine}"),
                (None, quanti) => println!("  sfondo        {quanti} fotogrammi animati"),
            }
        }

        // Un file citato ma assente e' il modo piu' comune in cui un layout
        // convertito sembra giusto e poi si vede male.
        let referenziati = feel_config::layout1::referenced_assets(&layout);
        let mancanti: Vec<&String> = referenziati
            .iter()
            .filter(|nome| !layout.assets.iter().any(|presente| presente == *nome))
            .collect();
        if !mancanti.is_empty() {
            println!("  file mancanti:");
            for nome in mancanti {
                println!("    - {nome}");
            }
        }

        if let Some(dir) = &out_dir {
            let destinazione = dir.join(format!("{name}.json"));
            if dry_run {
                println!("  --dry-run: non scrivo {}", destinazione.display());
            } else {
                std::fs::write(&destinazione, serde_json::to_string_pretty(&layout)?)?;
                println!("  scritto       {}", destinazione.display());
            }
        }
    }

    println!("\n{} layout.", layouts.len());
    Ok(())
}

fn descrivi_kind(kind: &ElementKind) -> String {
    match kind {
        ElementKind::List => "lista".to_owned(),
        ElementKind::Artwork(slot) => format!("immagine ({})", nome_slot(*slot)),
        ElementKind::Label(label) => format!("testo ({})", nome_label(*label)),
        ElementKind::Clock => "orologio".to_owned(),
        ElementKind::MarqueeBoard => "pannello secondario".to_owned(),
    }
}

fn nome_slot(slot: ArtworkSlot) -> &'static str {
    match slot {
        ArtworkSlot::Snap => "snap",
        ArtworkSlot::Marquee => "marquee",
        ArtworkSlot::Cabinet => "cabinet",
        ArtworkSlot::Bezel => "bezel",
        ArtworkSlot::Actors => "actors",
        ArtworkSlot::ControlPanel => "controls",
        ArtworkSlot::Title => "title",
        ArtworkSlot::Wheel => "wheel",
    }
}

fn nome_label(label: LabelKind) -> &'static str {
    match label {
        LabelKind::Platform => "piattaforma",
        LabelKind::Emulator => "emulatore",
        LabelKind::Gamelist => "lista",
        LabelKind::RomName => "nome rom",
        LabelKind::RomDescription => "titolo",
        LabelKind::RomManufacturer => "produttore",
        LabelKind::RomCategory => "categoria",
        LabelKind::RomControls => "comandi",
        LabelKind::RomStatus => "stato",
        LabelKind::RomDisplayType => "tipo schermo",
        LabelKind::RomCounter => "contatore",
    }
}

/// L'ancoraggio in parole: e' la cosa che si vuole controllare guardando
/// una conversione, perche' dice come si comportera' a un'altra risoluzione.
fn descrivi_ancoraggio(elemento: &Element) -> String {
    let point = elemento.placement.point;
    let (orizzontale, verticale) = match (point.horizontal, point.vertical) {
        (feel_core::Horizontal::Left, feel_core::Vertical::Top) => ("sinistra", "alto"),
        (feel_core::Horizontal::Left, feel_core::Vertical::Middle) => ("sinistra", "centro"),
        (feel_core::Horizontal::Left, feel_core::Vertical::Bottom) => ("sinistra", "basso"),
        (feel_core::Horizontal::Center, feel_core::Vertical::Top) => ("centro", "alto"),
        (feel_core::Horizontal::Center, feel_core::Vertical::Middle) => ("centro", "centro"),
        (feel_core::Horizontal::Center, feel_core::Vertical::Bottom) => ("centro", "basso"),
        (feel_core::Horizontal::Right, feel_core::Vertical::Top) => ("destra", "alto"),
        (feel_core::Horizontal::Right, feel_core::Vertical::Middle) => ("destra", "centro"),
        (feel_core::Horizontal::Right, feel_core::Vertical::Bottom) => ("destra", "basso"),
    };
    format!("{orizzontale} {verticale}")
}

fn descrivi_area(layout: &feel_core::Layout, elemento: &Element) -> String {
    let percentuale = |d: Dimension| match d {
        Dimension::Fraction(frazione) => format!("{:.1}%", frazione * 100.0),
        Dimension::Pixels(pixel) => format!("{pixel:.0}px"),
    };

    let dimensione = format!(
        "{} x {}",
        percentuale(elemento.size.width),
        percentuale(elemento.size.height)
    );

    // Il rettangolo originale e' la cosa piu' facile da confrontare con il
    // vecchio layout, quindi si mostra quando la risoluzione di progetto c'e'.
    match layout.design_size {
        Some(schermo) => {
            let rect = elemento.rect(schermo);
            format!(
                "{dimensione}  ->  {:.0},{:.0} su {:.0}x{:.0}",
                rect.x, rect.y, schermo.width, schermo.height
            )
        }
        None => dimensione,
    }
}

// ----------------------------------------------------------------------------------
// utilita'
// ----------------------------------------------------------------------------------

fn prossimo<'a>(args: &'a [String], index: &mut usize, opzione: &str) -> Result<&'a str, Box<dyn Error>> {
    *index += 1;
    args.get(*index)
        .map(String::as_str)
        .ok_or_else(|| format!("{opzione} vuole un valore dopo di se'").into())
}

fn stampa_riepilogo(library: &Library) {
    println!();
    println!("  piattaforme   {:>8}", library.platforms.len());
    println!("  emulatori     {:>8}", library.emulators.len());
    println!("  liste         {:>8}", library.gamelists.len());
    println!("  giochi        {:>8}", library.games.len());

    let con_statistiche: usize = library.plays.values().map(|by_game| by_game.len()).sum();
    println!("  con partite   {:>8}", con_statistiche);
}

fn tronca(value: &str, lunghezza: usize) -> String {
    if value.chars().count() <= lunghezza {
        return value.to_owned();
    }
    let tagliato: String = value.chars().take(lunghezza.saturating_sub(2)).collect();
    // ".." e non il carattere di omissione: le console Windows con codepage
    // legacy lo stampano come un quadratino.
    format!("{tagliato}..")
}
