//! Conversione di un layout di FEEL 1.x nel formato di FEEL 2.0.
//!
//! Il test che conta e' `converte_i_rettangoli_senza_perderli`: se la conversione
//! sposta un elemento di qualche pixel, i layout degli utenti cambierebbero
//! aspetto senza che nessuno abbia toccato niente.

use std::path::PathBuf;

use feel_config::layout1::{self, parse_color};
use feel_config::IniMap;
use feel_core::{
    ArtworkSlot, Color, ElementKind, Horizontal, LabelKind, Layout, Rect, ScreenSize, Vertical,
};

/// I file presenti nella cartella del layout.
const FILES: [&str; 4] = ["main1.png", "main2.png", "snap.png", "bezel.png"];

fn cartella() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layout1")
}

fn ini() -> IniMap {
    IniMap::from_file(cartella().join("layout.ini")).expect("la fixture deve essere leggibile")
}

fn convertito() -> Layout {
    let files: Vec<String> = FILES.iter().map(|f| (*f).to_owned()).collect();
    layout1::convert("prova", &ini(), &files)
}

/// Legge un rettangolo dal `.ini` come farebbe FEEL 1.x: pixel assoluti.
///
/// Volutamente indipendente dal codice di conversione: se leggesse i valori
/// passando per `layout1`, il test confermerebbe se stesso.
fn rettangolo_originale(ini: &IniMap, prefix: &str) -> Rect {
    Rect::new(
        ini.int(&format!("{prefix}_x_pos"), 0) as f32,
        ini.int(&format!("{prefix}_y_pos"), 0) as f32,
        ini.int(&format!("{prefix}_width"), 0) as f32,
        ini.int(&format!("{prefix}_height"), 0) as f32,
    )
}

fn vicino(a: f32, b: f32) {
    assert!((a - b).abs() < 0.001, "atteso {b}, ottenuto {a}");
}

// ----------------------------------------------------------------------------------

#[test]
fn converte_i_rettangoli_senza_perderli() {
    let ini = ini();
    let layout = convertito();
    let schermo = layout.design_size.expect("il layout dichiara la sua risoluzione");

    let elementi = [
        ("romlist", ElementKind::List),
        ("snapshot", ElementKind::Artwork(ArtworkSlot::Snap)),
        ("romcounter", ElementKind::Label(LabelKind::RomCounter)),
        ("platformname", ElementKind::Label(LabelKind::Platform)),
        ("romname", ElementKind::Label(LabelKind::RomName)),
    ];

    for (prefix, kind) in elementi {
        let atteso = rettangolo_originale(&ini, prefix);
        let elemento = layout
            .find(&kind)
            .unwrap_or_else(|| panic!("manca l'elemento per '{prefix}'"));
        let ottenuto = elemento.rect(schermo);

        vicino(ottenuto.x, atteso.x);
        vicino(ottenuto.y, atteso.y);
        vicino(ottenuto.width, atteso.width);
        vicino(ottenuto.height, atteso.height);
    }
}

#[test]
fn sceglie_l_ancoraggio_piu_vicino_a_ogni_elemento() {
    let layout = convertito();

    // La lista sta a sinistra (centro a 164 su 640).
    let lista = layout.find(&ElementKind::List).unwrap();
    assert_eq!(lista.placement.point.horizontal, Horizontal::Left);
    assert_eq!(lista.placement.point.vertical, Vertical::Middle);

    // Lo snapshot sta a destra (centro a 475).
    let snapshot = layout.find(&ElementKind::Artwork(ArtworkSlot::Snap)).unwrap();
    assert_eq!(snapshot.placement.point.horizontal, Horizontal::Right);

    // Il contatore e' in basso al centro (centro a 247).
    let contatore = layout.find(&ElementKind::Label(LabelKind::RomCounter)).unwrap();
    assert_eq!(contatore.placement.point.horizontal, Horizontal::Center);
    assert_eq!(contatore.placement.point.vertical, Vertical::Bottom);

    // Il nome della piattaforma e' in alto a destra.
    let piattaforma = layout.find(&ElementKind::Label(LabelKind::Platform)).unwrap();
    assert_eq!(piattaforma.placement.point.horizontal, Horizontal::Right);
    assert_eq!(piattaforma.placement.point.vertical, Vertical::Top);
}

#[test]
fn lo_stesso_layout_regge_a_una_risoluzione_diversa() {
    let layout = convertito();
    let progetto = layout.design_size.unwrap();
    let fullhd = ScreenSize::new(1920.0, 1080.0);

    for kind in [
        ElementKind::List,
        ElementKind::Artwork(ArtworkSlot::Snap),
        ElementKind::Label(LabelKind::RomCounter),
    ] {
        let elemento = layout.find(&kind).unwrap();
        let piccolo = elemento.rect(progetto);
        let grande = elemento.rect(fullhd);

        // Le proporzioni rispetto allo schermo devono restare le stesse:
        // un elemento a meta' schermo resta a meta' schermo.
        vicino(piccolo.x / progetto.width, grande.x / fullhd.width);
        vicino(piccolo.y / progetto.height, grande.y / fullhd.height);
        vicino(piccolo.width / progetto.width, grande.width / fullhd.width);
        vicino(piccolo.height / progetto.height, grande.height / fullhd.height);
    }

    // Ancorato a destra vuol dire che la distanza dal bordo destro si conserva
    // in proporzione: 640 - 589 = 51 su 640 diventa 153 su 1920.
    let snapshot = layout.find(&ElementKind::Artwork(ArtworkSlot::Snap)).unwrap();
    let piccolo = snapshot.rect(progetto);
    let grande = snapshot.rect(fullhd);
    vicino(
        (progetto.width - piccolo.right()) / progetto.width,
        (fullhd.width - grande.right()) / fullhd.width,
    );
}

#[test]
fn salta_gli_elementi_spenti_e_a_quelli_senza_area() {
    let layout = convertito();

    // `romstatus_visible 1` ma area 0x0: non e' un elemento.
    assert!(layout.find(&ElementKind::Label(LabelKind::RomStatus)).is_none());
    // `romdisplaytype_visible 0`: spento, anche se ha un'area.
    assert!(layout.find(&ElementKind::Label(LabelKind::RomDisplayType)).is_none());
    // La lista invece c'e' sempre.
    assert!(layout.find(&ElementKind::List).is_some());
}

#[test]
fn porta_con_se_stile_e_allineamento() {
    let layout = convertito();
    let schermo = layout.design_size.unwrap();

    let contatore = layout.find(&ElementKind::Label(LabelKind::RomCounter)).unwrap();
    let stile = contatore.style.as_ref().unwrap();

    assert_eq!(stile.font.as_deref(), Some("Arial"));
    assert_eq!(stile.align, Horizontal::Right);
    assert!(!stile.bold);
    assert_eq!(stile.color, Color::rgb(255, 128, 0));
    assert_eq!(stile.background, Some(Color::rgb(22, 7, 27)));
    // Otto punti su 480 di altezza.
    vicino(stile.size.resolve(schermo.height), 8.0);

    let nome = layout.find(&ElementKind::Label(LabelKind::RomName)).unwrap();
    let stile = nome.style.as_ref().unwrap();
    assert!(stile.italic, "font_style 2 e' corsivo");
    assert!(!stile.bold);
}

#[test]
fn lo_sfondo_animato_diventa_una_sequenza_di_fotogrammi() {
    // Nella cartella ci sono main1.png e main2.png, ma non main.png.
    let layout = convertito();
    let sfondo = layout.background.as_ref().unwrap();

    assert!(sfondo.image.is_none());
    assert_eq!(sfondo.frames.len(), 2);
    assert_eq!(sfondo.frames[0].image, "main1.png");
    assert_eq!(sfondo.frames[1].image, "main2.png");
    assert_eq!(sfondo.frames[0].duration_ms, 250);
    assert_eq!(sfondo.repeat_delay_ms, Some(3000));
    assert!(!sfondo.on_top);
}

#[test]
fn uno_sfondo_non_animato_resta_un_immagine_sola() {
    let files = vec!["main.png".to_owned(), "snap.png".to_owned()];
    let layout = layout1::convert("prova", &ini(), &files);
    let sfondo = layout.background.as_ref().unwrap();

    assert_eq!(sfondo.image.as_deref(), Some("main.png"));
    assert!(sfondo.frames.is_empty());
}

#[test]
fn il_bezel_diventa_un_elemento_a_tutto_schermo() {
    let layout = convertito();
    let bezel = layout
        .find(&ElementKind::Artwork(ArtworkSlot::Bezel))
        .expect("la fixture ha bezel.png");
    let schermo = layout.design_size.unwrap();
    let rect = bezel.rect(schermo);

    vicino(rect.x, 0.0);
    vicino(rect.y, 0.0);
    vicino(rect.width, schermo.width);
    vicino(rect.height, schermo.height);
}

#[test]
fn legge_il_menu_e_lo_screensaver() {
    let layout = convertito();
    let schermo = layout.design_size.unwrap();

    vicino(layout.menu.width.resolve(schermo.width), 200.0);
    vicino(layout.menu.item_height.resolve(schermo.height), 24.0);
    assert_eq!(layout.menu.color, Color::rgba(150, 200, 50, 200));
    assert_eq!(layout.menu.selected_background, Some(Color::rgba(196, 128, 196, 200)));
    assert_eq!(layout.menu.selected_color, Color::rgba(255, 255, 196, 255));

    assert_eq!(layout.screensaver.background, Color::rgb(0, 0, 0));
    assert_eq!(layout.screensaver.color, Color::rgb(0, 96, 96));
}

// ----------------------------------------------------------------------------------
// Colori
// ----------------------------------------------------------------------------------

#[test]
fn legge_i_colori_nei_due_formati() {
    // Tre numeri: opaco.
    assert_eq!(parse_color("230, 230, 0"), Some(Color::rgba(230, 230, 0, 255)));
    // Quattro numeri: con trasparenza.
    assert_eq!(parse_color("0, 255, 0,0"), Some(Color::rgba(0, 255, 0, 0)));
    assert_eq!(parse_color(" 22 , 7 , 27 , 200 "), Some(Color::rgba(22, 7, 27, 200)));
}

#[test]
fn un_colore_illeggibile_non_diventa_nero() {
    // Meglio nessun colore che un colore inventato: il chiamante tiene il
    // valore predefinito invece di mostrare qualcosa di sbagliato.
    assert_eq!(parse_color(""), None);
    assert_eq!(parse_color("boh"), None);
    assert_eq!(parse_color("1, 2"), None);
    assert_eq!(parse_color("1, 2, 3, 4, 5"), None);
    assert_eq!(parse_color("300, 0, 0"), None, "fuori intervallo");
}

#[test]
fn lo_sfondo_trasparente_non_e_uno_sfondo() {
    let ini = IniMap::parse(
        "screen_res_x 640\nscreen_res_y 480\n\
         romcounter_visible 1\nromcounter_x_pos 10\nromcounter_y_pos 10\n\
         romcounter_width 100\nromcounter_height 20\n\
         romcounter_font_color 255, 255, 255\nromcounter_backcolor 0, 0, 0, 0\n",
    );
    let layout = layout1::convert("prova", &ini, &[]);
    let contatore = layout.find(&ElementKind::Label(LabelKind::RomCounter)).unwrap();

    assert_eq!(contatore.style.as_ref().unwrap().background, None);
    assert!(!contatore.row_background);
}
