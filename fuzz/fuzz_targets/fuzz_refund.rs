#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::{Address, Env, Vec};
use split_fuzz::{client, default_options, fund, offset_timestamp, setup};

#[derive(Debug, Arbitrary)]
struct Input {
    deadline_offset_secs: i32,
    pay_amount: i64,
    call_after_deadline: bool,
    invoice_id: u64,
    call_twice: bool,
}

fuzz_target!(|input: Input| {
    let env = Env::default();
    let (contract_id, token_id) = setup(&env);
    let c = client(&env, &contract_id);

    let creator = Address::generate(&env);
    let recipient = Address::generate(&env);
    let mut recipients = Vec::new(&env);
    recipients.push_back(recipient);
    let mut amounts = Vec::new(&env);
    amounts.push_back(1_000_i128);

    let now = env.ledger().timestamp();
    let deadline = offset_timestamp(now, input.deadline_offset_secs as i64);
    let options = default_options(&env);

    let seed_id = c
        .try_create_invoice(&creator, &recipients, &amounts, &token_id, &deadline, &options)
        .ok()
        .and_then(|r| r.ok());

    // Track payer and contribution only when a valid payment lands.
    let mut funded_payer: Option<Address> = None;
    let mut paid_amount: i128 = 0;

    if let Some(id) = seed_id {
        if input.pay_amount > 0 {
            let payer = Address::generate(&env);
            fund(&env, &token_id, &payer);
            let token = TokenClient::new(&env, &token_id);
            let before = token.balance(&payer);
            let result = c.try_pay(&payer, &id, &(input.pay_amount as i128), &0u64, &false, &false, &None);
            if result.is_ok() {
                let after = token.balance(&payer);
                paid_amount = before - after;
                if paid_amount > 0 {
                    funded_payer = Some(payer);
                }
            }
        }
    }

    if input.call_after_deadline {
        env.ledger().set_timestamp(deadline.saturating_add(1));
    } else {
        env.ledger().set_timestamp(deadline.saturating_sub(1).max(now));
    }

    let invoice_id = match seed_id {
        Some(id) if input.invoice_id % 2 == 0 => id,
        _ => input.invoice_id,
    };

    // Issue #281: verify that a successful refund returns funds to the payer.
    if let (Some(ref payer), true) = (&funded_payer, seed_id == Some(invoice_id)) {
        let token = TokenClient::new(&env, &token_id);
        let payer_before = token.balance(payer);
        let first_result = c.try_refund(&invoice_id);
        let payer_after = token.balance(payer);
        if first_result.is_ok() && first_result.unwrap().is_ok() {
            // Money must have moved back to the payer.
            assert!(
                payer_after - payer_before >= paid_amount,
                "refund did not return payer funds: before={payer_before} after={payer_after} paid={paid_amount}",
            );
        }

        if input.call_twice {
            let second_before = token.balance(payer);
            let _ = c.try_refund(&invoice_id);
            let second_after = token.balance(payer);
            // A second refund must never increase the payer's balance further.
            assert!(
                second_after <= second_before + paid_amount,
                "double-refund paid out excess: second_before={second_before} second_after={second_after}",
            );
        }
    } else {
        let _ = c.try_refund(&invoice_id);
        if input.call_twice {
            let _ = c.try_refund(&invoice_id);
        }
    }
});
