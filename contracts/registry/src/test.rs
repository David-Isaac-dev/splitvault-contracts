use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

fn setup() -> (Env, RegistryContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, RegistryContract);
    let client = RegistryContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn test_register_and_get_contracts() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let contract = Address::generate(&env);

    client.register(&creator, &contract);

    let contracts = client.get_contracts(&creator);
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts.get(0).unwrap(), contract);
}

#[test]
fn test_register_rejects_account_address() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let account = Address::generate(&env);

    let result = client.try_register(&creator, &account);
    assert!(result.is_err());

    let contracts = client.get_contracts(&creator);
    assert_eq!(contracts.len(), 0);
    assert!(!client.is_registered(&account));
}

#[test]
fn test_deregister_removes_contract() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let contract_a = Address::generate(&env);
    let contract_b = Address::generate(&env);

    client.register(&creator, &contract_a);
    client.register(&creator, &contract_b);

    client.deregister(&creator, &contract_a);

    let contracts = client.get_contracts(&creator);
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts.get(0).unwrap(), contract_b);
}

#[test]
fn test_deregister_clears_registration_check() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let contract = Address::generate(&env);

    client.register(&creator, &contract);
    assert!(client.is_registered(&contract));

    client.deregister(&creator, &contract);
    assert!(!client.is_registered(&contract));
}

#[test]
fn test_deregister_unknown_contract_is_noop() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let contract = Address::generate(&env);

    client.deregister(&creator, &contract);

    let contracts = client.get_contracts(&creator);
    assert_eq!(contracts.len(), 0);
}

#[test]
fn test_pause_boundary_registration_flow() {
    let (env, client, _admin) = setup();
    let creator = Address::generate(&env);
    let preexisting = Address::generate(&env);
    let new_contract = Address::generate(&env);

    // Register a preexisting entry before pausing.
    client.register(&creator, &preexisting);
    assert!(client.is_registered(&preexisting));

    // Pause the registry.
    client.pause();

    // Registration fails while paused.
    let result = client.try_register(&creator, &new_contract);
    assert!(result.is_err());
    assert!(!client.is_registered(&new_contract));

    // Admin cleanup still works while paused: remove the preexisting entry.
    client.deregister(&creator, &preexisting);
    assert!(!client.is_registered(&preexisting));
    assert_eq!(client.get_contracts(&creator).len(), 0);

    // Unpause and confirm registration succeeds again.
    client.unpause();
    client.register(&creator, &new_contract);
    assert!(client.is_registered(&new_contract));

    let contracts = client.get_contracts(&creator);
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts.get(0).unwrap(), new_contract);
}
