//! Testo: rasterizzazione dei glifi in un atlante e disegno.
//!
//! Un font vettoriale non si disegna direttamente: si converte ogni carattere
//! in una piccola immagine e si impacchetta in una sola texture, l'**atlante**.
//! Disegnare una scritta diventa cosi' un rettangolo texturizzato per
//! carattere, cioe' un solo disegno per tutti i glifi.
//!
//! L'atlante cresce a richiesta: un carattere che non e' ancora stato usato
//! viene rasterizzato e aggiunto quando serve. Cosi' un font con migliaia di
//! glifi non costa niente finche' non si scrive qualcosa che li usa.

use std::collections::HashMap;
use std::path::Path;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use glow::HasContext;

/// Lato dell'atlante in pixel.
///
/// Milleventiquattro e' un compromesso: a sedici pixel di carattere ci stanno
/// qualche migliaio di glifi, e la texture resta piccola per qualunque GPU.
const LATO_ATLANTE: i32 = 1024;

/// Dove sta un glifo dentro l'atlante, e come si disegna.
#[derive(Debug, Clone, Copy)]
pub struct Glifo {
    /// Coordinate della texture: `[u0, v0, u1, v1]`.
    pub uv: [f32; 4],
    /// Spostamento dal punto di scrittura all'angolo in alto a sinistra.
    pub offset: [f32; 2],
    /// Dimensione in pixel.
    pub size: [f32; 2],
    /// Quanto avanza il punto di scrittura dopo questo carattere.
    pub advance: f32,
}

/// Un font a una certa dimensione, con il suo atlante.
pub struct Carattere {
    font: FontVec,
    scala: PxScale,
    pixel: f32,
    /// Distanza fra la cima e la linea di base, in pixel.
    ascender: f32,
    glifi: HashMap<char, Glifo>,
    texture: glow::Texture,
    dati: Vec<u8>,
    cursore_x: i32,
    cursore_y: i32,
    altezza_riga: i32,
    sporco: bool,
}

impl Carattere {
    /// Carica un font a una dimensione in pixel.
    pub fn new(
        gl: &glow::Context,
        percorso: &Path,
        dimensione_pixel: f32,
    ) -> Result<Self, String> {
        let dati = std::fs::read(percorso)
            .map_err(|e| format!("non riesco a leggere il font {}: {e}", percorso.display()))?;
        let font = FontVec::try_from_vec(dati)
            .map_err(|e| format!("{} non e' un font valido: {e}", percorso.display()))?;

        let pixel = dimensione_pixel.max(1.0);
        let scala = PxScale::from(pixel);
        let scalato = font.as_scaled(scala);
        let ascender = scalato.ascent();
        let altezza_riga = (scalato.height().ceil() as i32).max(1);

        let texture = unsafe {
            gl.create_texture()
                .map_err(|e| format!("non riesco a creare la texture del font: {e}"))?
        };

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        }

        let mut carattere = Self {
            font,
            scala,
            pixel,
            ascender,
            glifi: HashMap::new(),
            texture,
            dati: vec![0; (LATO_ATLANTE * LATO_ATLANTE * 4) as usize],
            cursore_x: 1,
            cursore_y: 1,
            altezza_riga,
            sporco: true,
        };
        carattere.carica(gl)?;
        Ok(carattere)
    }

    pub fn altezza(&self) -> f32 {
        self.pixel
    }

    pub fn ascender(&self) -> f32 {
        self.ascender
    }

    /// Larghezza in pixel di una scritta.
    pub fn misura(&mut self, gl: &glow::Context, testo: &str) -> Result<f32, String> {
        let mut larghezza = 0.0;
        for c in testo.chars() {
            if let Some(glifo) = self.glifo(gl, c)? {
                larghezza += glifo.advance;
            }
        }
        Ok(larghezza)
    }

    /// I glifi di una scritta, con la loro posizione orizzontale.
    pub fn impagina(
        &mut self,
        gl: &glow::Context,
        testo: &str,
    ) -> Result<Vec<(Glifo, f32)>, String> {
        let mut fuori = Vec::with_capacity(testo.len());
        let mut x = 0.0;
        for c in testo.chars() {
            if let Some(glifo) = self.glifo(gl, c)? {
                fuori.push((glifo, x));
                x += glifo.advance;
            }
        }
        Ok(fuori)
    }

    /// Aggiorna la texture se sono stati aggiunti glifi.
    pub fn aggiorna(&mut self, gl: &glow::Context) {
        if !self.sporco {
            return;
        }
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                LATO_ATLANTE,
                LATO_ATLANTE,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&self.dati[..]),
            );
        }
        self.sporco = false;
    }

    pub fn texture(&self) -> glow::Texture {
        self.texture
    }

    fn carica(&mut self, gl: &glow::Context) -> Result<(), String> {
        // L'atlante iniziale va caricato comunque, altrimenti la prima
        // campionatura legge una texture non definita.
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                LATO_ATLANTE,
                LATO_ATLANTE,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&self.dati[..]),
            );
        }
        self.sporco = false;
        Ok(())
    }

    /// Il glifo di un carattere, rasterizzandolo se non c'e' ancora.
    fn glifo(&mut self, gl: &glow::Context, c: char) -> Result<Option<Glifo>, String> {
        if let Some(glifo) = self.glifi.get(&c) {
            return Ok(Some(*glifo));
        }

        // Lo spazio non ha bisogno di un pixel disegnato, ma di un avanzamento.
        if c == ' ' {
            let scalato = self.font.as_scaled(self.scala);
            let glifo = Glifo {
                uv: [0.0; 4],
                offset: [0.0, 0.0],
                size: [0.0, 0.0],
                advance: scalato.h_advance(self.font.glyph_id(' ')),
            };
            self.glifi.insert(c, glifo);
            return Ok(Some(glifo));
        }

        let id = self.font.glyph_id(c);
        let scalato = self.font.as_scaled(self.scala);
        let advance = scalato.h_advance(id);

        let glifo = id.with_scale(self.scala);
        let Some(contorno) = self.font.outline_glyph(glifo) else {
            // Carattere non presente nel font: si avanza senza disegnare.
            let vuoto = Glifo {
                uv: [0.0; 4],
                offset: [0.0, 0.0],
                size: [0.0, 0.0],
                advance,
            };
            self.glifi.insert(c, vuoto);
            return Ok(Some(vuoto));
        };

        let limiti = contorno.px_bounds();
        let larghezza = limiti.width().ceil() as i32;
        let altezza = limiti.height().ceil() as i32;

        if larghezza <= 0 || altezza <= 0 {
            let vuoto = Glifo {
                uv: [0.0; 4],
                offset: [0.0, 0.0],
                size: [0.0, 0.0],
                advance,
            };
            self.glifi.insert(c, vuoto);
            return Ok(Some(vuoto));
        }

        // Impacchettamento a ripiani: si riempie una riga, poi si scende.
        if self.cursore_x + larghezza + 1 > LATO_ATLANTE {
            self.cursore_x = 1;
            self.cursore_y += self.altezza_riga + 1;
        }
        if self.cursore_y + altezza + 1 > LATO_ATLANTE {
            return Err(
                "l'atlante dei caratteri e' pieno: servono piu' dimensioni o un atlante piu' grande"
                    .into(),
            );
        }

        let origine_x = self.cursore_x;
        let origine_y = self.cursore_y;

        let mut celle: Vec<(i32, i32, u8)> = Vec::new();
        contorno.draw(|x, y, copertura| {
            celle.push((x as i32, y as i32, (copertura * 255.0) as u8));
        });

        for (x, y, copertura) in celle {
            let px = origine_x + x;
            let py = origine_y + y;
            if px < 0 || py < 0 || px >= LATO_ATLANTE || py >= LATO_ATLANTE {
                continue;
            }
            let indice = ((py * LATO_ATLANTE + px) * 4) as usize;
            self.dati[indice] = 255;
            self.dati[indice + 1] = 255;
            self.dati[indice + 2] = 255;
            self.dati[indice + 3] = copertura;
        }

        self.cursore_x += larghezza + 1;
        self.sporco = true;

        let glifo = Glifo {
            uv: [
                origine_x as f32 / LATO_ATLANTE as f32,
                origine_y as f32 / LATO_ATLANTE as f32,
                (origine_x + larghezza) as f32 / LATO_ATLANTE as f32,
                (origine_y + altezza) as f32 / LATO_ATLANTE as f32,
            ],
            offset: [limiti.min.x, limiti.min.y],
            size: [larghezza as f32, altezza as f32],
            advance,
        };

        self.glifi.insert(c, glifo);
        self.aggiorna(gl);
        Ok(Some(glifo))
    }
}
