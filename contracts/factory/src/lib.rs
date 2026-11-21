//! SplitFactory — clone-factory for per-invoice split contract instances.
//!
//! Deploying a fresh WASM instance per invoice is gas-intensive. This factory
//! deploys minimal-proxy clones pointing to a single canonical WASM blob,
//! reducing deployment cost while maintaining per-invoice state isolation.

#![no_std]

use soroban_sdk::{
    contract, contractimpl, symbol_short, xdr::ToXdr, Address, Bytes, BytesN, Env, Symbol, Val, Vec,
};

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

fn factory_admin_key() -> Symbol {
    symbol_short!("fct_adm")
}

/// Approved WASM hashes allowlist key — instance storage.
/// Maps to a `Vec<BytesN<32>>` containing all approved WASM hashes.
fn approved_wasm_hashes_key() -> Symbol {
    symbol_short!("appr_wh")
}

/// Per-creator deployed contracts list — persistent storage.
///
/// # Retention and Restoration Policy (#210)
/// Deployment records are retained in persistent storage with automated TTL
/// extension targeting [`FACTORY_RECORD_MAX_TTL`] whenever entries are created
/// or queried at or below [`FACTORY_RECORD_MIN_TTL`]. If an inactive record reaches
/// archival expiration, it can be restored via standard Soroban ledger entry
/// restoration transactions prior to subsequent deployment or lookup operations.
fn deployed_contracts_key(creator: &Address) -> (Symbol, Address) {
    (symbol_short!("deployed"), creator.clone())
}

/// Per-(creator, salt) deployed address — used to detect duplicate salts.
///
/// # Retention and Restoration Policy (#210)
/// Salt markers are retained in persistent storage with automated TTL
/// extension targeting [`FACTORY_RECORD_MAX_TTL`] whenever entries are created
/// or queried at or below [`FACTORY_RECORD_MIN_TTL`]. If an inactive marker reaches
/// archival expiration, it can be restored via standard Soroban ledger entry
/// restoration transactions prior to reusing or verifying the salt.
fn salt_key(creator: &Address, salt: &BytesN<32>) -> (Symbol, Address, BytesN<32>) {
    (symbol_short!("salt"), creator.clone(), salt.clone())
}

/// Maximum number of deployed contracts per creator (DoS bound).
const MAX_DEPLOYMENTS_PER_CREATOR: u32 = 10_000;

/// Maximum number of approved WASM hashes in instance storage (DoS bound, #209).
pub const MAX_APPROVED_WASM_HASHES: u32 = 20;

// ---------------------------------------------------------------------------
// Storage rent / TTL constants (#210)
// ---------------------------------------------------------------------------

/// Minimum TTL threshold before an extension is triggered (~30 days at 5 s/ledger).
pub const FACTORY_RECORD_MIN_TTL: u32 = 518_400;

/// Maximum TTL to extend to (~1 year at 5 s/ledger).
pub const FACTORY_RECORD_MAX_TTL: u32 = 6_307_200;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted when a new split contract is deployed by the factory.
///
/// Topics: `(factory, deployed, creator)`
/// Data: `(wasm_hash, salt, contract_address)`. The effective deployment
/// salt is `sha256(creator_xdr || salt)`.
///
/// # Parameters
/// * `env`              — The current Soroban execution environment.
/// * `creator`          — The address that triggered the deployment.
/// * `wasm_hash`        — The deployed WASM bytecode hash.
/// * `salt`             — The creator-supplied salt.
/// * `contract_address` — The address of the newly deployed contract.
pub fn contract_deployed(
    env: &Env,
    creator: &Address,
    wasm_hash: &BytesN<32>,
    salt: &BytesN<32>,
    contract_address: &Address,
) {
    env.events().publish(
        (
            symbol_short!("factory"),
            symbol_short!("deployed"),
            creator.clone(),
        ),
        (wasm_hash.clone(), salt.clone(), contract_address.clone()),
    );
}

/// Emitted when a WASM hash is approved by the factory administrator (#208).
///
/// Topics: `(factory, approved)`
/// Data: `wasm_hash`
pub fn wasm_hash_approved(env: &Env, wasm_hash: &BytesN<32>) {
    env.events().publish(
        (symbol_short!("factory"), symbol_short!("approved")),
        wasm_hash.clone(),
    );
}

/// Emitted when a WASM hash is revoked by the factory administrator (#208).
///
/// Topics: `(factory, revoked)`
/// Data: `wasm_hash`
pub fn wasm_hash_revoked(env: &Env, wasm_hash: &BytesN<32>) {
    env.events().publish(
        (symbol_short!("factory"), symbol_short!("revoked")),
        wasm_hash.clone(),
    );
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct SplitFactory;

#[contractimpl]
impl SplitFactory {
    /// Initialise the factory by recording its admin.
    ///
    /// Must be called exactly once after the contract is deployed. Subsequent
    /// calls panic with `"already initialized"`.
    ///
    /// # Parameters
    /// * `env`   — The current Soroban execution environment.
    /// * `admin` — The address that will become the factory administrator.
    ///
    /// # Panics
    /// Panics with `"already initialized"` if the factory has already been
    /// initialised.
    pub fn initialize(env: Env, admin: Address) {
        assert!(
            !env.storage().instance().has(&factory_admin_key()),
            "already initialized"
        );
        env.storage().instance().set(&factory_admin_key(), &admin);
    }

    /// Return the current factory administrator, or `None` if not yet initialized (#211).
    ///
    /// # Parameters
    /// * `env` — The current Soroban execution environment.
    ///
    /// # Returns
    /// `Some(Address)` containing the factory administrator if initialized,
    /// or `None` if the factory has not yet been initialized.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&factory_admin_key())
    }

    /// Add a WASM hash to the approved deployment allowlist.
    ///
    /// Only the factory admin may call this. Subsequent calls to
    /// [`Self::deploy_invoice_contract`] will only succeed when the supplied
    /// `wasm_hash` appears in the allowlist.
    ///
    /// # Parameters
    /// * `env`       — The current Soroban execution environment.
    /// * `admin`     — The factory administrator address; requires auth.
    /// * `wasm_hash` — The 32-byte SHA-256 hash of the WASM blob to approve.
    ///
    /// # Panics
    /// Panics if `admin` is not the factory administrator.
    pub fn approve_wasm_hash(env: Env, admin: Address, wasm_hash: BytesN<32>) {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&factory_admin_key())
            .expect("factory not initialized");
        assert!(stored_admin == admin, "not factory admin");
        admin.require_auth();

        let mut hashes: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&approved_wasm_hashes_key())
            .unwrap_or_else(|| Vec::new(&env));

        // Avoid duplicates and emit event only for effective changes (#208).
        if !hashes.contains(&wasm_hash) {
            // Enforce upper bound on approved hashes (#209).
            assert!(
                hashes.len() < MAX_APPROVED_WASM_HASHES,
                "allowlist limit exceeded"
            );

            hashes.push_back(wasm_hash.clone());
            env.storage()
                .instance()
                .set(&approved_wasm_hashes_key(), &hashes);

            wasm_hash_approved(&env, &wasm_hash);
        }
    }

    /// Remove a WASM hash from the approved deployment allowlist.
    ///
    /// Only the factory admin may call this. After removal, subsequent
    /// deployment requests specifying the revoked hash will be rejected.
    ///
    /// # Parameters
    /// * `env`       — The current Soroban execution environment.
    /// * `admin`     — The factory administrator address; requires auth.
    /// * `wasm_hash` — The 32-byte SHA-256 hash to remove from the allowlist.
    ///
    /// # Panics
    /// Panics if `admin` is not the factory administrator.
    pub fn revoke_wasm_hash(env: Env, admin: Address, wasm_hash: BytesN<32>) {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&factory_admin_key())
            .expect("factory not initialized");
        assert!(stored_admin == admin, "not factory admin");
        admin.require_auth();

        let hashes: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&approved_wasm_hashes_key())
            .unwrap_or_else(|| Vec::new(&env));

        // Only modify storage and emit event if the hash was actually approved (#208).
        if hashes.contains(&wasm_hash) {
            let mut new_hashes: Vec<BytesN<32>> = Vec::new(&env);
            for h in hashes.iter() {
                if h != wasm_hash {
                    new_hashes.push_back(h.clone());
                }
            }
            env.storage()
                .instance()
                .set(&approved_wasm_hashes_key(), &new_hashes);

            wasm_hash_revoked(&env, &wasm_hash);
        }
    }

    /// Return whether a specific WASM hash is in the approved allowlist.
    ///
    /// # Parameters
    /// * `env`       — The current Soroban execution environment.
    /// * `wasm_hash` — The 32-byte SHA-256 hash to query.
    pub fn is_wasm_hash_approved(env: Env, wasm_hash: BytesN<32>) -> bool {
        let hashes: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&approved_wasm_hashes_key())
            .unwrap_or_else(|| Vec::new(&env));
        hashes.contains(&wasm_hash)
    }

    /// Deploy a new split contract instance using the clone-factory pattern.
    ///
    /// Uses `env.deployer().with_current_contract(salt)` so that the deployed
    /// address is deterministic for a given `(creator, salt)` pair. Duplicate
    /// salts for the same creator are rejected.
    ///
    /// # Parameters
    /// * `creator`     — The creator address; requires auth.
    /// * `wasm_hash`   — SHA-256 hash of the pre-uploaded split-contract WASM.
    ///                   Must be present in the admin-managed approved allowlist.
    /// * `salt`        — 32-byte uniqueness salt; determines the deployed address.
    /// * `init_args`   — Constructor arguments forwarded to the deployed contract.
    ///
    /// # Returns
    /// The `Address` of the newly deployed contract.
    ///
    /// # Panics
    /// Panics with `"wasm hash not approved"` if `wasm_hash` is not in the allowlist.
    pub fn deploy_invoice_contract(
        env: Env,
        creator: Address,
        wasm_hash: BytesN<32>,
        salt: BytesN<32>,
        init_args: soroban_sdk::Vec<Val>,
    ) -> Address {
        creator.require_auth();

        // Reject any WASM hash that has not been explicitly approved by the admin.
        let approved_hashes: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&approved_wasm_hashes_key())
            .unwrap_or_else(|| Vec::new(&env));
        assert!(
            approved_hashes.contains(&wasm_hash),
            "wasm hash not approved"
        );

        // Reject duplicate salts for the same creator.
        let salt_storage_key = salt_key(&creator, &salt);
        assert!(
            !env.storage().persistent().has(&salt_storage_key),
            "duplicate salt for creator"
        );

        // Enforce per-creator deployment cap (DoS bound).
        let deployments_key = deployed_contracts_key(&creator);
        let deployments: Vec<Address> = env
            .storage()
            .persistent()
            .get(&deployments_key)
            .unwrap_or_else(|| Vec::new(&env));
        assert!(
            deployments.len() < MAX_DEPLOYMENTS_PER_CREATOR,
            "deployment limit exceeded"
        );

        // Derive a per-(creator, salt) effective deployment salt so different
        // creators can independently reuse the same user-provided salt without
        // address collisions. effective_salt = sha256(creator_xdr || user_salt).
        let mut salt_input = creator.clone().to_xdr(&env);
        salt_input.append(&Bytes::from_array(&env, &salt.to_array()));
        let effective_salt: BytesN<32> = env.crypto().sha256(&salt_input).into();

        // Deploy using the clone-factory pattern.
        let deployed_address = env
            .deployer()
            .with_current_contract(effective_salt)
            .deploy(wasm_hash.clone());

        // Invoke init on the deployed contract if init_args is non-empty.
        //
        // Safety / atomicity (#49): `env.invoke_contract` propagates any panic
        // from the callee back to this call frame, causing the entire
        // transaction to revert. Deployment-state mutations below are therefore
        // only reachable when initialization succeeds, guaranteeing that no
        // uninitialized contract is ever recorded in the deployment tracking.
        //
        // Callers MUST supply the exact argument list expected by the deployed
        // contract's `initialize` function. A mismatched signature will cause
        // the sub-call to panic and atomically roll back the deployment.
        if !init_args.is_empty() {
            env.invoke_contract::<Val>(
                &deployed_address,
                &Symbol::new(&env, "initialize"),
                init_args,
            );
        }

        // Deployment state is only committed here — after initialization has
        // completed successfully (#49). If the invoke_contract call above
        // panicked, execution never reaches this point.

        // Record the deployed address and extend retention (#210).
        let mut updated_deployments = deployments;
        updated_deployments.push_back(deployed_address.clone());
        env.storage()
            .persistent()
            .set(&deployments_key, &updated_deployments);
        env.storage().persistent().extend_ttl(
            &deployments_key,
            FACTORY_RECORD_MIN_TTL,
            FACTORY_RECORD_MAX_TTL,
        );

        // Mark the salt as used and extend retention (#210).
        env.storage()
            .persistent()
            .set(&salt_storage_key, &deployed_address);
        env.storage().persistent().extend_ttl(
            &salt_storage_key,
            FACTORY_RECORD_MIN_TTL,
            FACTORY_RECORD_MAX_TTL,
        );

        // Emit event.
        contract_deployed(&env, &creator, &wasm_hash, &salt, &deployed_address);

        deployed_address
    }

    /// Return all deployed contract addresses for a given creator.
    ///
    /// Looks up the persistent list of contracts that `creator` has deployed
    /// through this factory. Returns an empty `Vec` when the creator has not
    /// yet deployed any contracts. Automatically refreshes TTL when records exist (#210).
    ///
    /// # Parameters
    /// * `env`     — The current Soroban execution environment.
    /// * `creator` — The creator address to query.
    ///
    /// # Returns
    /// A `Vec<Address>` of contract addresses in deployment order. Empty if
    /// no contracts have been deployed for this creator.
    pub fn get_deployments(env: Env, creator: Address) -> Vec<Address> {
        let key = deployed_contracts_key(&creator);
        if env.storage().persistent().has(&key) {
            env.storage().persistent().extend_ttl(
                &key,
                FACTORY_RECORD_MIN_TTL,
                FACTORY_RECORD_MAX_TTL,
            );
        }
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Check whether a specific (creator, salt) pair has already been used.
    ///
    /// Because the deployed address is deterministic per `(creator, salt)`,
    /// the factory rejects duplicate salts for the same creator. Use this
    /// view function before calling [`Self::deploy_invoice_contract`] to
    /// pre-check whether a salt is available. Refreshes TTL when used (#210).
    ///
    /// # Parameters
    /// * `env`     — The current Soroban execution environment.
    /// * `creator` — The creator address to check.
    /// * `salt`    — The 32-byte salt to check.
    ///
    /// # Returns
    /// `true` if the `(creator, salt)` pair was already used in a previous
    /// deployment; `false` otherwise.
    pub fn is_salt_used(env: Env, creator: Address, salt: BytesN<32>) -> bool {
        let key = salt_key(&creator, &salt);
        if env.storage().persistent().has(&key) {
            env.storage().persistent().extend_ttl(
                &key,
                FACTORY_RECORD_MIN_TTL,
                FACTORY_RECORD_MAX_TTL,
            );
        }
        env.storage().persistent().has(&key)
    }

    /// Explicitly extend retention (rent) for a creator's deployment list and salt record (#210).
    ///
    /// Extends the persistent storage TTL for [`deployed_contracts_key`] and,
    /// if provided, [`salt_key`].
    pub fn extend_deployment_records_ttl(env: Env, creator: Address, salt: Option<BytesN<32>>) {
        let dep_key = deployed_contracts_key(&creator);
        if env.storage().persistent().has(&dep_key) {
            env.storage().persistent().extend_ttl(
                &dep_key,
                FACTORY_RECORD_MIN_TTL,
                FACTORY_RECORD_MAX_TTL,
            );
        }
        if let Some(s) = salt {
            let s_key = salt_key(&creator, &s);
            if env.storage().persistent().has(&s_key) {
                env.storage().persistent().extend_ttl(
                    &s_key,
                    FACTORY_RECORD_MIN_TTL,
                    FACTORY_RECORD_MAX_TTL,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events as _},
        BytesN, Env, TryFromVal, Val, Vec,
    };

    /// Minimal valid Soroban WASM stub; source and retrieval: docs/FACTORY_FIXTURE.md.
    const STUB_WASM: &[u8] = include_bytes!("../../../tests/fixtures/stub_contract.wasm");

    #[test]
    fn test_deploy_single_contract() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);
        // Approve the WASM hash before deployment.
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);

        let creator = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[1u8; 32]);
        let init_args = Vec::<Val>::new(&env);

        let addr = factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt, &init_args);

        let deployments = factory_client.get_deployments(&creator);
        assert_eq!(deployments.len(), 1);
        assert_eq!(deployments.get_unchecked(0), addr);
    }

    #[test]
    fn test_deploy_two_contracts_unique_addresses() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);
        // Approve the WASM hash before deployment.
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);

        let creator = Address::generate(&env);
        let init_args = Vec::<Val>::new(&env);

        let salt1 = BytesN::from_array(&env, &[1u8; 32]);
        let addr1 =
            factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt1, &init_args);

        let salt2 = BytesN::from_array(&env, &[2u8; 32]);
        let addr2 =
            factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt2, &init_args);

        assert_ne!(addr1, addr2);

        let deployments = factory_client.get_deployments(&creator);
        assert_eq!(deployments.len(), 2);
        assert_eq!(deployments.get_unchecked(0), addr1);
        assert_eq!(deployments.get_unchecked(1), addr2);
    }

    #[test]
    #[should_panic(expected = "duplicate salt for creator")]
    fn test_duplicate_salt_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);
        // Approve the WASM hash before deployment.
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);

        let creator = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[1u8; 32]);
        let init_args = Vec::<Val>::new(&env);

        factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt, &init_args);
        // Same salt, same creator — should panic.
        factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt, &init_args);
    }

    #[test]
    fn test_different_creators_same_salt_allowed() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);
        // Approve the WASM hash before deployment.
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);

        let init_args = Vec::<Val>::new(&env);
        let salt = BytesN::from_array(&env, &[1u8; 32]);

        let creator1 = Address::generate(&env);
        let addr1 =
            factory_client.deploy_invoice_contract(&creator1, &wasm_hash, &salt, &init_args);

        let creator2 = Address::generate(&env);
        let addr2 =
            factory_client.deploy_invoice_contract(&creator2, &wasm_hash, &salt, &init_args);

        assert_ne!(addr1, addr2);
    }

    #[test]
    fn test_contract_deployed_event_emitted() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);
        // Approve the WASM hash before deployment.
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);

        let creator = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[1u8; 32]);
        let init_args = Vec::<Val>::new(&env);

        let deployed =
            factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt, &init_args);
        let as_xdr = |value: Val| ScVal::try_from_val(&env, &value).unwrap();
        let expected_topics = [
            as_xdr(symbol_short!("factory").into_val(&env)),
            as_xdr(symbol_short!("deployed").into_val(&env)),
            as_xdr(creator.into_val(&env)),
        ];
        let expected_data = as_xdr((wasm_hash, salt, deployed).into_val(&env));
        let events = env.events().all();
        assert!(events.events().iter().any(|event| {
            let soroban_sdk::xdr::ContractEventBody::V0(body) = &event.body;
            let mut topics = Vec::new(&env);
            let topics_ref: &[soroban_sdk::xdr::ScVal] = body.topics.as_ref();
            for v in topics_ref.iter() {
                if let Ok(val) = Val::try_from_val(&env, v) {
                    topics.push_back(val);
                }
            }
            // topics: (factory, deployed, creator)
            if topics.len() >= 3 {
                if let Ok(t0) =
                    <Symbol as TryFromVal<Env, Val>>::try_from_val(&env, &topics.get_unchecked(0))
                {
                    if t0 == symbol_short!("factory") {
                        found = true;
                        break;
                    }
                }
            }
        }
        assert!(found, "ContractDeployed event not emitted");
    }

    #[test]
    fn test_is_salt_used() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);
        // Approve the WASM hash before deployment.
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);

        let creator = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[1u8; 32]);
        let init_args = Vec::<Val>::new(&env);

        assert!(!factory_client.is_salt_used(&creator, &salt));
        factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt, &init_args);
        assert!(factory_client.is_salt_used(&creator, &salt));
    }

    #[test]
    fn test_empty_get_deployments() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let creator = Address::generate(&env);
        assert_eq!(factory_client.get_deployments(&creator).len(), 0);
    }

    // -----------------------------------------------------------------------
    // Security tests — Issue #4: Unrestricted WASM Hash
    // -----------------------------------------------------------------------

    /// Deploying with an unapproved WASM hash must be rejected.
    #[test]
    #[should_panic(expected = "wasm hash not approved")]
    fn test_deploy_unapproved_wasm_hash_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        // Upload but deliberately DO NOT approve the hash.
        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);

        let creator = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[1u8; 32]);
        let init_args = Vec::<Val>::new(&env);

        // Should panic: "wasm hash not approved".
        factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt, &init_args);
    }

    /// A completely arbitrary (random) WASM hash must also be rejected.
    #[test]
    #[should_panic(expected = "wasm hash not approved")]
    fn test_deploy_arbitrary_wasm_hash_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        // Approve a real WASM hash but try to deploy with a different one.
        let real_hash = env.deployer().upload_contract_wasm(STUB_WASM);
        factory_client.approve_wasm_hash(&factory_admin, &real_hash);

        // Forge a completely different hash.
        let forged_hash = BytesN::from_array(&env, &[0xdeu8; 32]);

        let creator = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[1u8; 32]);
        let init_args = Vec::<Val>::new(&env);

        // Should panic: "wasm hash not approved".
        factory_client.deploy_invoice_contract(&creator, &forged_hash, &salt, &init_args);
    }

    /// After revoking an approved hash it must no longer be deployable.
    #[test]
    #[should_panic(expected = "wasm hash not approved")]
    fn test_revoked_wasm_hash_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);
        // Revoke the hash immediately.
        factory_client.revoke_wasm_hash(&factory_admin, &wasm_hash);

        let creator = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[1u8; 32]);
        let init_args = Vec::<Val>::new(&env);

        // Should panic: "wasm hash not approved".
        factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt, &init_args);
    }

    /// `is_wasm_hash_approved` reflects approve/revoke lifecycle correctly.
    #[test]
    fn test_is_wasm_hash_approved() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);

        assert!(!factory_client.is_wasm_hash_approved(&wasm_hash));
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);
        assert!(factory_client.is_wasm_hash_approved(&wasm_hash));
        factory_client.revoke_wasm_hash(&factory_admin, &wasm_hash);
        assert!(!factory_client.is_wasm_hash_approved(&wasm_hash));
    }

    // -----------------------------------------------------------------------
    // Issue #208: WASM allowlist change events
    // -----------------------------------------------------------------------

    fn count_factory_events(env: &Env, action_name: &str) -> usize {
        use soroban_sdk::TryIntoVal;
        let mut count = 0;
        for event in env.events().all().events().iter() {
            let soroban_sdk::xdr::ContractEventBody::V0(body) = &event.body;
            let mut topics = soroban_sdk::Vec::new(env);
            let topics_ref: &[soroban_sdk::xdr::ScVal] = body.topics.as_ref();
            for v in topics_ref.iter() {
                if let Ok(val) =
                    <Val as TryFromVal<Env, soroban_sdk::xdr::ScVal>>::try_from_val(env, v)
                {
                    topics.push_back(val);
                }
            }
            if topics.len() >= 2 {
                let is_factory = topics
                    .get(0)
                    .and_then(|v| {
                        let r: Result<Symbol, _> = v.try_into_val(env);
                        r.ok()
                    })
                    .map(|s| s == symbol_short!("factory"))
                    .unwrap_or(false);
                let is_action = topics
                    .get(1)
                    .and_then(|v| {
                        let r: Result<Symbol, _> = v.try_into_val(env);
                        r.ok()
                    })
                    .map(|s| s == Symbol::new(env, action_name))
                    .unwrap_or(false);
                if is_factory && is_action {
                    count += 1;
                }
            }
        }
        count
    }

    #[test]
    fn test_wasm_allowlist_events_and_noop() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);

        // First approval: effective change, exactly 1 event emitted in this invocation.
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);
        assert_eq!(count_factory_events(&env, "approved"), 1);

        // Repeated approval: effective approval did not change, emits none.
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);
        assert_eq!(count_factory_events(&env, "approved"), 0);

        // First revocation: effective change, exactly 1 event emitted in this invocation.
        factory_client.revoke_wasm_hash(&factory_admin, &wasm_hash);
        assert_eq!(count_factory_events(&env, "revoked"), 1);

        // Repeated revocation: effective approval did not change, emits none.
        factory_client.revoke_wasm_hash(&factory_admin, &wasm_hash);
        assert_eq!(count_factory_events(&env, "revoked"), 0);
    }

    // -----------------------------------------------------------------------
    // Issue #209: Bounded WASM allowlist
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "allowlist limit exceeded")]
    fn test_bound_wasm_allowlist_overflow_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        // Fill up to MAX_APPROVED_WASM_HASHES and then exceed it.
        for i in 0..=MAX_APPROVED_WASM_HASHES {
            let mut h_bytes = [0u8; 32];
            h_bytes[0] = (i + 1) as u8;
            let hash = BytesN::from_array(&env, &h_bytes);
            factory_client.approve_wasm_hash(&factory_admin, &hash);
        }
    }

    #[test]
    fn test_revocation_creates_room_in_allowlist() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let mut first_hash = BytesN::from_array(&env, &[0u8; 32]);
        for i in 0..MAX_APPROVED_WASM_HASHES {
            let mut h_bytes = [0u8; 32];
            h_bytes[0] = (i + 1) as u8;
            let hash = BytesN::from_array(&env, &h_bytes);
            if i == 0 {
                first_hash = hash.clone();
            }
            factory_client.approve_wasm_hash(&factory_admin, &hash);
        }

        // Re-approving existing entry does not fail even when allowlist is at capacity.
        factory_client.approve_wasm_hash(&factory_admin, &first_hash);

        // Revoke one entry: creates room.
        factory_client.revoke_wasm_hash(&factory_admin, &first_hash);
        assert!(!factory_client.is_wasm_hash_approved(&first_hash));

        // Now room is created for a new distinct entry.
        let mut new_bytes = [0u8; 32];
        new_bytes[0] = 99;
        let new_hash = BytesN::from_array(&env, &new_bytes);
        factory_client.approve_wasm_hash(&factory_admin, &new_hash);
        assert!(factory_client.is_wasm_hash_approved(&new_hash));
    }

    // -----------------------------------------------------------------------
    // Issue #210: Extend rent for factory deployment records
    // -----------------------------------------------------------------------

    #[test]
    fn test_deployment_records_rent_retention_and_restoration() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_admin = Address::generate(&env);
        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);
        factory_client.initialize(&factory_admin);

        let wasm_hash = env.deployer().upload_contract_wasm(STUB_WASM);
        factory_client.approve_wasm_hash(&factory_admin, &wasm_hash);

        let creator = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[7u8; 32]);
        let init_args = Vec::<Val>::new(&env);

        let deployed_addr =
            factory_client.deploy_invoice_contract(&creator, &wasm_hash, &salt, &init_args);

        // Verify records exist in persistent storage and are covered by TTL extension.
        env.as_contract(&factory_id, || {
            let dep_key = deployed_contracts_key(&creator);
            let s_key = salt_key(&creator, &salt);

            assert!(env.storage().persistent().has(&dep_key));
            assert!(env.storage().persistent().has(&s_key));

            // Confirm explicit extension call at TTL threshold executes successfully under policy.
            env.storage().persistent().extend_ttl(
                &dep_key,
                FACTORY_RECORD_MIN_TTL,
                FACTORY_RECORD_MAX_TTL,
            );
            env.storage().persistent().extend_ttl(
                &s_key,
                FACTORY_RECORD_MIN_TTL,
                FACTORY_RECORD_MAX_TTL,
            );
        });

        // Test that getter views and dedicated extension helper refresh retention without error.
        assert_eq!(
            factory_client.get_deployments(&creator).get_unchecked(0),
            deployed_addr
        );
        assert!(factory_client.is_salt_used(&creator, &salt));

        factory_client.extend_deployment_records_ttl(&creator, &Some(salt));
    }

    // -----------------------------------------------------------------------
    // Issue #211: Expose factory administrator
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_admin_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();

        let factory_id = env.register(SplitFactory, ());
        let factory_client = SplitFactoryClient::new(&env, &factory_id);

        // Pre-initialization behavior: returns None (documented result).
        assert_eq!(factory_client.get_admin(), None);

        // Initialize with admin.
        let factory_admin = Address::generate(&env);
        factory_client.initialize(&factory_admin);

        // Post-initialization behavior: returns Some(factory_admin).
        assert_eq!(factory_client.get_admin(), Some(factory_admin));
    }
}
