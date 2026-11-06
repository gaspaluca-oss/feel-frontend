//! Il formato dei layout di FEEL 2.0.
//!
//! FEEL 1.x descriveva un layout con un `.ini` di coordinate assolute su una
//! risoluzione fissa: `romlist_x_pos 386` significava 386 pixel su uno schermo
//! largo 640. Cambiare risoluzione, o passare da un CRT 4:3 a un LCD 16:9,
//! significava rifare il layout a mano. Era il difetto peggiore del vecchio
//! frontend.
//!
//! Qui ogni elemento si descrive cosi':
//!
//! - **un punto dell'elemento** che si aggancia (il suo angolo in alto a
//!   sinistra, il suo centro, il suo angolo in basso a destra...)
//! - **un punto dello schermo** a cui agganciarlo
//! - uno **spostamento**, espresso in frazione di schermo
//! - una **dimensione**, anch'essa in frazione di schermo
//!
//! Cosi' un layout pensato per 640x480 si presenta correttamente anche a 1920x1080
//! e su un cabinato verticale, senza duplicare niente.
//!
//! Le misure sono **frazioni** (0.5 = meta') e non percentuali: una percentuale
//! si dimentica di dividere per cento prima o poi, una frazione no.

use serde::{Deserialize, Serialize};

/// Dove si aggancia un elemento orizzontalmente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Horizontal {
    Left,
    Center,
    Right,
}

impl Horizontal {
    /// La frazione di larghezza a cui corrisponde: sinistra 0.0, centro 0.5, destra 1.0.
    pub fn fraction(self) -> f32 {
        match self {
            Horizontal::Left => 0.0,
            Horizontal::Center => 0.5,
            Horizontal::Right => 1.0,
        }
    }
}

impl Default for Horizontal {
    fn default() -> Self {
        Self::Left
    }
}

/// Dove si aggancia un elemento verticalmente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vertical {
    Top,
    Middle,
    Bottom,
}

impl Vertical {
    pub fn fraction(self) -> f32 {
        match self {
            Vertical::Top => 0.0,
            Vertical::Middle => 0.5,
            Vertical::Bottom => 1.0,
        }
    }
}

impl Default for Vertical {
    fn default() -> Self {
        Self::Top
    }
}

/// Un punto di riferimento: un angolo, un lato, il centro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub horizontal: Horizontal,
    pub vertical: Vertical,
}

impl Anchor {
    pub const TOP_LEFT: Self =
        Self { horizontal: Horizontal::Left, vertical: Vertical::Top };
    pub const CENTER: Self =
        Self { horizontal: Horizontal::Center, vertical: Vertical::Middle };
    pub const BOTTOM_RIGHT: Self =
        Self { horizontal: Horizontal::Right, vertical: Vertical::Bottom };

    pub const fn new(horizontal: Horizontal, vertical: Vertical) -> Self {
        Self { horizontal, vertical }
    }

    pub fn fraction(self) -> (f32, f32) {
        (self.horizontal.fraction(), self.vertical.fraction())
    }
}

impl Default for Anchor {
    fn default() -> Self {
        Self::TOP_LEFT
    }
}

/// Una misura, che puo' adattarsi allo schermo o restare fissa.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dimension {
    /// Frazione della dimensione di riferimento: 0.5 e' meta' schermo.
    Fraction(f32),
    /// Pixel fissi, uguali a qualunque risoluzione.
    ///
    /// Serve per le cose che non devono ingrandirsi: un bordo, una griglia.
    Pixels(f32),
}

impl Dimension {
    pub const fn fraction(value: f32) -> Self {
        Self::Fraction(value)
    }

    pub const fn pixels(value: f32) -> Self {
        Self::Pixels(value)
    }

    /// Quanti pixel vale, dato il riferimento (larghezza o altezza dello schermo).
    pub fn resolve(self, reference: f32) -> f32 {
        match self {
            Self::Fraction(fraction) => fraction * reference,
            Self::Pixels(pixels) => pixels,
        }
    }
}

impl Default for Dimension {
    fn default() -> Self {
        Self::Fraction(0.0)
    }
}

/// Uno spostamento in due dimensioni.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point {
    #[serde(default)]
    pub x: Dimension,
    #[serde(default)]
    pub y: Dimension,
}

impl Point {
    pub const ZERO: Self = Self { x: Dimension::Fraction(0.0), y: Dimension::Fraction(0.0) };
}

/// Quanto e' grande un elemento.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: Dimension,
    pub height: Dimension,
}

impl Size {
    pub const fn new(width: Dimension, height: Dimension) -> Self {
        Self { width, height }
    }

    /// Da frazioni di schermo.
    pub const fn fractions(width: f32, height: f32) -> Self {
        Self::new(Dimension::Fraction(width), Dimension::Fraction(height))
    }

    pub fn resolve(self, screen: ScreenSize) -> (f32, f32) {
        (self.width.resolve(screen.width), self.height.resolve(screen.height))
    }
}

impl Default for Size {
    fn default() -> Self {
        // Tutto lo schermo: e' la scelta giusta per uno sfondo e innocua per il resto,
        // perche' un elemento che non si vede non si nota.
        Self::fractions(1.0, 1.0)
    }
}

/// Dimensioni in pixel di uno schermo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScreenSize {
    pub width: f32,
    pub height: f32,
}

impl ScreenSize {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Vero se lo schermo e' piu' alto che largo: cabinato verticale.
    pub fn is_portrait(self) -> bool {
        self.height > self.width
    }
}

impl From<(u32, u32)> for ScreenSize {
    fn from((width, height): (u32, u32)) -> Self {
        Self::new(width as f32, height as f32)
    }
}

/// Dove finisce un elemento, in pixel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// Come un elemento si aggancia allo schermo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    /// Il punto dell'elemento che viene agganciato.
    pub point: Anchor,
    /// Il punto dello schermo a cui viene agganciato.
    pub reference: Anchor,
    /// Lo spostamento applicato dopo l'aggancio.
    #[serde(default)]
    pub offset: Point,
}

impl Placement {
    /// Un elemento agganciato in alto a sinistra, spostato di `x` e `y` pixel.
    pub fn top_left_pixels(x: f32, y: f32) -> Self {
        Self {
            point: Anchor::TOP_LEFT,
            reference: Anchor::TOP_LEFT,
            offset: Point { x: Dimension::pixels(x), y: Dimension::pixels(y) },
        }
    }

    /// Il rettangolo in pixel di un elemento di dimensione `size` su questo schermo.
    ///
    /// La formula e' una sola per tutti i casi: si prende il punto di riferimento
    /// sullo schermo, si arretra della parte di elemento che sta *prima* di quel
    /// punto (il suo angolo o il suo centro), e si applica lo spostamento.
    pub fn resolve(self, size: Size, screen: ScreenSize) -> Rect {
        let (width, height) = size.resolve(screen);
        let (reference_x, reference_y) = self.reference.fraction();
        let (point_x, point_y) = self.point.fraction();

        Rect {
            x: reference_x * screen.width - point_x * width + self.offset.x.resolve(screen.width),
            y: reference_y * screen.height - point_y * height
                + self.offset.y.resolve(screen.height),
            width,
            height,
        }
    }
}

impl Default for Placement {
    fn default() -> Self {
        Self { point: Anchor::TOP_LEFT, reference: Anchor::TOP_LEFT, offset: Point::ZERO }
    }
}

/// Un colore RGBA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "opaque")]
    pub a: u8,
}

fn opaque() -> u8 {
    255
}

impl Color {
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn is_transparent(self) -> bool {
        self.a == 0
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

/// Come si disegna un testo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    /// Nome del font. Se assente si usa quello predefinito del layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// Altezza del carattere, in frazione di altezza schermo.
    pub size: Dimension,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub color: Color,
    /// Sfondo dietro al testo. `None` significa trasparente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    #[serde(default)]
    pub align: Horizontal,
    /// Contorno, che e' quello che rende leggibile un testo sopra un'immagine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<Color>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: None,
            // Due per cento dell'altezza: su 480 pixel fa circa dieci pixel,
            // che era la dimensione tipica dei layout di FEEL 1.x.
            size: Dimension::fraction(0.02),
            bold: false,
            italic: false,
            color: Color::WHITE,
            background: None,
            align: Horizontal::Left,
            outline: None,
        }
    }
}

/// Quale immagine mostra un elemento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkSlot {
    /// L'immagine del gioco in corso.
    Snap,
    /// Il pannello sopra lo schermo.
    Marquee,
    /// La sagoma del cabinato.
    Cabinet,
    /// La cornice sovrapposta allo schermo.
    Bezel,
    /// Figure animate sopra al resto.
    Actors,
    /// L'immagine dei comandi.
    ControlPanel,
    /// Il logo del gioco.
    Title,
    /// La ruota dei loghi.
    Wheel,
}

/// Quale testo mostra un'etichetta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelKind {
    Platform,
    Emulator,
    Gamelist,
    RomName,
    RomDescription,
    RomManufacturer,
    RomCategory,
    RomControls,
    RomStatus,
    RomDisplayType,
    /// La posizione nella lista, come "12 / 1842".
    RomCounter,
}

/// Che cosa e' un elemento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    /// La lista dei giochi.
    List,
    /// Un'immagine.
    Artwork(ArtworkSlot),
    /// Una scritta.
    Label(LabelKind),
    /// L'orologio.
    Clock,
    /// Il pannello del monitor secondario.
    MarqueeBoard,
}

/// Un elemento del layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub kind: ElementKind,
    #[serde(default = "yes")]
    pub visible: bool,
    pub placement: Placement,
    #[serde(default)]
    pub size: Size,
    /// Stile del testo, per le etichette e la lista.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<TextStyle>,
    /// Per le immagini: allargare fino a riempire, o mantenere le proporzioni.
    #[serde(default)]
    pub stretch: bool,
    /// Se lo sfondo va disegnato dietro al testo.
    #[serde(default)]
    pub row_background: bool,
}

fn yes() -> bool {
    true
}

impl Element {
    pub fn new(kind: ElementKind, placement: Placement, size: Size) -> Self {
        Self {
            kind,
            visible: true,
            placement,
            size,
            style: None,
            stretch: false,
            row_background: false,
        }
    }

    /// Il rettangolo in pixel di questo elemento.
    pub fn rect(&self, screen: ScreenSize) -> Rect {
        self.placement.resolve(self.size, screen)
    }

    pub fn is_label(&self) -> bool {
        matches!(self.kind, ElementKind::Label(_))
    }
}

/// Un fotogramma di un'animazione.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub image: String,
    pub duration_ms: u32,
}

/// Lo sfondo del layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Background {
    /// L'immagine fissa. Se ci sono `frames`, questa non si usa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// I fotogrammi, per i layout animati.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<Frame>,
    /// Quanto aspettare prima di ricominciare il ciclo, se i fotogrammi sono animati.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_delay_ms: Option<u32>,
    /// Disegnare lo sfondo *sopra* agli elementi invece che sotto.
    #[serde(default)]
    pub on_top: bool,
}

/// L'effetto applicato all'immagine finale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Shader {
    #[default]
    None,
    /// Monitor a tubo: scanline, maschera RGB, curvatura.
    Crt,
    /// Schermo piatto: solo scala, nessuna scanline.
    Lcd,
    /// Solo le scanline, senza curvatura ne' maschera.
    Scanlines,
}

/// Le impostazioni del menu del frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuStyle {
    pub width: Dimension,
    pub item_height: Dimension,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    pub size: Dimension,
    #[serde(default)]
    pub color: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    #[serde(default)]
    pub selected_color: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_background: Option<Color>,
    /// Mostrare la barra laterale nel menu.
    #[serde(default = "yes")]
    pub show_sidebar: bool,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            width: Dimension::fraction(0.3),
            item_height: Dimension::fraction(0.05),
            font: None,
            size: Dimension::fraction(0.022),
            color: Color::WHITE,
            background: None,
            selected_color: Color::BLACK,
            selected_background: Some(Color::rgb(255, 168, 0)),
            show_sidebar: true,
        }
    }
}

/// Colori dello screensaver.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScreenSaverStyle {
    #[serde(default = "screensaver_background")]
    pub background: Color,
    #[serde(default = "screensaver_foreground")]
    pub color: Color,
}

fn screensaver_background() -> Color {
    Color::BLACK
}

fn screensaver_foreground() -> Color {
    Color::rgb(0, 96, 96)
}

impl Default for ScreenSaverStyle {
    fn default() -> Self {
        Self { background: screensaver_background(), color: screensaver_foreground() }
    }
}

/// Un layout completo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Versione del formato, per poterlo cambiare senza rompere i file esistenti.
    pub version: u32,
    pub name: String,
    /// Nome di un altro layout da cui ereditare. `None` significa nessuno.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    /// La risoluzione su cui il layout e' stato pensato.
    ///
    /// Non serve a disegnare — le misure sono frazioni — ma dice le proporzioni
    /// di partenza, ed e' quello che permette di convertire un layout di FEEL 1.x
    /// senza perdere niente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_size: Option<ScreenSize>,
    #[serde(default)]
    pub shader: Shader,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Background>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Element>,
    #[serde(default)]
    pub menu: MenuStyle,
    #[serde(default)]
    pub screensaver: ScreenSaverStyle,
    /// I file di immagine e suono che il layout usa.
    ///
    /// Serve a sapere cosa copiare quando si esporta un layout, e a segnalare
    /// un file mancante prima di arrivare sul cabinato.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<String>,
}

/// La versione corrente del formato dei layout.
pub const LAYOUT_VERSION: u32 = 1;

impl Layout {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: LAYOUT_VERSION,
            name: name.into(),
            based_on: None,
            design_size: None,
            shader: Shader::None,
            background: None,
            elements: Vec::new(),
            menu: MenuStyle::default(),
            screensaver: ScreenSaverStyle::default(),
            assets: Vec::new(),
        }
    }

    /// Gli elementi visibili, nell'ordine in cui vanno disegnati.
    ///
    /// Lo sfondo va prima di tutto, a meno che `on_top` dica il contrario.
    pub fn visible_elements(&self) -> impl Iterator<Item = &Element> {
        self.elements.iter().filter(|element| element.visible)
    }

    pub fn find(&self, kind: &ElementKind) -> Option<&Element> {
        self.elements.iter().find(|element| &element.kind == kind)
    }

    /// Tutti i rettangoli in pixel, per il controllo e per i test.
    pub fn resolved(&self, screen: ScreenSize) -> Vec<(ElementKind, Rect)> {
        self.visible_elements()
            .map(|element| (element.kind.clone(), element.rect(screen)))
            .collect()
    }
}
