//! Filtri e ricerca nella lista.
//!
//! Su un cabinato non c'e' una tastiera: si scorre con la leva e si salta con
//! i tasti. Con mille giochi, scorrere fino a "Pac-Man" non e' praticabile,
//! quindi servono tre modi veloci per arrivarci: il salto per lettera, i
//! preferiti, e il filtro per categoria.
//!
//! La logica sta qui e non nel disegno, cosi' si verifica senza aprire una
//! finestra: con quali giochi si finisce, cosa succede quando il filtro non
//! lascia niente, e che il salto per lettera non resti fermo.

use std::collections::HashSet;

use feel_core::{Game, GameKey};

/// Cosa si sta mostrando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filtro {
    /// Tutti i giochi della lista.
    Tutti,
    /// Solo quelli segnati.
    Preferiti,
    /// Solo quelli di una categoria.
    Categoria(String),
}

impl Filtro {
    /// Come si chiama a video.
    pub fn etichetta(&self) -> String {
        match self {
            Self::Tutti => "tutti".into(),
            Self::Preferiti => "preferiti".into(),
            Self::Categoria(nome) => nome.clone(),
        }
    }
}

/// Gli indici dei giochi da mostrare, nell'ordine della lista.
pub fn visibili(giochi: &[Game], filtro: &Filtro, preferiti: &HashSet<GameKey>) -> Vec<usize> {
    giochi
        .iter()
        .enumerate()
        .filter(|(_, gioco)| match filtro {
            Filtro::Tutti => true,
            Filtro::Preferiti => preferiti.contains(&gioco.key),
            Filtro::Categoria(categoria) => gioco
                .category
                .as_deref()
                .map(|sua| sua.eq_ignore_ascii_case(categoria))
                .unwrap_or(false),
        })
        .map(|(indice, _)| indice)
        .collect()
}

/// Le categorie presenti, in ordine alfabetico.
///
/// Serve a costruire il giro dei filtri: senza, si potrebbe ciclare solo fra
/// "tutti" e "preferiti".
pub fn categorie(giochi: &[Game]) -> Vec<String> {
    let mut trovate: Vec<String> = Vec::new();
    for gioco in giochi {
        let Some(categoria) = gioco.category.as_deref() else {
            continue;
        };
        let categoria = categoria.trim();
        if categoria.is_empty() {
            continue;
        }
        if !trovate.iter().any(|gia| gia.eq_ignore_ascii_case(categoria)) {
            trovate.push(categoria.to_owned());
        }
    }
    trovate.sort_by_key(|nome| nome.to_lowercase());
    trovate
}

/// Il prossimo filtro del giro.
pub fn prossimo_filtro(attuale: &Filtro, giochi: &[Game]) -> Filtro {
    let mut giro = vec![Filtro::Tutti, Filtro::Preferiti];
    giro.extend(categorie(giochi).into_iter().map(Filtro::Categoria));

    let posizione = giro.iter().position(|filtro| filtro == attuale);
    match posizione {
        Some(indice) => giro[(indice + 1) % giro.len()].clone(),
        // Se il filtro attuale non e' piu' valido (per esempio la categoria non
        // esiste piu'), si riparte da "tutti" invece di restare bloccati.
        None => Filtro::Tutti,
    }
}

/// Il primo gioco che comincia con `lettera`, cercando da `da` in poi e poi
/// ricominciando dall'inizio.
///
/// Si cerca senza distinzione fra maiuscole e minuscole, e si ignorano gli
/// accenti comuni: chi preme "e" vuole "È".

pub fn salta_a_lettera(visibili: &[usize], giochi: &[Game], lettera: char) -> Option<usize> {
    if visibili.is_empty() {
        return None;
    }

    let cercata = normalizza(lettera);
    // Prima da qui in avanti: cosi' premere la stessa lettera piu' volte
    // scorre i giochi che cominciano cosi', invece di restare sul primo.
    visibili
        .iter()
        .position(|indice| {
            giochi
                .get(*indice)
                .and_then(|gioco| gioco.title.chars().next())
                .map(normalizza)
                == Some(cercata)
        })
        .map(|posizione| visibili[posizione])
}

/// Il gioco successivo o precedente dentro i visibili.
pub fn sposta(visibili: &[usize], attuale: usize, avanti: bool) -> Option<usize> {
    if visibili.is_empty() {
        return None;
    }
    let posizione = visibili.iter().position(|indice| *indice == attuale);

    Some(match posizione {
        Some(posizione) if avanti => visibili[(posizione + 1).min(visibili.len() - 1)],
        Some(posizione) => visibili[posizione.saturating_sub(1)],
        None => visibili[0],
    })
}

fn normalizza(carattere: char) -> char {
    match carattere.to_lowercase().next() {
        Some('à' | 'á' | 'â' | 'ä' | 'ã') => 'a',
        Some('è' | 'é' | 'ê' | 'ë') => 'e',
        Some('ì' | 'í' | 'î' | 'ï') => 'i',
        Some('ò' | 'ó' | 'ô' | 'ö' | 'õ') => 'o',
        Some('ù' | 'ú' | 'û' | 'ü') => 'u',
        Some('ç') => 'c',
        Some(altro) => altro,
        None => carattere,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gioco(nome: &str, titolo: &str, categoria: Option<&str>) -> Game {
        Game {
            key: GameKey::new(nome),
            gamelist: feel_core::GamelistKey::new("mame-0"),
            title: titolo.to_owned(),
            year: None,
            manufacturer: None,
            clone_of: None,
            bios: None,
            category: categoria.map(str::to_owned),
            rotation: None,
            controls: None,
            status: None,
            relative_path: None,
        }
    }

    fn libreria() -> Vec<Game> {
        vec![
            gioco("pacman", "Pac-Man (Midway)", Some("Maze")),
            gioco("galaga", "Galaga (Namco)", Some("Shooter")),
            gioco("dkong", "Donkey Kong", Some("Platform")),
            gioco("1943", "1943", Some("Shooter")),
        ]
    }

    #[test]
    fn il_filtro_tutti_non_toglie_niente() {
        let giochi = libreria();
        let visibili = visibili(&giochi, &Filtro::Tutti, &HashSet::new());
        assert_eq!(visibili, vec![0, 1, 2, 3]);
    }

    #[test]
    fn i_preferiti_mostrano_solo_quelli_segnati() {
        let giochi = libreria();
        let preferiti: HashSet<GameKey> = [GameKey::new("galaga"), GameKey::new("1943")]
            .into_iter()
            .collect();

        let visibili = visibili(&giochi, &Filtro::Preferiti, &preferiti);
        assert_eq!(visibili, vec![1, 3]);
    }

    #[test]
    fn il_filtro_per_categoria_non_distingue_maiuscole() {
        let giochi = libreria();
        let visibili = visibili(&giochi, &Filtro::Categoria("shooter".into()), &HashSet::new());
        assert_eq!(visibili, vec![1, 3]);
    }

    #[test]
    fn un_filtro_senza_risultati_non_e_un_errore() {
        let giochi = libreria();
        let visibili = visibili(&giochi, &Filtro::Preferiti, &HashSet::new());
        assert!(visibili.is_empty());

        // E non si resta bloccati: il giro va avanti lo stesso, alla prima
        // categoria. Con zero preferiti si vede la lista vuota per un attimo,
        // poi si preme di nuovo e si passa oltre.
        assert_eq!(
            prossimo_filtro(&Filtro::Preferiti, &giochi),
            Filtro::Categoria("Maze".into())
        );

        // Un filtro che non esiste piu' (la categoria e' sparita) riparte da
        // "tutti" invece di lasciare lo schermo vuoto per sempre.
        assert_eq!(
            prossimo_filtro(&Filtro::Categoria("Sparita".into()), &giochi),
            Filtro::Tutti
        );
    }

    #[test]
    fn il_giro_dei_filtri_comprende_le_categorie() {
        let giochi = libreria();
        assert_eq!(prossimo_filtro(&Filtro::Tutti, &giochi), Filtro::Preferiti);

        // Le categorie in ordine alfabetico: Maze, Platform, Shooter.
        let categorie = categorie(&giochi);
        assert_eq!(categorie, vec!["Maze", "Platform", "Shooter"]);
        assert_eq!(
            prossimo_filtro(&Filtro::Preferiti, &giochi),
            Filtro::Categoria("Maze".into())
        );
    }

    #[test]
    fn il_salto_per_lettera_trova_il_primo_che_comincia_cosi() {
        let giochi = libreria();
        let tutti = vec![0, 1, 2, 3];

        assert_eq!(salta_a_lettera(&tutti, &giochi, 'g'), Some(1));
        assert_eq!(salta_a_lettera(&tutti, &giochi, 'P'), Some(0));
        assert_eq!(salta_a_lettera(&tutti, &giochi, '1'), Some(3));
        // Una lettera che non c'e' non sposta niente.
        assert_eq!(salta_a_lettera(&tutti, &giochi, 'z'), None);
    }

    #[test]
    fn il_salto_ignora_gli_accenti() {
        let giochi = vec![gioco("eden", "Èden", None)];
        let tutti = vec![0];
        assert_eq!(salta_a_lettera(&tutti, &giochi, 'e'), Some(0));
    }

    #[test]
    fn lo_spostamento_resta_dentro_i_visibili() {
        let visibili = vec![1, 3];

        assert_eq!(sposta(&visibili, 1, true), Some(3));
        // L'ultimo non va oltre.
        assert_eq!(sposta(&visibili, 3, true), Some(3));
        // Il primo non va sotto.
        assert_eq!(sposta(&visibili, 1, false), Some(1));
        // Se la selezione non e' fra i visibili, si riparte dal primo.
        assert_eq!(sposta(&visibili, 0, true), Some(1));
        // Con la lista vuota non si fa niente.
        assert_eq!(sposta(&[], 0, true), None);
    }
}
