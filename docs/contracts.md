# Soroban Smart Contracts Architecture

This document describes the smart contract architecture of **Stellar Game Studio**, including the standard Game Hub integration trait, storage patterns, security best practices, and reference game implementations.

---

## 1. Game Hub Standard Protocol

All games in the Stellar Game Studio interact with a centralized **Game Hub** contract that standardizes player matchmaking, scoring, and leaderboard event tracking across the Stellar ecosystem.

### `GameHub` Trait Interface

Defined in `contracts/game-hub-core`:

```rust
use soroban_sdk::{contractclient, Address, Env};

#[contractclient(name = "GameHubClient")]
pub trait GameHub {
    /// Registers the start of a two-player game session.
    /// Locks or stakes player points and emits canonical start events.
    fn start_game(
        env: Env,
        game_id: Address,
        session_id: u32,
        player1: Address,
        player2: Address,
        player1_points: i128,
        player2_points: i128,
    );

    /// Reports the completion of a game session.
    /// Distributes rewards/points and records the winner.
    fn end_game(
        env: Env,
        session_id: u32,
        player1_won: bool
    );
}
```

### Canonical Rules for Contract Authors
1. **Exactly Two Players**: Every match operates with two distinct player addresses (`player1 != player2`).
2. **GameHub Notification Order**:
   - `game_hub.start_game(...)` must be called *before* storing initial game state.
   - `game_hub.end_game(...)` must be called *before* writing final winner state and clearing temporary session data.
3. **No Duplicate Events**: Individual game contracts should not emit redundant start/end events; the Game Hub acts as the single source of truth.

---

## 2. Storage Model & State Lifetime

Soroban provides three storage tiers: **Persistent**, **Instance**, and **Temporary**. Stellar Game Studio contracts follow a strict storage tiering policy:

### Instance Storage
Used exclusively for immutable or contract-wide configuration:
- `Admin`: The administrator/deployer address.
- `GameHubAddress`: The registered Game Hub contract address.

```rust
#[contracttype]
pub enum DataKey {
    Admin,
    GameHub,
    Game(u32), // Maps session_id to GameState
}
```

### Temporary Storage (Active Sessions)
Active game sessions are ephemeral. All game states (`GameState`) are saved in **Temporary Storage** with an explicit 30-day Time-To-Live (TTL):

```rust
// TTL constants (in ledgers, ~5 seconds per ledger)
const DAY_IN_LEDGERS: u32 = 17_280;
const SESSION_TTL_LEDGERS: u32 = 30 * DAY_IN_LEDGERS;

// Writing game session with TTL extension
env.storage().temporary().set(&DataKey::Game(session_id), &game_state);
env.storage().temporary().extend_ttl(
    &DataKey::Game(session_id),
    SESSION_TTL_LEDGERS,
    SESSION_TTL_LEDGERS,
);
```

**Why Temporary Storage?**
- Significantly lowers state storage fees on the Stellar ledger.
- Orphaned or abandoned games are automatically reclaimed by the network after 30 days without manual cleanup transactions.

---

## 3. Reference Contracts

### 3.1 `contracts/number-guess`
A turn-based secret guessing game:
- **Phase 1**: Player 1 submits a hash commitment `H = keccak256(secret_number, salt)`.
- **Phase 2**: Player 2 submits a guess within range `[1, 100]`.
- **Phase 3**: Player 1 reveals `secret_number` and `salt`. Contract verifies `keccak256(secret_number, salt) == H`.
- Winner is decided based on whether Player 2 guessed correctly.

### 3.2 `contracts/twenty-one`
A two-player card duel inspired by Blackjack:
- Players draw cards from a deterministic virtual deck shuffled using seed entropy.
- Actions: `hit` or `stand`.
- Closest score to 21 without busting wins. Ties split points or trigger a push according to rules.

### 3.3 `contracts/dice-duel`
Turn-based dice wagering:
- Players roll virtual dice with entropy sourced from `env.prng()` seeded with player addresses and session IDs.
- High roll takes the round; multi-round matches decide the final winner.

---

## 4. Deterministic Randomness & PRNG

To prevent front-running, simulation divergence, and validator discrepancies:
1. **Never use ledger timestamp or sequence numbers as direct entropy.** Ledger timestamp in Soroban is fixed during transaction simulation, which leads to predictable outcomes.
2. **Use `env.prng()` with Derived Seeds**:
   ```rust
   let mut seed_payload = Bytes::new(&env);
   seed_payload.append(&session_id.to_xdr(&env));
   seed_payload.append(&player1.to_xdr(&env));
   seed_payload.append(&player2.to_xdr(&env));
   
   let entropy = env.crypto().keccak256(&seed_payload);
   env.prng().seed(entropy.into());
   let random_value: u32 = env.prng().gen_range(1..=6);
   ```
3. For games with hidden information, always use **Commitment Schemes** (Commit-Reveal) rather than on-chain randomness.

---

## 5. Error Handling

All contracts define strongly typed error enums decorated with `#[contracterror]`:

```rust
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotAuthorized = 2,
    GameNotFound = 3,
    GameAlreadyStarted = 4,
    GameAlreadyFinished = 5,
    InvalidMove = 6,
    InvalidSecretReveal = 7,
}
```
Client frontends decode these errors directly via generated TypeScript bindings.
