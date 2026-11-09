//! Video snap.
//!
//! I video dei giochi sono mp4: decodificarli in Rust puro non e' realistico,
//! quindi si delega a **FFmpeg come processo esterno**, che manda i fotogrammi
//! grezzi su stdout. E' la stessa scelta che fanno molti frontend, ed evita di
//! legare il binario a una versione specifica delle librerie FFmpeg.
//!
//! Il comando e' un parametro e non una costante: cosi' si puo' sostituire con
//! un altro decodificatore, e soprattutto si puo' provare questa logica con uno
//! stub, senza avere un video vero.
//!
//! Se FFmpeg non c'e', il frontend lo dice e mostra l'immagine ferma. Un video
//! che non parte non deve impedire di usare il cabinato.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Un video aperto, da cui si leggono i fotogrammi uno alla volta.
pub struct Video {
    figlio: Child,
    buffer: Vec<u8>,
}

impl Video {
    /// Apre un video usando FFmpeg.
    pub fn da_ffmpeg(percorso: &Path, larghezza: u32, altezza: u32) -> std::io::Result<Self> {
        // `-r 30` forza un ritmo fisso: senza, i video a 60 fps farebbero
        // scorrere il doppio dei fotogrammi e il tempo di riproduzione
        // dipenderebbe dal file.
        let argomenti: Vec<String> = vec![
            "-loglevel".into(),
            "error".into(),
            "-i".into(),
            percorso.to_string_lossy().into_owned(),
            "-f".into(),
            "rawvideo".into(),
            "-pix_fmt".into(),
            "rgba".into(),
            "-s".into(),
            format!("{larghezza}x{altezza}"),
            "-r".into(),
            "30".into(),
            "-".into(),
        ];
        Self::avvia(Path::new("ffmpeg"), &argomenti, larghezza, altezza)
    }

    /// Avvia un decodificatore qualsiasi che scriva RGBA su stdout.
    pub fn avvia(
        programma: &Path,
        argomenti: &[String],
        larghezza: u32,
        altezza: u32,
    ) -> std::io::Result<Self> {
        let figlio = Command::new(programma)
            .args(argomenti)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        Ok(Self {
            figlio,
            buffer: vec![0; (larghezza * altezza * 4) as usize],
        })
    }

    /// Legge il fotogramma successivo.
    ///
    /// Ritorna `false` a fine video: chi chiama deve tornare all'immagine ferma
    /// o ricominciare, non restare con l'ultimo fotogramma per sempre.
    pub fn leggi_frame(&mut self) -> std::io::Result<bool> {
        let Some(stdout) = self.figlio.stdout.as_mut() else {
            return Ok(false);
        };

        match stdout.read_exact(&mut self.buffer) {
            Ok(()) => Ok(true),
            // Un fotogramma a meta' e' la fine del video, non un errore.
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// I pixel dell'ultimo fotogramma letto, in RGBA.
    pub fn pixel(&self) -> &[u8] {
        &self.buffer
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        // Senza questo, ogni cambio di gioco lascerebbe un ffmpeg che continua
        // a decodificare in memoria.
        let _ = self.figlio.kill();
        let _ = self.figlio.wait();
    }
}

/// Cerca il video di un gioco nella cartella dei video snap.
pub fn trova_video(cartella: &Path, nome: &str) -> Option<PathBuf> {
    for estensione in ["mp4", "avi", "mkv", "webm", "MP4"] {
        let candidato = cartella.join(format!("{nome}.{estensione}"));
        if candidato.is_file() {
            return Some(candidato);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un decodificatore finto che scrive su stdout il contenuto di un file.
    ///
    /// Serve a verificare la lettura dei fotogrammi senza avere FFmpeg ne' un
    /// video vero: la parte che puo' rompersi e' il conteggio dei byte, non il
    /// decodificatore.
    fn decodificatore_finto(dati: &[u8]) -> (PathBuf, Vec<String>) {
        let cartella = std::env::temp_dir().join(format!(
            "feel-video-{}-{}",
            std::process::id(),
            dati.len()
        ));
        std::fs::create_dir_all(&cartella).unwrap();
        let percorso = cartella.join("finti.rgba");
        std::fs::write(&percorso, dati).unwrap();

        if cfg!(windows) {
            (
                PathBuf::from("cmd.exe"),
                vec![
                    "/C".into(),
                    "type".into(),
                    percorso.to_string_lossy().into_owned(),
                ],
            )
        } else {
            (
                PathBuf::from("/bin/cat"),
                vec![percorso.to_string_lossy().into_owned()],
            )
        }
    }

    #[test]
    fn legge_un_fotogramma_alla_volta() {
        // Due fotogrammi da 2x2 pixel RGBA, con valori riconoscibili.
        let dati: Vec<u8> = (0..(2 * 2 * 2 * 4) as u16).map(|i| i as u8).collect();
        let (programma, argomenti) = decodificatore_finto(&dati);

        let mut video = Video::avvia(&programma, &argomenti, 2, 2).expect("decodificatore");

        assert!(video.leggi_frame().unwrap(), "primo fotogramma");
        assert_eq!(&video.pixel()[..4], &dati[..4]);

        assert!(video.leggi_frame().unwrap(), "secondo fotogramma");
        assert_eq!(&video.pixel()[..4], &dati[16..20]);

        // Fine del flusso: non e' un errore, e' la fine del video.
        assert!(!video.leggi_frame().unwrap());
    }

    #[test]
    fn un_fotogramma_incompleto_chiude_il_video() {
        // Un byte in meno di un fotogramma: fine del video, non panico.
        let dati = vec![0u8; (2 * 2 * 4) - 1];
        let (programma, argomenti) = decodificatore_finto(&dati);
        let mut video = Video::avvia(&programma, &argomenti, 2, 2).expect("decodificatore");

        assert!(!video.leggi_frame().unwrap());
    }

    #[test]
    fn un_decodificatore_inesistente_da_un_errore_subito() {
        let argomenti: Vec<String> = Vec::new();
        let esito = Video::avvia(Path::new("programma-che-non-esiste-xyz"), &argomenti, 2, 2);
        assert!(esito.is_err(), "l'errore deve arrivare all'apertura, non dopo");
    }

    #[test]
    fn trova_il_video_tra_le_estensioni() {
        let cartella =
            std::env::temp_dir().join(format!("feel-video-trova-{}", std::process::id()));
        std::fs::create_dir_all(&cartella).unwrap();
        std::fs::write(cartella.join("pacman.avi"), b"finto").unwrap();

        let trovato = trova_video(&cartella, "pacman").expect("il video deve saltare fuori");
        assert!(trovato.ends_with("pacman.avi"));
        assert!(trova_video(&cartella, "galaga").is_none());

        let _ = std::fs::remove_dir_all(&cartella);
    }
}
