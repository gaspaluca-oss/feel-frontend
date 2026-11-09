//! Costruzione di una lista leggendo le rom su disco.
//!
//! FEEL 1.x sapeva farlo in quattro modi diversi (`list_type`), e il piu'
//! semplice e' anche il piu' utile quando si comincia: guardare la cartella
//! delle rom e fare una lista con quello che c'e'.
//!
//! I dati che il nome del file non puo' dare — anno, produttore, categoria,
//! numero di giocatori — restano vuoti. Arrivano dallo scraping o dal listxml
//! di MAME, non da qui: inventarli dal nome sarebbe peggio che lasciarli vuoti.

use std::io;
use std::path::{Path, PathBuf};

use feel_core::{cleanup_name, Emulator, Game, GameKey, GamelistKey};

/// Quante cartelle si scendono al massimo, per non inseguire un collegamento
/// che punta a se stesso.
const PROFONDITA_MASSIMA: usize = 8;

/// Legge la cartella delle rom dichiarata dall'emulatore.
///
/// `recursive` decide se scendere nelle sottocartelle: alcuni emulatori
/// organizzano le rom in cartelle per sistema, e senza ricorsione la lista
/// uscirebbe vuota.
pub fn leggi_cartella(
    emulatore: &Emulator,
    gamelist: &GamelistKey,
    ricorsivo: bool,
) -> io::Result<Vec<Game>> {
    let radice = &emulatore.rom_path;
    if !radice.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("la cartella delle rom non esiste: {}", radice.display()),
        ));
    }

    let estensioni: Vec<String> = emulatore
        .rom_extensions
        .iter()
        .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
        .collect();

    let mut giochi = Vec::new();
    raccogli(
        radice,
        radice,
        &estensioni,
        ricorsivo,
        0,
        gamelist,
        &mut giochi,
    )?;

    giochi.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(giochi)
}

fn raccogli(
    radice: &Path,
    cartella: &Path,
    estensioni: &[String],
    ricorsivo: bool,
    profondita: usize,
    gamelist: &GamelistKey,
    fuori: &mut Vec<Game>,
) -> io::Result<()> {
    if profondita > PROFONDITA_MASSIMA {
        return Ok(());
    }

    let voci = match std::fs::read_dir(cartella) {
        Ok(voci) => voci,
        // Una sottocartella illeggibile non deve far fallire tutta la lista:
        // si salta e si prosegue, perche' una lista parziale e' meglio di
        // nessuna lista.
        Err(_) => return Ok(()),
    };

    for voce in voci.flatten() {
        let percorso = voce.path();
        let nome = voce.file_name().to_string_lossy().into_owned();

        if percorso.is_dir() {
            if ricorsivo && !nome.starts_with('.') {
                raccogli(
                    radice,
                    &percorso,
                    estensioni,
                    ricorsivo,
                    profondita + 1,
                    gamelist,
                    fuori,
                )?;
            }
            continue;
        }

        let estensione = percorso
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !estensioni.is_empty() && !estensioni.iter().any(|e| e == &estensione) {
            continue;
        }

        let Some(gambo) = percorso.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        if gambo.is_empty() {
            continue;
        }

        // La sottocartella rispetto alla radice, se non e' la radice stessa.
        let relativa = percorso
            .parent()
            .and_then(|padre| padre.strip_prefix(radice).ok())
            .filter(|relativa| !relativa.as_os_str().is_empty())
            .map(PathBuf::from);

        fuori.push(Game {
            key: GameKey::new(gambo.clone()),
            gamelist: gamelist.clone(),
            // "Pac-Man (Midway)" -> "Pac-Man": stesse regole di FEEL 1.x.
            title: cleanup_name(&gambo),
            year: None,
            manufacturer: None,
            clone_of: None,
            bios: None,
            category: None,
            rotation: None,
            controls: None,
            status: None,
            relative_path: relativa,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use feel_core::{ArtworkPaths, EmulatorKey, PlatformKey};

    fn emulatore(rom_path: PathBuf, estensioni: &[&str], ricorsivo: bool) -> Emulator {
        Emulator {
            key: EmulatorKey::new("mame"),
            platform: PlatformKey::new("arcade"),
            title: "MAME".into(),
            executable: rom_path.join("mame"),
            arguments: vec!["[name]".into()],
            working_directory: None,
            rom_path,
            rom_extensions: estensioni.iter().map(|e| (*e).to_owned()).collect(),
            artwork: ArtworkPaths::default(),
            layout: None,
            gamelists: Vec::new(),
            recursive_roms: ricorsivo,
            pre_launch: None,
            post_launch: None,
        }
    }

    struct Cartella(PathBuf);

    impl Cartella {
        fn nuova(nome: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "feel-scan-{}-{nome}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn file(&self, relativo: &str) {
            let percorso = self.0.join(relativo);
            if let Some(padre) = percorso.parent() {
                std::fs::create_dir_all(padre).unwrap();
            }
            std::fs::write(percorso, b"finta rom").unwrap();
        }
    }

    impl Drop for Cartella {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn leggi(cartella: &Cartella, estensioni: &[&str], ricorsivo: bool) -> Vec<Game> {
        let emu = emulatore(cartella.0.clone(), estensioni, ricorsivo);
        leggi_cartella(&emu, &GamelistKey::new("mame-0"), ricorsivo).unwrap()
    }

    #[test]
    fn trova_le_rom_con_l_estensione_giusta() {
        let cartella = Cartella::nuova("estensioni");
        cartella.file("pacman.zip");
        cartella.file("galaga.zip");
        cartella.file("leggimi.txt");
        cartella.file("copertina.png");

        let giochi = leggi(&cartella, &["zip"], false);
        assert_eq!(giochi.len(), 2);
        assert_eq!(giochi[0].key.as_str(), "galaga");
        assert_eq!(giochi[1].key.as_str(), "pacman");
    }

    #[test]
    fn piu_estensioni_insieme() {
        let cartella = Cartella::nuova("multiple");
        cartella.file("gioco.smc");
        cartella.file("altro.sfc");
        cartella.file("terzo.zip");

        let giochi = leggi(&cartella, &["smc", "sfc"], false);
        assert_eq!(giochi.len(), 2);
    }

    #[test]
    fn il_titolo_perde_la_parte_tra_parentesi() {
        let cartella = Cartella::nuova("titoli");
        cartella.file("pacman (Midway).zip");

        let giochi = leggi(&cartella, &["zip"], false);
        assert_eq!(giochi[0].key.as_str(), "pacman (Midway)");
        assert_eq!(giochi[0].title, "pacman");
    }

    #[test]
    fn senza_ricorsione_le_sottocartelle_restano_fuori() {
        let cartella = Cartella::nuova("piatto");
        cartella.file("uno.zip");
        cartella.file("psikyo/due.zip");

        let giochi = leggi(&cartella, &["zip"], false);
        assert_eq!(giochi.len(), 1);
        assert_eq!(giochi[0].key.as_str(), "uno");
    }

    #[test]
    fn con_la_ricorsione_entra_nelle_sottocartelle_e_le_ricorda() {
        let cartella = Cartella::nuova("ricorsivo");
        cartella.file("uno.zip");
        cartella.file("psikyo/due.zip");

        let giochi = leggi(&cartella, &["zip"], true);
        assert_eq!(giochi.len(), 2);

        let due = giochi.iter().find(|g| g.key.as_str() == "due").unwrap();
        assert_eq!(due.relative_path.as_ref().unwrap().to_string_lossy(), "psikyo");

        // Quella nella radice non ha sottocartella.
        let uno = giochi.iter().find(|g| g.key.as_str() == "uno").unwrap();
        assert!(uno.relative_path.is_none());
    }

    #[test]
    fn una_cartella_che_non_esiste_da_un_errore_chiaro() {
        let emu = emulatore(
            std::env::temp_dir().join("feel-scan-cartella-inesistente"),
            &["zip"],
            false,
        );
        let esito = leggi_cartella(&emu, &GamelistKey::new("mame-0"), false);
        assert!(esito.is_err());
    }

    #[test]
    fn una_cartella_vuota_da_una_lista_vuota() {
        let cartella = Cartella::nuova("vuota");
        assert!(leggi(&cartella, &["zip"], false).is_empty());
    }
}
