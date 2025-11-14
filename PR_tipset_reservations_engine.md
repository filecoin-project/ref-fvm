# feat: add engine‑managed tipset gas reservations to ref‑fvm

This PR implements engine‑managed tipset‑scope gas reservations inside ref‑fvm, as described in `AGENTS.md` (Option A). Lotus orchestrates Begin/End sessions around explicit messages; ref‑fvm remains network‑version agnostic and treats reservations as an internal, tipset‑local ledger.

## Summary

- Add a reservation session ledger on the default executor keyed by `ActorID`.
- Implement `begin_reservation_session` / `end_reservation_session` with affordability checks and invariants.
- Rewrite preflight to assert coverage without pre‑deducting gas funds in reservation mode.
- Enforce transfers against free balance `balance − reserved_remaining`.
- Rewrite settlement to net‑charge gas and realize refunds via reservation release.
- Add reservation telemetry and tests (unit + integration).

## Changes

### Executor: session lifecycle and preflight

- **`fvm/src/executor/mod.rs`**
  - Add:
    - `ReservationSession` struct (re‑exported from `default.rs`) and `ReservationError` enum:
      - `NotImplemented`
      - `InsufficientFundsAtBegin { sender }`
      - `SessionOpen`
      - `SessionClosed`
      - `NonZeroRemainder`
      - `PlanTooLarge`
      - `Overflow`
      - `ReservationInvariant(String)`

- **`fvm/src/executor/default.rs`**
  - Add `ReservationSession { reservations: HashMap<ActorID, TokenAmount>, open: bool }` and an `Arc<Mutex<_>>` on `DefaultExecutor`.
  - Implement:
    - `begin_reservation_session(&mut self, plan: &[(Address, TokenAmount)]) -> Result<(), ReservationError>`:
      - Empty plan = no‑op (do not enter reservation mode).
      - Enforce `MAX_SENDERS` (65,536) and track plan failures via telemetry.
      - Resolve senders via the state tree (robust or ID addresses → `ActorID`).
      - Aggregate Σ(plan) per actor and enforce `reserved_total <= balance` per sender.
      - Enforce single active session.
    - `end_reservation_session(&mut self) -> Result<(), ReservationError>`:
      - Require `open == true` and all reservation entries to be zero.
      - Clear the ledger and close the session; update telemetry.
  - Preflight:
    - Compute `gas_cost = gas_fee_cap * gas_limit` using big‑int; treat negative results as `ReservationError::Overflow`.
    - In reservation mode:
      - Assert coverage via `reservation_assert_coverage(sender, &gas_cost)`; do not pre‑deduct funds.
      - On prevalidation failures (invalid sender, bad nonce, inclusion gas > limit), call `reservation_prevalidation_decrement` so the ledger can end at zero.
    - Legacy mode:
      - Preserve existing behaviour (check balance ≥ gas_cost, pre‑deduct from sender).

### Transfer enforcement and settlement

- **`fvm/src/call_manager/default.rs` & `fvm/src/call_manager/mod.rs`**
  - Thread `Arc<Mutex<ReservationSession>>` into the default call manager.
  - In `transfer(from, to, value)`:
    - When the reservation session is open:
      - Compute `reserved_remaining = reservations.get(from).unwrap_or(0)`.
      - Enforce `value + reserved_remaining <= from.balance`; otherwise return `InsufficientFunds`.
    - When no session is open:
      - Preserve existing `value <= balance` semantics.

- **`fvm/src/executor/default.rs`**
  - Settlement (`finish_message`) in reservation mode:
    - Compute `GasOutputs` as today.
    - Define `consumption = base_fee_burn + over_estimation_burn + miner_tip`.
    - Deduct `consumption` from the sender’s actor balance.
    - Deposit burns and tip to the existing reward/burn actors.
    - Do not deposit `refund` to the sender; the “refund effect” is realized by releasing the reservation ledger.
    - Decrement `reservations[sender]` by `gas_cost` using `reservation_prevalidation_decrement`, update telemetry, and remove entries at zero.
  - Legacy mode settlement is unchanged.
  - Preserve the invariant:
    - `base_fee_burn + over_estimation_burn + refund + miner_tip == gas_cost`.

### Telemetry

- **`fvm/src/executor/telemetry.rs`**
  - Add `ReservationTelemetry` and helpers:
    - Track:
      - `reservations_open`
      - `reservation_begin_failed`
      - `settle_basefee_burn`
      - `settle_tip_credit`
      - `settle_overburn`
      - `settle_refund_virtual`
      - Per‑sender reservation totals and remaining amounts.
  - Expose `snapshot()` (for potential host export) and `reset()` under `#[cfg(test)]`.

### Kernel and test harness adjustments

- **`fvm/src/kernel/default.rs`**
  - Plumb the updated `CallManager` type where necessary so that all value‑moving operations (`SendOps`, SELFDESTRUCT, etc.) route through the reservation‑aware `transfer`.

- **`fvm/tests/dummy.rs`**
  - Update the dummy `CallManager` impl to accept the new `ReservationSession` argument in `CallManager::new`, keeping tests compiling against the updated trait.

### Tests

- Unit tests in `fvm/src/executor/default.rs`:
  - Session lifecycle: empty plan, begin twice, end with non‑zero remainder, plan too large, unknown actors.
  - Preflight behaviour under reservations:
    - Coverage assertion, no balance deduction.
    - Under‑reserved ledger → `ReservationError::Overflow`.
  - Transfer enforcement:
    - `transfer`, send to existing actors, SELFDESTRUCT, and implicit sends must respect free balance.
  - Settlement invariants:
    - Net sender delta equals `consumption`.
    - Reservation ledger clears so `end_reservation_session` succeeds.
  - Gas output property test (with `--features arb`) continues to assert:
    - All components non‑negative.
    - `base_fee_burn + over_estimation_burn + refund + miner_tip == gas_cost`.

- Integration tests:
  - `testing/integration/tests/reservation_transfer_enforcement.rs`:
    - Uses the integration tester to exercise reservation mode and confirm that:
      - Sends and actor creation fail when `value > free = balance − reserved_remaining`.
      - Failed transfers do not credit receivers.

## Activation and host behaviour

- ref‑fvm does not contain any network‑version logic for reservations.
- Hosts (e.g., Lotus) control:
  - When to call `begin_reservation_session` / `end_reservation_session`.
  - How to treat `ReservationError` variants (legacy fallback, tipset invalid, node error) based on network version and feature flags.

## Notes

- This PR is designed to preserve receipts (`ExitCode`, `GasUsed`, events) and `GasOutputs` relative to pre‑reservation behaviour, while removing miner exposure to intra‑tipset underfunded messages when hosts enable reservations.

