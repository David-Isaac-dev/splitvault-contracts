//! Exercise ratio validation and check that full release conserves the deposit.
//! Run with: `cargo fuzz run fuzz_ratio_validation`

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env, Vec,
};
use split_fuzz::{client, default_options, fund, offset_timestamp, setup};

#[derive(Debug, Arbitrary)]
struct Input {
    n_recipients: u8,
    cut_points: [u16; 3],
    pay_amount: u64,
}

fuzz_target!(|input: Input| {
    const BASIS_POINTS: u32 = 10_000;
    let env = Env::default();
    let (contract_id, token_id) = setup(&env);
    let c = client(&env, &contract_id);
    // Issue #279: `setup` now initializes the Split instance with a default
    // baseline, so this target no longer needs to call `initialize` itself.

    let n = usize::from(input.n_recipients % 3) + 2;
    let mut cuts = input
        .cut_points
        .map(|cut| u32::from(cut) % (BASIS_POINTS - n as u32 + 1));
    let cuts = &mut cuts[..n - 1];
    cuts.sort_unstable();

    let total = i128::from(input.pay_amount).max(n as i128);
    let mut recipients = Vec::new(&env);
    let mut amounts = Vec::new(&env);
    let mut opts = default_options(&env);
    let mut previous = 0;
    for i in 0..n {
        // Offsetting sorted cuts by their index leaves each recipient at least one BPS.
        let cut = cuts.get(i).map_or(BASIS_POINTS, |cut| cut + i as u32 + 1);
        opts.ratios.push_back(cut - previous);
        previous = cut;
        recipients.push_back(Address::generate(&env));
        amounts.push_back(total / n as i128 + if i == 0 { total % n as i128 } else { 0 });
    }
    opts.ext.ratio_denominator = u64::from(BASIS_POINTS);

    let creator = Address::generate(&env);
    let deadline = offset_timestamp(env.ledger().timestamp(), 86_400);
    let invoice_id = c
        .try_create_invoice(&creator, &recipients, &amounts, &token_id, &deadline, &opts)
        .expect("normalized ratios should be accepted")
        .expect("invoice ID should decode");

    let payer = Address::generate(&env);
    fund(&env, &token_id, &payer);
    c.try_pay(&payer, &invoice_id, &total, &0, &false, &false, &None)
        .expect("full payment should release the invoice")
        .expect("payment result should decode");

    let token = token::Client::new(&env, &token_id);
    let mut allocated = 0;
    for i in 0..recipients.len() {
        let balance = token.balance(&recipients.get(i).unwrap());
        let expected = if i == recipients.len() - 1 {
            total - allocated
        } else {
            total * i128::from(opts.ratios.get(i).unwrap()) / i128::from(BASIS_POINTS)
        };
        assert_eq!(balance, expected);
        allocated += balance;
    }
    assert_eq!(allocated, total);
    assert_eq!(token.balance(&contract_id), 0);
});
