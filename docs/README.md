# Stellar Game Studio Documentation

Welcome to the **Stellar Game Studio** technical documentation. This directory provides comprehensive guides on the Soroban smart contract architecture, multi-player game session lifecycles, and deployment procedures for web3 games built on Stellar.

---

## 📚 Documentation Index

1. **[Smart Contracts Architecture (`docs/contracts.md`)](./contracts.md)**
   - Overview of the Game Hub protocol (`game-hub-core` and `mock-game-hub`).
   - Reference contract implementations:
     - `number-guess` (Commit-reveal secret number matching).
     - `twenty-one` (Simplified on-chain Blackjack with deterministic cards).
     - `dice-duel` (Two-player turn-based dice rolling with Keccak256 entropy).
   - Storage semantics (Instance vs Temporary Storage with 30-day TTL extension).
   - Auth verification (`require_auth`, `require_auth_for_args`).

2. **[Game Lifecycle & Deployment Guide (`docs/game-lifecycle.md`)](./game-lifecycle.md)**
   - End-to-end lifecycle of a game match:
     - Initialization (`start_game`) and escrow/points lock.
     - State transitions and deterministic PRNG.
     - Game completion (`end_game`) and settlement.
   - Step-by-step Testnet deployment guide using `scripts/deploy.ts` and `scripts/setup.ts`.
   - Client binding generation with `scripts/bindings.ts`.
   - Standalone frontend integration via `@stellar/stellar-sdk` and `@creit-tech/stellar-wallets-kit`.

---

## 🏗️ Repository Architecture

```
Stellar-Game-Studio/
├── contracts/               # Rust Soroban smart contracts
│   ├── game-hub-core/       # Shared GameHub traits, events, and interface specs
│   ├── mock-game-hub/       # Local/testnet mock implementation of the GameHub
│   ├── number-guess/        # 2-player secret number guessing game
│   ├── twenty-one/          # 2-player card duel game
│   ├── dice-duel/           # 2-player dice wagering game
│   └── _template/           # Scaffolding template for new Soroban games
├── template_frontend/       # Standalone React+Vite game frontend template
├── sgs_frontend/            # Studio catalog & showcase UI
├── scripts/                 # Bun automation scripts (build, deploy, bindings, create)
├── docs/                    # Architectural & developer documentation
└── bindings/                # Generated TypeScript contract clients (git-ignored)
```

---

## 🚀 Getting Started

To install dependencies and build all smart contracts:

```bash
# 1. Install prerequisites (Rust >= 1.84, Bun >= 1.2, Stellar CLI >= 22)
rustup target add wasm32v1-none

# 2. Build contracts
bun run build

# 3. Run contract test suites
cargo test --workspace

# 4. Deploy and generate bindings for local dev
bun run setup
```
