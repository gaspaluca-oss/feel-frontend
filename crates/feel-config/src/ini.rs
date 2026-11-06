//! Lettore dei file di configurazione di FEEL 1.x.
//!
//! Il formato **non e' un ini**: non c'e' il `=`, la chiave e il valore sono
//! separati da spazi, e il valore puo' contenere spazi e backslash
//! (`C:\Program Files\mame\roms`). I commenti iniziano con `#` o `;`.
//!
//! Le chiavi sono normalizzate in minuscolo, come fa FEEL 1.x.

use std::collections::BTreeMap;
use std::path::Path;

/// Chiavi e valori di un file, con le chiavi in minuscolo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IniMap(BTreeMap<String, String>);

impl IniMap {
    /// Legge un file. Se non esiste ritorna una mappa vuota.
    ///
    /// Le configurazioni di FEEL 1.x sono spesso parziali: un file mancante
    /// significa "usa i valori predefiniti", non un errore.
    pub fn from_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Self::parse(&text)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err),
        }
    }

    pub fn parse(text: &str) -> Self {
        let mut map = BTreeMap::new();

        for raw in text.lines() {
            // I file di FEEL iniziano con il BOM.
            let line = raw.trim_start_matches('\u{feff}').trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            let (key, value) = match line.find(char::is_whitespace) {
                Some(index) => (&line[..index], line[index..].trim()),
                None => (line, ""),
            };

            if !is_valid_key(key) {
                continue;
            }

            map.insert(key.to_ascii_lowercase(), value.to_owned());
        }

        Self(map)
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(&key.to_ascii_lowercase()).map(String::as_str)
    }

    /// Il valore, o stringa vuota se assente.
    pub fn string(&self, key: &str) -> String {
        self.get(key).unwrap_or_default().to_owned()
    }

    /// Il valore, o `None` se assente o vuoto.
    ///
    /// Distinguere "assente" da "vuoto" serve: in FEEL 1.x un campo vuoto e un
    /// campo mancante non significano la stessa cosa nei percorsi delle immagini.
    pub fn non_empty(&self, key: &str) -> Option<String> {
        let value = self.get(key).unwrap_or("").trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_owned())
        }
    }

    /// Un flag di FEEL: `1` e' vero, tutto il resto e' falso.
    pub fn flag(&self, key: &str, default: bool) -> bool {
        match self.get(key) {
            Some(value) => value.trim().starts_with('1'),
            None => default,
        }
    }

    /// Un intero. Se il valore c'e' ma non e' un numero, ritorna il default.
    pub fn int(&self, key: &str, default: i64) -> i64 {
        self.get(key)
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Una lista separata da virgole, senza voci vuote.
    pub fn list(&self, key: &str) -> Vec<String> {
        split_list(self.get(key).unwrap_or(""))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Tutte le coppie, in ordine di chiave.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Divide una lista separata da virgole, scartando le voci vuote.
pub fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Una chiave valida inizia con una lettera o `_` e prosegue con lettere, cifre o `_`.
fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESEMPIO: &str = "\u{feff}####################################################################\n\
        # F.E.E.L. - FrontEnd (Emulator Launcher)\n\
        ####################################################################\n\
        \n\
        emulator_title                          M.A.M.E.\n\
        \n\
        rom_path                                C:\\Program Files\\mame\\roms\n\
        emulator_commandline                    [emulator_path]\\mame.exe\n\
        emulator_arguments                      [rom_name]\n\
        list_type                               1\n\
        show_clones                             1\n\
        #emulator_useshell                       0\n\
        pre_emulator_app_commandline            \n\
        input_controls                          joy4way - 1P - 4Bt\n";

    #[test]
    fn legge_chiave_e_valore_separati_da_spazi() {
        let map = IniMap::parse(ESEMPIO);
        assert_eq!(map.string("emulator_title"), "M.A.M.E.");
        assert_eq!(map.int("list_type", 0), 1);
        assert!(map.flag("show_clones", false));
    }

    #[test]
    fn il_valore_puo_contenere_spazi_e_backslash() {
        let map = IniMap::parse(ESEMPIO);
        assert_eq!(map.string("rom_path"), r"C:\Program Files\mame\roms");
        assert_eq!(map.string("input_controls"), "joy4way - 1P - 4Bt");
    }

    #[test]
    fn ignora_i_commenti_e_il_bom() {
        let map = IniMap::parse(ESEMPIO);
        assert!(map.get("emulator_useshell").is_none());
        assert_eq!(map.len(), 8);
    }

    #[test]
    fn le_chiavi_sono_case_insensitive() {
        let map = IniMap::parse("Rom_Path  C:\\roms\n");
        assert_eq!(map.string("rom_path"), r"C:\roms");
        assert_eq!(map.string("ROM_PATH"), r"C:\roms");
    }

    #[test]
    fn distingue_vuoto_da_assente() {
        let map = IniMap::parse(ESEMPIO);
        // Presente ma vuoto.
        assert_eq!(map.get("pre_emulator_app_commandline"), Some(""));
        assert_eq!(map.non_empty("pre_emulator_app_commandline"), None);
        // Assente del tutto.
        assert_eq!(map.get("mai_visto"), None);
        assert_eq!(map.string("mai_visto"), "");
    }

    #[test]
    fn le_liste_scartano_le_voci_vuote() {
        let map = IniMap::parse("platform_list   arcade, console , , computer\n");
        assert_eq!(map.list("platform_list"), vec!["arcade", "console", "computer"]);
        // Caso reale: `emulator_list` vuoto in all_emu.ini.
        let vuoto = IniMap::parse("emulator_list\n");
        assert!(vuoto.list("emulator_list").is_empty());
    }

    #[test]
    fn un_valore_non_numerico_non_e_un_errore() {
        let map = IniMap::parse("list_type   boh\n");
        assert_eq!(map.int("list_type", 7), 7);
    }

    #[test]
    fn ignora_le_righe_che_non_sono_chiavi() {
        let map = IniMap::parse("=== qualcosa ===\n123\nchiave valida\n");
        assert_eq!(map.len(), 1);
        assert_eq!(map.string("chiave"), "valida");
    }

    #[test]
    fn un_file_mancante_e_una_mappa_vuota() {
        let map = IniMap::from_file("questa/cartella/non/esiste.ini").unwrap();
        assert!(map.is_empty());
    }
}
