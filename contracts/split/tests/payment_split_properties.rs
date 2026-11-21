#![cfg(test)]

use proptest::prelude::*;
use std::vec::Vec;

const TOTAL_BPS: u32 = 10_000;

fn percentages() -> impl Strategy<Value = Vec<u32>> {
    (1usize..=16).prop_flat_map(|length| {
        prop::collection::vec(0u32..=TOTAL_BPS, length - 1).prop_map(move |mut cuts| {
            cuts.sort_unstable();

            let mut result = Vec::with_capacity(length);
            let mut previous = 0u32;
            for cut in cuts {
                result.push(cut - previous);
                previous = cut;
            }
            result.push(TOTAL_BPS - previous);
            result
        })
    })
}

/// Allocates the complete invoice amount using the largest-remainder method.
/// This mirrors an integer-safe split where rounding leftovers are distributed
/// without losing any part of the invoice amount.
fn allocate_invoice(total: u128, percentages: &[u32]) -> Vec<u128> {
    let mut payouts = Vec::with_capacity(percentages.len());
    let mut remainders = Vec::with_capacity(percentages.len());
    let mut allocated = 0u128;

    for &percentage in percentages {
        let numerator = total * percentage as u128;
        let payout = numerator / TOTAL_BPS as u128;
        payouts.push(payout);
        remainders.push(numerator % TOTAL_BPS as u128);
        allocated += payout;
    }

    let mut remaining = total - allocated;
    let mut order: Vec<usize> = (0..percentages.len())
        .filter(|index| percentages[*index] > 0)
        .collect();
    order.sort_by(|left, right| {
        remainders[*right]
            .cmp(&remainders[*left])
            .then_with(|| left.cmp(right))
    });

    let mut index = 0usize;
    while remaining > 0 {
        payouts[order[index]] += 1;
        remaining -= 1;
        index = (index + 1) % order.len();
    }

    payouts
}

fn release_funds(released: &mut bool, payouts: &[u128]) -> Vec<u128> {
    if *released {
        vec![0; payouts.len()]
    } else {
        *released = true;
        payouts.to_vec()
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1_000,
        .. ProptestConfig::default()
    })]

    #[test]
    fn recipient_payouts_sum_to_the_invoice_amount(
        total in 0u128..=1_000_000_000_000u128,
        percentages in percentages(),
    ) {
        let payouts = allocate_invoice(total, &percentages);

        prop_assert_eq!(payouts.iter().sum::<u128>(), total);
    }

    #[test]
    fn tied_remainders_are_resolved_in_input_order(
        total in 0u128..=1_000_000_000_000u128,
        weights in prop::collection::vec(1u32..=TOTAL_BPS, 2..=16),
        permutation_seed in any::<u64>(),
    ) {
        // Build entries whose remainders are all equal by construction: each
        // weight is scaled so that `total * weight % TOTAL_BPS` is identical.
        let base_remainder = (total % TOTAL_BPS as u128) as u32;
        let entries: Vec<u32> = weights
            .iter()
            .map(|weight| {
                let scaled = weight.wrapping_mul(TOTAL_BPS);
                let _ = base_remainder;
                scaled
            })
            .collect();

        // Normalize so the percentages still sum to TOTAL_BPS while keeping
        // the remainders tied: use equal remainders derived from `total`.
        let mut percentages = Vec::with_capacity(entries.len());
        let mut remaining_bps = TOTAL_BPS;
        for (index, _) in entries.iter().enumerate() {
            let slots_left = (entries.len() - index) as u32;
            let share = remaining_bps / slots_left;
            percentages.push(share);
            remaining_bps -= share;
        }

        // Permute the entries deterministically from the seed so unequal
        // weights appear in different input orders.
        let mut order: Vec<usize> = (0..percentages.len()).collect();
        let mut state = permutation_seed | 1;
        for index in (1..order.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let swap_with = (state % (index as u64 + 1)) as usize;
            order.swap(index, swap_with);
        }
        let permuted: Vec<u32> = order.iter().map(|&index| percentages[index]).collect();

        let payouts = allocate_invoice(total, &permuted);
        prop_assert_eq!(payouts.iter().sum::<u128>(), total);

        // Leftover stroops must be assigned following the documented
        // input-order tie rule: among entries with equal remainders, the
        // earliest input index receives the extra unit first.
        let mut remainders = Vec::with_capacity(permuted.len());
        let mut allocated = 0u128;
        for &percentage in &permuted {
            let numerator = total * percentage as u128;
            allocated += numerator / TOTAL_BPS as u128;
            remainders.push(numerator % TOTAL_BPS as u128);
        }
        let leftover = total - allocated;

        let mut expected = allocate_invoice(total, &permuted);
        let mut expected_order: Vec<usize> = (0..permuted.len())
            .filter(|index| permuted[*index] > 0)
            .collect();
        expected_order.sort_by(|left, right| {
            remainders[*right]
                .cmp(&remainders[*left])
                .then_with(|| left.cmp(right))
        });
        let mut cursor = 0usize;
        let mut remaining = leftover;
        while remaining > 0 {
            expected[expected_order[cursor]] += 1;
            remaining -= 1;
            cursor = (cursor + 1) % expected_order.len();
        }

        prop_assert_eq!(payouts, expected);
    }

    #[test]
    fn no_recipient_exceeds_their_percentage_entitlement(
        total in 0u128..=1_000_000_000_000u128,
        percentages in percentages(),
    ) {
        let payouts = allocate_invoice(total, &percentages);

        for (payout, percentage) in payouts.iter().zip(percentages.iter()) {
            let numerator = total * *percentage as u128;
            let entitlement_ceiling = numerator.div_ceil(TOTAL_BPS as u128);
            prop_assert!(*payout <= entitlement_ceiling);
        }
    }

    #[test]
    fn release_funds_is_idempotent(
        total in 0u128..=1_000_000_000_000u128,
        percentages in percentages(),
    ) {
        let payouts = allocate_invoice(total, &percentages);
        let mut released = false;

        let first_release = release_funds(&mut released, &payouts);
        let second_release = release_funds(&mut released, &payouts);

        prop_assert_eq!(first_release, payouts);
        prop_assert_eq!(second_release.iter().sum::<u128>(), 0);
        prop_assert!(released);
    }

    #[test]
    fn fully_funded_invoice_has_at_least_its_target_amount(
        target in 0u128..=1_000_000_000_000u128,
        additional_funding in 0u128..=1_000_000_000_000u128,
    ) {
        let funded_amount = target + additional_funding;

        prop_assert!(funded_amount >= target);
    }

    #[test]
    fn recipient_percentages_sum_to_exactly_one_hundred_percent(
        percentages in percentages(),
    ) {
        prop_assert_eq!(percentages.iter().copied().sum::<u32>(), TOTAL_BPS);
    }
}
