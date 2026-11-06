//! Configurazione di FEEL 2.0.
//!
//! Due responsabilita':
//!
//! - [`ini`] — il lettore dei file di configurazione di FEEL 1.x, che non sono
//!   ini veri e propri: separano chiave e valore con spazi e non con `=`.
//! - [`feel1`] — l'importatore che trasforma una installazione FEEL 1.x nel
//!   modello di [`feel_core`].
//!
//! L'importatore non indovina e non "aggiusta": quando un dato non e'
//! interpretabile lo segnala in [`feel1::Feel1Report`] e va avanti.

pub mod feel1;
pub mod ini;
pub mod paths;

pub use feel1::{
    load, load_with, Feel1Import, Feel1Report, Feel1Settings, ImportError, ImportOptions,
};
pub use ini::IniMap;
pub use paths::PathRules;
