//! Gamepad e joystick.
//!
//! Un cabinato si usa con il joystick, non con la tastiera. I joystick USB
//! arrivano come gamepad standard, mentre gli encoder arcade si presentano
//! spesso come tastiera: i primi li legge questo modulo, i secondi li legge
//! gia' la finestra come tasti.
//!
//! La parte delicata non e' leggere il gamepad, e' **la leva analogica**: se
//! ogni fotogramma in cui il valore supera la soglia contasse come una pressione,
//! tenere la leva in giu' per mezzo secondo farebbe saltare cinquanta giochi.
//! Serve isteresi: si attiva sopra una soglia, si riarma solo quando la leva
//! torna vicino al centro.

/// Un comando del frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Azione {
    Su,
    Giu,
    Sinistra,
    Destra,
    Conferma,
    Indietro,
    /// Apre la schermata delle impostazioni.
    Menu,
}

/// Una leva analogica con isteresi.
///
/// `soglia` e' dove scatta, `rilascio` dove si riarma. Devono stare in
/// quest'ordine (rilascio < soglia), altrimenti si riarma subito e si torna a
/// contare una pressione per fotogramma.
#[derive(Debug, Clone)]
pub struct Leva {
    negativo: Azione,
    positivo: Azione,
    stato: i8,
}

impl Leva {
    pub fn nuova(negativo: Azione, positivo: Azione) -> Self {
        Self { negativo, positivo, stato: 0 }
    }

    /// Aggiorna la leva. Ritorna l'azione **solo quando cambia stato**.
    pub fn aggiorna(&mut self, valore: f32, soglia: f32, rilascio: f32) -> Option<Azione> {
        let nuovo = if valore > soglia {
            1
        } else if valore < -soglia {
            -1
        } else if valore.abs() < rilascio {
            0
        } else {
            // Fra rilascio e soglia si resta come si era: e' l'isteresi.
            self.stato
        };

        if nuovo == self.stato {
            return None;
        }
        self.stato = nuovo;

        match nuovo {
            1 => Some(self.positivo),
            -1 => Some(self.negativo),
            _ => None,
        }
    }

    pub fn riarma(&mut self) {
        self.stato = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOGLIA: f32 = 0.6;
    const RILASCIO: f32 = 0.35;

    #[test]
    fn la_leva_scatta_una_volta_sola() {
        let mut leva = Leva::nuova(Azione::Su, Azione::Giu);

        // Primo movimento oltre la soglia: un'azione.
        assert_eq!(leva.aggiorna(-1.0, SOGLIA, RILASCIO), Some(Azione::Su));
        // Tenuta in giu': nessuna azione, altrimenti si scorre di cinquanta.
        assert_eq!(leva.aggiorna(-1.0, SOGLIA, RILASCIO), None);
        assert_eq!(leva.aggiorna(-0.9, SOGLIA, RILASCIO), None);
        // Nemmeno nella zona di isteresi.
        assert_eq!(leva.aggiorna(-0.5, SOGLIA, RILASCIO), None);
    }

    #[test]
    fn si_riarma_solo_tornando_al_centro() {
        let mut leva = Leva::nuova(Azione::Su, Azione::Giu);
        leva.aggiorna(-1.0, SOGLIA, RILASCIO);

        // Tornata al centro: si riarma, senza produrre un'azione.
        assert_eq!(leva.aggiorna(0.0, SOGLIA, RILASCIO), None);
        // Ora un nuovo movimento conta di nuovo.
        assert_eq!(leva.aggiorna(-1.0, SOGLIA, RILASCIO), Some(Azione::Su));
    }

    #[test]
    fn cambiare_direzione_conta_subito() {
        let mut leva = Leva::nuova(Azione::Su, Azione::Giu);
        leva.aggiorna(-1.0, SOGLIA, RILASCIO);

        // Dal basso all'alto, passando per la zona di isteresi: cambia stato.
        assert_eq!(leva.aggiorna(1.0, SOGLIA, RILASCIO), Some(Azione::Giu));
    }

    #[test]
    fn il_rumore_vicino_al_centro_non_conta() {
        let mut leva = Leva::nuova(Azione::Sinistra, Azione::Destra);
        for valore in [0.1, -0.2, 0.05, -0.3, 0.0] {
            assert_eq!(leva.aggiorna(valore, SOGLIA, RILASCIO), None);
        }
    }
}
