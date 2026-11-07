//! Il frontend grafico di FEEL 2.0.
//!
//! Prima fetta verticale: una finestra che mostra la lista dei giochi presa dal
//! database, disegnata secondo il layout convertito da FEEL 1.x.
//!
//! Manca ancora: testo, immagini, video, input da joystick. Ma questa e' la
//! parte che dice se l'impianto regge — se la lista scorre e la selezione si
//! muove, il resto e' lavoro; se non regge, si scopre ora.
//!
//! Uso:
//!   feel-app <database> <lista> <layout.json>
//!
//! Il layout si ottiene da: feel layout "cartella-feel" --out cartella

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use feel_core::{Color, ElementKind, Layout, Rect};
use feel_db::Database;
use feel_render::{Quad, Session};
use winit::event::{ElementState, Event, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("errore: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // `--screenshot FILE.bmp` salva un fotogramma ed esce: serve a verificare
    // cosa si vede senza dover guardare lo schermo.
    let mut scatto: Option<PathBuf> = None;
    if let Some(posizione) = args.iter().position(|a| a == "--screenshot") {
        if let Some(valore) = args.get(posizione + 1) {
            scatto = Some(PathBuf::from(valore));
        }
        args.drain(posizione..=(posizione + 1).min(args.len() - 1));
    }

    // `--auto-exit N` chiude da solo dopo N secondi: serve alle prove
    // automatiche, dove una finestra che aspetta un tasto blocca tutto.
    let mut auto_exit: Option<Duration> = None;
    if let Some(posizione) = args.iter().position(|a| a == "--auto-exit") {
        if let Some(valore) = args.get(posizione + 1).and_then(|v| v.parse::<u64>().ok()) {
            auto_exit = Some(Duration::from_secs(valore));
        }
        args.drain(posizione..=(posizione + 1).min(args.len() - 1));
    }

    if args.len() < 3 {
        eprintln!("Uso: feel-app <database> <lista> <layout.json> [--auto-exit N]");
        return Ok(());
    }

    let database = PathBuf::from(&args[0]);
    let nome_lista = &args[1];
    let percorso_layout = PathBuf::from(&args[2]);

    let layout: Layout = serde_json::from_str(&std::fs::read_to_string(&percorso_layout)?)?;

    let db = Database::open(&database)?;
    let chiave = feel_core::GamelistKey::new(nome_lista.clone());
    let mut giochi = db.games_of(&chiave)?;
    let totale = giochi.len();
    if totale == 0 {
        println!("La lista '{nome_lista}' e' vuota: non c'e' niente da mostrare.");
    }
    giochi.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    let titolo = format!("FEEL 2.0 - {nome_lista}");
    let Session { event_loop, mut graphics } = Session::open(&titolo)?;

    println!("OpenGL : {}", graphics.open_gl_version());
    println!("Giochi : {totale}");
    println!("Layout : {}", layout.name);
    println!("Esc per uscire, frecce per scorrere.");

    let mut scelta = 0usize;
    let mut primo_visibile = 0usize;
    let mut font: Option<usize> = None;
    let mut scatto_fatto = false;
    let avvio = Instant::now();

    event_loop.run(move |event, controllo| {
        controllo.set_control_flow(winit::event_loop::ControlFlow::Wait);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => controllo.exit(),
                WindowEvent::Resized(dimensione) => {
                    graphics.resize(dimensione.width, dimensione.height);
                }
                WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => controllo.exit(),
                        PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
                            if scelta + 1 < totale {
                                scelta += 1;
                            }
                            graphics.window().request_redraw();
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
                            scelta = scelta.saturating_sub(1);
                            graphics.window().request_redraw();
                        }
                        PhysicalKey::Code(KeyCode::PageDown) => {
                            scelta = (scelta + 10).min(totale.saturating_sub(1));
                            graphics.window().request_redraw();
                        }
                        PhysicalKey::Code(KeyCode::PageUp) => {
                            scelta = scelta.saturating_sub(10);
                            graphics.window().request_redraw();
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
            Event::AboutToWait => {
                if let Some(limite) = auto_exit {
                    if avvio.elapsed() > limite {
                        controllo.exit();
                        return;
                    }
                }
                let schermo = graphics.screen();
                let mut quads = Vec::new();

                // Sfondo del layout.
                quads.push(Quad::full(schermo, layout.screensaver.background));

                let area_lista = layout
                    .find(&ElementKind::List)
                    .map(|elemento| elemento.rect(screen_size(schermo)))
                    .unwrap_or_else(|| Rect::new(20.0, 20.0, schermo.0 - 40.0, schermo.1 - 40.0));

                let stile_lista = layout
                    .find(&ElementKind::List)
                    .and_then(|elemento| elemento.style.clone());

                // Riquadro della lista.
                let sfondo_lista = stile_lista
                    .as_ref()
                    .and_then(|stile| stile.background)
                    .unwrap_or(Color::rgb(10, 10, 16));
                quads.push(Quad::new(area_lista, sfondo_lista));

                // Altezza di riga derivata dal carattere: quando ci sara' il
                // testo, la riga dovra' contenerlo.
                let altezza_carattere = stile_lista
                    .as_ref()
                    .map(|stile| stile.size.resolve(schermo.1))
                    .unwrap_or(schermo.1 * 0.02);
                let altezza_riga = (altezza_carattere * 1.8).max(8.0);
                let righe_visibili = ((area_lista.height / altezza_riga).floor() as usize).max(1);

                // Mantiene la selezione dentro la finestra visibile.
                if scelta < primo_visibile {
                    primo_visibile = scelta;
                } else if scelta >= primo_visibile + righe_visibili {
                    primo_visibile = scelta + 1 - righe_visibili;
                }

                let colore_testo = stile_lista
                    .as_ref()
                    .map(|stile| stile.color)
                    .unwrap_or(Color::rgb(150, 200, 50));
                let sfondo_scelto = Color::rgb(196, 128, 196);

                // Righe visibili: prima i rettangoli, poi i testi sopra.
                let mut righe: Vec<(Rect, usize)> = Vec::new();
                let ultima = (primo_visibile + righe_visibili).min(totale);
                for indice in primo_visibile..ultima {
                    let y = area_lista.y + (indice - primo_visibile) as f32 * altezza_riga;
                    let riga = Rect::new(
                        area_lista.x + 2.0,
                        y,
                        area_lista.width - 4.0,
                        altezza_riga - 1.0,
                    );
                    if indice == scelta {
                        quads.push(Quad::new(riga, sfondo_scelto));
                    }
                    righe.push((riga, indice));
                }

                // Area dello snapshot: solo il contorno, per far vedere dove
                // finira' l'immagine del gioco.
                if let Some(elemento) = layout.find(&ElementKind::Artwork(feel_core::ArtworkSlot::Snap))
                {
                    let area = elemento.rect(screen_size(schermo));
                    let bordo = 2.0;
                    let cornice = Color::rgb(40, 70, 120);
                    quads.push(Quad::new(Rect::new(area.x, area.y, area.width, bordo), cornice));
                    quads.push(Quad::new(
                        Rect::new(area.x, area.bottom() - bordo, area.width, bordo),
                        cornice,
                    ));
                    quads.push(Quad::new(Rect::new(area.x, area.y, bordo, area.height), cornice));
                    quads.push(Quad::new(
                        Rect::new(area.right() - bordo, area.y, bordo, area.height),
                        cornice,
                    ));
                }

                graphics.clear(Color::BLACK);
                graphics.draw_quads(&quads);

                // Il testo si disegna dopo i rettangoli, altrimenti finisce
                // sotto alla selezione.
                let pixel = altezza_carattere.max(9.0);
                if font.map_or(true, |id| {
                    (graphics.line_height(id) / 1.25 - pixel).abs() > 1.0
                }) {
                    font = carica_font(&mut graphics, pixel);
                }

                if let Some(id) = font {
                    let colore_scelto = contrasto(sfondo_scelto);
                    for (riga, indice) in &righe {
                        let colore = if *indice == scelta { colore_scelto } else { colore_testo };
                        let titolo = &giochi[*indice].title;
                        let y = riga.y + (riga.height - pixel) / 2.0;
                        let _ = graphics.draw_text(id, titolo, (riga.x + 6.0, y), colore);
                    }

                    // Contatore e nome della lista, se il layout li prevede.
                    if let Some(elemento) =
                        layout.find(&ElementKind::Label(feel_core::LabelKind::RomCounter))
                    {
                        let area = elemento.rect(screen_size(schermo));
                        let testo = format!("{} / {totale}", scelta + 1);
                        let colore = elemento
                            .style
                            .as_ref()
                            .map(|s| s.color)
                            .unwrap_or(Color::rgb(255, 128, 0));
                        let _ = graphics.draw_text(id, &testo, (area.x, area.y), colore);
                    }

                    if let Some(elemento) =
                        layout.find(&ElementKind::Label(feel_core::LabelKind::Gamelist))
                    {
                        let area = elemento.rect(screen_size(schermo));
                        let colore = elemento
                            .style
                            .as_ref()
                            .map(|s| s.color)
                            .unwrap_or(Color::rgb(128, 192, 192));
                        let _ = graphics.draw_text(id, nome_lista, (area.x, area.y), colore);
                    }
                }

                graphics.present();

                // Cattura del fotogramma, per verificare cosa si vede davvero
                // senza dover guardare lo schermo.
                if let Some(percorso) = &scatto {
                    if !scatto_fatto {
                        let (larghezza, altezza) = graphics.screen();
                        let pixel = graphics.read_pixels();
                        match scrivi_bmp(percorso, larghezza as u32, altezza as u32, &pixel) {
                            Ok(()) => println!("Fotogramma salvato: {}", percorso.display()),
                            Err(err) => eprintln!("non riesco a salvare {}: {err}", percorso.display()),
                        }
                        scatto_fatto = true;
                        controllo.exit();
                        return;
                    }
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}

fn screen_size(schermo: (f32, f32)) -> feel_core::ScreenSize {
    feel_core::ScreenSize::new(schermo.0, schermo.1)
}

/// Dove cercare un font, in ordine di preferenza.
///
/// Non ne includiamo uno nostro: sarebbe un file binario da versionare e una
/// licenza da verificare. I font di sistema bastano, e su Windows e sulle
/// Debian/Ubuntu ce n'e' sempre almeno uno di questi.
const FONT_CANDIDATI: [&str; 8] = [
    r"C:\Windows\Fonts\consola.ttf",
    r"C:\Windows\Fonts\segoeui.ttf",
    r"C:\Windows\Fonts\arial.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
];

/// Scrive un BMP a 24 bit non compresso.
///
/// BMP e non PNG perche' il formato non richiede nessuna libreria: quaranta
/// righe di intestazione e i pixel. Serve solo a guardare un fotogramma.
fn scrivi_bmp(
    percorso: &std::path::Path,
    larghezza: u32,
    altezza: u32,
    rgba: &[u8],
) -> std::io::Result<()> {
    let imbottitura = ((4 - (larghezza as usize * 3) % 4) % 4) as usize;
    let riga_byte = larghezza as usize * 3 + imbottitura;
    let dimensione_immagine = riga_byte * altezza as usize;
    let dimensione_file = 54 + dimensione_immagine;

    let mut out = Vec::with_capacity(dimensione_file);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(dimensione_file as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(larghezza as i32).to_le_bytes());
    // Altezza positiva: il BMP conserva le righe dal basso verso l'alto, come
    // arrivano da OpenGL, quindi non serve ribaltare niente.
    out.extend_from_slice(&(altezza as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(dimensione_immagine as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    for y in 0..altezza {
        for x in 0..larghezza {
            let i = ((y * larghezza + x) * 4) as usize;
            out.push(rgba[i + 2]);
            out.push(rgba[i + 1]);
            out.push(rgba[i]);
        }
        out.extend(std::iter::repeat(0).take(imbottitura));
    }

    std::fs::write(percorso, out)
}

fn carica_font(graphics: &mut feel_render::Graphics, pixel: f32) -> Option<usize> {
    for percorso in FONT_CANDIDATI {
        let p = std::path::Path::new(percorso);
        if !p.is_file() {
            continue;
        }
        match graphics.load_font(p, pixel) {
            Ok(id) => {
                println!("Font   : {percorso} a {pixel:.0} px");
                return Some(id);
            }
            Err(err) => eprintln!("font {percorso}: {err}"),
        }
    }
    eprintln!("Nessun font di sistema trovato: il testo non verra' disegnato.");
    None
}

/// Un colore leggibile sopra `sfondo`.
///
/// La luminosita' e' quella percepita, non la media dei canali: il verde pesa
/// molto piu' del blu, ed e' il motivo per cui un testo bianco su giallo e' piu'
/// leggibile di un testo bianco su blu.
fn contrasto(sfondo: Color) -> Color {
    let luminosita =
        0.299 * sfondo.r as f32 + 0.587 * sfondo.g as f32 + 0.114 * sfondo.b as f32;
    if luminosita > 140.0 {
        Color::rgb(16, 12, 16)
    } else {
        Color::rgb(245, 245, 245)
    }
}
