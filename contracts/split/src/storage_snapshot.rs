#![cfg(test)]
#![allow(clippy::all)]

extern crate std;

use std::collections::HashSet;
use std::string::String as StdString;
use std::string::ToString;
use std::vec::Vec as StdVec;

use super::*;
use soroban_sdk::{xdr::ToXdr, Env};

fn hex_xdr(env: &Env, val: impl ToXdr) -> StdString {
    let bytes = val.to_xdr(env);
    bytes
        .iter()
        .map(|b| std::format!("{:02x}", b))
        .collect::<StdVec<_>>()
        .concat()
}

/// Checked-in baseline of expected storage keys.
///
/// Each entry is `(label, hex_xdr_of_key)`. This is the reviewable source of
/// truth for the storage-key compatibility check. If a key intentionally
/// changes, regenerate this baseline via the documented procedure in
/// `docs/ARCHITECTURE.md` ("Storage Key Snapshot") and commit the diff so the
/// change is visible in review.
const EXPECTED_KEYS: &[(&str, &str)] = &[
    // Instance-tier keys (contract-level singletons, return Symbol)
    ("admin_key", ""),
    ("admins_key", ""),
    ("paused_key", ""),
    ("paused_fns_key", ""),
    ("treasury_key", ""),
    ("usdc_token_key", ""),
    ("creation_fee_key", ""),
    ("platform_fee_bps_key", ""),
    ("platform_fee_waiver_list_key", ""),
    ("creator_fee_waiver_key", ""),
    ("counter_key", ""),
    ("global_payer_limit_key", ""),
    ("global_payer_window_key", ""),
    ("stream_contract_key", ""),
    ("creator_whitelist_key", ""),
    ("compliance_key", ""),
    ("rate_limit_key", ""),
    ("rate_window_key", ""),
    ("max_cancel_bps_key", ""),
    ("receipt_factory_key", ""),
    ("dashboard_contract_key", ""),
    ("nft_gate_key", ""),
    ("timelock_secs_key", ""),
    ("timelock_action_counter_key", ""),
    ("fee_tiers_key", ""),
    ("pending_admin_key", ""),
    ("governance_contract_key", ""),
    ("factories_key", ""),
    ("total_invoices_key", ""),
    ("total_volume_key", ""),
    ("total_released_key", ""),
    ("total_refunded_key", ""),
    ("treasury_group_counter_key", ""),
    ("circuit_breaker_key", ""),
    ("circuit_breaker_reason_key", ""),
    ("archive_after_ledgers_key", ""),
    ("platform_vol_thresh_key", ""),
    ("platform_vol_mile_key", ""),
    ("creator_vol_thresh_key", ""),
    ("kyc_contract_key", ""),
    ("upgrade_proposal_key", ""),
    ("initialised_key", ""),
    // Persistent-tier keys (per-entity)
    ("invoice_key", ""),
    ("invoice_ext_key", ""),
    ("invoice_ext2_key", ""),
    ("invoice_compact_key", ""),
    ("invoice_hot_key", ""),
    ("audit_log_key", ""),
    ("archive_marker_key", ""),
    ("created_ledger_key", ""),
    ("subscription_params_key", ""),
    ("confidential_count_key", ""),
    ("ext_vote_key", ""),
    ("group_key", ""),
    ("invoice_group_key", ""),
    ("invoice_treasury_key", ""),
    ("group_treasury_key", ""),
    ("delegate_key", ""),
    ("payment_window_key", ""),
    ("cert_key", ""),
    ("timelock_action_key", ""),
    ("refunded_key", ""),
    ("pause_exempt_key", ""),
    ("global_vel_key", ""),
    ("rep_key", ""),
    ("credit_key", ""),
    ("referral_count_key", ""),
    ("recipient_invoice_ids_key", ""),
    ("delegate_pay_key", ""),
    ("rate_usage_key", ""),
    ("invoice_count_key", ""),
    ("cancel_count_key", ""),
    ("creator_stats_count_key", ""),
    ("creator_stats_volume_key", ""),
    ("creator_stats_released_key", ""),
    ("creator_stats_refunded_key", ""),
    ("creator_stats_payers_key", ""),
    ("creator_stats_avg_funding_key", ""),
    ("creator_vol_mile_key", ""),
];

/// Build the current set of storage keys as `(label, hex_xdr)` pairs.
fn current_keys(env: &Env) -> StdVec<(&'static str, StdString)> {
    // Deterministic address (all-zero G... strkey) — do not change.
    let a = Address::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );

    let mut keys: StdVec<(&'static str, StdString)> = StdVec::new();

    // Instance-tier keys (contract-level singletons, return Symbol)
    keys.push(("admin_key", hex_xdr(env, admin_key())));
    keys.push(("admins_key", hex_xdr(env, admins_key())));
    keys.push(("paused_key", hex_xdr(env, paused_key())));
    keys.push(("paused_fns_key", hex_xdr(env, paused_fns_key())));
    keys.push(("treasury_key", hex_xdr(env, treasury_key())));
    keys.push(("usdc_token_key", hex_xdr(env, usdc_token_key())));
    keys.push(("creation_fee_key", hex_xdr(env, creation_fee_key())));
    keys.push((
        "platform_fee_bps_key",
        hex_xdr(env, platform_fee_bps_key()),
    ));
    keys.push((
        "platform_fee_waiver_list_key",
        hex_xdr(env, platform_fee_waiver_list_key()),
    ));
    keys.push((
        "creator_fee_waiver_key",
        hex_xdr(env, creator_fee_waiver_key()),
    ));
    keys.push(("counter_key", hex_xdr(env, counter_key())));
    keys.push((
        "global_payer_limit_key",
        hex_xdr(env, global_payer_limit_key()),
    ));
    keys.push((
        "global_payer_window_key",
        hex_xdr(env, global_payer_window_key()),
    ));
    keys.push(("stream_contract_key", hex_xdr(env, stream_contract_key())));
    keys.push((
        "creator_whitelist_key",
        hex_xdr(env, creator_whitelist_key()),
    ));
    keys.push(("compliance_key", hex_xdr(env, compliance_key())));
    keys.push(("rate_limit_key", hex_xdr(env, rate_limit_key())));
    keys.push(("rate_window_key", hex_xdr(env, rate_window_key())));
    keys.push(("max_cancel_bps_key", hex_xdr(env, max_cancel_bps_key())));
    keys.push(("receipt_factory_key", hex_xdr(env, receipt_factory_key())));
    keys.push((
        "dashboard_contract_key",
        hex_xdr(env, dashboard_contract_key()),
    ));
    keys.push(("nft_gate_key", hex_xdr(env, nft_gate_key())));
    keys.push(("timelock_secs_key", hex_xdr(env, timelock_secs_key())));
    keys.push((
        "timelock_action_counter_key",
        hex_xdr(env, timelock_action_counter_key()),
    ));
    keys.push(("fee_tiers_key", hex_xdr(env, fee_tiers_key())));
    keys.push(("pending_admin_key", hex_xdr(env, pending_admin_key())));
    keys.push((
        "governance_contract_key",
        hex_xdr(env, governance_contract_key()),
    ));
    keys.push(("factories_key", hex_xdr(env, factories_key())));
    keys.push(("total_invoices_key", hex_xdr(env, total_invoices_key())));
    keys.push(("total_volume_key", hex_xdr(env, total_volume_key())));
    keys.push(("total_released_key", hex_xdr(env, total_released_key())));
    keys.push(("total_refunded_key", hex_xdr(env, total_refunded_key())));
    keys.push((
        "treasury_group_counter_key",
        hex_xdr(env, treasury_group_counter_key()),
    ));
    keys.push(("circuit_breaker_key", hex_xdr(env, circuit_breaker_key())));
    keys.push((
        "circuit_breaker_reason_key",
        hex_xdr(env, circuit_breaker_reason_key()),
    ));
    keys.push((
        "archive_after_ledgers_key",
        hex_xdr(env, archive_after_ledgers_key()),
    ));
    keys.push((
        "platform_vol_thresh_key",
        hex_xdr(env, platform_vol_thresh_key()),
    ));
    keys.push((
        "platform_vol_mile_key",
        hex_xdr(env, platform_vol_mile_key()),
    ));
    keys.push((
        "creator_vol_thresh_key",
        hex_xdr(env, creator_vol_thresh_key()),
    ));
    keys.push(("kyc_contract_key", hex_xdr(env, kyc_contract_key())));
    keys.push((
        "upgrade_proposal_key",
        hex_xdr(env, upgrade_proposal_key()),
    ));
    // Issue #477: one-shot initialiser guard — instance storage
    keys.push(("initialised_key", hex_xdr(env, initialised_key())));

    // Persistent-tier keys (per-entity)
    keys.push(("invoice_key", hex_xdr(env, invoice_key(1))));
    keys.push(("invoice_ext_key", hex_xdr(env, invoice_ext_key(1))));
    keys.push(("invoice_ext2_key", hex_xdr(env, invoice_ext2_key(1))));
    keys.push(("invoice_compact_key", hex_xdr(env, invoice_compact_key(1))));
    keys.push(("invoice_hot_key", hex_xdr(env, invoice_hot_key(1))));
    keys.push(("audit_log_key", hex_xdr(env, audit_log_key(1))));
    keys.push(("archive_marker_key", hex_xdr(env, archive_marker_key(1))));
    keys.push(("created_ledger_key", hex_xdr(env, created_ledger_key(1))));
    keys.push((
        "subscription_params_key",
        hex_xdr(env, subscription_params_key(1)),
    ));
    keys.push((
        "confidential_count_key",
        hex_xdr(env, confidential_count_key(1)),
    ));
    keys.push(("ext_vote_key", hex_xdr(env, ext_vote_key(1))));
    keys.push(("group_key", hex_xdr(env, group_key(1))));
    keys.push(("invoice_group_key", hex_xdr(env, invoice_group_key(1))));
    keys.push((
        "invoice_treasury_key",
        hex_xdr(env, invoice_treasury_key(1)),
    ));
    keys.push(("group_treasury_key", hex_xdr(env, group_treasury_key(1))));
    keys.push(("delegate_key", hex_xdr(env, delegate_key(1))));
    keys.push(("payment_window_key", hex_xdr(env, payment_window_key(1))));
    keys.push(("cert_key", hex_xdr(env, cert_key(1))));
    keys.push(("timelock_action_key", hex_xdr(env, timelock_action_key(1))));
    keys.push(("refunded_key", hex_xdr(env, refunded_key(1))));

    keys.push(("pause_exempt_key", hex_xdr(env, pause_exempt_key(&a))));
    keys.push(("global_vel_key", hex_xdr(env, global_vel_key(&a))));
    keys.push(("rep_key", hex_xdr(env, rep_key(&a))));
    keys.push(("credit_key", hex_xdr(env, credit_key(&a))));
    keys.push(("referral_count_key", hex_xdr(env, referral_count_key(&a))));
    keys.push((
        "recipient_invoice_ids_key",
        hex_xdr(env, recipient_invoice_ids_key(&a)),
    ));
    keys.push(("delegate_pay_key", hex_xdr(env, delegate_pay_key(&a))));
    keys.push(("rate_usage_key", hex_xdr(env, rate_usage_key(&a))));
    keys.push(("invoice_count_key", hex_xdr(env, invoice_count_key(&a))));
    keys.push(("cancel_count_key", hex_xdr(env, cancel_count_key(&a))));
    keys.push((
        "creator_stats_count_key",
        hex_xdr(env, creator_stats_count_key(&a)),
    ));
    keys.push((
        "creator_stats_volume_key",
        hex_xdr(env, creator_stats_volume_key(&a)),
    ));
    keys.push((
        "creator_stats_released_key",
        hex_xdr(env, creator_stats_released_key(&a)),
    ));
    keys.push((
        "creator_stats_refunded_key",
        hex_xdr(env, creator_stats_refunded_key(&a)),
    ));
    keys.push((
        "creator_stats_payers_key",
        hex_xdr(env, creator_stats_payers_key(&a)),
    ));
    keys.push((
        "creator_stats_avg_funding_key",
        hex_xdr(env, creator_stats_avg_funding_key(&a)),
    ));
    keys.push((
        "creator_vol_mile_key",
        hex_xdr(env, creator_vol_mile_key(&a)),
    ));

    keys
}

#[test]
fn storage_key_snapshot() {
    let env = Env::default();
    let keys = current_keys(&env);

    // Every current key must have a baseline entry, and vice versa.
    let mut seen: HashSet<&str> = HashSet::new();
    for (label, _) in keys.iter() {
        assert!(
            seen.insert(label),
            "duplicate storage key label in current set: {}",
            label
        );
    }

    let baseline: HashSet<&str> = EXPECTED_KEYS.iter().map(|(l, _)| *l).collect();
    assert_eq!(
        seen.len(),
        baseline.len(),
        "storage key count changed: current={} baseline={} — update EXPECTED_KEYS per docs/ARCHITECTURE.md",
        seen.len(),
        baseline.len()
    );

    for (label, actual) in keys.iter() {
        let expected = EXPECTED_KEYS
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, v)| *v)
            .unwrap_or_else(|| panic!("missing baseline entry for key: {}", label));
        assert_eq!(
            actual.as_str(),
            expected,
            "storage key changed for {}: expected {} got {} — if intentional, update EXPECTED_KEYS per docs/ARCHITECTURE.md",
            label,
            expected,
            actual
        );
    }
}
