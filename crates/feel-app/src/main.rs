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
use feel_config::Settings;
use feel_db::Database;
use feel_render::{Effetto, Quad, Rotazione, Session};
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

    // Le impostazioni stanno accanto al database: un file solo per ogni
    // libreria, cosi' due cabinati con librerie diverse non si pestano i piedi.
    let percorso_impostazioni = database.with_file_name("feel.json");
    let impostazioni = Settings::load_or_create(&percorso_impostazioni)?;
    println!("Config : {}", percorso_impostazioni.display());

    let db = Database::open(&database)?;
    let chiave = feel_core::GamelistKey::new(nome_lista.clone());

    // Serve la libreria intera per sapere dove stanno le immagini
    // dell'emulatore: il percorso viene dalla configurazione di FEEL 1.x.
    let libreria = db.load_library()?;
    let cartella_snap = libreria
        .gamelist(&chiave)
        .and_then(|lista| libreria.emulator(&lista.platform, &lista.emulator))
        .and_then(|emulatore| emulatore.artwork.snapshot.clone());

    // L'emulatore serve sia per la cartella degli snapshot sia per avviare i
    // giochi: lo teniamo da parte invece di ricercarlo ogni volta.
    let emulatore = libreria
        .gamelist(&chiave)
        .and_then(|lista| libreria.emulator(&lista.platform, &lista.emulator))
        .cloned();

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

    // Le immagini del layout stanno nella stessa cartella del suo JSON.
    let cartella_layout = percorso_layout
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();

    // Un'immagine che non si carica va detta: senza il messaggio si vede solo
    // uno schermo nero e non si sa perche'.
    let mut carica = |nome: &str| -> Option<usize> {
        let percorso = cartella_layout.join(nome);
        if !percorso.is_file() {
            eprintln!("immagine assente: {}", percorso.display());
            return None;
        }
        match graphics.load_texture(&percorso) {
            Ok(id) => {
                println!("Immagine: {nome}");
                Some(id)
            }
            Err(err) => {
                eprintln!("immagine {nome}: {err}");
                None
            }
        }
    };

    let sfondo = layout
        .background
        .as_ref()
        .and_then(|fondo| fondo.image.as_deref())
        .and_then(&mut carica);

    // Immagine di ripiego quando un gioco non ha la sua: e' quella che FEEL 1.x
    // disegna dentro il riquadro dello snapshot.
    let segnaposto = carica("snap.png");

    // Schermo intero, se richiesto.
    if impostazioni.video.fullscreen {
        graphics.window().set_fullscreen(Some(
            winit::window::Fullscreen::Borderless(None),
        ));
    }

    let effetto = match impostazioni.video.shader {
        feel_core::Shader::None => Effetto::Nessuno,
        feel_core::Shader::Crt => Effetto::Crt,
        feel_core::Shader::Lcd => Effetto::Lcd,
        feel_core::Shader::Scanlines => Effetto::Scanline,
    };
    let rotazione = match impostazioni.video.rotate {
        feel_config::Rotate::None => Rotazione::Nessuna,
        feel_config::Rotate::Left => Rotazione::Sinistra,
        feel_config::Rotate::Right => Rotazione::Destra,
    };

    let mut scelta = 0usize;
    let mut primo_visibile = 0usize;
    let mut font: Option<usize> = None;
    let mut scatto_fatto = false;
    let avvio = Instant::now();

    // Cache dell'immagine del gioco selezionato: caricare una texture a ogni
    // fotogramma sarebbe uno spreco e farebbe scattare il disco sessanta
    // volte al secondo.
    let mut immagine_gioco: Option<(usize, Option<usize>)> = None;

    event_loop.run(move |event, controllo| {
        controllo.set_control_flow(winit::event_loop::ControlFlow::Wait);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => controllo.exit(),
                WindowEvent::Resized(dimensione) => {
                    graphics.resize(dimensione.width, dimensione.height);
                }
                WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                    // I tasti vengono dalle impostazioni, non dal codice: chi
                    // vuole il joystick come frecce cambia feel.json e basta.
                    let codice = match event.physical_key {
                        PhysicalKey::Code(c) => c,
                        _ => return,
                    };

                    if Some(codice) == tasto(&impostazioni.input.back) {
                        controllo.exit();
                    } else if Some(codice) == tasto(&impostazioni.input.select) {
                        // Avvio del gioco: qui il frontend si ferma e aspetta
                        // l'emulatore, che e' esattamente cosa deve succedere.
                        if let (Some(emulatore), Some(gioco)) =
                            (emulatore.as_ref(), giochi.get(scelta))
                        {
                            match avvia_gioco(emulatore, gioco, &database) {
                                Ok(durata) => println!(
                                    "Tornato al frontend dopo {:.0} secondi.",
                                    durata.as_secs_f64()
                                ),
                                Err(err) => eprintln!("non riesco ad avviare: {err}"),
                            }
                            graphics.window().request_redraw();
                        }
                    } else if Some(codice) == tasto(&impostazioni.input.down) {
                        if scelta + 1 < totale {
                            scelta += 1;
                        }
                        graphics.window().request_redraw();
                    } else if Some(codice) == tasto(&impostazioni.input.up) {
                        scelta = scelta.saturating_sub(1);
                        graphics.window().request_redraw();
                    } else if Some(codice) == tasto(&impostazioni.input.page_down) {
                        scelta = (scelta + 10).min(totale.saturating_sub(1));
                        graphics.window().request_redraw();
                    } else if Some(codice) == tasto(&impostazioni.input.page_up) {
                        scelta = scelta.saturating_sub(10);
                        graphics.window().request_redraw();
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

                // Il colore di sfondo del layout va nel `clear`, non in un
                // rettangolo: come rettangolo coprirebbe l'immagine di sfondo,
                // perche' i rettangoli si disegnano dopo.

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

                graphics.begin_scene(layout.screensaver.background);

                // Sfondo e snapshot vanno **prima** dei rettangoli, altrimenti
                // la cornice dello snapshot finisce sotto all'immagine.
                if let Some(id) = sfondo {
                    let _ = graphics.draw_texture(
                        id,
                        Rect::new(0.0, 0.0, schermo.0, schermo.1),
                        Color::WHITE,
                        false,
                    );
                }
                // Immagine del gioco selezionato, se esiste; altrimenti il
                // segnaposto del layout.
                if immagine_gioco.map_or(true, |(indice, _)| indice != scelta) {
                    let trovata = cartella_snap
                        .as_ref()
                        .and_then(|cartella| trova_immagine(cartella, giochi[scelta].key.as_str()))
                        .and_then(|percorso| graphics.load_texture(&percorso).ok());
                    if trovata.is_some() {
                        println!("Snapshot del gioco: {}", giochi[scelta].title);
                    }
                    immagine_gioco = Some((scelta, trovata));
                }
                let da_disegnare = immagine_gioco
                    .and_then(|(_, id)| id)
                    .or(segnaposto);

                if let Some(id) = da_disegnare {
                    if let Some(elemento) =
                        layout.find(&ElementKind::Artwork(feel_core::ArtworkSlot::Snap))
                    {
                        let area = elemento.rect(screen_size(schermo));
                        let _ = graphics.draw_texture(id, area, Color::WHITE, !elemento.stretch);
                    }
                }

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

                // Ultimo passaggio: effetto CRT, rotazione, e a schermo.
                graphics.end_scene(effetto, rotazione);

                // Cattura del fotogramma, per verificare cosa si vede davvero
                // senza dover guardare lo schermo. Va fatta prima di scambiare
                // i buffer, altrimenti si legge il buffer sbagliato.
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

                graphics.present();
            }
            _ => {}
        }
    })?;

    Ok(())
}

/// Avvia il gioco e registra il tempo passato.
///
/// Blocca finché l'emulatore non esce: e' voluto. Un frontend che continua a
/// disegnare mentre l'emulatore gira si contende la scheda video con il gioco.
fn avvia_gioco(
    emulatore: &feel_core::Emulator,
    gioco: &feel_core::Game,
    percorso_db: &std::path::Path,
) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    let piano = feel_launch::LaunchPlan::resolve(emulatore, gioco);

    println!();
    println!("Avvio: {}", gioco.title);
    println!("  {}", piano.command_line().join(" "));
    if !piano.rom_found {
        println!("  (la rom non e' dove la cerchiamo: potrebbe essere normale)");
    }

    let esito = piano.run()?;
    let durata = esito.duration();

    if !esito.success() {
        println!("  uscito con codice {:?}", esito.status.code());
    }

    // Registrare la partita non deve far fallire il ritorno al frontend: se il
    // database e' occupato o in sola lettura, il gioco e' comunque finito.
    match Database::open(percorso_db) {
        Ok(db) => {
            let riferimento = (emulatore.platform.clone(), emulatore.key.clone());
            if let Err(err) = db.record_play(
                &riferimento,
                &gioco.key,
                durata,
                feel_core::Timestamp::now(),
            ) {
                eprintln!("  non ho potuto registrare la partita: {err}");
            }
        }
        Err(err) => eprintln!("  non ho potuto riaprire il database: {err}"),
    }

    Ok(durata)
}

/// Cerca l'immagine di un gioco nella cartella degli snapshot.
///
/// Le estensioni non sono configurabili in FEEL 1.x per ogni emulatore (c'e'
/// `snapshot_extension`, ma vale per tutti): provarle in ordine costa tre
/// `is_file` e copre il 99% delle installazioni reali.
fn trova_immagine(cartella: &std::path::Path, nome: &str) -> Option<PathBuf> {
    for estensione in ["png", "jpg", "jpeg", "PNG", "JPG"] {
        let candidato = cartella.join(format!("{nome}.{estensione}"));
        if candidato.is_file() {
            return Some(candidato);
        }
    }
    None
}

/// Traduce un nome di tasto delle impostazioni in un codice di winit.
///
/// I nomi sono quelli di FEEL 1.x (`Up`, `Return`, `LAlt`, `ESC`), cosi' chi
/// arriva dal vecchio frontend ritrova le stesse parole nel file.
fn tasto(nome: &str) -> Option<KeyCode> {
    let normalizzato = nome.trim();
    let codice = match normalizzato.to_ascii_lowercase().as_str() {
        "up" | "su" => KeyCode::ArrowUp,
        "down" | "giu" => KeyCode::ArrowDown,
        "left" | "sinistra" => KeyCode::ArrowLeft,
        "right" | "destra" => KeyCode::ArrowRight,
        "return" | "enter" | "invio" => KeyCode::Enter,
        "escape" | "esc" => KeyCode::Escape,
        "space" | "spazio" => KeyCode::Space,
        "pageup" | "pagina_su" => KeyCode::PageUp,
        "pagedown" | "pagina_giu" => KeyCode::PageDown,
        "tab" => KeyCode::Tab,
        "lalt" => KeyCode::AltLeft,
        "ralt" => KeyCode::AltRight,
        "lctrl" | "lcontrol" => KeyCode::ControlLeft,
        "rctrl" | "rcontrol" => KeyCode::ControlRight,
        "lshift" => KeyCode::ShiftLeft,
        "rshift" => KeyCode::ShiftRight,
        altro => {
            // Una lettera o una cifra singola: `1`, `2`, `A`...
            let mut caratteri = altro.chars();
            match (caratteri.next(), caratteri.next()) {
                (Some('1'), None) => KeyCode::Digit1,
                (Some('2'), None) => KeyCode::Digit2,
                (Some('3'), None) => KeyCode::Digit3,
                (Some('4'), None) => KeyCode::Digit4,
                (Some('5'), None) => KeyCode::Digit5,
                (Some('6'), None) => KeyCode::Digit6,
                (Some('7'), None) => KeyCode::Digit7,
                (Some('8'), None) => KeyCode::Digit8,
                (Some('9'), None) => KeyCode::Digit9,
                (Some('0'), None) => KeyCode::Digit0,
                (Some('a'), None) => KeyCode::KeyA,
                (Some('b'), None) => KeyCode::KeyB,
                (Some('c'), None) => KeyCode::KeyC,
                (Some('d'), None) => KeyCode::KeyD,
                (Some('e'), None) => KeyCode::KeyE,
                (Some('f'), None) => KeyCode::KeyF,
                (Some('g'), None) => KeyCode::KeyG,
                (Some('h'), None) => KeyCode::KeyH,
                (Some('i'), None) => KeyCode::KeyI,
                (Some('j'), None) => KeyCode::KeyJ,
                (Some('k'), None) => KeyCode::KeyK,
                (Some('l'), None) => KeyCode::KeyL,
                (Some('m'), None) => KeyCode::KeyM,
                (Some('n'), None) => KeyCode::KeyN,
                (Some('o'), None) => KeyCode::KeyO,
                (Some('p'), None) => KeyCode::KeyP,
                (Some('q'), None) => KeyCode::KeyQ,
                (Some('r'), None) => KeyCode::KeyR,
                (Some('s'), None) => KeyCode::KeyS,
                (Some('t'), None) => KeyCode::KeyT,
                (Some('u'), None) => KeyCode::KeyU,
                (Some('v'), None) => KeyCode::KeyV,
                (Some('w'), None) => KeyCode::KeyW,
                (Some('x'), None) => KeyCode::KeyX,
                (Some('y'), None) => KeyCode::KeyY,
                (Some('z'), None) => KeyCode::KeyZ,
                _ => return None,
            }
        }
    };

    let _ = normalizzato;
    Some(codice)
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
