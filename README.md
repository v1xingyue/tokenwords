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

The repository ships with `.github/workflows/deploy-testnet.yml` to build the BPF shared object and deploy it to testnet on pushes to `main` or via the **Run workflow** button. Configure the following secrets in your GitHub repository before running it:

- `SOLANA_DEPLOY_KEYPAIR` — Base64-encoded contents of the JSON keypair that will pay fees and own/upgrade the program. You can create it with `base64 < ~/.config/solana/id.json | tr -d '\n'` and paste the result.
- `SOLANA_RPC_URL` (optional) — Custom RPC endpoint; defaults to `https://api.testnet.solana.com` when not set.

When dispatching the workflow manually you may optionally supply a `program_id` input to upgrade an existing deployment. Otherwise `solana program deploy` will generate or reuse the program ID based on the provided keypair.
