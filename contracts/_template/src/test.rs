#![cfg(test)]
#![allow(unused_imports)]

//! Los tests que toda partida de dos jugadores necesita.
//!
//! Se copian con la plantilla a proposito: son los caminos feos, que es
//! justamente lo que nadie escribe cuando arranca un juego nuevo.

use super::*;
use soroban_sdk::testutils::Address as _;

#[test]
fn un_jugador_no_puede_jugar_contra_si_mismo() {
    // El contrato tiene que rechazarlo: si no, alguien juega solo, gana solo,
    // y el hub le mueve puntos de un bolsillo al otro cobrando comision.
}

#[test]
fn el_compromiso_de_otra_sesion_no_valida() {
    // commitment() ata el secreto a la sesion. Un compromiso reusado de otra
    // partida tiene que fallar con HashMismatch.
}

#[test]
fn no_se_puede_revelar_dos_veces() {
    // Si se pudiera, el segundo intento pisaria al primero y un jugador
    // elegiria su secreto despues de ver el del otro.
}

#[test]
fn vencido_el_plazo_no_se_puede_revelar() {
    // El plazo existe para que el que va perdiendo no pueda estirar la partida
    // indefinidamente. Si se puede revelar tarde, no sirve de nada.
}

#[test]
fn el_que_revela_le_gana_al_que_no() {
    // No revelar tiene que costar. Si no, abandonar cuando ves que perdes es
    // gratis y nadie revela nunca.
}

#[test]
fn si_no_revela_ninguno_no_hay_ganador() {
    // cobrar_incomparecencia tiene que fallar. Declarar un ganador ahi seria
    // elegir el resultado.
}

#[test]
fn no_se_liquida_dos_veces() {
    // El segundo intento tiene que dar GameAlreadyEnded, o el hub paga dos
    // veces la misma partida.
}
