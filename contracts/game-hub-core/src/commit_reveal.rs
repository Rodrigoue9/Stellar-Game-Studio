//! # Compromiso y revelacion
//!
//! El patron que hace que el resultado de una partida no se pueda anticipar.
//!
//! El problema que resuelve es concreto y esta vivo en este repositorio: si la
//! semilla sale de datos que se conocen antes de apostar, quien arma la
//! transaccion puede calcular el resultado offline y presentar a la firma
//! solamente uno donde gana. El resultado no se sortea: se elige.
//!
//! La solucion separa el momento de apostar del momento en que la aleatoriedad
//! queda determinada:
//!
//! 1. Cada jugador publica `hash(secreto)` junto con su apuesta. Nadie puede
//!    calcular el resultado porque le falta el secreto del otro.
//! 2. Vencido el plazo, los dos revelan y el contrato comprueba cada hash.
//! 3. La semilla sale de los dos secretos combinados.
//! 4. Si uno no revela, el otro cobra por incomparecencia.
//!
//! ## Lo que el compromiso tiene que atar
//!
//! Un `hash(secreto)` a secas se puede reusar: el mismo compromiso vale en otra
//! partida, en otro juego, o en otra instancia del mismo contrato. Por eso
//! [`commitment`] mezcla el contrato, el juego, la sesion, el jugador y la
//! apuesta. Un compromiso viejo deja de servir en cualquier otro contexto.
//!
//! ## Sobre la sal
//!
//! No hay parametro de sal a proposito. Con un secreto uniformemente aleatorio
//! de 256 bits, una sal separada no agrega proteccion y si agrega tamano y
//! costo. Lo que hace falta es que el secreto sea realmente aleatorio, no que
//! haya dos campos.

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol};

/// Cuanto tiempo tiene un jugador para revelar, en segundos, si el juego no
/// elige otro. Cinco minutos alcanza para una firma humana y es corto para que
/// el que no revela no bloquee los fondos.
pub const DEFAULT_REVEAL_WINDOW: u64 = 300;

/// El estado de una partida con compromiso y revelacion.
///
/// Las transiciones son en un solo sentido y ninguna vuelve atras:
/// `Committing -> Revealing -> Settled`, o `-> Forfeited` si alguien no revela.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// Faltan compromisos. Todavia se puede apostar.
    Committing,
    /// Los dos comprometieron. Corre el plazo para revelar.
    Revealing,
    /// Los dos revelaron y la partida se liquido.
    Settled,
    /// Alguien no revelo a tiempo y se cobro por incomparecencia.
    Forfeited,
}

/// Los dos lados de una partida con compromiso y revelacion.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitReveal {
    pub player1: Address,
    pub player2: Address,
    pub commitment1: BytesN<32>,
    pub commitment2: Option<BytesN<32>>,
    pub revealed1: Option<BytesN<32>>,
    pub revealed2: Option<BytesN<32>>,
    /// Vence el plazo para revelar. Se fija cuando entra el segundo compromiso,
    /// no cuando entra el primero: si se fijara antes, el que compromete
    /// primero le come tiempo al otro.
    pub reveal_deadline: u64,
    pub phase: Phase,
}

/// Calcula el compromiso que un jugador tiene que publicar.
///
/// Ata el secreto a todo lo que hace unico a este turno de esta partida. Un
/// compromiso calculado para otra sesion, otro juego, otro contrato u otra
/// apuesta no valida aca.
///
/// ```ignore
/// let c = commitment(&env, &env.current_contract_address(),
///                    &symbol_short!("dice"), session_id, &player, stake, &secret);
/// ```
pub fn commitment(
    env: &Env,
    contract: &Address,
    game: &Symbol,
    session_id: u32,
    player: &Address,
    stake: i128,
    secret: &BytesN<32>,
) -> BytesN<32> {
    let mut buf = Bytes::new(env);
    buf.append(&contract.to_string().to_bytes());
    // Un Symbol corto (hasta 9 caracteres) vive dentro del propio Val, asi que
    // su payload lo identifica sin ambiguedad y se serializa como un u64.
    buf.append(&Bytes::from_array(
        env,
        &game.to_val().get_payload().to_be_bytes(),
    ));
    buf.append(&Bytes::from_array(env, &session_id.to_be_bytes()));
    buf.append(&player.to_string().to_bytes());
    buf.append(&Bytes::from_array(env, &stake.to_be_bytes()));
    buf.append(&Bytes::from_array(env, &secret.to_array()));
    env.crypto().sha256(&buf).into()
}

/// Comprueba que un secreto revelado corresponde al compromiso publicado.
///
/// Devuelve `false` si no coincide. El contrato que llama decide si eso es un
/// error o una incomparecencia; aca no se decide por el.
#[allow(clippy::too_many_arguments)]
pub fn verifies(
    env: &Env,
    expected: &BytesN<32>,
    contract: &Address,
    game: &Symbol,
    session_id: u32,
    player: &Address,
    stake: i128,
    secret: &BytesN<32>,
) -> bool {
    &commitment(env, contract, game, session_id, player, stake, secret) == expected
}

/// Deriva la semilla compartida de los dos secretos revelados.
///
/// El orden esta fijado por los propios secretos, no por quien revelo primero.
/// Si dependiera del orden de llegada, el segundo en revelar podria esperar a
/// ver el secreto del otro y elegir el momento para inclinar el resultado, que
/// es la misma falla que este patron viene a cerrar.
pub fn seed(env: &Env, secret1: &BytesN<32>, secret2: &BytesN<32>) -> BytesN<32> {
    let a = secret1.to_array();
    let b = secret2.to_array();
    let (primero, segundo) = if a <= b { (a, b) } else { (b, a) };

    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_array(env, &primero));
    buf.append(&Bytes::from_array(env, &segundo));
    env.crypto().sha256(&buf).into()
}

/// Un entero uniforme en `[0, n)` derivado de la semilla y un indice.
///
/// El indice permite sacar varios valores de la misma semilla sin repetirlos:
/// dos dados de la misma partida usan `0` y `1`.
///
/// El sesgo por modulo es despreciable para las `n` que usa un juego (un dado,
/// una carta, un mazo). Para una `n` cercana a `u32::MAX` haria falta rechazo,
/// y este helper no lo hace.
pub fn roll(env: &Env, seed: &BytesN<32>, index: u32, n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_array(env, &seed.to_array()));
    buf.append(&Bytes::from_array(env, &index.to_be_bytes()));
    let h = env.crypto().sha256(&buf).to_array();
    let v = u32::from_be_bytes([h[0], h[1], h[2], h[3]]);
    v % n
}

impl CommitReveal {
    /// Arranca una partida con el compromiso del primer jugador.
    pub fn start(player1: Address, player2: Address, commitment1: BytesN<32>) -> Self {
        Self {
            player1,
            player2,
            commitment1,
            commitment2: None,
            revealed1: None,
            revealed2: None,
            reveal_deadline: 0,
            phase: Phase::Committing,
        }
    }

    /// Suma el compromiso del segundo jugador y abre el plazo de revelacion.
    pub fn accept(&mut self, env: &Env, commitment2: BytesN<32>, window: u64) {
        self.commitment2 = Some(commitment2);
        self.reveal_deadline = env.ledger().timestamp() + window;
        self.phase = Phase::Revealing;
    }

    /// Vencio el plazo para revelar?
    pub fn expired(&self, env: &Env) -> bool {
        self.phase == Phase::Revealing && env.ledger().timestamp() > self.reveal_deadline
    }

    /// Revelaron los dos?
    pub fn complete(&self) -> bool {
        self.revealed1.is_some() && self.revealed2.is_some()
    }

    /// Quien cobra por incomparecencia, si vencio el plazo.
    ///
    /// Devuelve `None` si revelaron los dos, si no revelo ninguno, o si el
    /// plazo no vencio. El caso de "ninguno revela" no tiene ganador y el
    /// contrato que llama tiene que devolver las apuestas: declarar uno
    /// arbitrariamente seria elegir el resultado, que es justo lo que este
    /// patron evita.
    pub fn no_show_winner(&self, env: &Env) -> Option<Address> {
        if !self.expired(env) {
            return None;
        }
        match (self.revealed1.is_some(), self.revealed2.is_some()) {
            (true, false) => Some(self.player1.clone()),
            (false, true) => Some(self.player2.clone()),
            _ => None,
        }
    }
}
