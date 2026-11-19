//! Type definitions for the invoice-escrow contract.

use soroban_sdk::{contracttype, Address, BytesN};

/// Lifecycle state of an escrow invoice.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowStatus {
    /// Invoice has been created but no funds deposited yet.
    Pending,
    /// At least one deposit has been received; not yet fully funded.
    Active,
    /// Invoice is fully funded and funds have been released to recipients.
    Released,
    /// Deadline passed without full funding; payers have been refunded.
    Refunded,
    /// Admin explicitly cancelled the invoice before release.
    Cancelled,
}

/// Core escrow invoice record.
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowInvoice {
    /// Address that created the invoice.
    pub creator: Address,
    /// Token contract address (e.g. USDC).
    pub token: Address,
    /// Total amount required to fully fund this invoice.
    pub total_amount: i128,
    /// Amount deposited so far.
    pub funded_amount: i128,
    /// Unix timestamp after which refunds are permitted if not fully funded.
    pub deadline: u64,
    /// Current lifecycle status.
    pub status: EscrowStatus,
}

/// Emitted when a new escrow invoice is created.
///
/// Event topics: `(escrow, created)`
/// Event data: `(invoice_id, creator, token, amount, deadline)`
///
/// The `token` field carries the escrow asset (denomination) so indexers can
/// distinguish invoices created with different assets from event data alone.
#[contracttype]
#[derive(Clone, Debug)]
pub struct InvoiceCreatedEvent {
    /// The ID of the newly created invoice.
    pub invoice_id: u64,
    /// The address that created the invoice.
    pub creator: Address,
    /// The escrow token (asset) contract address for this invoice.
    pub token: Address,
    /// The total amount required to fully fund this invoice.
    pub amount: i128,
    /// Unix timestamp after which refunds are permitted if not fully funded.
    pub deadline: u64,
}

/// Emitted when the admin initiates a two-step admin authority transfer.
///
/// Event topics: `(escrow, adm_prop)`
/// Event data: `(current_admin, proposed_admin, ledger)`
#[contracttype]
#[derive(Clone, Debug)]
pub struct AdminTransferProposedEvent {
    /// The current admin initiating the transfer.
    pub current_admin: Address,
    /// The new admin being proposed.
    pub proposed_admin: Address,
    /// Ledger sequence at which the proposal was made.
    pub ledger: u32,
}

/// Emitted when the proposed admin accepts the role.
///
/// Event topics: `(escrow, adm_accpt)`
/// Event data: `(new_admin, ledger)`
#[contracttype]
#[derive(Clone, Debug)]
pub struct AdminTransferAcceptedEvent {
    /// The address that accepted the admin role.
    pub new_admin: Address,
    /// Ledger sequence at which acceptance occurred.
    pub ledger: u32,
}

/// Emitted when the current admin cancels a pending transfer.
///
/// Event topics: `(escrow, adm_cncl)`
/// Event data: `(admin, cancelled_pending, ledger)`
#[contracttype]
#[derive(Clone, Debug)]
pub struct AdminTransferCancelledEvent {
    /// The admin who cancelled the pending transfer.
    pub admin: Address,
    /// The pending admin address that was cancelled.
    pub cancelled_pending: Address,
    /// Ledger sequence at which cancellation occurred.
    pub ledger: u32,
}

// ──────────────────────────────────────────────────────────────────────
// Escrow release event
// ──────────────────────────────────────────────────────────────────────

/// Emitted after funds are successfully transferred to a recipient during
/// a release call.
///
/// Event topics: `(escrow, released)`
/// Event data: `EscrowReleased { invoice_id, recipient, amount }`
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowReleased {
    /// The ID of the invoice that was released.
    pub invoice_id: u64,
    /// The address that received the released funds.
    pub recipient: Address,
    /// The amount of tokens transferred to the recipient.
    pub amount: i128,
}

// ──────────────────────────────────────────────────────────────────────
// Payer Blacklist types
// ──────────────────────────────────────────────────────────────────────

/// Entry in the payer blacklist.
///
/// When a payer is blacklisted by the admin, an entry is created with
/// `finalised: false`. The payer can submit an appeal during the
/// `APPEAL_WINDOW_LEDGERS` window. After the window closes (or the payer
/// submits an appeal), the admin calls `finalise_blacklist` with either
/// `uphold: true` (ban stays) or `uphold: false` (payer reinstated).
///
/// ## Retirement lifecycle for upheld entries
///
/// Once an entry is finalised with `upheld: true`, `finalise_blacklist`
/// rejects any further calls. To correct an administrative error or retire
/// an obsolete ban, the admin uses the separate `retire_blacklist` operation.
/// Retirement:
///
/// - restores the payer's eligibility (the payer is no longer considered
///   blacklisted by `is_payer_blacklisted`),
/// - preserves the history of the outcome by recording a
///   [`BlacklistRetirement`] record (prior upheld outcome, reason hash and
///   retirement ledger) rather than mutating or deleting the original entry,
/// - is idempotent-safe: an entry can only be retired once.
///
/// Only entries that were finalised with `upheld: true` may be retired;
/// entries finalised with `upheld: false` already restored eligibility.
#[contracttype]
#[derive(Clone, Debug)]
pub struct BlacklistEntry {
    /// Unix timestamp when the blacklist entry was created.
    pub banned_at: u64,
    /// Ledger sequence number when the blacklist entry was created.
    /// Used for precise appeal-window calculation instead of a timestamp
    /// approximation, eliminating drift caused by variable ledger close times.
    pub ban_ledger: u32,
    /// Optional appeal hash submitted by the payer (32-byte hash).
    pub appeal_hash: Option<BytesN<32>>,
    /// Whether the blacklist entry has been finalised by the admin.
    pub finalised: bool,
    /// Whether the ban was upheld after finalisation. Only meaningful
    /// when `finalised` is `true`.
    pub upheld: bool,
    /// Hash of the reason for blacklisting (provided by admin).
    pub reason_hash: BytesN<32>,
}

/// History record capturing the retirement of a previously upheld blacklist
/// entry.
///
/// When the admin retires an upheld ban (see [`BlacklistEntry`] docs), the
/// original entry is left intact and a `BlacklistRetirement` record is stored
/// so the outcome history is preserved. The payer's eligibility is restored
/// while the audit trail of the original ban and its retirement remains
/// queryable.
///
/// Event topics: `(escrow, bl_retire)`
/// Event data: `BlacklistRetirement { payer, prior_upheld, reason_hash, retired_ledger }`
#[contracttype]
#[derive(Clone, Debug)]
pub struct BlacklistRetirement {
    /// The payer whose upheld ban was retired.
    pub payer: Address,
    /// The prior upheld outcome of the finalised entry (always `true` for a
    /// valid retirement, recorded for history).
    pub prior_upheld: bool,
    /// Hash of the reason for the original blacklisting, carried over from
    /// the retired entry to preserve the outcome history.
    pub reason_hash: BytesN<32>,
    /// Ledger sequence at which the retirement occurred.
    pub retired_ledger: u32,
}