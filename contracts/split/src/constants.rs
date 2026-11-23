//! Centralized constant definitions for the SplitVault contract.

/// Issue #563: Minimum invoice TTL in ledgers.
/// Set to ~60 days of ledgers (assuming ~5 seconds per ledger on Soroban).
/// This ensures invoices remain accessible during typical dispute/resolution windows.
pub const MIN_INVOICE_TTL_LEDGERS: u32 = 518_400;

/// Issue #563: Maximum invoice TTL in ledgers.
/// Set to ~1 year of ledgers to allow long-term invoice archival and dispute resolution.
/// Invoices can be bumped multiple times within this window to extend their lifetime.
pub const MAX_INVOICE_TTL_LEDGERS: u32 = 31_536_000;

/// Issue #37: Maximum age (in seconds) of an oracle price before it is considered stale.
/// Defaults to 300 seconds (5 minutes); configurable via `set_oracle_max_staleness`.
pub const ORACLE_MAX_STALE_SECONDS: u64 = 300;

/// Issue #241: Maximum number of accepted bids an auction may hold.
/// This bounds the work performed by `place_bid` (rejecting bids beyond capacity)
/// and by `settle_auction` (which iterates every accepted bid), ensuring an auction
/// at the documented capacity settles within the supported resource budget.
pub const MAX_AUCTION_BIDS: u32 = 100;

/// Issue #247: Maximum supported count input for `estimate_compute`.
/// Aligned with the recipient limit so that the largest valid inputs return
/// finite CPU and memory estimates; larger values return a typed error.
pub const MAX_ESTIMATE_COUNT: u64 = 100;

/// Issue #272: Maximum number of tranches permitted in a single instalment plan.
/// Bounding the vector length keeps per-plan persistent storage and the per-payment
/// index-lookup within a predictable instruction budget regardless of how the plan
/// was constructed. Plans with more than this many tranches are rejected at
/// registration time before any storage write occurs.
pub const MAX_INSTALMENT_TRANCHES: u32 = 64;