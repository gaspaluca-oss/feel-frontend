//! Modello dati di FEEL 2.0.
//!
//! Questo crate non tocca Windows e non tocca il disco: si compila e si testa
//! ovunque, e contiene solo i tipi che descrivono una libreria di giochi.
//!
//! La separazione non e' formale. Il grosso della logica di FEEL 1.x
//! (liste, filtri, statistiche, configurazione) era mescolato al rendering e
//! alle API di Windows, ed e' il motivo per cui il frontend era impossibile da
//! testare. Qui quella parte e' isolata.

pub mod model;
pub mod text;

pub use model::{
    ArtworkPaths, Emulator, EmulatorKey, EmulatorRef, Game, GameKey, Gamelist, GamelistKey,
    LayoutKey, Library, Platform, PlatformKey, PlayStats, Rotation, Timestamp,
};
pub use text::cleanup_name;
