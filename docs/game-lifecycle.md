# Game Lifecycle & Deployment Guide

This guide details the operational lifecycle of game sessions in **Stellar Game Studio**, covering state progression, authorization requirements, testnet deployment, and frontend wiring.

---

## 1. Game Session Lifecycle

Every game session progresses through a deterministic four-stage lifecycle:

```
[1. Match Setup] ──────> [2. In-Game Moves] ──────> [3. Evaluation] ──────> [4. Settlement]
  Player 1 & 2 Auth        Player Turn Actions        Score / Reveal           GameHub end_game
  GameHub start_game       Temporary Storage          Winner Determination    TTL Extension
```

### Phase 1: Match Setup & Registration
1. **Player Authentication**:
   Both players authorize their participation and points allocation:
   ```rust
   player1.require_auth_for_args((session_id, player1_points).into_val(&env));
   player2.require_auth_for_args((session_id, player2_points).into_val(&env));
   ```
2. **Game Hub Invocation**:
   The game contract calls the Game Hub to register session start:
   ```rust
   let game_hub = GameHubClient::new(&env, &game_hub_address);
   game_hub.start_game(
       &env.current_contract_address(),
       &session_id,
       &player1,
       &player2,
       &player1_points,
       &player2_points,
   );
   ```
3. **Session Initialization**:
   Initial `GameState` is recorded into Temporary Storage with a 30-day TTL.

### Phase 2: In-Game Moves & State Progression
- Each turn requires `player.require_auth()`.
- The contract validates move legality according to game rules.
- State is updated in Temporary Storage, resetting the 30-day TTL window:
  ```rust
  env.storage().temporary().set(&DataKey::Game(session_id), &updated_state);
  env.storage().temporary().extend_ttl(&DataKey::Game(session_id), SESSION_TTL, SESSION_TTL);
  ```

### Phase 3: Evaluation & Win Condition
- When terminal conditions are met (e.g., number guessed, card totals compared, dice rounds concluded):
  - Winner address is determined (`player1_won: bool`).
  - Terminal state is prepared.

### Phase 4: Settlement & Reporting
- The game contract reports the outcome to Game Hub:
  ```rust
  game_hub.end_game(&session_id, &player1_won);
  ```
- Game state marks `is_finished = true`.
- Temporary storage is retained for historical query queries until natural TTL expiration.

---

## 2. Testnet Deployment Guide

### Step 1: Preflight Verification
Confirm prerequisites are installed:
```bash
rustup target add wasm32v1-none
stellar --version
bun --version
```

### Step 2: Build WASM Binaries
Compile all contracts in the workspace:
```bash
bun run build
# Or build a specific contract:
bun run build number-guess
```
Compiled WASM files will be located in `target/wasm32v1-none/release/*.wasm`.

### Step 3: Deploy to Stellar Testnet
Run the automated deployment script:
```bash
bun run deploy
```
The script will:
1. Verify existing or generate testnet keypairs.
2. Fund accounts via Stellar Friendbot.
3. Install contract WASMs onto the testnet ledger.
4. Instantiate contracts with constructor arguments (`admin`, `game_hub_address`).
5. Save addresses to `deployment.json`.

### Step 4: Generate TypeScript Bindings
Generate strongly typed client libraries using `stellar contract bindings`:
```bash
bun run bindings
```
Bindings will be output to `bindings/<contract_name>/`.

### Step 5: Full Automated Setup
Run the end-to-end setup script to execute build, deploy, bindings, and `.env` generation in one command:
```bash
bun run setup
```

---

## 3. Frontend Integration

### 1. Scaffolding a New Game
Create a contract and standalone frontend pair:
```bash
bun run create my-duel
```
This generates:
- `contracts/my-duel/` (Rust contract template)
- `my-duel-frontend/` (React + Vite + Tailwind CSS frontend)

### 2. Wiring Bindings
Copy generated bindings into the frontend:
```bash
cp -r bindings/my-duel/* my-duel-frontend/src/bindings/
```

### 3. Local Frontend Development
Run the dev server with integrated testnet wallet switcher:
```bash
bun run dev:game my-duel
```

### 4. Production Export
Export a production-ready web container:
```bash
bun run publish my-duel --build
```
The compiled output is placed in `dist/my-duel-frontend/`.
