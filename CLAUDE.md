# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build the SBF program (required before running tests)
cargo build-sbf

# Run all tests
cargo test

# Run a single test file
cargo test --test initialize
cargo test --test deposit
cargo test --test withdraw
cargo test --test crank_split
cargo test --test crank_initialize_reserve
cargo test --test crank_merge_reserve
```

Tests require the compiled `.so` file at `target/deploy/solana_liquid_staking.so` (produced by `cargo build-sbf`). Tests use LiteSVM for local Solana simulation.

## Architecture

This is a Solana liquid staking program built with **pinocchio** (a lightweight alternative to Anchor/solana-program). It manages native SOL staking and issues LST (Liquid Staking Token) in return.

### Program Flow

1. **Initialize** (discriminator 0): Sets up the pool - creates a Config PDA, main stake account, reserve stake account, LST mint, and delegates to a validator. Mints 1 LST to the initializer.
2. **CrankInitializeReserve** (discriminator 1): Initializes and delegates the reserve stake account to the validator (run after deposits fill the reserve).
3. **CrankMergeReserve** (discriminator 2): Merges the reserve stake account into the main stake account.
4. **Deposit** (discriminator 3): User transfers SOL to the reserve stake account and receives LST minted 1:1 with lamports.
5. **CrankSplit** (discriminator 4): Splits lamports from main stake into a per-user PDA (`b"split_account" + withdrawer_pubkey`), deactivates it, and burns LST based on the exchange rate.
6. **Withdraw** (discriminator 5): Withdraws SOL from a deactivated split stake account to the user's wallet.

### Key Design Patterns

- **No Anchor**: Uses pinocchio directly. The entrypoint dispatches via `instruction_data[0]` as a discriminator byte.
- **Instruction pattern**: Each instruction has an `Accounts` struct (with `TryFrom<&[AccountInfo]>` for validation), an optional `Data` struct, and a wrapper struct with a `process()` method.
- **Account validation**: `helpers.rs` defines trait-based account checkers (`AccountCheck`, `MintInit`, `AccountInit`, etc.) for SPL Token, Token-2022, and stake program accounts.
- **State**: Single `Config` struct (160 bytes, `#[repr(C, packed)]`) stored in a PDA seeded with `b"config"`. Uses raw pointer transmutation for zero-copy load/store.
- **PDA seeds**: `b"config"` for the config account, `b"stake_main"` for main stake, `b"stake_reserve"` for reserve stake, `b"split_account" + user_pubkey` for withdrawal accounts.
- **Stake operations**: Raw CPI calls to the native Stake program (not using any wrapper crate). Instruction discriminators are encoded as `u32` little-endian bytes.

### Program ID

The program ID corresponds to the base58 address `22222222222222222222222222222222222222222222` and is hardcoded as a byte array in `src/lib.rs`.

### MVP Specification

Mini Liquid Staking Token (LST) — MVP Specification

Overview
Build a Solana program that allows users to deposit SOL and receive a liquid staking token (LST).
The LST represents a proportional claim on pooled SOL that is staked to a single validator. As staking rewards accrue, the value of the LST increases.
This is an MVP implementation. No additional features beyond what is specified here should be included.

Core Rules
Deposits
Users deposit SOL into the staking pool.
The program mints LST to the user.
Rules:
The initial exchange rate is 1 LST = 1 SOL

Subsequent deposits must preserve proportional ownership

Deposits must not dilute existing LST holders

Staking
All deposited SOL must be staked using the Solana stake program.
Staking rewards accrue to the pool as a whole.
Rules:
Rewards increase the value of LST

Rewards cannot be claimed directly by users

Exchange Rate
The exchange rate defines how much SOL one LST is worth.
Rules:
The exchange rate must be derived from on-chain state

The exchange rate increases only due to staking rewards

The exchange rate applies equally to all LST holders

Redemption
Users redeem by burning LST.
Users receive SOL based on the current exchange rate.
Rules:
Redemption must respect staking cooldowns

Users must not receive more SOL than their proportional share

Cooldown
Staked SOL cannot be withdrawn immediately.
Rules:
Stake deactivation rules must be enforced

Redemptions cannot bypass lockups

Invariants
The following must always hold:
Total SOL owed to LST holders is less than or equal to total SOL managed

LST supply accurately represents ownership of the pool

Gains and losses are shared proportionally across all LST holders

Deliverables
Program source code

Tests covering deposits and redemptions

A short README explaining the design and assumptions
