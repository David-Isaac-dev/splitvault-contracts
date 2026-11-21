#![cfg(test)]
#![allow(clippy::all)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let split_id = Address::generate(&env);
    let admin = Address::generate(&env);
    let dashboard_id = env.register(DashboardContract, ());

    let client = DashboardContractClient::new(&env, &dashboard_id);
    client.initialize(&admin, &split_id);

    (env, split_id, dashboard_id)
}

#[test]
fn test_initial_dashboard_empty() {
    let (env, _, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator = Address::generate(&env);
    let dash = client.get_creator_dashboard(&creator);

    assert_eq!(dash.invoice_count, 0);
    assert_eq!(dash.total_volume, 0);
    assert_eq!(dash.released_count, 0);
    assert_eq!(dash.refunded_count, 0);
    assert_eq!(dash.released_volume, 0);
}

#[test]
fn test_record_created_updates_stats() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator = Address::generate(&env);

    client.record_created(&split_id, &creator, &1000_i128);
    client.record_created(&split_id, &creator, &2000_i128);

    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 2);
    assert_eq!(dash.total_volume, 3000);
    assert_eq!(dash.released_count, 0);
    assert_eq!(dash.refunded_count, 0);
    assert_eq!(dash.released_volume, 0);
}

#[test]
fn test_record_released_updates_stats() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator = Address::generate(&env);

    client.record_created(&split_id, &creator, &5000_i128);
    client.record_released(&split_id, &creator, &3000_i128);
    client.record_released(&split_id, &creator, &2000_i128);

    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 1);
    assert_eq!(dash.total_volume, 5000);
    assert_eq!(dash.released_count, 2);
    assert_eq!(dash.refunded_count, 0);
    assert_eq!(dash.released_volume, 5000);
}

#[test]
fn test_record_refunded_updates_stats() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator = Address::generate(&env);

    client.record_created(&split_id, &creator, &1000_i128);
    client.record_refunded(&split_id, &creator, &1000_i128);

    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 1);
    assert_eq!(dash.total_volume, 1000);
    assert_eq!(dash.released_count, 0);
    assert_eq!(dash.refunded_count, 1);
    assert_eq!(dash.released_volume, 0);
    assert_eq!(dash.refunded_volume, 1000);
}

#[test]
fn test_stats_are_per_creator() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let creator_a = Address::generate(&env);
    let creator_b = Address::generate(&env);

    client.record_created(&split_id, &creator_a, &100_i128);
    client.record_created(&split_id, &creator_a, &200_i128);
    client.record_created(&split_id, &creator_b, &500_i128);
    client.record_released(&split_id, &creator_a, &300_i128);
    client.record_refunded(&split_id, &creator_b, &500_i128);

    let dash_a = client.get_creator_dashboard(&creator_a);
    assert_eq!(dash_a.invoice_count, 2);
    assert_eq!(dash_a.total_volume, 300);
    assert_eq!(dash_a.released_count, 1);
    assert_eq!(dash_a.refunded_count, 0);
    assert_eq!(dash_a.released_volume, 300);

    let dash_b = client.get_creator_dashboard(&creator_b);
    assert_eq!(dash_b.invoice_count, 1);
    assert_eq!(dash_b.total_volume, 500);
    assert_eq!(dash_b.released_count, 0);
    assert_eq!(dash_b.refunded_count, 1);
    assert_eq!(dash_b.released_volume, 0);
}

#[test]
fn test_unauthorized_caller_rejected() {
    let env = Env::default();

    let split_id = Address::generate(&env);
    let admin = Address::generate(&env);
    let dashboard_id = env.register(DashboardContract, ());

    let client = DashboardContractClient::new(&env, &dashboard_id);
    client.initialize(&admin, &split_id);

    let creator = Address::generate(&env);

    // Call without mock_all_auths — require_auth() on the authorized address will fail
    // because the test caller is not the authorized split contract.
    assert!(client.try_record_created(&split_id, &creator, &100_i128).is_err());
}

// ---------------------------------------------------------------------------
// #83 — Multi-instance split contract registry tests
// ---------------------------------------------------------------------------

#[contract]
struct DashboardReporter;

#[contractimpl]
impl DashboardReporter {
    pub fn create(env: Env, dashboard: Address, creator: Address, total: i128) {
        DashboardContractClient::new(&env, &dashboard).record_created(
            &env.current_contract_address(),
            &creator,
            &total,
        );
    }
}

#[test]
fn test_multiple_split_contracts_can_report() {
    let env = Env::default();
    let split_a = env.register(DashboardReporter, ());
    let split_b = env.register(DashboardReporter, ());
    let admin = Address::generate(&env);
    let dashboard_id = env.register(DashboardContract, ());
    let client = DashboardContractClient::new(&env, &dashboard_id);
    client.initialize(&admin, &split_a);

    let creator = Address::generate(&env);
    client.record_created(&split_a, &creator, &1000_i128);
    client.record_created(&split_b, &creator, &2000_i128);

    reporter_b.create(&dashboard_id, &creator, &2000);
    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 2);
    assert_eq!(dash.total_volume, 3000);
}

/// Registering the same split contract twice is idempotent (#83).
#[test]
fn test_register_split_contract_idempotent() {
    let (env, split_a, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    client.register_split_contract(&split_a);
    assert!(client.is_authorized_split_contract(&split_a));
}

/// An unregistered split contract is not authorized (#83).
#[test]
fn test_unregistered_split_contract_not_authorized() {
    let (env, _split_a, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let stranger = Address::generate(&env);
    assert!(!client.is_authorized_split_contract(&stranger));
}

// ---------------------------------------------------------------------------
// #84 — Admin rotation and authorized contract management tests
// ---------------------------------------------------------------------------

/// The admin can update the authorized contract, and the new address can report (#84).
#[test]
fn test_set_authorized_contract_as_admin() {
    let (env, _, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let new_split = Address::generate(&env);
    client.set_authorized_contract(&new_split);

    assert!(client.is_authorized_split_contract(&new_split));

    let creator = Address::generate(&env);
    client.record_created(&new_split, &creator, &1000_i128);

    let dash = client.get_creator_dashboard(&creator);
    assert_eq!(dash.invoice_count, 1);
    assert_eq!(dash.total_volume, 1000);
}

/// The admin can rotate the admin address (#84).
#[test]
fn test_set_admin_rotates_admin() {
    let (env, _, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);

    let new_admin = Address::generate(&env);
    client.set_admin(&new_admin);

    // New admin can still perform administrative actions.
    let new_split = Address::generate(&env);
    client.set_authorized_contract(&new_split);
    assert!(client.is_authorized_split_contract(&new_split));
}

// ---------------------------------------------------------------------------
// #50 — Dashboard event emission tests
// ---------------------------------------------------------------------------

use soroban_sdk::testutils::Events as _;
use soroban_sdk::{symbol_short, IntoVal, TryFromVal, Val};

/// `record_created` must emit `(dash, created, creator)` (#50).
#[test]
fn test_record_created_emits_event() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_created(&split_id, &creator, &500_i128);

    assert!(
        find_dash_event_data(&env, symbol_short!("created")).is_some(),
        "record_created did not emit (dash, created) event",
    );
}

/// `record_released` must emit `(dash, released, creator)` (#50).
#[test]
fn test_record_released_emits_event() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_created(&split_id, &creator, &500_i128);
    client.record_released(&split_id, &creator, &500_i128);

    assert!(
        find_dash_event_data(&env, symbol_short!("released")).is_some(),
        "record_released did not emit (dash, released) event",
    );
}

/// `record_refunded` must emit `(dash, refunded, creator)` (#50).
#[test]
fn test_record_refunded_emits_event() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_refunded(&split_id, &creator, &100_i128);

    assert!(
        find_dash_event_data(&env, symbol_short!("refunded")).is_some(),
        "record_refunded did not emit (dash, refunded) event",
    );
}

// ---------------------------------------------------------------------------
// #107 — Dashboard event payload tests
// ---------------------------------------------------------------------------

/// Read the creator topic and volume metrics from the latest lifecycle event.
fn find_dash_event_data(
    env: &Env,
    action: soroban_sdk::Symbol,
) -> Option<(Address, u64, i128)> {
    let events = env.events().all();
    for event in events.events().iter().rev() {
        let soroban_sdk::xdr::ContractEventBody::V0(body) = &event.body;
        let mut topics = soroban_sdk::Vec::new(env);
        let topics_ref: &[soroban_sdk::xdr::ScVal] = body.topics.as_ref();
        for v in topics_ref.iter() {
            if let Ok(val) = <Val as TryFromVal<Env, soroban_sdk::xdr::ScVal>>::try_from_val(env, v) {
                topics.push_back(val);
            }
        }
        if topics.len() >= 2 {
            let t0 = <soroban_sdk::Symbol as TryFromVal<Env, Val>>::try_from_val(env, &topics.get_unchecked(0));
            let t1 = <soroban_sdk::Symbol as TryFromVal<Env, Val>>::try_from_val(env, &topics.get_unchecked(1));
            if let (Ok(topic0), Ok(topic1)) = (t0, t1) {
                if topic0 == symbol_short!("dash") && topic1 == action {
                    let data_val = <Val as TryFromVal<Env, soroban_sdk::xdr::ScVal>>::try_from_val(
                        env,
                        &body.data,
                    )
                    .ok()?;
                    let creator = Address::try_from_val(env, &topics.get(2)?).ok()?;
                    let (count, volume) = <(u64, i128)>::try_from_val(env, &data_val).ok()?;
                    return Some((creator, count, volume));
                }
            }
        }
    }
    None
}

/// `record_created` event data must carry creator, invoice_count, total_volume (#107).
#[test]
fn test_record_created_event_payload() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_created(&split_id, &creator, &1000_i128);
    client.record_created(&split_id, &creator, &2000_i128);

    let (ev_creator, data) =
        find_dash_event_data(&env, symbol_short!("created")).expect("missing created event");
    let (count, volume) = <(u64, i128)>::try_from_val(&env, &data).unwrap();
    assert_eq!(ev_creator, creator);
    assert_eq!(count, 2);
    assert_eq!(volume, 3000);
}

/// `record_released` event data must carry creator, released_count, released_volume (#107).
#[test]
fn test_record_released_event_payload() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_released(&split_id, &creator, &3000_i128);
    client.record_released(&split_id, &creator, &2000_i128);

    let (ev_creator, data) =
        find_dash_event_data(&env, symbol_short!("released")).expect("missing released event");
    let (count, volume) = <(u64, i128)>::try_from_val(&env, &data).unwrap();
    assert_eq!(ev_creator, creator);
    assert_eq!(count, 2);
    assert_eq!(volume, 5000);
}

/// `record_refunded` event data must carry creator and refunded_count (#107).
#[test]
fn test_record_refunded_event_payload() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_refunded(&split_id, &creator, &1);
    client.record_refunded(&split_id, &creator, &1);

    let expected = soroban_sdk::vec![&env, (
        dashboard_id,
        (symbol_short!("dash"), symbol_short!("refunded"), creator).into_val(&env),
        2u64.into_val(&env),
    )];
    assert_eq!(env.events().all(), expected);
}

#[test]
fn test_record_amount_boundaries() {
    let (env, split_id, dashboard_id) = setup();
    let client = DashboardContractClient::new(&env, &dashboard_id);
    let creator = Address::generate(&env);

    client.record_created(&split_id, &creator, &0);
    assert!(find_dash_event(&env, symbol_short!("created")));
    client.record_released(&split_id, &creator, &0);
    assert!(find_dash_event(&env, symbol_short!("released")));
    let before = client.get_creator_dashboard(&creator);
    assert_eq!(before.invoice_count, 1);
    assert_eq!(before.released_count, 1);
    assert_eq!(before.total_volume, 0);
    assert_eq!(before.released_volume, 0);

    assert!(client.try_record_created(&split_id, &creator, &-1).is_err());
    assert!(env.events().all().events().is_empty());
    assert_eq!(client.get_creator_dashboard(&creator), before);
    assert!(client.try_record_released(&split_id, &creator, &-1).is_err());
    assert!(env.events().all().events().is_empty());
    assert_eq!(client.get_creator_dashboard(&creator), before);
}
