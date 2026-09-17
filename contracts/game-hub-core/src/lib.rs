#![no_std]

//! # game-hub-core
//!
//! Lo que todo juego del studio necesita para hablar con el Game Hub, definido
//! una sola vez.
//!
//! Antes de este crate, `dice-duel`, `number-guess` y `twenty-one` declaraban
//! cada uno su propia copia del trait `GameHub`, textualmente identica. Y cada
//! uno numeraba sus errores por su cuenta, con el resultado de que
//! `GameAlreadyEnded` vale 5 en dos de ellos y 4 en el tercero, donde el 4 es
//! "faltan jugadores por jugar". Un cliente que traduzca codigos numericos a
//! mensajes muestra el texto equivocado segun contra que juego este hablando.
//!
//! ## Como se usa
//!
//! ```ignore
//! use game_hub_core::{GameHub, GameHubClient, CommonError};
//!
//! #[contracterror]
//! #[repr(u32)]
//! pub enum Error {
//!     // 1..=19 los define este crate. Los propios del juego arrancan en 20.
//!     AlreadyRolled = 20,
//!     BothPlayersNotRolled = 21,
//! }
//! ```

pub mod commit_reveal;
mod test;

use soroban_sdk::{contractclient, contracterror, Address, Env};

// ============================================================================
// La interfaz del Game Hub
// ============================================================================

/// La interfaz que expone el Game Hub y que cada juego invoca.
///
/// Se declara aca una sola vez. Si el hub agrega una operacion, se agrega en
/// este archivo y la ven los tres juegos, en vez de tener que acordarse de
/// tocar tres copias.
#[contractclient(name = "GameHubClient")]
pub trait GameHub {
    /// Abre una sesion y bloquea los puntos de ambos jugadores.
    fn start_game(
        env: Env,
        game_id: Address,
        session_id: u32,
        player1: Address,
        player2: Address,
        player1_points: i128,
        player2_points: i128,
    );

    /// Cierra la sesion declarando un ganador y libera los puntos.
    ///
    /// Ojo con el `bool`: obliga a que siempre haya un ganador. No existe hoy
    /// una liquidacion neutral, asi que un juego que necesite terminar sin
    /// ganador (por ejemplo, un commit-reveal donde ninguno revela) no tiene
    /// como expresarlo sin declarar a alguien arbitrariamente.
    fn end_game(env: Env, session_id: u32, player1_won: bool);
}

// ============================================================================
// Errores comunes
// ============================================================================

/// Los errores que todo juego de dos jugadores necesita, con numeros fijos.
///
/// **Los codigos 1 a 19 estan reservados para este enum.** Un juego que agregue
/// errores propios arranca en 20. Asi un cliente puede traducir los codigos
/// bajos sin preguntarse contra que contrato esta hablando.
///
/// Los numeros salen de lo que ya usaban `dice-duel` y `number-guess`, que
/// coincidian entre si, para que la migracion de esos dos no cambie ningun
/// codigo observable desde afuera.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CommonError {
    /// No hay ninguna sesion con ese `session_id`.
    GameNotFound = 1,
    /// Quien llama no es ninguno de los dos jugadores de la sesion.
    NotPlayer = 2,
    /// La sesion ya fue liquidada y no admite mas operaciones.
    GameAlreadyEnded = 3,
    /// Todavia falta que algun jugador haga su jugada.
    BothPlayersNotPlayed = 4,
    /// Un jugador intento jugar dos veces en la misma sesion.
    AlreadyPlayed = 5,
    /// Los dos lados de la partida son la misma direccion.
    SelfPlay = 6,
    /// La apuesta es cero o negativa.
    InvalidStake = 7,
    /// La operacion llego despues del plazo.
    DeadlineReached = 8,
    /// La operacion llego antes de que el plazo venciera.
    DeadlineNotReached = 9,
    /// Lo revelado no coincide con lo que se habia comprometido.
    HashMismatch = 10,
}

/// El primer codigo que puede usar un juego para sus errores propios.
///
/// Existe para que se pueda escribir `= GAME_ERROR_BASE + 0` en vez de un 20
/// suelto, y para que si algun dia el rango comun crece, quede un solo lugar
/// donde cambiarlo.
pub const GAME_ERROR_BASE: u32 = 20;
