//! Riscritture di percorso per l'importazione.
//!
//! FEEL 2.0 gira su Windows come FEEL 1.x, quindi **di norma i percorsi restano
//! quelli che sono**: `C:\Program Files\mame\roms` continua a valere, e
//! riscriverlo sarebbe un danno. Queste regole servono solo a chi ha spostato
//! le rom o ha cambiato macchina.
//!
//! Senza regole configurate il percorso viene solo normalizzato. **L'uscita usa
//! sempre `/`**, anche su Windows: le API di Windows accettano entrambi i
//! separatori, e cosi' la libreria si legge e si scrive senza cambiare aspetto
//! a seconda del sistema che l'ha toccata.

/// Un insieme di riscritture di prefisso, applicate alla piu' specifica.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathRules {
    /// `(prefisso normalizzato in minuscolo, prefisso di destinazione)`.
    mappings: Vec<(String, String)>,
}

impl PathRules {
    pub fn new() -> Self {
        Self::default()
    }

    /// Riscrive tutto cio' che sta sotto `from` come `to`.
    ///
    /// ```
    /// # use feel_config::PathRules;
    /// let rules = PathRules::new().remap(r"C:\Games", r"D:\MAME\roms");
    /// assert_eq!(rules.translate(r"C:\Games\mame"), "D:/MAME/roms/mame");
    /// ```
    pub fn remap(mut self, from: &str, to: &str) -> Self {
        let key = normalize(from).to_ascii_lowercase();
        let key = key.trim_end_matches('/').to_owned();
        self.mappings.push((key, normalize(to)));
        self
    }

    /// Normalizza `path` e applica, se esiste, la regola piu' specifica.
    pub fn translate(&self, path: &str) -> String {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        let normalized = normalize(trimmed);
        let lowered = normalized.to_ascii_lowercase();

        let mut best: Option<(&str, &str)> = None;
        for (from, to) in &self.mappings {
            let matches =
                lowered.as_str() == from.as_str() || lowered.starts_with(&format!("{}/", from));
            if !matches {
                continue;
            }
            // La regola piu' lunga vince: `C:\Games\MAME` deve battere `C:\Games`.
            if best.map_or(true, |(current, _)| from.len() > current.len()) {
                best = Some((from.as_str(), to.as_str()));
            }
        }

        match best {
            Some((from, to)) => {
                format!("{}{}", to.trim_end_matches('/'), &normalized[from.len()..])
            }
            None => normalized,
        }
    }
}

/// Uniforma i separatori a `/` e toglie spazi ai bordi.
pub fn normalize(path: &str) -> String {
    path.trim().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn senza_regole_normalizza_e_basta() {
        let rules = PathRules::new();
        assert_eq!(
            rules.translate(r"C:\Program Files\mame\roms"),
            "C:/Program Files/mame/roms"
        );
    }

    #[test]
    fn riscrive_solo_il_prefisso() {
        let rules = PathRules::new().remap(r"C:\Program Files\mame", r"D:\MAME");
        assert_eq!(rules.translate(r"C:\Program Files\mame\roms"), "D:/MAME/roms");
        assert_eq!(rules.translate(r"C:\Altro\roms"), "C:/Altro/roms");
    }

    #[test]
    fn la_regola_piu_specifica_vince() {
        let rules = PathRules::new()
            .remap(r"C:\Games", r"D:\GENERICO")
            .remap(r"C:\Games\MAME", r"D:\SPECIFICO");
        assert_eq!(rules.translate(r"C:\Games\MAME\roms"), "D:/SPECIFICO/roms");
        assert_eq!(rules.translate(r"C:\Games\SNES\roms"), "D:/GENERICO/SNES/roms");
    }

    #[test]
    fn non_confonde_un_prefisso_con_un_inizio_di_parola() {
        let rules = PathRules::new().remap(r"C:\Game", r"D:\X");
        assert_eq!(rules.translate(r"C:\Games\roms"), "C:/Games/roms");
    }

    #[test]
    fn il_confronto_e_case_insensitive() {
        let rules = PathRules::new().remap(r"c:\program files\MAME", r"D:\mame");
        assert_eq!(rules.translate(r"C:\PROGRAM FILES\mame\roms"), "D:/mame/roms");
    }

    #[test]
    fn la_stringa_vuota_resta_vuota() {
        let rules = PathRules::new().remap(r"C:\X", r"D:\Y");
        assert_eq!(rules.translate("   "), "");
        assert_eq!(rules.translate(""), "");
    }
}
