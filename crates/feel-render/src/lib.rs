//! Backend grafico di FEEL 2.0.
//!
//! Una finestra, un contesto OpenGL 3.3, e il disegno di rettangoli colorati.
//! Nient'altro: il testo, le immagini e gli shader arrivano sopra a questa base.
//!
//! Le scelte tecniche stanno in `docs/ARCHITETTURA.md` (decisione D2). Qui vale
//! la pena annotare le due cose che sono costate tempo per scoprirle:
//!
//! - `glutin_winit::DisplayBuilder` vuole **`with_window_builder`**, non
//!   `with_window_attributes`: le due API si somigliano e gli errori del
//!   compilatore non aiutano.
//! - winit 0.29 usa ancora `raw_window_handle()` di raw-window-handle 0.5, non
//!   `window_handle()` della 0.6. Insieme sbagliato: non compila.

use std::num::NonZeroU32;

use feel_core::{Color, Rect};
use glutin::config::{Api, ConfigTemplateBuilder};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext, Version,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{
    GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface,
};
use glutin_winit::DisplayBuilder;
use glow::HasContext;
use raw_window_handle::HasRawWindowHandle;
use thiserror::Error;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

mod text;

pub use text::{Carattere, Glifo};

/// Un rettangolo colorato da disegnare.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub rect: Rect,
    pub color: Color,
}

impl Quad {
    pub fn new(rect: Rect, color: Color) -> Self {
        Self { rect, color }
    }

    /// Un rettangolo pieno che copre tutto lo schermo.
    pub fn full(screen: (f32, f32), color: Color) -> Self {
        Self::new(Rect::new(0.0, 0.0, screen.0, screen.1), color)
    }
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("non riesco a creare la finestra: {0}")]
    Window(String),

    #[error("non riesco a creare il contesto OpenGL: {0}")]
    Context(String),

    #[error("OpenGL: {0}")]
    Gl(String),
}

/// La finestra e il contesto, pronti per disegnare.
///
/// Il ciclo di eventi resta separato: chi crea una `Session` lo prende e lo
/// esegue, cosi' questo crate non impone come si gestiscono gli eventi.
pub struct Session {
    pub event_loop: EventLoop<()>,
    pub graphics: Graphics,
}

/// Quello che serve per disegnare.
/// Un'immagine caricata in una texture.
#[derive(Debug, Clone, Copy)]
pub struct Testura {
    id: glow::Texture,
    pub width: u32,
    pub height: u32,
}

/// Rotazione dell'immagine finale, per i cabinati verticali.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rotazione {
    #[default]
    Nessuna,
    Sinistra,
    Destra,
}

impl Rotazione {
    fn indice(self) -> i32 {
        match self {
            Self::Nessuna => 0,
            Self::Sinistra => 1,
            Self::Destra => 2,
        }
    }
}

/// Effetto applicato all'immagine finale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Effetto {
    #[default]
    Nessuno,
    Crt,
    Lcd,
    Scanline,
}

impl Effetto {
    fn indice(self) -> i32 {
        match self {
            Self::Nessuno => 0,
            Self::Crt => 1,
            Self::Lcd => 2,
            Self::Scanline => 3,
        }
    }
}

pub struct Graphics {
    window: Window,
    gl: glow::Context,
    context: PossiblyCurrentContext,
    surface: Surface<WindowSurface>,
    program: glow::Program,
    vertex_array: glow::VertexArray,
    vertices: glow::Buffer,
    /// Programma per i rettangoli texturizzati (testo).
    programma_testo: glow::Program,
    vao_testo: glow::VertexArray,
    vbo_testo: glow::Buffer,
    caratteri: Vec<Carattere>,
    textures: Vec<Testura>,
    programma_composizione: glow::Program,
    fbo: glow::Framebuffer,
    fbo_texture: glow::Texture,
    fbo_dimensione: (i32, i32),
    screen: (f32, f32),
}

const VERTICE: &str = r#"#version 330 core
layout(location = 0) in vec2 posizione;
layout(location = 1) in vec4 colore;
uniform vec2 schermo;
out vec4 v_colore;
void main() {
    // Da pixel con l'origine in alto a sinistra a coordinate normalizzate.
    vec2 n = vec2(posizione.x / schermo.x * 2.0 - 1.0,
                  1.0 - posizione.y / schermo.y * 2.0);
    gl_Position = vec4(n, 0.0, 1.0);
    v_colore = colore;
}
"#;

const FRAMMENTO: &str = r#"#version 330 core
in vec4 v_colore;
out vec4 colore_finale;
void main() {
    colore_finale = v_colore;
}
"#;

const TESTO_VERTICE: &str = r#"#version 330 core
layout(location = 0) in vec2 posizione;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec4 colore;
uniform vec2 schermo;
out vec2 v_uv;
out vec4 v_colore;
void main() {
    vec2 n = vec2(posizione.x / schermo.x * 2.0 - 1.0,
                  1.0 - posizione.y / schermo.y * 2.0);
    gl_Position = vec4(n, 0.0, 1.0);
    v_uv = uv;
    v_colore = colore;
}
"#;

/// Il passaggio finale: prende la scena disegnata in una texture e la porta a
/// schermo applicando l'effetto e la rotazione.
///
/// Serve un passaggio in piu' perche' scanline, curvatura e maschera RGB sono
/// effetti *sull'immagine intera*: per applicarli bisogna averla gia' disegnata.
const COMPOSIZIONE_FRAMMENTO: &str = r#"#version 330 core
in vec2 v_uv;
in vec4 v_colore;

uniform sampler2D scena;
uniform vec2 risoluzione;
uniform int effetto;
uniform int rotazione;

out vec4 colore_finale;

void main() {
    vec2 uv = v_uv;

    // Rotazione per i cabinati verticali.
    if (rotazione == 1) {
        uv = vec2(uv.y, 1.0 - uv.x);
    } else if (rotazione == 2) {
        uv = vec2(1.0 - uv.y, uv.x);
    }

    vec2 p = uv * 2.0 - 1.0;

    // Curvatura del tubo: si allontanano le coordinate dal centro in modo
    // quadratico, cosi' i bordi si incurvano come su un CRT vero.
    if (effetto == 1) {
        p *= 1.0 + 0.045 * dot(p, p);
        uv = p * 0.5 + 0.5;
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            colore_finale = vec4(0.0, 0.0, 0.0, 1.0);
            return;
        }
    }

    vec3 c = texture(scena, uv).rgb;

    // Scanline: una riga chiara e una scura, come su un tubo.
    //
    // Il periodo e' di due pixel *fisici* e si legge da gl_FragCoord, non da
    // uv: con uv e l'altezza dello schermo si otterrebbero centinaia di cicli
    // su un monitor moderno, che non si vedono e producono moire'.
    if (effetto == 1 || effetto == 3) {
        float linea = sin(gl_FragCoord.y * 3.14159265);
        float attenuazione = linea > 0.0 ? 1.0 : 0.72;
        c *= attenuazione;
    }

    // Maschera RGB: le tre componenti non stanno negli stessi pixel.
    if (effetto == 1) {
        float colonna = mod(gl_FragCoord.x, 3.0);
        vec3 maschera = vec3(0.90);
        if (colonna < 1.0) {
            maschera.r = 1.0;
        } else if (colonna < 2.0) {
            maschera.g = 1.0;
        } else {
            maschera.b = 1.0;
        }
        c *= maschera;

        // Vignettatura e leggero bagliore al centro.
        float vignetta = clamp(1.0 - 0.38 * dot(p, p), 0.0, 1.0);
        c *= vignetta;
        c *= 1.08;
    }

    colore_finale = vec4(clamp(c, 0.0, 1.0), 1.0);
}
"#;

const TESTO_FRAMMENTO: &str = r#"#version 330 core
in vec2 v_uv;
in vec4 v_colore;
uniform sampler2D atlante;
out vec4 colore_finale;
void main() {
    // Moltiplicare per il colore del vertice serve a entrambi i casi:
    // l'atlante dei caratteri e' bianco con la copertura nell'alfa (quindi
    // esce il colore del testo), un'immagine viene tinta dal colore.
    colore_finale = texture(atlante, v_uv) * v_colore;
}
"#;

impl Session {
    /// Apre una finestra con un contesto OpenGL 3.3.
    pub fn open(titolo: &str) -> Result<Self, RenderError> {
        let event_loop = EventLoop::new().map_err(|e| RenderError::Window(e.to_string()))?;

        let builder = WindowBuilder::new()
            .with_title(titolo)
            // Una dimensione di partenza sensata: senza, il sistema sceglie,
            // e su uno schermo 1440p la finestra diventa enorme.
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .with_resizable(true);

        let template = ConfigTemplateBuilder::new().with_api(Api::OPENGL);

        let (window, gl_config) = DisplayBuilder::new()
            .with_window_builder(Some(builder))
            .build(&event_loop, template, |configs| {
                configs
                    .reduce(|_, migliore| migliore)
                    .expect("il driver non offre nessuna configurazione OpenGL")
            })
            .map_err(|e| RenderError::Window(e.to_string()))?;

        let window = window.ok_or_else(|| RenderError::Window("nessuna finestra".into()))?;
        let raw = window.raw_window_handle();

        let display = gl_config.display();
        let attributi = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(raw));
        let context = unsafe { display.create_context(&gl_config, &attributi) }
            .map_err(|e| RenderError::Context(e.to_string()))?;

        let dimensione = window.inner_size();
        let superficie = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw,
            NonZeroU32::new(dimensione.width.max(1)).unwrap(),
            NonZeroU32::new(dimensione.height.max(1)).unwrap(),
        );
        let surface = unsafe { display.create_window_surface(&gl_config, &superficie) }
            .map_err(|e| RenderError::Context(e.to_string()))?;

        let context = context
            .make_current(&surface)
            .map_err(|e| RenderError::Context(e.to_string()))?;

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|nome| display.get_proc_address(nome) as *const _)
        };

        let program = unsafe { compila_programma(&gl)? };
        let vertex_array = unsafe {
            gl.create_vertex_array()
                .map_err(|e| RenderError::Gl(e.to_string()))?
        };
        let vertices = unsafe { gl.create_buffer().map_err(|e| RenderError::Gl(e.to_string()))? };

        let programma_testo = unsafe { compila_programma_da(&gl, TESTO_VERTICE, TESTO_FRAMMENTO)? };
        let vao_testo = unsafe { gl.create_vertex_array().map_err(|e| RenderError::Gl(e.to_string()))? };
        let vbo_testo = unsafe { gl.create_buffer().map_err(|e| RenderError::Gl(e.to_string()))? };

        unsafe {
            // posizione (2) + uv (2) + colore (4) = 32 byte
            gl.bind_vertex_array(Some(vao_testo));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo_testo));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 32, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 32, 8);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 4, glow::FLOAT, false, 32, 16);

            // Il testo ha bisogno della trasparenza: senza, i bordi dei
            // caratteri diventano rettangoli neri.
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.bind_vertex_array(None);
        }

        unsafe {
            gl.bind_vertex_array(Some(vertex_array));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertices));
            // posizione (2 float) + colore (4 float)
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 24, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, 24, 8);
        }

        surface
            .set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
            .ok();

        let programma_composizione =
            unsafe { compila_programma_da(&gl, TESTO_VERTICE, COMPOSIZIONE_FRAMMENTO)? };
        let fbo = unsafe { gl.create_framebuffer().map_err(|e| RenderError::Gl(e.to_string()))? };
        let fbo_texture = unsafe { gl.create_texture().map_err(|e| RenderError::Gl(e.to_string()))? };

        let mut graphics = Graphics {
            window,
            gl,
            context,
            surface,
            program,
            vertex_array,
            vertices,
            programma_testo,
            vao_testo,
            vbo_testo,
            caratteri: Vec::new(),
            textures: Vec::new(),
            programma_composizione,
            fbo,
            fbo_texture,
            fbo_dimensione: (0, 0),
            screen: (dimensione.width as f32, dimensione.height as f32),
        };
        graphics.ricrea_fbo(dimensione.width as i32, dimensione.height as i32);

        Ok(Self { event_loop, graphics })
    }
}

impl Graphics {
    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Dimensioni in pixel fisici.
    pub fn screen(&self) -> (f32, f32) {
        self.screen
    }

    pub fn resize(&mut self, larghezza: u32, altezza: u32) {
        if larghezza == 0 || altezza == 0 {
            return;
        }
        self.screen = (larghezza as f32, altezza as f32);
        self.surface.resize(
            &self.context,
            NonZeroU32::new(larghezza).unwrap(),
            NonZeroU32::new(altezza).unwrap(),
        );
        self.ricrea_fbo(larghezza as i32, altezza as i32);
    }

    /// Ricrea la texture su cui si disegna la scena.
    ///
    /// Va rifatta a ogni cambio di dimensione: una texture di dimensioni
    /// sbagliate produce un'immagine tagliata o sfocata, non un errore.
    fn ricrea_fbo(&mut self, larghezza: i32, altezza: i32) {
        if larghezza <= 0 || altezza <= 0 {
            return;
        }
        if self.fbo_dimensione == (larghezza, altezza) {
            return;
        }

        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.fbo_texture));
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                larghezza,
                altezza,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );

            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(self.fbo_texture),
                0,
            );
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            self.gl.bind_texture(glow::TEXTURE_2D, None);
        }

        self.fbo_dimensione = (larghezza, altezza);
    }

    /// Inizia a disegnare la scena nella texture intermedia.
    ///
    /// Da chiamare al posto di `clear`: pulisce e prepara il bersaglio.
    pub fn begin_scene(&self, colore: Color) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.viewport(0, 0, self.fbo_dimensione.0, self.fbo_dimensione.1);
            self.gl.clear_color(
                colore.r as f32 / 255.0,
                colore.g as f32 / 255.0,
                colore.b as f32 / 255.0,
                colore.a as f32 / 255.0,
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    /// Chiude la scena e la porta a schermo con l'effetto scelto.
    pub fn end_scene(&self, effetto: Effetto, rotazione: Rotazione) {
        let schermo = Rect::new(0.0, 0.0, self.screen.0, self.screen.1);
        // L'asse verticale della texture e' invertito rispetto allo schermo:
        // nei framebuffer OpenGL v=0 sta in basso, mentre il nostro vertice
        // in alto a sinistra deve campionare l'alto della scena.
        let dati = vertici_texturizzati(schermo, [0.0, 1.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]);

        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            self.gl.viewport(0, 0, self.screen.0 as i32, self.screen.1 as i32);

            self.gl.use_program(Some(self.programma_composizione));

            let pos = self.gl.get_uniform_location(self.programma_composizione, "schermo");
            self.gl.uniform_2_f32(pos.as_ref(), self.screen.0, self.screen.1);
            let risoluzione = self
                .gl
                .get_uniform_location(self.programma_composizione, "risoluzione");
            self.gl
                .uniform_2_f32(risoluzione.as_ref(), self.screen.0, self.screen.1);
            let eff = self.gl.get_uniform_location(self.programma_composizione, "effetto");
            self.gl.uniform_1_i32(eff.as_ref(), effetto.indice());
            let rot = self.gl.get_uniform_location(self.programma_composizione, "rotazione");
            self.gl.uniform_1_i32(rot.as_ref(), rotazione.indice());
            let scena = self.gl.get_uniform_location(self.programma_composizione, "scena");
            self.gl.uniform_1_i32(scena.as_ref(), 0);

            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.fbo_texture));

            self.gl.bind_vertex_array(Some(self.vao_testo));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo_testo));
            let byte: &[u8] = core::slice::from_raw_parts(
                dati.as_ptr() as *const u8,
                dati.len() * std::mem::size_of::<f32>(),
            );
            self.gl
                .buffer_data_u8_slice(glow::ARRAY_BUFFER, byte, glow::STREAM_DRAW);
            self.gl.draw_arrays(glow::TRIANGLES, 0, 6);
            self.gl.bind_vertex_array(None);
        }
    }

    /// Pulisce lo schermo.
    pub fn clear(&self, colore: Color) {
        unsafe {
            self.gl.clear_color(
                colore.r as f32 / 255.0,
                colore.g as f32 / 255.0,
                colore.b as f32 / 255.0,
                colore.a as f32 / 255.0,
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    /// Disegna i rettangoli, nell'ordine dato: l'ultimo sta sopra.
    pub fn draw_quads(&self, quads: &[Quad]) {
        if quads.is_empty() {
            return;
        }

        // Sei vertici per rettangolo, in modo da non serve un indice.
        let mut dati: Vec<f32> = Vec::with_capacity(quads.len() * 6 * 6);
        for quad in quads {
            let (x, y, w, h) = (
                quad.rect.x,
                quad.rect.y,
                quad.rect.width,
                quad.rect.height,
            );
            let colore = [
                quad.color.r as f32 / 255.0,
                quad.color.g as f32 / 255.0,
                quad.color.b as f32 / 255.0,
                quad.color.a as f32 / 255.0,
            ];
            let angoli = [(x, y), (x + w, y), (x + w, y + h), (x, y), (x + w, y + h), (x, y + h)];
            for (px, py) in angoli {
                dati.push(px);
                dati.push(py);
                dati.extend_from_slice(&colore);
            }
        }

        unsafe {
            self.gl.use_program(Some(self.program));
            let posizione = self.gl.get_uniform_location(self.program, "schermo");
            self.gl
                .uniform_2_f32(posizione.as_ref(), self.screen.0, self.screen.1);

            self.gl.bind_vertex_array(Some(self.vertex_array));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vertices));

            let byte: &[u8] = core::slice::from_raw_parts(
                dati.as_ptr() as *const u8,
                dati.len() * std::mem::size_of::<f32>(),
            );
            self.gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, byte, glow::STREAM_DRAW);

            self.gl.draw_arrays(glow::TRIANGLES, 0, (quads.len() * 6) as i32);
        }
    }

    /// Mostra quello che e' stato disegnato.
    pub fn present(&self) {
        let _ = self.surface.swap_buffers(&self.context);
    }

    /// Versione OpenGL, per il log di avvio.
    pub fn open_gl_version(&self) -> String {
        unsafe { self.gl.get_parameter_string(glow::VERSION) }
    }

    /// Copia il fotogramma corrente in RGBA.
    ///
    /// Le righe arrivano **dal basso verso l'alto**, come le consegna OpenGL:
    /// chi le scrive su file deve tenerne conto (il BMP, per fortuna, e' gia'
    /// orientato cosi').
    pub fn read_pixels(&self) -> Vec<u8> {
        let larghezza = self.screen.0 as i32;
        let altezza = self.screen.1 as i32;
        let mut dati = vec![0u8; (larghezza.max(0) * altezza.max(0) * 4) as usize];

        unsafe {
            self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            self.gl.read_pixels(
                0,
                0,
                larghezza,
                altezza,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(&mut dati),
            );
        }

        dati
    }

    // -- testo -----------------------------------------------------------------

    /// Carica un font a una certa dimensione in pixel.
    ///
    /// Ritorna l'indice da passare a `draw_text`.
    pub fn load_font(
        &mut self,
        percorso: &std::path::Path,
        pixel: f32,
    ) -> Result<usize, RenderError> {
        let carattere = Carattere::new(&self.gl, percorso, pixel).map_err(RenderError::Gl)?;
        self.caratteri.push(carattere);
        Ok(self.caratteri.len() - 1)
    }

    /// Altezza di una riga per quel font.
    pub fn line_height(&self, font: usize) -> f32 {
        self.caratteri
            .get(font)
            .map(|c| c.altezza() * 1.25)
            .unwrap_or(16.0)
    }

    /// Larghezza in pixel di una scritta.
    pub fn measure_text(&mut self, font: usize, testo: &str) -> f32 {
        let (gl, caratteri) = (&self.gl, &mut self.caratteri);
        match caratteri.get_mut(font) {
            Some(carattere) => carattere.misura(gl, testo).unwrap_or(0.0),
            None => 0.0,
        }
    }

    /// Disegna una scritta con l'angolo in alto a sinistra in `posizione`.
    pub fn draw_text(
        &mut self,
        font: usize,
        testo: &str,
        posizione: (f32, f32),
        colore: Color,
    ) -> Result<(), RenderError> {
        let (gl, caratteri) = (&self.gl, &mut self.caratteri);
        let Some(carattere) = caratteri.get_mut(font) else {
            return Ok(());
        };


        carattere.aggiorna(gl);
        let glifi = carattere.impagina(gl, testo).map_err(RenderError::Gl)?;
        if glifi.is_empty() {
            return Ok(());
        }

        let texture = carattere.texture();
        let rgba = [
            colore.r as f32 / 255.0,
            colore.g as f32 / 255.0,
            colore.b as f32 / 255.0,
            colore.a as f32 / 255.0,
        ];

        let mut dati: Vec<f32> = Vec::with_capacity(glifi.len() * 6 * 8);
        for (glifo, avanzamento) in glifi {
            if glifo.size[0] == 0.0 || glifo.size[1] == 0.0 {
                continue;
            }
            let x = posizione.0 + avanzamento + glifo.offset[0];
            let y = posizione.1 + glifo.offset[1];
            let (w, h) = (glifo.size[0], glifo.size[1]);
            let [u0, v0, u1, v1] = glifo.uv;

            let angoli = [
                (x, y, u0, v0),
                (x + w, y, u1, v0),
                (x + w, y + h, u1, v1),
                (x, y, u0, v0),
                (x + w, y + h, u1, v1),
                (x, y + h, u0, v1),
            ];
            for (px, py, u, v) in angoli {
                dati.push(px);
                dati.push(py);
                dati.push(u);
                dati.push(v);
                dati.extend_from_slice(&rgba);
            }
        }

        if dati.is_empty() {
            return Ok(());
        }

        self.disegna_texturizzato(texture, &dati);
        Ok(())
    }

    // -- immagini ---------------------------------------------------------------

    /// Carica un PNG e ritorna l'indice da passare a `draw_texture`.
    pub fn load_texture(&mut self, percorso: &std::path::Path) -> Result<usize, RenderError> {
        let file = std::fs::File::open(percorso)
            .map_err(|e| RenderError::Gl(format!("{}: {e}", percorso.display())))?;
        let mut lettore = png::Decoder::new(std::io::BufReader::new(file))
            .read_info()
            .map_err(|e| RenderError::Gl(format!("{}: {e}", percorso.display())))?;

        let mut buffer = vec![0; lettore.output_buffer_size()];
        let info = lettore
            .next_frame(&mut buffer)
            .map_err(|e| RenderError::Gl(format!("{}: {e}", percorso.display())))?;

        let (larghezza, altezza) = (info.width, info.height);

        // OpenGL vuole RGBA: il PNG puo' arrivare in RGB, in scala di grigi o
        // con una palette, quindi si converte qui invece di pretendere un formato.
        let rgba = match info.color_type {
            png::ColorType::Rgba => buffer[..info.buffer_size()].to_vec(),
            png::ColorType::Rgb => buffer[..info.buffer_size()]
                .chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
            png::ColorType::Grayscale => buffer[..info.buffer_size()]
                .iter()
                .flat_map(|g| [*g, *g, *g, 255])
                .collect(),
            png::ColorType::GrayscaleAlpha => buffer[..info.buffer_size()]
                .chunks_exact(2)
                .flat_map(|p| [p[0], p[0], p[0], p[1]])
                .collect(),
            altro => {
                return Err(RenderError::Gl(format!(
                    "{}: formato PNG non gestito ({altro:?})",
                    percorso.display()
                )))
            }
        };

        let id = unsafe { self.gl.create_texture().map_err(|e| RenderError::Gl(e.to_string()))? };
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(id));
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                larghezza as i32,
                altezza as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&rgba),
            );
        }

        self.textures.push(Testura { id, width: larghezza, height: altezza });
        Ok(self.textures.len() - 1)
    }

    /// Sostituisce i pixel di una texture gia' caricata.
    ///
    /// Serve ai video: ricreare la texture a ogni fotogramma significherebbe
    /// riallocare memoria trenta volte al secondo.
    pub fn update_texture(
        &self,
        indice: usize,
        pixel: &[u8],
        larghezza: u32,
        altezza: u32,
    ) -> Result<(), RenderError> {
        let Some(testura) = self.textures.get(indice) else {
            return Ok(());
        };

        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(testura.id));
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                larghezza as i32,
                altezza as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(pixel),
            );
        }
        Ok(())
    }

    /// Crea una texture vuota, da riempire con `update_texture`.
    ///
    pub fn create_texture(&mut self, larghezza: u32, altezza: u32) -> Result<usize, RenderError> {
        let vuoto = vec![0u8; (larghezza * altezza * 4) as usize];
        let id = unsafe { self.gl.create_texture().map_err(|e| RenderError::Gl(e.to_string()))? };
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(id));
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                larghezza as i32,
                altezza as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&vuoto),
            );
        }
        self.textures.push(Testura { id, width: larghezza, height: altezza });
        Ok(self.textures.len() - 1)
    }

    pub fn texture_size(&self, indice: usize) -> Option<(u32, u32)> {
        self.textures.get(indice).map(|t| (t.width, t.height))
    }

    /// Disegna un'immagine dentro `rect`, tinta di `tinta`.
    ///
    /// Se `mantieni_proporzioni` e' vero l'immagine viene centrata e ridotta
    /// fino a entrare, invece di essere deformata.
    pub fn draw_texture(
        &mut self,
        indice: usize,
        area: Rect,
        tinta: Color,
        mantieni_proporzioni: bool,
    ) -> Result<(), RenderError> {
        let Some(testura) = self.textures.get(indice).copied() else {
            return Ok(());
        };

        let mut rettangolo = area;
        if mantieni_proporzioni && testura.width > 0 && testura.height > 0 {
            let rapporto_immagine = testura.width as f32 / testura.height as f32;
            let rapporto_area = area.width / area.height;
            if rapporto_immagine > rapporto_area {
                let altezza = area.width / rapporto_immagine;
                rettangolo = Rect::new(
                    area.x,
                    area.y + (area.height - altezza) / 2.0,
                    area.width,
                    altezza,
                );
            } else {
                let larghezza = area.height * rapporto_immagine;
                rettangolo = Rect::new(
                    area.x + (area.width - larghezza) / 2.0,
                    area.y,
                    larghezza,
                    area.height,
                );
            }
        }

        let rgba = [
            tinta.r as f32 / 255.0,
            tinta.g as f32 / 255.0,
            tinta.b as f32 / 255.0,
            tinta.a as f32 / 255.0,
        ];
        let dati = vertici_texturizzati(rettangolo, [0.0, 0.0, 1.0, 1.0], rgba);
        self.disegna_texturizzato(testura.id, &dati);
        Ok(())
    }

    /// Il disegno vero e proprio: programma, texture, vertici.
    fn disegna_texturizzato(&self, texture: glow::Texture, dati: &[f32]) {
        unsafe {
            self.gl.use_program(Some(self.programma_testo));
            let pos = self.gl.get_uniform_location(self.programma_testo, "schermo");
            self.gl
                .uniform_2_f32(pos.as_ref(), self.screen.0, self.screen.1);
            let unita = self.gl.get_uniform_location(self.programma_testo, "atlante");
            self.gl.uniform_1_i32(unita.as_ref(), 0);

            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));

            self.gl.bind_vertex_array(Some(self.vao_testo));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo_testo));
            let byte: &[u8] = core::slice::from_raw_parts(
                dati.as_ptr() as *const u8,
                dati.len() * std::mem::size_of::<f32>(),
            );
            self.gl
                .buffer_data_u8_slice(glow::ARRAY_BUFFER, byte, glow::STREAM_DRAW);
            self.gl
                .draw_arrays(glow::TRIANGLES, 0, (dati.len() / 8) as i32);
            self.gl.bind_vertex_array(None);
        }
    }
}

/// I sei vertici di un rettangolo texturizzato.
fn vertici_texturizzati(rect: Rect, uv: [f32; 4], rgba: [f32; 4]) -> Vec<f32> {
    let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
    let [u0, v0, u1, v1] = uv;
    let angoli = [
        (x, y, u0, v0),
        (x + w, y, u1, v0),
        (x + w, y + h, u1, v1),
        (x, y, u0, v0),
        (x + w, y + h, u1, v1),
        (x, y + h, u0, v1),
    ];
    let mut dati = Vec::with_capacity(48);
    for (px, py, u, v) in angoli {
        dati.push(px);
        dati.push(py);
        dati.push(u);
        dati.push(v);
        dati.extend_from_slice(&rgba);
    }
    dati
}

unsafe fn compila_programma(gl: &glow::Context) -> Result<glow::Program, RenderError> {
    let vertice = compila(gl, glow::VERTEX_SHADER, VERTICE)?;
    let frammento = compila(gl, glow::FRAGMENT_SHADER, FRAMMENTO)?;

    let program = gl
        .create_program()
        .map_err(|e| RenderError::Gl(e.to_string()))?;
    gl.attach_shader(program, vertice);
    gl.attach_shader(program, frammento);
    gl.link_program(program);
    if !gl.get_program_link_status(program) {
        return Err(RenderError::Gl(format!(
            "collegamento degli shader fallito: {}",
            gl.get_program_info_log(program)
        )));
    }

    gl.delete_shader(vertice);
    gl.delete_shader(frammento);
    Ok(program)
}

unsafe fn compila_programma_da(
    gl: &glow::Context,
    vertice_src: &str,
    frammento_src: &str,
) -> Result<glow::Program, RenderError> {
    let vertice = compila(gl, glow::VERTEX_SHADER, vertice_src)?;
    let frammento = compila(gl, glow::FRAGMENT_SHADER, frammento_src)?;
    let program = gl
        .create_program()
        .map_err(|e| RenderError::Gl(e.to_string()))?;
    gl.attach_shader(program, vertice);
    gl.attach_shader(program, frammento);
    gl.link_program(program);
    if !gl.get_program_link_status(program) {
        return Err(RenderError::Gl(format!(
            "collegamento degli shader fallito: {}",
            gl.get_program_info_log(program)
        )));
    }
    gl.delete_shader(vertice);
    gl.delete_shader(frammento);
    Ok(program)
}

unsafe fn compila(
    gl: &glow::Context,
    tipo: u32,
    sorgente: &str,
) -> Result<glow::Shader, RenderError> {
    let shader = gl
        .create_shader(tipo)
        .map_err(|e| RenderError::Gl(e.to_string()))?;

    gl.shader_source(shader, sorgente);
    gl.compile_shader(shader);
    if !gl.get_shader_compile_status(shader) {
        return Err(RenderError::Gl(format!(
            "compilazione dello shader fallita: {}",
            gl.get_shader_info_log(shader)
        )));
    }
    Ok(shader)
}
