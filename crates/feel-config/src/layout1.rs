//! Conversione dei layout di FEEL 1.x nel formato di FEEL 2.0.
//!
//! Un `layout.ini` di FEEL 1.x descrive rettangoli in pixel su una risoluzione
//! fissa. Qui quei rettangoli diventano ancoraggi con spostamenti in frazione di
//! schermo.
//!
//! La conversione **non perde niente**: [`super::layout1::convert`] sceglie per
//! ogni elemento l'ancoraggio piu' vicino, e risolvendo il risultato alla stessa
//! risoluzione di partenza si riottiene esattamente il rettangolo originale.
//! C'e' un test che lo verifica elemento per elemento — se un giorno la formula
//! cambia, il test lo dice invece di lasciare i layout degli utenti spostati di
//! qualche pixel senza che nessuno se ne accorga.

use std::collections::BTreeSet;
use std::path::Path;

use feel_core::{
    Anchor, ArtworkSlot, Background, Color, Dimension, Element, ElementKind, Frame, Horizontal,
    LabelKind, Layout, MenuStyle, Placement, Point, ScreenSaverStyle, ScreenSize, Size, TextStyle,
    Vertical, LAYOUT_VERSION,
};

use crate::feel1::ImportError;
use crate::ini::IniMap;

/// Risoluzione usata da FEEL 1.x quando il layout non la dichiara.
const DEFAULT_SCREEN: ScreenSize = ScreenSize::new(640.0, 480.0);

/// Gli elementi di un layout, nell'ordine in cui compaiono nel file.
///
/// La tabella e' l'unico posto da toccare per aggiungere un elemento: il resto
/// della conversione e' generico.
const ELEMENTS: [(&str, ElementKind); 15] = [
    ("romlist", ElementKind::List),
    ("snapshot", ElementKind::Artwork(ArtworkSlot::Snap)),
    ("marquee", ElementKind::Artwork(ArtworkSlot::Marquee)),
    ("cabinet", ElementKind::Artwork(ArtworkSlot::Cabinet)),
    ("romcounter", ElementKind::Label(LabelKind::RomCounter)),
    ("platformname", ElementKind::Label(LabelKind::Platform)),
    ("emulatorname", ElementKind::Label(LabelKind::Emulator)),
    ("gamelistname", ElementKind::Label(LabelKind::Gamelist)),
    ("romname", ElementKind::Label(LabelKind::RomName)),
    ("romdescription", ElementKind::Label(LabelKind::RomDescription)),
    ("rommanufacturer", ElementKind::Label(LabelKind::RomManufacturer)),
    ("romcategory", ElementKind::Label(LabelKind::RomCategory)),
    ("rominputcontrol", ElementKind::Label(LabelKind::RomControls)),
    ("romstatus", ElementKind::Label(LabelKind::RomStatus)),
    ("romdisplaytype", ElementKind::Label(LabelKind::RomDisplayType)),
];

/// Converte un layout di FEEL 1.x.
///
/// `files` sono i nomi dei file presenti nella cartella del layout: servono a
/// riconoscere le immagini e ad animare lo sfondo.
pub fn convert(name: &str, ini: &IniMap, files: &[String]) -> Layout {
    let screen = ScreenSize::new(
        ini.int("screen_res_x", DEFAULT_SCREEN.width as i64) as f32,
        ini.int("screen_res_y", DEFAULT_SCREEN.height as i64) as f32,
    );

    let mut layout = Layout::new(name);
    layout.version = LAYOUT_VERSION;
    layout.design_size = Some(screen);
    layout.background = read_background(ini, files);
    layout.menu = read_menu(ini, screen);
    layout.screensaver = read_screensaver(ini);
    layout.assets = files.to_vec();

    for (prefix, kind) in ELEMENTS {
        if let Some(element) = read_element(ini, screen, prefix, kind) {
            layout.elements.push(element);
        }
    }

    // Bezel e attori non hanno coordinate: in FEEL 1.x si stendono su tutto lo
    // schermo, sopra al resto, e possono essere animati.
    for (slot, gambo) in [(ArtworkSlot::Bezel, "bezel"), (ArtworkSlot::Actors, "actors")] {
        let durata = ini.int(&format!("{gambo}_frame_duration_ms"), 200).max(1) as u32;
        let fotogrammi = raccogli_frames(gambo, files, durata);
        if fotogrammi.is_empty() {
            continue;
        }

        let mut element = Element::new(
            ElementKind::Artwork(slot),
            Placement::default(),
            Size::fractions(1.0, 1.0),
        );
        element.stretch = true;
        element.frames = fotogrammi;
        layout.elements.push(element);
    }

    layout
}

/// Legge un layout da una cartella.
pub fn load(dir: &Path) -> Result<Layout, ImportError> {
    let name = dir
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "layout".to_owned());

    let ini_path = dir.join("layout.ini");
    let ini = IniMap::from_file(&ini_path).map_err(|source| ImportError::Io {
        path: ini_path.clone(),
        source,
    })?;

    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name != "layout.ini" && !name.ends_with(".db") {
                    files.push(name.to_owned());
                }
            }
        }
    }
    files.sort();

    Ok(convert(&name, &ini, &files))
}

// ----------------------------------------------------------------------------------
// Geometria
// ----------------------------------------------------------------------------------

/// Converte un rettangolo in pixel in un aggancio con spostamento in frazione.
///
/// Si sceglie l'ancoraggio piu' vicino all'elemento — sinistra, centro o destra —
/// invece di agganciare sempre in alto a sinistra. Cosi' un elemento che sta a
/// destra resta a destra anche se cambiano le proporzioni dello schermo, che e'
/// esattamente il motivo per cui questo formato esiste.
fn placement(x: f32, y: f32, width: f32, height: f32, screen: ScreenSize) -> Placement {
    let (horizontal_anchor, offset_x) = horizontal_anchor(x, width, screen.width);
    let (vertical_anchor, offset_y) = vertical_anchor(y, height, screen.height);

    let anchor = Anchor::new(horizontal_anchor, vertical_anchor);
    Placement {
        point: anchor,
        reference: anchor,
        offset: Point {
            x: Dimension::Fraction(offset_x / screen.width),
            y: Dimension::Fraction(offset_y / screen.height),
        },
    }
}

fn horizontal_anchor(x: f32, width: f32, screen_width: f32) -> (Horizontal, f32) {
    let center = x + width / 2.0;
    if center < screen_width / 3.0 {
        (Horizontal::Left, x)
    } else if center > screen_width * 2.0 / 3.0 {
        (Horizontal::Right, x + width - screen_width)
    } else {
        (Horizontal::Center, center - screen_width / 2.0)
    }
}

fn vertical_anchor(y: f32, height: f32, screen_height: f32) -> (Vertical, f32) {
    let center = y + height / 2.0;
    if center < screen_height / 3.0 {
        (Vertical::Top, y)
    } else if center > screen_height * 2.0 / 3.0 {
        (Vertical::Bottom, y + height - screen_height)
    } else {
        (Vertical::Middle, center - screen_height / 2.0)
    }
}

// ----------------------------------------------------------------------------------
// Elementi
// ----------------------------------------------------------------------------------

fn read_element(
    ini: &IniMap,
    screen: ScreenSize,
    prefix: &str,
    kind: ElementKind,
) -> Option<Element> {
    // In FEEL 1.x la lista e' visibile per difetto, le etichette no: un layout
    // che non parla di un'etichetta non la vuole.
    let default_visible = matches!(kind, ElementKind::List);
    if !ini.flag(&format!("{prefix}_visible"), default_visible) {
        return None;
    }

    let x = ini.int(&format!("{prefix}_x_pos"), 0) as f32;
    let y = ini.int(&format!("{prefix}_y_pos"), 0) as f32;
    let width = ini.int(&format!("{prefix}_width"), 0) as f32;
    let height = ini.int(&format!("{prefix}_height"), 0) as f32;

    // Un elemento dichiarato visibile ma senza area non e' un elemento.
    if width <= 0.0 && height <= 0.0 {
        return None;
    }

    let mut element = Element::new(
        kind.clone(),
        placement(x, y, width, height, screen),
        Size::fractions(width / screen.width, height / screen.height),
    );

    element.stretch = ini.flag(&format!("{prefix}_stretch"), false);

    if matches!(kind, ElementKind::List) || matches!(kind, ElementKind::Label(_)) {
        element.style = Some(read_text_style(ini, screen, prefix));
        element.row_background = ini
            .non_empty(&format!("{prefix}_backcolor"))
            .map(|raw| parse_color(&raw).map(|color| !color.is_transparent()).unwrap_or(false))
            .unwrap_or(false);
    }

    Some(element)
}

fn read_text_style(ini: &IniMap, screen: ScreenSize, prefix: &str) -> TextStyle {
    let mut style = TextStyle::default();

    if let Some(font) = ini.non_empty(&format!("{prefix}_font_name")) {
        style.font = Some(font);
    }

    let font_size = ini.int(&format!("{prefix}_font_size"), 0) as f32;
    if font_size > 0.0 {
        // FEEL 1.x misura il carattere in punti sull'altezza dello schermo di
        // progetto: undici punti su 480 fa circa il due per cento.
        style.size = Dimension::Fraction(font_size / screen.height);
    }

    let font_style = ini.int(&format!("{prefix}_font_style"), 0);
    style.bold = font_style & 1 != 0;
    style.italic = font_style & 2 != 0;

    if let Some(color) = ini.non_empty(&format!("{prefix}_font_color")).and_then(|raw| parse_color(&raw)) {
        style.color = color;
    }

    // Il colore di sfondo a trasparenza zero significa "nessuno sfondo", non
    // "sfondo nero".
    style.background = ini
        .non_empty(&format!("{prefix}_backcolor"))
        .and_then(|raw| parse_color(&raw))
        .filter(|color| !color.is_transparent());

    style.align = match ini.int(&format!("{prefix}_text_align"), 0) {
        2 => Horizontal::Right,
        1 => Horizontal::Center,
        _ => Horizontal::Left,
    };

    style
}

fn read_background(ini: &IniMap, files: &[String]) -> Option<Background> {
    let mut background = Background::default();

    // I fotogrammi animati: `main1.png`, `main2.png`, ...
    let duration = ini.int("background_frame_duration_ms", 200).max(1) as u32;
    let frames = raccogli_frames("main", files, duration);

    if frames.is_empty() {
        return None;
    }

    // Un fotogramma solo di durata zero e' un'immagine ferma.
    if frames.len() == 1 && frames[0].duration_ms == 0 {
        background.image = Some(frames[0].image.clone());
    } else {
        let repeat = ini.int("background_repeat_delay_ms", 0);
        if repeat > 0 {
            background.repeat_delay_ms = Some(repeat as u32);
        }
        background.frames = frames;
    }

    background.on_top = ini.flag("background_ontop", false);
    Some(background)
}

/// I fotogrammi di un'immagine, animata o ferma.
///
/// FEEL 1.x anima `bezel`, `actors` e `main` con una serie `nome1.png`,
/// `nome2.png`, ... Quando la serie non c'e' usa `nome.png` come immagine
/// ferma: la rappresentiamo come un fotogramma di durata zero, cosi' chi
/// disegna ha un solo caso da gestire.
fn raccogli_frames(gambo: &str, files: &[String], durata_ms: u32) -> Vec<Frame> {
    let mut animati = Vec::new();
    for indice in 1..=50 {
        let nome = format!("{gambo}{indice}.png");
        if !files.iter().any(|presente| presente == &nome) {
            break;
        }
        animati.push(Frame { image: nome, duration_ms: durata_ms });
    }
    if !animati.is_empty() {
        return animati;
    }

    let fermo = format!("{gambo}.png");
    if files.iter().any(|presente| presente == &fermo) {
        return vec![Frame { image: fermo, duration_ms: 0 }];
    }

    Vec::new()
}

fn read_menu(ini: &IniMap, screen: ScreenSize) -> MenuStyle {
    let mut menu = MenuStyle::default();

    let width = ini.int("menu_width", 200) as f32;
    if width > 0.0 {
        menu.width = Dimension::Fraction(width / screen.width);
    }

    let item_height = ini.int("menu_item_height", 24) as f32;
    if item_height > 0.0 {
        menu.item_height = Dimension::Fraction(item_height / screen.height);
    }

    if let Some(font) = ini.non_empty("menu_font_name") {
        menu.font = Some(font);
    }

    let font_size = ini.int("menu_font_size", 0) as f32;
    if font_size > 0.0 {
        menu.size = Dimension::Fraction(font_size / screen.height);
    }

    if let Some(color) = ini.non_empty("menu_font_color").and_then(|raw| parse_color(&raw)) {
        menu.color = color;
    }
    menu.background = ini
        .non_empty("menu_backcolor")
        .and_then(|raw| parse_color(&raw))
        .filter(|color| !color.is_transparent());
    if let Some(color) =
        ini.non_empty("menu_selected_font_color").and_then(|raw| parse_color(&raw))
    {
        menu.selected_color = color;
    }
    menu.selected_background = ini
        .non_empty("menu_selected_backcolor")
        .and_then(|raw| parse_color(&raw))
        .filter(|color| !color.is_transparent());

    menu.show_sidebar = ini.flag("menu_show_sidebar", true);
    menu
}

fn read_screensaver(ini: &IniMap) -> ScreenSaverStyle {
    let mut style = ScreenSaverStyle::default();
    if let Some(color) =
        ini.non_empty("screen_saver_backcolor").and_then(|raw| parse_color(&raw))
    {
        style.background = color;
    }
    if let Some(color) =
        ini.non_empty("screen_saver_font_color").and_then(|raw| parse_color(&raw))
    {
        style.color = color;
    }
    style
}

/// Legge un colore di FEEL 1.x: `230, 230, 0` oppure `0, 255, 0, 40`.
///
/// Il terzo e il quarto numero possono avere spazi irregolari (`0, 255, 0,0`),
/// quindi si tolgono gli spazi a ogni pezzo invece di fidarsi della forma.
pub fn parse_color(raw: &str) -> Option<Color> {
    let parts: Vec<u8> = raw
        .split(',')
        .map(|part| part.trim().parse::<u8>().ok())
        .collect::<Option<Vec<_>>>()?;

    match parts.as_slice() {
        [r, g, b] => Some(Color::rgba(*r, *g, *b, 255)),
        [r, g, b, a] => Some(Color::rgba(*r, *g, *b, *a)),
        _ => None,
    }
}

/// Trova le cartelle dei layout dentro una installazione FEEL 1.x.
///
/// FEEL li tiene in `layouts/`, ma nelle installazioni copiate dall'archivio
/// si trovano anche in `__init_files__/layouts/`.
pub fn find_layouts(root: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut out: Vec<(String, std::path::PathBuf)> = Vec::new();

    let bases = [root.join("layouts"), root.join("__init_files__").join("layouts")];
    for base in bases {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };

        let mut directories: Vec<_> = entries.flatten().collect();
        directories.sort_by_key(|entry| entry.file_name());

        for entry in directories {
            let dir = entry.path();
            if !dir.is_dir() || !dir.join("layout.ini").is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !out.iter().any(|(presente, _)| presente == &name) {
                out.push((name, dir));
            }
        }
    }

    out
}

/// I nomi dei file che un layout usa, ricavati dalle immagini referenziate.
pub fn referenced_assets(layout: &Layout) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Some(background) = &layout.background {
        if let Some(image) = &background.image {
            out.insert(image.clone());
        }
        for frame in &background.frames {
            out.insert(frame.image.clone());
        }
    }
    for element in &layout.elements {
        if let ElementKind::Artwork(slot) = element.kind {
            out.insert(default_asset_name(slot).to_owned());
        }
    }
    out
}

/// Il nome del file che FEEL 1.x usa per un elemento grafico.
pub fn default_asset_name(slot: ArtworkSlot) -> &'static str {
    match slot {
        ArtworkSlot::Snap => "snap.png",
        ArtworkSlot::Marquee => "marquee.png",
        ArtworkSlot::Cabinet => "cabinet.png",
        ArtworkSlot::Bezel => "bezel.png",
        ArtworkSlot::Actors => "actors.png",
        ArtworkSlot::ControlPanel => "controls.png",
        ArtworkSlot::Title => "title.png",
        ArtworkSlot::Wheel => "wheel.png",
    }
}
