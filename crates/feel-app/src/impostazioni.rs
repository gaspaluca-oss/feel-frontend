//! La schermata delle impostazioni dentro al frontend.
//!
//! In FEEL 1.x per cambiare una cosa bisognava sapere in quale dei venti file
//! `.ini` stava, e trovare la chiave giusta fra centinaia di righe. Qui si
//! preme un tasto, si scorre, si cambia: il file si aggiorna da solo.
//!
//! La logica sta qui e non nel ciclo di disegno perche' cosi' si puo' testare
//! senza aprire una finestra: quali voci ci sono, come si formattano i valori,
//! e cosa succede quando si cambia l'ultimo valore di una lista.

use feel_config::{Rotate, Settings};
use feel_core::Shader;

/// Cosa si puo' cambiare dalla schermata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    Fullscreen,
    Shader,
    Rotate,
    Vsync,
    AudioMaster,
    ScreensaverEnabled,
    ScreensaverDelay,
    ScreensaverSlide,
}

/// Una riga della schermata.
pub struct Voce {
    pub campo: Campo,
    pub etichetta: &'static str,
    /// Spiegazione breve, mostrata sotto.
    pub nota: &'static str,
}

/// Le voci, nell'ordine in cui compaiono.
pub const VOCI: &[Voce] = &[
    Voce { campo: Campo::Fullscreen, etichetta: "Schermo intero", nota: "finestra intera invece che ridotta" },
    Voce { campo: Campo::Shader, etichetta: "Effetto schermo", nota: "crt per i tubi, lcd per i pannelli piatti" },
    Voce { campo: Campo::Rotate, etichetta: "Rotazione", nota: "per i cabinati con lo schermo girato" },
    Voce { campo: Campo::Vsync, etichetta: "Sincronia verticale", nota: "evita lo strappo dell'immagine" },
    Voce { campo: Campo::AudioMaster, etichetta: "Volume generale", nota: "percentuale" },
    Voce { campo: Campo::ScreensaverEnabled, etichetta: "Modalita' attrattiva", nota: "il cabinato si anima da solo" },
    Voce { campo: Campo::ScreensaverDelay, etichetta: "Attesa prima di partire", nota: "secondi di inattivita'" },
    Voce { campo: Campo::ScreensaverSlide, etichetta: "Tempo per gioco", nota: "secondi che ogni gioco resta a video" },
];

/// Cosa bisogna fare dopo aver cambiato un valore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esito {
    /// Niente di speciale: basta ridisegnare.
    Ridisegna,
    /// Va applicato alla finestra (schermo intero).
    Finestra,
}

/// Il valore come va mostrato.
pub fn valore(campo: Campo, impostazioni: &Settings) -> String {
    match campo {
        Campo::Fullscreen => si_no(impostazioni.video.fullscreen),
        Campo::Shader => match impostazioni.video.shader {
            Shader::None => "nessuno".into(),
            Shader::Crt => "crt".into(),
            Shader::Lcd => "lcd".into(),
            Shader::Scanlines => "scanline".into(),
        },
        Campo::Rotate => match impostazioni.video.rotate {
            Rotate::None => "nessuna".into(),
            Rotate::Left => "sinistra".into(),
            Rotate::Right => "destra".into(),
        },
        Campo::Vsync => si_no(impostazioni.video.vsync),
        Campo::AudioMaster => format!("{}%", impostazioni.audio.master),
        Campo::ScreensaverEnabled => si_no(impostazioni.screensaver.enabled),
        Campo::ScreensaverDelay => format!("{} s", impostazioni.screensaver.delay_secs),
        Campo::ScreensaverSlide => format!("{} s", impostazioni.screensaver.slide_secs),
    }
}

/// Applica un cambiamento. `verso` e' +1 o -1.
///
/// Le liste girano: dopo l'ultimo valore si torna al primo, cosi' con due soli
/// tasti si passa da "nessuno" a "crt" senza dover indovinare quante volte
/// premere.
pub fn cambia(campo: Campo, verso: i32, impostazioni: &mut Settings) -> Esito {
    let positivo = verso >= 0;

    match campo {
        Campo::Fullscreen => {
            impostazioni.video.fullscreen = !impostazioni.video.fullscreen;
            Esito::Finestra
        }
        Campo::Shader => {
            let ordine = [Shader::None, Shader::Crt, Shader::Lcd, Shader::Scanlines];
            let attuale = ordine
                .iter()
                .position(|s| *s == impostazioni.video.shader)
                .unwrap_or(0);
            let nuovo = gira(attuale, ordine.len(), positivo);
            impostazioni.video.shader = ordine[nuovo];
            Esito::Ridisegna
        }
        Campo::Rotate => {
            let ordine = [Rotate::None, Rotate::Left, Rotate::Right];
            let attuale = ordine
                .iter()
                .position(|r| *r == impostazioni.video.rotate)
                .unwrap_or(0);
            let nuovo = gira(attuale, ordine.len(), positivo);
            impostazioni.video.rotate = ordine[nuovo];
            Esito::Ridisegna
        }
        Campo::Vsync => {
            impostazioni.video.vsync = !impostazioni.video.vsync;
            Esito::Ridisegna
        }
        Campo::AudioMaster => {
            let passo = 10i32;
            let valore = impostazioni.audio.master as i32 + if positivo { passo } else { -passo };
            impostazioni.audio.master = valore.clamp(0, 100) as u8;
            Esito::Ridisegna
        }
        Campo::ScreensaverEnabled => {
            impostazioni.screensaver.enabled = !impostazioni.screensaver.enabled;
            Esito::Ridisegna
        }
        Campo::ScreensaverDelay => {
            let valore = impostazioni.screensaver.delay_secs as i32 + if positivo { 10 } else { -10 };
            impostazioni.screensaver.delay_secs = valore.clamp(10, 600) as u32;
            Esito::Ridisegna
        }
        Campo::ScreensaverSlide => {
            let valore = impostazioni.screensaver.slide_secs as i32 + if positivo { 1 } else { -1 };
            impostazioni.screensaver.slide_secs = valore.clamp(2, 60) as u32;
            Esito::Ridisegna
        }
    }
}

fn gira(attuale: usize, quanti: usize, avanti: bool) -> usize {
    if quanti == 0 {
        return 0;
    }
    if avanti {
        (attuale + 1) % quanti
    } else {
        (attuale + quanti - 1) % quanti
    }
}

fn si_no(valore: bool) -> String {
    if valore { "si'".into() } else { "no".into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_liste_girano_in_entrambi_i_versi() {
        let mut impostazioni = Settings::default();
        impostazioni.video.shader = Shader::None;

        cambia(Campo::Shader, 1, &mut impostazioni);
        assert_eq!(impostazioni.video.shader, Shader::Crt);
        cambia(Campo::Shader, 1, &mut impostazioni);
        assert_eq!(impostazioni.video.shader, Shader::Lcd);
        cambia(Campo::Shader, 1, &mut impostazioni);
        assert_eq!(impostazioni.video.shader, Shader::Scanlines);
        // Dopo l'ultimo si torna al primo.
        cambia(Campo::Shader, 1, &mut impostazioni);
        assert_eq!(impostazioni.video.shader, Shader::None);
        // E indietro si va all'ultimo.
        cambia(Campo::Shader, -1, &mut impostazioni);
        assert_eq!(impostazioni.video.shader, Shader::Scanlines);
    }

    #[test]
    fn i_numeri_si_fermano_ai_limiti() {
        let mut impostazioni = Settings::default();
        impostazioni.audio.master = 0;
        cambia(Campo::AudioMaster, -1, &mut impostazioni);
        assert_eq!(impostazioni.audio.master, 0, "non scende sotto zero");

        impostazioni.audio.master = 100;
        cambia(Campo::AudioMaster, 1, &mut impostazioni);
        assert_eq!(impostazioni.audio.master, 100, "non sale sopra cento");

        impostazioni.screensaver.slide_secs = 2;
        cambia(Campo::ScreensaverSlide, -1, &mut impostazioni);
        assert_eq!(impostazioni.screensaver.slide_secs, 2);
    }

    #[test]
    fn lo_schermo_intero_chiede_alla_finestra_di_adeguarsi() {
        let mut impostazioni = Settings::default();
        let prima = impostazioni.video.fullscreen;
        assert_eq!(cambia(Campo::Fullscreen, 1, &mut impostazioni), Esito::Finestra);
        assert_ne!(impostazioni.video.fullscreen, prima);
    }

    #[test]
    fn i_valori_si_leggono_in_italiano() {
        let mut impostazioni = Settings::default();
        impostazioni.video.shader = Shader::Crt;
        impostazioni.audio.master = 70;
        impostazioni.screensaver.delay_secs = 60;

        assert_eq!(valore(Campo::Shader, &impostazioni), "crt");
        assert_eq!(valore(Campo::AudioMaster, &impostazioni), "70%");
        assert_eq!(valore(Campo::ScreensaverDelay, &impostazioni), "60 s");
        assert_eq!(valore(Campo::Fullscreen, &impostazioni), "si'");
    }

    #[test]
    fn ogni_voce_corrisponde_a_un_campo_distinto() {
        // Un campo ripetuto vorrebbe dire due righe che cambiano la stessa cosa
        // e una che non cambia niente.
        for (indice, voce) in VOCI.iter().enumerate() {
            for altra in &VOCI[indice + 1..] {
                assert_ne!(voce.campo, altra.campo);
            }
        }
    }
}
