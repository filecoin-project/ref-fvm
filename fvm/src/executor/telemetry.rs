// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use fvm_shared::ActorID;
use fvm_shared::econ::TokenAmount;

/// Telemetry for reservation sessions and settlement.
///
/// This module keeps lightweight, process-local counters and gauges for
/// reservation lifecycle and settlement behavior. It is intentionally simple
/// and embedder-agnostic; embedders may choose to periodically snapshot these
/// values and export them to their metrics backends.
#[derive(Default, Clone)]
pub struct ReservationTelemetry {
    /// Gauge for the number of open reservation sessions.
    pub reservations_open: u64,

    /// Counter for failed reservation session begins.
    pub reservation_begin_failed: u64,

    /// Sum of base-fee burn amounts settled across messages.
    pub settle_basefee_burn: TokenAmount,

    /// Sum of miner tip credits settled across messages.
    pub settle_tip_credit: TokenAmount,

    /// Sum of over-estimation burn amounts settled across messages.
    pub settle_overburn: TokenAmount,

    /// Sum of virtual refunds realized via reservation release (reservation mode only).
    pub settle_refund_virtual: TokenAmount,

    /// Total reservation per sender at session begin, keyed by ActorID.
    pub reservation_total_per_sender: HashMap<ActorID, TokenAmount>,

    /// Remaining reserved amount per sender, keyed by ActorID.
    pub reserved_remaining_per_sender: HashMap<ActorID, TokenAmount>,
}

static TELEMETRY: OnceLock<Mutex<ReservationTelemetry>> = OnceLock::new();

fn metrics() -> &'static Mutex<ReservationTelemetry> {
    TELEMETRY.get_or_init(|| Mutex::new(ReservationTelemetry::default()))
}

/// Record a successful reservation session begin with the per-sender totals.
pub fn reservation_begin_succeeded(reservations: &HashMap<ActorID, TokenAmount>) {
    let mut m = metrics()
        .lock()
        .expect("reservation telemetry mutex poisoned");
    m.reservations_open = m.reservations_open.saturating_add(1);
    m.reservation_total_per_sender = reservations.clone();
    m.reserved_remaining_per_sender = reservations.clone();
}

/// Record a failed reservation session begin.
pub fn reservation_begin_failed() {
    let mut m = metrics()
        .lock()
        .expect("reservation telemetry mutex poisoned");
    m.reservation_begin_failed = m.reservation_begin_failed.saturating_add(1);
}

/// Record a successful reservation session end and clear per-sender gauges.
pub fn reservation_end_succeeded() {
    let mut m = metrics()
        .lock()
        .expect("reservation telemetry mutex poisoned");
    m.reservations_open = m.reservations_open.saturating_sub(1);
    m.reservation_total_per_sender.clear();
    m.reserved_remaining_per_sender.clear();
}

/// Update the remaining reserved amount for a sender.
pub fn reservation_remaining_update(sender: ActorID, remaining: &TokenAmount) {
    let mut m = metrics()
        .lock()
        .expect("reservation telemetry mutex poisoned");

    if remaining.is_zero() {
        m.reserved_remaining_per_sender.remove(&sender);
    } else {
        m.reserved_remaining_per_sender
            .insert(sender, remaining.clone());
    }
}

/// Record settlement amounts for a single message.
///
/// The `refund_virtual` argument should be `Some(refund)` in reservation mode,
/// where refunds are realized via reservation release instead of a direct
/// balance transfer.
pub fn settlement_record(
    base_fee_burn: &TokenAmount,
    miner_tip: &TokenAmount,
    over_estimation_burn: &TokenAmount,
    refund_virtual: Option<&TokenAmount>,
) {
    let mut m = metrics()
        .lock()
        .expect("reservation telemetry mutex poisoned");
    m.settle_basefee_burn += base_fee_burn.clone();
    m.settle_tip_credit += miner_tip.clone();
    m.settle_overburn += over_estimation_burn.clone();

    if let Some(refund) = refund_virtual {
        m.settle_refund_virtual += refund.clone();
    }
}

/// Snapshot the current reservation telemetry.
pub fn snapshot() -> ReservationTelemetry {
    metrics()
        .lock()
        .expect("reservation telemetry mutex poisoned")
        .clone()
}

#[cfg(test)]
pub fn reset() {
    *metrics()
        .lock()
        .expect("reservation telemetry mutex poisoned") = ReservationTelemetry::default();
}
