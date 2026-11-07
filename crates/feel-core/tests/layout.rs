//! Il calcolo del fotogramma da mostrare.

use std::time::Duration;

use feel_core::{indice_frame, Frame};

fn frames(durate: &[u32]) -> Vec<Frame> {
    durate
        .iter()
        .enumerate()
        .map(|(indice, durata)| Frame {
            image: format!("main{}.png", indice + 1),
            duration_ms: *durata,
        })
        .collect()
}

fn ms(valore: u64) -> Duration {
    Duration::from_millis(valore)
}

#[test]
fn scorre_i_fotogrammi_nell_ordine() {
    let fotogrammi = frames(&[100, 100, 100]);

    assert_eq!(indice_frame(&fotogrammi, ms(0), None), 0);
    assert_eq!(indice_frame(&fotogrammi, ms(99), None), 0);
    assert_eq!(indice_frame(&fotogrammi, ms(100), None), 1);
    assert_eq!(indice_frame(&fotogrammi, ms(250), None), 2);
}

#[test]
fn ricomincia_dopo_l_ultimo() {
    let fotogrammi = frames(&[100, 100, 100]);
    // A 300 ms il ciclo e' finito: si torna al primo.
    assert_eq!(indice_frame(&fotogrammi, ms(300), None), 0);
    assert_eq!(indice_frame(&fotogrammi, ms(400), None), 1);
}

#[test]
fn durante_la_pausa_resta_l_ultimo() {
    let fotogrammi = frames(&[100, 100]);
    // Ciclo di 200 ms, poi 500 ms di pausa: fra 200 e 700 resta il secondo.
    assert_eq!(indice_frame(&fotogrammi, ms(200), Some(500)), 1);
    assert_eq!(indice_frame(&fotogrammi, ms(699), Some(500)), 1);
    // A 700 ms ricomincia.
    assert_eq!(indice_frame(&fotogrammi, ms(700), Some(500)), 0);
}

#[test]
fn durata_zero_e_immagine_ferma() {
    let fermo = frames(&[0]);
    assert_eq!(indice_frame(&fermo, ms(0), None), 0);
    assert_eq!(indice_frame(&fermo, ms(10_000), None), 0);
}

#[test]
fn una_lista_vuota_non_manda_in_panico() {
    assert_eq!(indice_frame(&[], ms(1234), None), 0);
}

#[test]
fn regge_un_fotogramma_lunghissimo() {
    // Un ciclo da un fotogramma di un'ora: nessun overflow, nessun salto.
    let lungo = frames(&[3_600_000]);
    assert_eq!(indice_frame(&lungo, ms(1_800_000), None), 0);
    assert_eq!(indice_frame(&lungo, ms(3_600_000), None), 0);
}
