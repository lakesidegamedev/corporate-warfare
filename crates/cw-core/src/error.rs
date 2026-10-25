//! Typed domain errors shared by the simulation, session and FFI layers.
//!
//! Panics are bugs. Every expected rejection is one of these values, and each carries a
//! stable message key so the phone can explain the remedy without Rust text leaking into
//! product copy.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DomainError {
    /// The actor is not a known investor or firm.
    InvalidActor { actor: u64 },
    /// The actor exists but lacks authority for this command.
    NoAuthority { actor: u64, reason: AuthorityReason },
    /// The command was prepared against an older state revision.
    StaleRevision { expected: u64, actual: u64 },
    /// A logical handle is unknown, retired or generation-mismatched.
    InvalidEntity { handle: u64, kind: EntityKind },
    /// The legal entity cannot fund the action from its own uncommitted cash.
    InsufficientCash {
        entity: u64,
        required: i64,
        available: i64,
    },
    /// Physical stock is insufficient for the reserved quantity.
    InsufficientStock {
        product: u16,
        required: i64,
        available: i64,
    },
    /// Labour, capacity, freight or construction capacity is unavailable.
    CapacityUnavailable { entity: u64, detail: CapacityReason },
    /// A solvency, payout or borrowing rule forbids the action.
    SolvencyRestriction { entity: u64, reason: SolvencyReason },
    /// The action would create a direct or indirect equity cycle.
    OwnershipCycle { buyer: u64, target: u64 },
    /// A quote or offer has expired and must be re-requested.
    QuoteExpired { quote: u64 },
    /// A competition or legal review is required before settlement.
    LegalReviewRequired { deal: u64 },
    /// The command was already applied with the same identifier.
    DuplicateCommand { command: u128 },
    /// A numeric operation would overflow or divide by zero.
    Arithmetic { detail: ArithmeticFault },
    /// Saved bytes are corrupt, truncated, oversized or from the future.
    InvalidSave { detail: SaveFault },
    /// A save schema is newer than this engine understands.
    UnsupportedVersion { found: u32, supported: u32 },
    /// A declared resource limit (entities, bytes, list rows) was exceeded.
    ResourceLimit { limit: u64, requested: u64 },
    /// The command is not valid in the current state machine phase.
    WrongPhase {
        expected: &'static str,
        actual: &'static str,
    },
    /// An internal invariant failed. Never used as a normal rejection path.
    InternalFault { diagnostic: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorityReason {
    NotOwner,
    NotManager,
    Insolvent,
    PayoutFrozen,
    BoardApprovalRequired,
    OutsideLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityReason {
    NoWorkers,
    NoInput,
    NoFreight,
    NoConstructionService,
    QueueFull,
    RegionUnreachable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolvencyReason {
    NegativeEquity,
    PayoutFrozen,
    CoverageTooLow,
    LiquidityTooLow,
    CapitalTooLow,
    PastDue,
    CashBufferTooThin,
    CreditGradeTooLow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArithmeticFault {
    Overflow,
    DivideByZero,
    NegativeQuantity,
    OutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveFault {
    BadMagic,
    Checksum,
    Truncated,
    Oversize,
    DecompressOversize,
    MalformedPayload,
    MissingLastGood,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    Investor,
    Firm,
    Facility,
    Product,
    Region,
    Contract,
    Loan,
    Bond,
    Policy,
    Deal,
    Order,
    Event,
    Bank,
    Insurer,
    Union,
}

impl EntityKind {
    pub const fn key(self) -> &'static str {
        match self {
            EntityKind::Investor => "investor",
            EntityKind::Firm => "firm",
            EntityKind::Facility => "facility",
            EntityKind::Product => "product",
            EntityKind::Region => "region",
            EntityKind::Contract => "contract",
            EntityKind::Loan => "loan",
            EntityKind::Bond => "bond",
            EntityKind::Policy => "policy",
            EntityKind::Deal => "deal",
            EntityKind::Order => "order",
            EntityKind::Event => "event",
            EntityKind::Bank => "bank",
            EntityKind::Insurer => "insurer",
            EntityKind::Union => "union",
        }
    }
}

impl DomainError {
    /// Stable machine key. Product copy maps this to localised text.
    pub const fn key(&self) -> &'static str {
        match self {
            DomainError::InvalidActor { .. } => "error.invalid_actor",
            DomainError::NoAuthority { .. } => "error.no_authority",
            DomainError::StaleRevision { .. } => "error.stale_revision",
            DomainError::InvalidEntity { .. } => "error.invalid_entity",
            DomainError::InsufficientCash { .. } => "error.insufficient_cash",
            DomainError::InsufficientStock { .. } => "error.insufficient_stock",
            DomainError::CapacityUnavailable { .. } => "error.capacity_unavailable",
            DomainError::SolvencyRestriction { .. } => "error.solvency_restriction",
            DomainError::OwnershipCycle { .. } => "error.ownership_cycle",
            DomainError::QuoteExpired { .. } => "error.quote_expired",
            DomainError::LegalReviewRequired { .. } => "error.legal_review_required",
            DomainError::DuplicateCommand { .. } => "error.duplicate_command",
            DomainError::Arithmetic { .. } => "error.arithmetic",
            DomainError::InvalidSave { .. } => "error.invalid_save",
            DomainError::UnsupportedVersion { .. } => "error.unsupported_version",
            DomainError::ResourceLimit { .. } => "error.resource_limit",
            DomainError::WrongPhase { .. } => "error.wrong_phase",
            DomainError::InternalFault { .. } => "error.internal_fault",
        }
    }

    /// The entity a remedy screen should open, when one is meaningful.
    pub const fn affected(&self) -> Option<u64> {
        match self {
            DomainError::InvalidActor { actor }
            | DomainError::NoAuthority { actor, .. }
            | DomainError::InsufficientCash { entity: actor, .. }
            | DomainError::CapacityUnavailable { entity: actor, .. }
            | DomainError::SolvencyRestriction { entity: actor, .. }
            | DomainError::InvalidEntity { handle: actor, .. } => Some(*actor),
            DomainError::InsufficientStock { product, .. } => Some(*product as u64),
            DomainError::OwnershipCycle { buyer, .. } => Some(*buyer),
            DomainError::QuoteExpired { quote }
            | DomainError::LegalReviewRequired { deal: quote } => Some(*quote),
            _ => None,
        }
    }
}

pub type DomainResult<T> = Result<T, DomainError>;
