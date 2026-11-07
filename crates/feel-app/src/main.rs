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

                for (indice, _gioco) in giochi
                    .iter()
                    .enumerate()
                    .skip(primo_visibile)
                    .take(righe_visibili)
                {
                    let y = area_lista.y + (indice - primo_visibile) as f32 * altezza_riga;
                    let riga = Rect::new(
                        area_lista.x + 2.0,
                        y,
                        area_lista.width - 4.0,
                        altezza_riga - 1.0,
                    );

                    if indice == scelta {
                        quads.push(Quad::new(riga, sfondo_scelto));
                    } else {
                        // Una barra che occupa una frazione della riga: senza
                        // testo, e' quello che rende visibile la lunghezza del
                        // titolo. Sparisce quando ci sara' il testo vero.
                        let lunghezza = 0.35 + ((indice * 37) % 55) as f32 / 100.0;
                        let barra = Rect::new(
                            riga.x + 2.0,
                            riga.y + riga.height * 0.25,
                            (riga.width - 8.0) * lunghezza.min(1.0),
                            riga.height * 0.5,
                        );
                        quads.push(Quad::new(barra, colore_testo));
                    }
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
                graphics.present();
            }
            _ => {}
        }
    })?;

    Ok(())
}

fn screen_size(schermo: (f32, f32)) -> feel_core::ScreenSize {
    feel_core::ScreenSize::new(schermo.0, schermo.1)
}
