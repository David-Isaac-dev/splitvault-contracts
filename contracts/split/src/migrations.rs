//! Storage migration framework (versioned schema keys).
//!
//! `schema_version` is a `u32` kept in instance storage that records which
//! on-chain storage shape the contract currently has. The shared guard
//! helpers used across the contract (`require_admin`, `require_admin_role`,
//! `require_not_paused`, `check_not_paused`, `require_not_frozen`) all call
//! [`require_schema_current`] first, so a contract left on a stale schema
//! after a Wasm upgrade panics with `MigrationRequired` instead of reading or
//! writing storage in the wrong shape. `SplitContract::migrate` is the only
//! entry point exempt from that guard, since it is what performs the
//! upgrade.
//!
//! Fresh deployments call `initialize()`, which stamps the schema at
//! [`CURRENT_SCHEMA_VERSION`] immediately via [`init_schema_version`] — there
//! is nothing to migrate for storage that never existed in an older shape.
//! Only a contract whose Wasm was upgraded out from under existing data
//! (leaving `schema_version` at a stale value) needs `migrate`.
//!
//! Migrations are chunked: each `migrate` call processes at most
//! [`DEFAULT_MIGRATION_BATCH_SIZE`] invoice ids (or a caller-supplied
//! `batch_size`) and persists a per-target-version cursor so consecutive
//! transactions resume where the previous one left off. This keeps a single
//! transaction within Soroban's instruction budget even for contracts with
//! thousands of invoices.

use super::*;
use soroban_sdk::contracttype;

/// Latest schema version this contract build understands. Bump when adding a
/// new `migration_vN` and wire it into [`run_pending_migrations`].
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

/// Default number of invoice ids processed per `migrate` invocation when the
/// caller does not supply an explicit `batch_size`.
pub const DEFAULT_MIGRATION_BATCH_SIZE: u32 = 50;

/// Minimum lease (in ledgers) applied to invoice note metadata written by the
/// v2 -> v3 key rename. Soroban persistent entries expire unless their TTL is
/// extended; a migrated note must remain readable for at least this many
/// ledgers after the migration runs. At ~5s per ledger this is roughly 30
/// days, comfortably above the documented minimum lease for migrated
/// metadata.
pub const MIGRATED_META_MIN_TTL_LEDGERS: u32 = 518_400;

/// Instance storage: current on-chain schema version.
pub(crate) fn schema_version_key() -> Symbol {
    symbol_short!("schm_ver")
}

/// Instance storage: per-target-version migration cursor (last processed id).
pub(crate) fn migration_cursor_key(target_version: u32) -> (Symbol, u32) {
    (symbol_short!("mig_cur"), target_version)
}

fn get_migration_cursor(env: &Env, target_version: u32) -> u64 {
    env.storage()
        .instance()
        .get(&migration_cursor_key(target_version))
        .unwrap_or(0)
}

fn set_migration_cursor(env: &Env, target_version: u32, cursor: u64) {
    env.storage()
        .instance()
        .set(&migration_cursor_key(target_version), &cursor);
}

fn clear_migration_cursor(env: &Env, target_version: u32) {
    env.storage()
        .instance()
        .remove(&migration_cursor_key(target_version));
}

/// Current schema version. Contracts deployed before this framework existed
/// never wrote the key, so absence means "version 1" — the original,
/// pre-migration-framework storage layout.
pub fn schema_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&schema_version_key())
        .unwrap_or(1)
}

fn set_schema_version(env: &Env, version: u32) {
    env.storage()
        .instance()
        .set(&schema_version_key(), &version);
}

/// Stamp a freshly-initialized contract at the current schema version.
pub fn init_schema_version(env: &Env) {
    set_schema_version(env, CURRENT_SCHEMA_VERSION);
}

/// Guard applied by every entry point except `migrate` itself: refuse to
/// operate on stale storage so a partially-upgraded contract can't silently
/// read or write data in the wrong shape.
pub fn require_schema_current(env: &Env) {
    if schema_version(env) < CURRENT_SCHEMA_VERSION {
        panic_with_error!(env, ContractError::MigrationRequired);
    }
}

/// Read-only progress view for upgrade operators, keyed by target version.
///
/// Reports how far a bounded, multi-call migration toward `target_version`
/// has progressed. `processed` is the last invoice id the cursor has reached
/// (0 when no chunk has run yet), `total` is the highest assigned invoice id
/// at the time of the query, and `complete` is true once the schema has
/// reached `target_version` (the cursor is cleared after each version
/// completes). This lets operators distinguish a stalled upgrade from a
/// progressing one without mutating any state.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MigrationProgress {
    pub target_version: u32,
    pub processed: u64,
    pub total: u64,
    pub complete: bool,
}

/// Query migration progress toward `target_version`. Read-only: never writes
/// storage, so it is safe to call from any client at any time.
pub fn migration_progress(env: &Env, target_version: u32) -> MigrationProgress {
    let total: u64 = env.storage().persistent().get(&counter_key()).unwrap_or(0);
    let complete = schema_version(env) >= target_version;
    let processed = if complete {
        total
    } else {
        get_migration_cursor(env, target_version)
    };
    MigrationProgress {
        target_version,
        processed,
        total,
        complete,
    }
}

// ---------------------------------------------------------------------------
// v1 -> v2: introduce a per-invoice note record.
// ---------------------------------------------------------------------------
// Every invoice created under schema v1 has no associated `InvoiceMeta`. This
// migration backfills a default record for each existing invoice id so later
// code can assume the record exists once schema_version >= 2.

/// Per-invoice metadata introduced at schema v2, stored independently of the
/// core `Invoice` record so existing invoice fields are untouched.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct InvoiceMeta {
    pub note: String,
    pub priority: u32,
}

pub(crate) fn invoice_meta_key_v2(id: u64) -> (Symbol, u64) {
    (symbol_short!("inv_meta"), id)
}

/// Process a bounded chunk of the v1 -> v2 backfill. Returns `true` when the
/// migration is complete (cursor reached `last_id`), `false` when more chunks
/// remain.
fn migration_v2(env: &Env, batch_size: u32) -> bool {
    // The invoice counter holds the highest assigned id (ids are 1-based),
    // not a next-id cursor — see `_create_invoice_inner`.
    let last_id: u64 = env.storage().persistent().get(&counter_key()).unwrap_or(0);
    let mut cursor = get_migration_cursor(env, 2);
    if cursor >= last_id {
        clear_migration_cursor(env, 2);
        return true;
    }
    let default_meta = InvoiceMeta {
        note: String::from_str(env, ""),
        priority: 0,
    };
    let end = cursor.saturating_add(batch_size as u64).min(last_id);
    let mut id = cursor + 1;
    while id <= end {
        if env.storage().persistent().has(&invoice_key(id))
            && !env.storage().persistent().has(&invoice_meta_key_v2(id))
        {
            env.storage()
                .persistent()
                .set(&invoice_meta_key_v2(id), &default_meta);
        }
        id += 1;
    }
    cursor = end;
    if cursor >= last_id {
        clear_migration_cursor(env, 2);
        true
    } else {
        set_migration_cursor(env, 2, cursor);
        false
    }
}

// ---------------------------------------------------------------------------
// v2 -> v3: rename the InvoiceMeta storage key.
// ---------------------------------------------------------------------------
// The `inv_meta` key introduced at v2 is renamed to `inv_note` to match the
// public `get_invoice_note` entry point. This demonstrates a pure key rename:
// read under the old key, write under the new key, then drop the old entry.
// Sparse legacy data (holes in the assigned id range) is handled by skipping
// ids that have no v2 record, so no metadata is fabricated for holes.

pub(crate) fn invoice_meta_key_v3(id: u64) -> (Symbol, u64) {
    (symbol_short!("inv_note"), id)
}

/// Process a bounded chunk of the v2 -> v3 key rename. Returns `true` when the
/// migration is complete (cursor reached `last_id`), `false` when more chunks
/// remain.
fn migration_v3(env: &Env, batch_size: u32) -> bool {
    let last_id: u64 = env.storage().persistent().get(&counter_key()).unwrap_or(0);
    let mut cursor = get_migration_cursor(env, 3);
    if cursor >= last_id {
        clear_migration_cursor(env, 3);
        return true;
    }
    let end = cursor.saturating_add(batch_size as u64).min(last_id);
    let mut id = cursor + 1;
    while id <= end {
        let old_key = invoice_meta_key_v2(id);
        if env.storage().persistent().has(&old_key) {
            let meta: InvoiceMeta = env.storage().persistent().get(&old_key).unwrap();
            let new_key = invoice_meta_key_v3(id);
            env.storage().persistent().set(&new_key, &meta);
            env.storage().persistent().extend_ttl(
                &new_key,
                MIGRATED_META_MIN_TTL_LEDGERS,
                MIGRATED_META_MIN_TTL_LEDGERS,
            );
            env.storage().persistent().remove(&old_key);
        }
        id += 1;
    }
    cursor = end;
    if cursor >= last_id {
        clear_migration_cursor(env, 3);
        true
    } else {
        set_migration_cursor(env, 3, cursor);
        false
    }
}

/// Run every migration between the stored schema version and
/// [`CURRENT_SCHEMA_VERSION`], one bounded chunk at a time. Returns `true`
/// once the schema is fully current, `false` when more `migrate` calls are
/// needed.
pub fn run_pending_migrations(env: &Env, batch_size: u32) -> bool {
    let mut version = schema_version(env);
    while version < CURRENT_SCHEMA_VERSION {
        let done = match version {
            1 => migration_v2(env, batch_size),
            2 => migration_v3(env, batch_size),
            _ => true,
        };
        if !done {
            return false;
        }
        version += 1;
        set_schema_version(env, version);
    }
    true
}

#[cfg(test)]
mod sparse_id_tests {
    use super::*;

    /// Build a v1-shaped store with sparse invoice ids: 1, 2, 5, 6, 9 exist
    /// while 3, 4, 7, 8 are holes. The counter records the highest assigned
    /// id (9), so the bounded cursor must walk across the holes.
    fn seed_sparse_v1(env: &Env) {
        let ids = [1u64, 2, 5, 6, 9];
        for id in ids.iter() {
            env.storage().persistent().set(&invoice_key(*id), &id);
        }
        env.storage().persistent().set(&counter_key(), &9u64);
        // Absence of the schema key means version 1.
        env.storage().instance().remove(&schema_version_key());
    }

    #[test]
    fn sparse_ids_resume_across_batch_boundary() {
        let env = Env::default();
        seed_sparse_v1(&env);

        // Batch size 2: first chunk covers ids 1..=2, second 3..=4 (all
        // holes), third 5..=6, fourth 7..=8 (all holes), fifth 9..=9.
        assert!(!run_pending_migrations(&env, 2));
        assert_eq!(schema_version(&env), 1);
        assert!(!run_pending_migrations(&env, 2));
        assert!(!run_pending_migrations(&env, 2));
        assert!(!run_pending_migrations(&env, 2));
        assert!(run_pending_migrations(&env, 2));
        assert_eq!(schema_version(&env), CURRENT_SCHEMA_VERSION);

        // Existing ids got metadata; holes did not.
        for id in [1u64, 2, 5, 6, 9].iter() {
            assert!(env.storage().persistent().has(&invoice_meta_key_v3(*id)));
            assert!(!env.storage().persistent().has(&invoice_meta_key_v2(*id)));
        }
        for id in [3u64, 4, 7, 8].iter() {
            assert!(!env.storage().persistent().has(&invoice_meta_key_v2(*id)));
            assert!(!env.storage().persistent().has(&invoice_meta_key_v3(*id)));
        }
    }

    #[test]
    fn sparse_ids_preserve_preexisting_metadata() {
        let env = Env::default();
        seed_sparse_v1(&env);

        // Pre-seed a v2 note for id 5 (an existing invoice) with custom data.
        let custom = InvoiceMeta {
            note: String::from_str(&env, "legacy note"),
            priority: 7,
        };
        env.storage()
            .persistent()
            .set(&invoice_meta_key_v2(5), &custom);

        while !run_pending_migrations(&env, 2) {}

        // The preexisting note survives the rename intact.
        let migrated: InvoiceMeta = env
            .storage()
            .persistent()
            .get(&invoice_meta_key_v3(5))
            .unwrap();
        assert_eq!(migrated, custom);
        assert!(!env.storage().persistent().has(&invoice_meta_key_v2(5)));

        // Holes still have no metadata at either key.
        for id in [3u64, 4, 7, 8].iter() {
            assert!(!env.storage().persistent().has(&invoice_meta_key_v2(*id)));
            assert!(!env.storage().persistent().has(&invoice_meta_key_v3(*id)));
        }
    }
}
