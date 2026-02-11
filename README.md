# Solana Liquid Staking Token (LST)

⚠️ **UNAUDITED MVP** - This is an experimental implementation for educational purposes. Not production-ready. Do not use with real funds without a professional security audit.

## Overview

This program implements a minimal liquid staking token (LST) system on Solana, allowing users to stake SOL with a single validator while maintaining liquidity through a fungible token. Users deposit SOL and receive LST tokens representing proportional ownership of the staking pool. As staking rewards accrue to the delegated stake accounts, the exchange rate between LST and SOL increases, automatically distributing rewards to all token holders.

The protocol prioritizes simplicity, correctness, and efficiency. It uses **pinocchio** (a lightweight alternative to Anchor) for minimal binary size and zero-copy state management for performance-critical operations. The design makes explicit trade-offs: single validator delegation, no fees, no governance, and immutable configuration post-deployment.

This is an MVP implementation satisfying the core requirements: proportional deposits, staking rewards accrual, exchange rate preservation, and enforced cooldown periods.

## Design

### Architecture Overview

The program manages **three types of stake accounts** to handle the asynchronous nature of Solana staking:

1. **Main Stake Account** (PDA: `b"stake_main"`): Primary staking pool holding the majority of delegated SOL. Always actively staking to the configured validator, continuously earning rewards.

2. **Reserve Stake Account** (PDA: `b"stake_reserve"`): Deposit buffer receiving incoming user deposits. Once filled, it's delegated to the validator and eventually merged into the main account. This solves the critical constraint that active Solana stake accounts cannot receive additional deposits.

3. **Split Stake Accounts** (PDA: `b"split_account" + user_pubkey + nonce`): Per-user withdrawal accounts. Created when a user initiates withdrawal, immediately deactivated to start the cooldown period, and available for final withdrawal after ~2-3 epochs.

**Design rationale**: Solana stake accounts cannot be deposited to while actively staking. The reserve acts as a hot buffer, the main account remains continuously delegated (maximizing rewards), and split accounts isolate withdrawal cooldowns per user without blocking pool operations.

### State Management

**Config PDA** (seed: `b"config"`): Immutable 160-byte struct stored as a program-owned account.

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

Set once during initialization, never modified. Uses `#[repr(C, packed)]` for deterministic memory layout and **zero-copy deserialization** via unsafe pointer transmutation ([src/state.rs:26-30](src/state.rs)):

```rust
Ok(unsafe { &*core::mem::transmute::<*const u8, *const Self>(bytes.as_ptr()) })
```

This avoids borsh deserialization overhead on every instruction invocation—critical for high-throughput staking operations.

### Exchange Rate Mechanism

**Dynamic proportional minting**: Deposit amounts are converted to LST using current pool state to preserve proportional ownership.

**Formula** ([src/instructions/deposit.rs:167-175](src/instructions/deposit.rs)):
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

**Withdrawal exchange rate** ([src/instructions/crank_split.rs:240-255](src/instructions/crank_split.rs)): Symmetric calculation ensuring users receive their proportional share:
```rust
lst_to_burn = (lamports_to_split * total_lst_supply) / total_lamports_managed
```

Where `total_lamports_managed = main + reserve + new_split_account` lamports.

**Arithmetic safety**: All calculations use `u128` intermediate values with `.checked_mul()` and `.checked_div()` to prevent overflow.

### Why Pinocchio Instead of Anchor

**Binary size**: Pinocchio produces significantly smaller binaries (~30-50% reduction). This reduces deployment costs and on-chain storage fees.

**Zero runtime overhead**: Anchor's discriminator checks, account validation macros, and constraint system add instruction overhead. Pinocchio forces explicit validation, giving full control over compute unit usage.

**Direct CPI control**: Staking program interactions require raw instruction building (discriminators as `u32::to_le_bytes()`). Pinocchio's low-level approach maps naturally to native program interfaces without abstractions.

**No IDL dependency**: Simplifies build pipeline. For an MVP, client SDKs can manually construct instructions without codegen.

**Trade-off**: Loss of developer ergonomics and compile-time safety guarantees. Acceptable for a small, security-critical program where manual review is mandatory anyway.

## Key Assumptions

### Validator Assumptions
- **Single validator model**: Program delegates to one validator specified at initialization. No validator rotation, no diversification.
- **Validator liveness**: Assumes validator remains operational. No fallback mechanism, no validator health checks.
- **Honest validator**: Assumes validator correctly processes stake delegations and does not engage in malicious behavior.
- **No slashing protection**: Solana currently doesn't implement slashing, but future protocol changes could introduce risks.

### User Assumptions
- **Minimum deposits**: 1 SOL (1,000,000,000 lamports) enforced to prevent dust spam attacks.
- **Minimum withdrawals**: 1 SOL + stake account rent-exempt minimum (~0.00228288 SOL = ~2,282,880 lamports) to ensure economic viability.
- **Cooldown awareness**: Users must wait approximately 2-3 epochs (4-6 days) after initiating withdrawal before funds are accessible. No partial withdrawals during cooldown.
- **Nonce management**: Users are responsible for tracking nonces for multiple concurrent withdrawals (client-side concern).

### Solana Staking Assumptions
- **Epoch boundaries**: Stake activation/deactivation occurs at epoch boundaries. Program does not optimize for intra-epoch timing.
- **Standard cooldown**: Warmup/cooldown periods follow standard Solana staking rules (~2-3 epochs).
- **Rent exemption**: All stake accounts maintain rent-exempt balance (200 bytes = ~0.00228288 SOL).
- **Merge semantics**: Stake program's `Merge` instruction requires both accounts delegated to the same validator with matching state (both active or both inactive).

### Technical Assumptions
- **No explicit reward tracking**: Exchange rate increases implicitly via stake account balance growth. No separate reward distribution mechanism.
- **No fee mechanism**: MVP does not extract protocol fees. All rewards accrue to LST holders. Protocol is economically unsustainable without future fee implementation.
- **Immutable config**: Config PDA is never modified post-initialization. No emergency pause, no validator change, no parameter adjustments (unless program is upgraded via deploy authority).
- **SPL Token standard**: LST uses standard SPL Token (not Token-2022). 9 decimals to match SOL precision (1 LST = 1,000,000,000 units).
- **No MEV protection**: Exchange rates calculated on-chain without oracle. Theoretically susceptible to sandwich attacks, though limited profitability due to staking cooldowns.

## Invariants

The program enforces these mathematical and safety guarantees:

### 1. Proportional Ownership Invariant
For all LST holders at any time T:
```
holder_sol_claim = (holder_lst_balance / total_lst_supply) * total_pool_sol
```

**Guaranteed by**:
- Exchange rate uses atomic snapshot of `total_lst_supply` and `total_pool_sol`
- No LST minting without corresponding SOL deposit
- No SOL withdrawal without corresponding LST burn
- u128 arithmetic prevents overflow/underflow/rounding errors

### 2. Exchange Rate Monotonicity
Exchange rate is non-decreasing over time (excluding withdrawals):
```
exchange_rate(T+1) >= exchange_rate(T)
where exchange_rate = total_pool_sol / total_lst_supply
```

**Guaranteed by**:
- Staking rewards increase `total_pool_sol` without increasing `total_lst_supply`
- Deposits mint proportionally (neutral to exchange rate)
- Withdrawals remove LST and SOL proportionally (neutral to exchange rate)

### 3. Pool Solvency Invariant
Total SOL under management always sufficient to redeem all LST at current exchange rate:
```
total_pool_lamports >= sum(all_holder_claims)
```

**Guaranteed by**:
- No SOL leaves pool except via withdrawal (which burns corresponding LST)
- No LST created except via deposit (which adds corresponding SOL)
- Split accounts counted in `total_pool_lamports` calculation until withdrawn

### 4. LST Supply Accounting
LST supply accurately represents pool ownership:
```
total_lst_supply = initial_supply + sum(deposits) - sum(withdrawals)
```

**Guaranteed by**:
- Mint authority exclusively held by Config PDA
- No external LST minting possible
- Burn enforced before SOL withdrawal in `CrankSplit` instruction

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
4. Split account immediately deactivated (starts cooldown timer)
5. LST burned from user's ATA

**Cooldown period**: Approximately 2-3 epochs (4-6 days) for stake deactivation to complete.

**Completion**: User invokes `Withdraw` after cooldown

1. Program validates split account is fully deactivated
2. All lamports withdrawn from split account to user's wallet
3. Split account closed

**Critical**: User cannot access SOL until deactivation completes. LST is burned immediately upon split, so user loses liquidity during cooldown. This is an unavoidable constraint of Solana's staking design.

### Crank Operations (Permissionless)

**CrankInitializeReserve** (discriminator 1): Once reserve accumulates deposits, anyone can invoke to initialize and delegate reserve to validator. Incentivized by MEV—earlier delegation means earlier reward accrual for pool (and thus for LST holders).

**CrankMergeReserve** (discriminator 2): After reserve finishes warmup (~2-3 epochs after initialization), anyone can merge it into main account. Consolidates pool stake, simplifies accounting, and frees reserve for next deposit batch.

Both cranks are permissionless economic games. Users, bots, or altruistic parties execute when conditions are met.

## Instruction Reference

| Discriminator | Instruction | Signer Required | Description |
|---------------|-------------|-----------------|-------------|
| 0 | Initialize | Initializer, Mint | Sets up pool: creates Config PDA, main/reserve stake accounts, LST mint. Delegates main to validator. Mints 1 LST to initializer. |
| 1 | CrankInitializeReserve | None (permissionless) | Initializes reserve stake account and delegates to validator. Callable once reserve has deposits. |
| 2 | CrankMergeReserve | None (permissionless) | Merges reserve into main stake account. Requires both accounts actively delegated to same validator. |
| 3 | Deposit | Depositor | Transfers SOL to reserve, mints LST to depositor's ATA based on exchange rate. Minimum 1 SOL. |
| 4 | CrankSplit | Withdrawer | Splits lamports from main into per-user split PDA, deactivates split, burns LST. Minimum 1 SOL + rent. |
| 5 | Withdraw | Withdrawer | Withdraws lamports from deactivated split account to user's wallet. Requires cooldown complete. |

## Security Considerations

### Centralization Risks
- **Single validator**: All SOL staked to one validator chosen by admin at initialization. No diversification, no decentralized validator selection.
- **Immutable configuration**: No governance mechanism to change validators, adjust parameters, or respond to changing conditions post-deployment.
- **Admin control**: Initializer sets validator permanently. Potential for collusion or validator capture.

### Validator Risk
- **Liveness dependency**: If validator stops performing duties, staking rewards cease. No automatic validator switching.
- **Future slashing**: Solana currently doesn't implement slashing, but protocol upgrades could introduce stake penalties. No slashing protection implemented.

### Smart Contract Risk
- **Unaudited code**: This program has not undergone professional security audit. May contain critical vulnerabilities.
- **Upgradeability**: If deploy authority retained, program can be upgraded maliciously. Immutable deployment recommended for production.
- **Pinocchio safety**: Manual account validation increases risk of validation bypass bugs compared to Anchor's macro-based constraints.

### Economic Risks
- **Exchange rate manipulation**: Theoretical front-running risk on large deposits/withdrawals. Limited profitability due to staking cooldowns making arbitrage difficult.
- **Cooldown griefing**: Users who split but never withdraw leave unclaimed split accounts. No cleanup mechanism. Could accumulate over time.
- **No fee sustainability**: Protocol extracts zero fees. Unsustainable for long-term operation without revenue model.

### Operational Risks
- **No emergency pause**: Cannot halt deposits/withdrawals in case of discovered vulnerability.
- **No admin functions**: Cannot adapt to changing validator performance, economic conditions, or protocol upgrades.
- **Crank dependency**: Reserve initialization and merge require external parties to invoke cranks. If no one executes, deposits sit idle (earning no rewards until cranked).

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

**Byte representation** ([src/lib.rs:20-23](src/lib.rs)):
```rust
pub const ID: Pubkey = [
    0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07, 0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb, 0xee,
    0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07, 0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc, 0x11, 0xf7,
];
```

⚠️ **Test deployment only**. Do not use this program with real funds.

## Building and Testing

For detailed build and test instructions, see [CLAUDE.md](CLAUDE.md).

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

## Future Enhancements (Out of MVP Scope)

- Multi-validator delegation with automatic rebalancing
- Protocol fee extraction (e.g., 5-10% of staking rewards)
- Governance token for validator selection and parameter adjustment
- Instant unstaking via liquidity pools (e.g., Orca CLMM integration)
- Automated crank execution via Clockwork/Jito
- MEV-resistant exchange rate calculation via oracles (Pyth, Switchboard)
- Emergency pause mechanism with timelock
- Integration with DeFi protocols (Solend, MarginFi, Kamino)
- Validator health monitoring and automatic rotation
- Partial withdrawal support
- Batch withdrawal optimization
- Comprehensive on-chain metrics and indexing

## Implementation Notes

**Discriminator-based routing** ([src/lib.rs:30-56](src/lib.rs)): Entrypoint uses first byte of instruction data as discriminator (0-5 for six instructions). Simple pattern matching without Anchor's automatic discriminator generation.

**Stake program CPIs**: Raw instruction construction without wrapper crates. Discriminators encoded as `u32::to_le_bytes()`:
- Initialize: 0
- Delegate: 2
- Split: 3
- Withdraw: 4
- Deactivate: 5
- Merge: 7

**Account validation**: Trait-based validators in [src/instructions/helpers.rs](src/instructions/helpers.rs) provide reusable checks for SPL Token, Token-2022, and stake program accounts.

**Error handling**: Custom error types in [src/errors.rs](src/errors.rs) with descriptive messages. All errors map to `ProgramError::Custom(code)`.
