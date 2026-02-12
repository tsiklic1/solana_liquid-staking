# Solana Liquid Staking Token (LST)

⚠️ **UNAUDITED MVP** - This is an experimental implementation for educational purposes. Not production-ready. Do not use with real funds without a professional security audit.

## Overview

This program implements a minimal liquid staking token (LST) system on Solana, allowing users to stake SOL with a single validator while maintaining liquidity through a fungible token. Users deposit SOL and receive LST tokens representing proportional ownership of the staking pool. As staking rewards accrue to the delegated stake accounts, the exchange rate between LST and SOL increases, automatically distributing rewards to all token holders.

## Design

### Architecture Overview

The program manages **three types of stake accounts** to handle the asynchronous nature of Solana staking:

1. **Main Stake Account** (PDA: `b"stake_main"`): Primary staking pool holding the majority of delegated SOL. Always actively staking to the configured validator, continuously earning rewards.

2. **Reserve Stake Account** (PDA: `b"stake_reserve"`): Receiving incoming user deposits with the purpose of merging into main stake account.

3. **Split Stake Accounts** (PDA: `b"split_account" + user_pubkey + nonce`): Per-user withdrawal accounts. Created when a user initiates withdrawal.

### State Management

**Config PDA** (seed: `b"config"`): Immutable 160-byte struct stored as a program-owned account. Set once during initialization, never modified.

```rust
#[repr(C, packed)]
pub struct Config {
    pub admin: [u8; 32],                    // Initializer pubkey
    pub lst_mint: [u8; 32],                 // LST token mint
    pub stake_account_main: [u8; 32],       // Main stake account
    pub stake_account_reserve: [u8; 32],    // Reserve stake account
    pub validator_vote_pubkey: [u8; 32],    // Target validator
}
```

This avoids borsh deserialization overhead on every instruction invocation—critical for high-throughput staking operations.

### Exchange Rate Mechanism

**Dynamic proportional minting**: Deposit amounts are converted to LST using current pool state to preserve proportional ownership.

**Formula:**

```rust
lst_to_mint = if total_lst_supply == 0 || total_sol_in_pool == 0 {
    deposit_lamports  // Initial 1:1 rate
} else {
    (deposit_lamports * total_lst_supply) / total_sol_in_pool
}
```

**Invariant preservation**: For any deposit D at time T, the depositor receives LST such that:

```
(depositor_lst / total_lst) = (D / (total_sol + D))
```

This ensures:

- Initial depositors receive 1:1 exchange rate (cold start)
- Subsequent deposits cannot dilute existing holders
- Staking rewards accrue proportionally to all holders

**Withdrawal exchange rate:**

```rust
lst_to_burn = (lamports_to_split * total_lst_supply) / total_lamports_managed
```

Where `total_lamports_managed = main + reserve + new_split_account` lamports.

**Arithmetic safety**: All calculations use `u128` intermediate values with `.checked_mul()` and `.checked_div()` to prevent overflow.

## Key Assumptions

### Validator Assumptions

- **Single validator model**: Program delegates to one validator specified at initialization.
- **Validator liveness**: Assumes validator remains operational. No fallback mechanism, no validator health checks.
- **Single token**: Assumes a single liquid staking token for the whole contract.

### User Assumptions

- **Minimum deposits**: 1 SOL
- **Minimum withdrawals**: 1 SOL + stake account rent-exempt minimum (~0.00228288 SOL = ~2,282,880 lamports).
- **Nonce**: Users can have multiple withdrawals.

## How It Works

### Depositing SOL

1. User invokes `Deposit` instruction with desired lamport amount (≥1 SOL)
2. Program calculates LST to mint based on current exchange rate
3. User's SOL transferred to reserve stake account (native SOL transfer)
4. LST minted to user's associated token account (ATA)
5. User immediately receives tradeable LST representing pool ownership

**Note**: SOL sits in reserve as "unstaked" until crank operations executed.

### Receiving LST

LST tokens are standard SPL tokens with full DeFi composability:

- Tradeable on AMMs (e.g., Orca, Raydium, Jupiter)
- Transferable peer-to-peer via SPL Token program
- Usable as collateral in lending protocols
- Value appreciates automatically as staking rewards accrue to pool

### Withdrawing (with Cooldown)

**Initiation**: User invokes `CrankSplit` with desired lamport amount

1. Program calculates LST burn amount based on exchange rate
2. Split stake account created (PDA seeded with user pubkey + nonce)
3. Lamports split from main stake into split account via `split` CPI
4. Split account immediately deactivated
5. LST burned from user's ATA
6. Program validates split account is fully deactivated
7. All lamports withdrawn from split account to user's wallet
8. Split account closed

**Important**: User cannot access SOL until deactivation completes. LST is burned immediately upon split, so user loses liquidity during cooldown. This is an unavoidable constraint of Solana's staking design.

### Crank Operations (Permissionless)

**CrankInitializeReserve** (discriminator 1): Once reserve accumulates deposits, anyone can invoke to initialize and delegate reserve to validator. Incentivized by MEV—earlier delegation means earlier reward accrual for pool (and thus for LST holders).

**CrankMergeReserve** (discriminator 2): After reserve finishes warmup (~2-3 epochs after initialization), anyone can merge it into main account. Consolidates pool stake, simplifies accounting, and frees reserve for next deposit batch.

Both cranks are permissionless economic games. Users, bots, or altruistic parties execute when conditions are met.

## Instruction Reference

| Discriminator | Instruction            | Signer Required       | Description                                                                                                                       |
| ------------- | ---------------------- | --------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| 0             | Initialize             | Initializer, Mint     | Sets up pool: creates Config PDA, main/reserve stake accounts, LST mint. Delegates main to validator. Mints 1 LST to initializer. |
| 1             | CrankInitializeReserve | None (permissionless) | Initializes reserve stake account and delegates to validator. Callable once reserve has deposits.                                 |
| 2             | CrankMergeReserve      | None (permissionless) | Merges reserve into main stake account. Requires both accounts actively delegated to same validator.                              |
| 3             | Deposit                | Depositor             | Transfers SOL to reserve, mints LST to depositor's ATA based on exchange rate. Minimum 1 SOL.                                     |
| 4             | CrankSplit             | Withdrawer            | Splits lamports from main into per-user split PDA, deactivates split, burns LST. Minimum 1 SOL + rent.                            |
| 5             | Withdraw               | Withdrawer            | Withdraws lamports from deactivated split account to user's wallet. Requires cooldown complete.                                   |

## Limitations

- **Single validator only**: No validator diversification, no rebalancing, no performance-based rotation
- **No protocol fees**: All rewards accrue to LST holders. No revenue for protocol maintenance/development
- **Immutable post-deployment**: No parameter adjustment, no validator change, no emergency controls
- **No partial withdrawals**: Users must withdraw in discrete chunks (minimum 1 SOL + rent)
- **No MEV protection**: Exchange rates calculated on-chain. Susceptible to front-running in theory
- **No metrics/observability**: Would require off-chain indexing for analytics, APY tracking, etc.
- **Cooldown UX**: Users lose liquidity for 4-6 days during withdrawal. No instant unstaking option

## Program ID

**Hardcoded Program ID**: `22222222222222222222222222222222222222222222` (base58)

⚠️ **Test deployment only**. Do not use this program with real funds.

## Building and Testing

**Quick reference**:

```bash
# Build the SBF program (required before tests)
cargo build-sbf

# Run all tests
cargo test

# Run specific instruction test suite
cargo test --test deposit
cargo test --test withdraw
cargo test --test crank_split
```

Tests use **LiteSVM** for local Solana simulation. No devnet/testnet required for development.

## Implementation Notes

**Discriminator-based routing**: Entrypoint uses first byte of instruction data as discriminator (0-5 for six instructions).

**Stake program CPIs**: Raw instruction construction without wrapper crates. Discriminators encoded as `u32::to_le_bytes()`:

- Initialize: 0
- Delegate: 2
- Split: 3
- Withdraw: 4
- Deactivate: 5
- Merge: 7

**Error handling**: Custom error types in [src/errors.rs](src/errors.rs) with descriptive messages. All errors map to `ProgramError::Custom(code)`.
