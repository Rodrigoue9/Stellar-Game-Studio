#![cfg(test)]

//! Tests del modulo de compromiso y revelacion.
//!
//! Cada uno prueba una propiedad que, si se rompe, deja al juego manipulable.
//! No hay tests de "esto devuelve algo": todos fallan si la propiedad se pierde.

use super::commit_reveal::*;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env,
};

fn secreto(env: &Env, n: u8) -> BytesN<32> {
    BytesN::from_array(env, &[n; 32])
}

#[test]
fn el_compromiso_no_sirve_en_otra_sesion() {
    let env = Env::default();
    let contrato = Address::generate(&env);
    let jugador = Address::generate(&env);
    let juego = symbol_short!("dice");
    let s = secreto(&env, 1);

    let c = commitment(&env, &contrato, &juego, 7, &jugador, 100, &s);

    // el mismo secreto, la misma apuesta, el mismo jugador, otra sesion
    assert!(
        !verifies(&env, &c, &contrato, &juego, 8, &jugador, 100, &s),
        "un compromiso de la sesion 7 no puede validar en la 8"
    );
}

#[test]
fn el_compromiso_no_sirve_en_otro_juego() {
    let env = Env::default();
    let contrato = Address::generate(&env);
    let jugador = Address::generate(&env);
    let s = secreto(&env, 1);

    let c = commitment(
        &env,
        &contrato,
        &symbol_short!("dice"),
        7,
        &jugador,
        100,
        &s,
    );

    assert!(
        !verifies(
            &env,
            &c,
            &contrato,
            &symbol_short!("guess"),
            7,
            &jugador,
            100,
            &s
        ),
        "un compromiso de dice-duel no puede validar en number-guess"
    );
}

#[test]
fn el_compromiso_no_sirve_en_otro_contrato() {
    let env = Env::default();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let jugador = Address::generate(&env);
    let juego = symbol_short!("dice");
    let s = secreto(&env, 1);

    let c = commitment(&env, &a, &juego, 7, &jugador, 100, &s);

    assert!(
        !verifies(&env, &c, &b, &juego, 7, &jugador, 100, &s),
        "un compromiso no puede migrar a otra instancia del contrato"
    );
}

#[test]
fn el_compromiso_no_sirve_con_otra_apuesta() {
    let env = Env::default();
    let contrato = Address::generate(&env);
    let jugador = Address::generate(&env);
    let juego = symbol_short!("dice");
    let s = secreto(&env, 1);

    let c = commitment(&env, &contrato, &juego, 7, &jugador, 100, &s);

    assert!(
        !verifies(&env, &c, &contrato, &juego, 7, &jugador, 999, &s),
        "cambiar la apuesta tiene que invalidar el compromiso"
    );
}

#[test]
fn el_compromiso_valido_valida() {
    let env = Env::default();
    let contrato = Address::generate(&env);
    let jugador = Address::generate(&env);
    let juego = symbol_short!("dice");
    let s = secreto(&env, 1);

    let c = commitment(&env, &contrato, &juego, 7, &jugador, 100, &s);

    assert!(
        verifies(&env, &c, &contrato, &juego, 7, &jugador, 100, &s),
        "el caso correcto tiene que pasar, o los otros cuatro no prueban nada"
    );
}

#[test]
fn la_semilla_no_depende_del_orden_de_revelacion() {
    let env = Env::default();
    let a = secreto(&env, 1);
    let b = secreto(&env, 2);

    assert_eq!(
        seed(&env, &a, &b),
        seed(&env, &b, &a),
        "si el orden importara, el segundo en revelar elegiria cuando hacerlo"
    );
}

#[test]
fn secretos_distintos_dan_semillas_distintas() {
    let env = Env::default();
    let a = secreto(&env, 1);
    let b = secreto(&env, 2);
    let c = secreto(&env, 3);

    assert_ne!(seed(&env, &a, &b), seed(&env, &a, &c));
}

#[test]
fn la_tirada_queda_en_rango_y_es_determinista() {
    let env = Env::default();
    let s = seed(&env, &secreto(&env, 1), &secreto(&env, 2));

    for i in 0..20u32 {
        let v = roll(&env, &s, i, 6);
        assert!(v < 6, "un dado de seis caras no puede sacar {}", v);
        assert_eq!(
            v,
            roll(&env, &s, i, 6),
            "la misma semilla y el mismo indice dan lo mismo"
        );
    }
}

#[test]
fn indices_distintos_no_dan_siempre_lo_mismo() {
    let env = Env::default();
    let s = seed(&env, &secreto(&env, 7), &secreto(&env, 9));

    let valores: [u32; 10] = core::array::from_fn(|i| roll(&env, &s, i as u32, 1000));
    let primero = valores[0];
    assert!(
        valores.iter().any(|v| *v != primero),
        "diez tiradas de la misma semilla no pueden ser todas iguales"
    );
}

#[test]
fn ninguno_revela_no_declara_ganador() {
    let env = Env::default();
    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    let mut cr = CommitReveal::start(p1, p2, secreto(&env, 1));
    cr.accept(&env, secreto(&env, 2), 60);

    // vence el plazo sin que nadie revele
    env.ledger().set_timestamp(env.ledger().timestamp() + 61);

    assert!(cr.expired(&env));
    assert!(
        cr.no_show_winner(&env).is_none(),
        "si no revela ninguno no hay ganador: declarar uno seria elegir el resultado"
    );
}

#[test]
fn el_que_revela_le_gana_al_que_no() {
    let env = Env::default();
    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    let mut cr = CommitReveal::start(p1.clone(), p2, secreto(&env, 1));
    cr.accept(&env, secreto(&env, 2), 60);
    cr.revealed1 = Some(secreto(&env, 1));

    env.ledger().set_timestamp(env.ledger().timestamp() + 61);

    assert_eq!(
        cr.no_show_winner(&env),
        Some(p1),
        "el que cumplio cobra, o no revelar seria gratis"
    );
}

#[test]
fn antes_del_plazo_no_hay_incomparecencia() {
    let env = Env::default();
    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    let mut cr = CommitReveal::start(p1, p2, secreto(&env, 1));
    cr.accept(&env, secreto(&env, 2), 600);
    cr.revealed1 = Some(secreto(&env, 1));

    assert!(!cr.expired(&env));
    assert!(
        cr.no_show_winner(&env).is_none(),
        "no se puede cobrar por incomparecencia antes de que venza el plazo"
    );
}
