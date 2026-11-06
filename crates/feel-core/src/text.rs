//! Manipolazione dei nomi, con le stesse regole di FEEL 1.x.
//!
//! Queste regole sono riprodotte fedelmente e non "migliorate": i nomi delle
//! liste esistenti devono restare identici, altrimenti l'importazione cambia
//! cio' che l'utente vede a video.

/// Il titolo senza la parte tra parentesi.
///
/// Riproduce `RomDesc.Description` di FEEL 1.x: si taglia dal primo `(` o `[`,
/// ma solo se non e' il primo carattere. La condizione non e' un dettaglio:
/// e' il motivo per cui un titolo che *inizia* con una parentesi resta intero.
///
/// ```
/// # use feel_core::cleanup_name;
/// assert_eq!(cleanup_name("Pac-Man (Midway)"), "Pac-Man");
/// assert_eq!(cleanup_name("Puck Man (Japan set 1)"), "Puck Man");
/// assert_eq!(cleanup_name("Super Game [bootleg] (1985)"), "Super Game");
/// // La parentesi in prima posizione non viene toccata.
/// assert_eq!(cleanup_name("(test) abc"), "(test) abc");
/// ```
pub fn cleanup_name(value: &str) -> String {
    let end = match value.find('(') {
        Some(index) if index > 0 => index,
        _ => value.len(),
    };
    let end = match value.find('[') {
        Some(index) if index > 0 => end.min(index),
        _ => end,
    };
    value[..end].trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taglia_alla_parentesi_tonda() {
        assert_eq!(cleanup_name("Pac-Man (Midway)"), "Pac-Man");
        assert_eq!(cleanup_name("Puck Man (Japan set 1)"), "Puck Man");
    }

    #[test]
    fn taglia_alla_parentesi_quadra() {
        assert_eq!(cleanup_name("Super Game [bootleg]"), "Super Game");
    }

    #[test]
    fn vince_quella_che_viene_prima() {
        assert_eq!(cleanup_name("Super Game [bootleg] (1985)"), "Super Game");
        assert_eq!(cleanup_name("Super Game (1985) [bootleg]"), "Super Game");
    }

    #[test]
    fn la_parentesi_in_prima_posizione_non_conta() {
        // Quirk ereditato: e' cosi' che FEEL 1.x si comporta, e i nomi
        // gia' mostrati agli utenti devono restare gli stessi.
        assert_eq!(cleanup_name("(test) abc"), "(test) abc");
        assert_eq!(cleanup_name("[test] abc"), "[test] abc");
    }

    #[test]
    fn senza_parentesi_resta_uguale() {
        assert_eq!(cleanup_name("Pac-Man"), "Pac-Man");
        assert_eq!(cleanup_name(""), "");
    }

    #[test]
    fn toglie_gli_spazi_attorno() {
        assert_eq!(cleanup_name("  Pac-Man  "), "Pac-Man");
        assert_eq!(cleanup_name("Pac-Man  (Midway)"), "Pac-Man");
    }

    #[test]
    fn regge_i_caratteri_non_ascii() {
        // Il taglio avviene su un confine di carattere, non di byte.
        assert_eq!(cleanup_name("Città (edizione)"), "Città");
        assert_eq!(cleanup_name("サッカー (Japan)"), "サッカー");
    }
}
