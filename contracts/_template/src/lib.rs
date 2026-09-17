#![no_std]
// El nombre del contrato es un marcador que `bun run create` reemplaza, asi que
// hasta entonces no cumple la convencion de mayusculas. Sin esto el workspace
// entero arrastra un warning permanente, y cualquier gate de `-D warnings`
// (como el que usa el CI de los juegos) falla por culpa de la plantilla.
#![allow(non_camel_case_types)]

//! # __GAME_NAME__
//!
//! Plantilla de juego de dos jugadores para Stellar Game Studio.
//!
//! Lo que ya viene resuelto acá, y no hace falta volver a escribir:
//!
//! - La interfaz con el Game Hub, que llega de `game_hub_core`.
//! - Los errores comunes, con codigos fijos. Los propios de tu juego arrancan
//!   en `GAME_ERROR_BASE`.
//! - Aleatoriedad que nadie puede anticipar, con compromiso y revelacion.
//!
//! Lo que tenes que escribir es la regla del juego: dados los dos secretos
//! revelados, quien gana. Eso vive en [`decidir_ganador`], y es lo unico que
//! deberias necesitar tocar para que esto sea otro juego.

use game_hub_core::{
    commit_reveal::{commitment, roll, seed, CommitReveal, Phase, DEFAULT_REVEAL_WINDOW},
    CommonError, GameHubClient,
};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol,
};

/// Identifica a este juego dentro de un compromiso, para que uno hecho para
/// otro juego no valide aca.
const JUEGO: Symbol = symbol_short!("__SLUG__");

// ============================================================================
// Errores
// ============================================================================

/// Los errores propios de este juego.
///
/// Arrancan en 20 porque los codigos 1 a 19 los usa `CommonError`, definido una
/// sola vez en `game-hub-core`. Asi un cliente puede traducir los codigos bajos
/// sin saber contra que juego esta hablando.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // El macro contracterror necesita un literal, no acepta GAME_ERROR_BASE
    // como discriminante. El 20 es su valor: si alguna vez cambia, hay que
    // cambiar estos numeros tambien.
    /// Ejemplo. Borralo y pone los tuyos.
    JugadaInvalida = 20,
}

// ============================================================================
// Estado
// ============================================================================

#[contracttype]
#[derive(Clone)]
pub enum Clave {
    Partida(u32),
    Hub,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Partida {
    pub cr: CommitReveal,
    pub puntos1: i128,
    pub puntos2: i128,
    pub terminada: bool,
}

#[contract]
pub struct __GAME_STRUCT__;

#[contractimpl]
impl __GAME_STRUCT__ {
    /// Guarda la direccion del Game Hub. Se llama una sola vez, al desplegar.
    pub fn __constructor(env: Env, game_hub: Address) {
        env.storage().instance().set(&Clave::Hub, &game_hub);
    }

    /// Fase 1: el primer jugador abre la partida y compromete su secreto.
    ///
    /// El compromiso es `hash(secreto)` atado a este contrato, a este juego, a
    /// esta sesion, a este jugador y a esta apuesta. Nadie puede calcular el
    /// resultado todavia, porque falta el secreto del otro.
    pub fn crear(
        env: Env,
        session_id: u32,
        player1: Address,
        player2: Address,
        puntos1: i128,
        puntos2: i128,
        compromiso1: BytesN<32>,
    ) -> Result<(), CommonError> {
        player1.require_auth();

        if player1 == player2 {
            return Err(CommonError::SelfPlay);
        }
        if puntos1 <= 0 || puntos2 <= 0 {
            return Err(CommonError::InvalidStake);
        }
        if env.storage().persistent().has(&Clave::Partida(session_id)) {
            return Err(CommonError::AlreadyPlayed);
        }

        let hub: Address = env.storage().instance().get(&Clave::Hub).unwrap();
        GameHubClient::new(&env, &hub).start_game(
            &env.current_contract_address(),
            &session_id,
            &player1,
            &player2,
            &puntos1,
            &puntos2,
        );

        env.storage().persistent().set(
            &Clave::Partida(session_id),
            &Partida {
                cr: CommitReveal::start(player1, player2, compromiso1),
                puntos1,
                puntos2,
                terminada: false,
            },
        );
        Ok(())
    }

    /// Fase 1b: el segundo jugador acepta y compromete el suyo. Arranca el plazo.
    pub fn aceptar(env: Env, session_id: u32, compromiso2: BytesN<32>) -> Result<(), CommonError> {
        let mut p: Partida = env
            .storage()
            .persistent()
            .get(&Clave::Partida(session_id))
            .ok_or(CommonError::GameNotFound)?;

        p.cr.player2.require_auth();

        if p.cr.phase != Phase::Committing {
            return Err(CommonError::AlreadyPlayed);
        }

        p.cr.accept(&env, compromiso2, DEFAULT_REVEAL_WINDOW);
        env.storage()
            .persistent()
            .set(&Clave::Partida(session_id), &p);
        Ok(())
    }

    /// Fase 2: revelar. El secreto tiene que coincidir con lo comprometido.
    pub fn revelar(
        env: Env,
        session_id: u32,
        jugador: Address,
        secreto: BytesN<32>,
    ) -> Result<(), CommonError> {
        jugador.require_auth();

        let mut p: Partida = env
            .storage()
            .persistent()
            .get(&Clave::Partida(session_id))
            .ok_or(CommonError::GameNotFound)?;

        if p.cr.phase != Phase::Revealing {
            return Err(CommonError::GameAlreadyEnded);
        }
        if p.cr.expired(&env) {
            return Err(CommonError::DeadlineReached);
        }

        let contrato = env.current_contract_address();
        let (esperado, apuesta, es_primero) = if jugador == p.cr.player1 {
            (p.cr.commitment1.clone(), p.puntos1, true)
        } else if jugador == p.cr.player2 {
            (
                p.cr.commitment2.clone().ok_or(CommonError::GameNotFound)?,
                p.puntos2,
                false,
            )
        } else {
            return Err(CommonError::NotPlayer);
        };

        let calculado = commitment(
            &env, &contrato, &JUEGO, session_id, &jugador, apuesta, &secreto,
        );
        if calculado != esperado {
            return Err(CommonError::HashMismatch);
        }

        if es_primero {
            if p.cr.revealed1.is_some() {
                return Err(CommonError::AlreadyPlayed);
            }
            p.cr.revealed1 = Some(secreto);
        } else {
            if p.cr.revealed2.is_some() {
                return Err(CommonError::AlreadyPlayed);
            }
            p.cr.revealed2 = Some(secreto);
        }

        env.storage()
            .persistent()
            .set(&Clave::Partida(session_id), &p);
        Ok(())
    }

    /// Fase 3: liquidar. Los dos revelaron, se decide y se cierra la sesion.
    pub fn liquidar(env: Env, session_id: u32) -> Result<Address, CommonError> {
        let mut p: Partida = env
            .storage()
            .persistent()
            .get(&Clave::Partida(session_id))
            .ok_or(CommonError::GameNotFound)?;

        if p.terminada {
            return Err(CommonError::GameAlreadyEnded);
        }
        if !p.cr.complete() {
            return Err(CommonError::BothPlayersNotPlayed);
        }

        let s1 = p.cr.revealed1.clone().unwrap();
        let s2 = p.cr.revealed2.clone().unwrap();
        let semilla = seed(&env, &s1, &s2);

        let gana_primero = Self::decidir_ganador(&env, &semilla);
        let ganador = if gana_primero {
            p.cr.player1.clone()
        } else {
            p.cr.player2.clone()
        };

        p.terminada = true;
        p.cr.phase = Phase::Settled;
        env.storage()
            .persistent()
            .set(&Clave::Partida(session_id), &p);

        let hub: Address = env.storage().instance().get(&Clave::Hub).unwrap();
        GameHubClient::new(&env, &hub).end_game(&session_id, &gana_primero);

        Ok(ganador)
    }

    /// Camino de incomparecencia: vencio el plazo y alguien no revelo.
    ///
    /// Si revelo uno solo, ese cobra: no revelar tiene que costar algo o
    /// abandonar seria gratis.
    ///
    /// Si no revelo ninguno, esto falla a proposito. No hay ganador, y
    /// declarar uno seria elegir el resultado, que es justo lo que el
    /// compromiso y la revelacion vienen a evitar. La devolucion de las
    /// apuestas necesita una operacion del Game Hub que hoy no existe: su
    /// `end_game` toma un `bool` y obliga a que alguien gane.
    pub fn cobrar_incomparecencia(env: Env, session_id: u32) -> Result<Address, CommonError> {
        let mut p: Partida = env
            .storage()
            .persistent()
            .get(&Clave::Partida(session_id))
            .ok_or(CommonError::GameNotFound)?;

        if p.terminada {
            return Err(CommonError::GameAlreadyEnded);
        }
        if !p.cr.expired(&env) {
            return Err(CommonError::DeadlineNotReached);
        }

        let ganador =
            p.cr.no_show_winner(&env)
                .ok_or(CommonError::BothPlayersNotPlayed)?;
        let gana_primero = ganador == p.cr.player1;

        p.terminada = true;
        p.cr.phase = Phase::Forfeited;
        env.storage()
            .persistent()
            .set(&Clave::Partida(session_id), &p);

        let hub: Address = env.storage().instance().get(&Clave::Hub).unwrap();
        GameHubClient::new(&env, &hub).end_game(&session_id, &gana_primero);

        Ok(ganador)
    }

    /// Lectura, para el frontend.
    pub fn partida(env: Env, session_id: u32) -> Result<Partida, CommonError> {
        env.storage()
            .persistent()
            .get(&Clave::Partida(session_id))
            .ok_or(CommonError::GameNotFound)
    }

    // ------------------------------------------------------------------
    // ACA VA TU JUEGO
    // ------------------------------------------------------------------

    /// La regla: dada la semilla, gana el primero?
    ///
    /// Todo lo de arriba es plomeria que no cambia entre juegos. Esto si.
    ///
    /// La semilla sale de los dos secretos combinados, asi que ninguno de los
    /// dos jugadores la controla ni la puede anticipar. Usa [`roll`] para sacar
    /// valores de ella: el indice te deja sacar varios sin que se repitan.
    ///
    /// El ejemplo tira un dado para cada uno y gana el mas alto, con el empate
    /// a favor del primero. Cambialo por lo que haga tu juego.
    fn decidir_ganador(env: &Env, semilla: &BytesN<32>) -> bool {
        let dado1 = roll(env, semilla, 0, 6) + 1;
        let dado2 = roll(env, semilla, 1, 6) + 1;
        dado1 >= dado2
    }
}

mod test;
