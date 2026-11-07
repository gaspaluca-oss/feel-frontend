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
pub struct Graphics {
    window: Window,
    gl: glow::Context,
    context: PossiblyCurrentContext,
    surface: Surface<WindowSurface>,
    program: glow::Program,
    vertex_array: glow::VertexArray,
    vertices: glow::Buffer,
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

impl Session {
    /// Apre una finestra con un contesto OpenGL 3.3.
    pub fn open(titolo: &str) -> Result<Self, RenderError> {
        let event_loop = EventLoop::new().map_err(|e| RenderError::Window(e.to_string()))?;

        let builder = WindowBuilder::new()
            .with_title(titolo)
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

        let mut graphics = Graphics {
            window,
            gl,
            context,
            surface,
            program,
            vertex_array,
            vertices,
            screen: (dimensione.width as f32, dimensione.height as f32),
        };
        graphics.apply_viewport();

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
        self.apply_viewport();
    }

    fn apply_viewport(&mut self) {
        unsafe {
            self.gl
                .viewport(0, 0, self.screen.0 as i32, self.screen.1 as i32);
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
