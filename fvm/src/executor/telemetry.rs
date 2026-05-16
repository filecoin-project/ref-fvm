// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::HashMap;

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

impl ReservationTelemetry {
    /// Record a successful reservation session begin with the per-sender totals.
    pub fn reservation_begin_succeeded(&mut self, reservations: &HashMap<ActorID, TokenAmount>) {
        self.reservations_open = self.reservations_open.saturating_add(1);
        self.reservation_total_per_sender = reservations.clone();
        self.reserved_remaining_per_sender = reservations.clone();
    }

    /// Record a failed reservation session begin.
    pub fn reservation_begin_failed(&mut self) {
        self.reservation_begin_failed = self.reservation_begin_failed.saturating_add(1);
    }

    /// Record a successful reservation session end and clear per-sender gauges.
    pub fn reservation_end_succeeded(&mut self) {
        self.reservations_open = self.reservations_open.saturating_sub(1);
        self.reservation_total_per_sender.clear();
        self.reserved_remaining_per_sender.clear();
    }

    /// Update the remaining reserved amount for a sender.
    pub fn reservation_remaining_update(&mut self, sender: ActorID, remaining: &TokenAmount) {
        if remaining.is_zero() {
            self.reserved_remaining_per_sender.remove(&sender);
        } else {
            self.reserved_remaining_per_sender
                .insert(sender, remaining.clone());
        }
    }

    /// Record settlement amounts for a single message.
    ///
    /// The `refund_virtual` argument should be `Some(refund)` in reservation mode,
    /// where refunds are realized via reservation release instead of a direct
    /// balance transfer.
    pub fn settlement_record(
        &mut self,
        base_fee_burn: &TokenAmount,
        miner_tip: &TokenAmount,
        over_estimation_burn: &TokenAmount,
        refund_virtual: Option<&TokenAmount>,
    ) {
        self.settle_basefee_burn += base_fee_burn.clone();
        self.settle_tip_credit += miner_tip.clone();
        self.settle_overburn += over_estimation_burn.clone();

        if let Some(refund) = refund_virtual {
            self.settle_refund_virtual += refund.clone();
        }
    }
}
