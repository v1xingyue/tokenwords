# Predict Chat Program

This repository contains a lightweight Solana program sketch for a token-staked prediction chat room. Users join a room by staking tokens, publish time-bound predictions, and later settle them using an oracle price feed (e.g., Pyth). The current code focuses on account/state management so it can be integrated into a full dApp with off-chain chat and UI components.

## Architecture

- **Room state** tracks the oracle feed, staking mint, and vault PDA so multiple rooms (chat channels) can exist in the same program.
- **Prediction state** captures a user's stake, expected price, expiry slot, and settlement result.
- **Instructions**
  - `InitializeRoom` — seeds a room PDA and records oracle/staking configuration.
  - `StakeAndCommit` — records a user's prediction and stake commitment once their escrow PDA has been funded client-side.
  - `SettlePrediction` — reads an oracle account (first 8 bytes interpreted as little-endian price), checks expiry, and flags the prediction as won/lost.

## Program notes

- Token transfers/escrow are intentionally omitted in this MVP to keep the core prediction flow focused and testable; clients should handle vault funding before calling `StakeAndCommit`.
- Settlement currently treats prices greater than or equal to the user's target as a win. Extend this to support "above/below" semantics or spreads as needed.
- The oracle layout is simplified for local testing; integrate a full Pyth client in production to parse prices, confidence intervals, and status flags.

## Local development

```bash
cargo test -p predict-chat-program
```

The tests cover Borsh serialization for account structs and a minimal settlement flow that toggles the `won` flag based on oracle data.

## Building the `.so`

Use the helper script to build a shared object for deployment:

```bash
./scripts/build-so.sh
```

- If `cargo build-sbf` is available (from `solana-cli`/`solana-install`), the script will emit `target/sbf/release/predict_chat_program.so`.
- Otherwise it will attempt `cargo build --release --target bpfel-unknown-unknown`, which outputs `target/bpfel-unknown-unknown/release/predict_chat_program.so`.
- If neither toolchain is present, install Solana 1.18.x or add the BPF target as noted by the script.

## CI deployment to Solana testnet

The repository ships with `.github/workflows/deploy-testnet.yml` to build the BPF shared object and deploy it to testnet on pushes to `main` or via the **Run workflow** button.

### Configure the deploy account

1. Create or pick a keypair on the machine that will own and pay for the deployment. The CLI default lives at `~/.config/solana/id.json`. Keep this file private.
2. Base64-encode the JSON so GitHub Actions can write it back out during the workflow:

   ```bash
   base64 < ~/.config/solana/id.json | tr -d '\n'
   ```

3. In your repository settings, add the value above to a secret named `SOLANA_DEPLOY_KEYPAIR`.
4. (Optional) Add `SOLANA_RPC_URL` to point at a custom testnet RPC provider. If omitted, the workflow defaults to `https://api.testnet.solana.com`.
5. When dispatching the workflow manually you may supply a `program_id` input to upgrade an existing program; leaving it empty performs a fresh deploy and derives the program ID from the deploy keypair.

The workflow installs the Solana CLI, builds the program shared object using the helper script, and runs `solana program deploy` with the provided configuration.
