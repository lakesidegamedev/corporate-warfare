//! Adversarial reliability evidence for packet P32 (requirements R05 and R30).
//!
//! These tests drive the session boundary the way a hostile or buggy host would: a long mixed
//! sequence of valid and invalid intents, duplicate and stale envelopes, truncated, bit-flipped,
//! mis-sized and future/past-schema saves, a save/import resume and a long career that must keep
//! financial history bounded. Every assertion is an observable identity - canonical hash,
//! revision, ledger balance, stock sign, journal window - not a private implementation shape.
//!
//! Limitations are recorded here on purpose: file-system interrupted writes, OS suspension and
//! last-good recovery live in the Swift save manager and are not emulated from Rust. Command
//! parsing bounds are covered by the typed rejections below, not by a fuzz crate.

use std::collections::BTreeMap;

use cw_core::error::{DomainError, SaveFault};
use cw_core::ids::Handle;
use cw_data::Difficulty;
use cw_session::save::{MAX_BYTES, MIN_SCHEMA_VERSION, SCHEMA_VERSION};
use cw_session::{ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR};
use cw_sim::state::{ScenarioKind, ScenarioMode};

fn config(seed: u64, scenario: ScenarioKind) -> ScenarioConfig {
    ScenarioConfig {
        seed,
        scenario,
        mode: ScenarioMode::FiveYear,
        dataset_hash: String::new(),
        difficulty: Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    }
}

fn session(seed: u64, scenario: ScenarioKind) -> Session {
    Session::create(config(seed, scenario)).expect("session creation")
}

/// Answer every waiting mandatory player card with its first choice through `RespondEvent`.
fn answer_mandatory(session: &mut Session) {
    let waiting: Vec<(u64, String)> = session
        .world
        .events
        .pending
        .iter()
        .filter(|event| {
            !event.resolved
                && event.mandatory
                && event
                    .subject_firm
                    .map(|firm| cw_sim::economy::events_engine::player_side(&session.world, firm))
                    .unwrap_or(true)
        })
        .filter_map(|event| {
            let card = session.dataset.event(&event.card)?;
            Some((event.id, card.choices.first()?.id.clone()))
        })
        .collect();
    for (event, choice) in waiting {
        let command = 0xE7E0_0000_0000_0000_0000_0000_0000_0000u128 | event as u128;
        let answer = envelope(
            session,
            command,
            CommandIntent::RespondEvent { event, choice },
        );
        session
            .submit(answer)
            .expect("a player card can be answered");
    }
}

fn envelope(session: &Session, command: u128, intent: CommandIntent) -> CommandEnvelope {
    CommandEnvelope {
        command_id_high: (command >> 64) as u64,
        command_id_low: command as u64,
        actor: INVESTOR_ACTOR,
        expected_revision: session.revision(),
        submitted_tick: session.world.tick.0,
        intent,
    }
}

/// Pinned SplitMix64 from `docs/rules/INTERFACE.md`; a local copy keeps the test independent of
/// the engine's own generator and adds no dependency.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next_u64() % upper as u64) as usize
        }
    }

    fn between(&mut self, low: i64, high: i64) -> i64 {
        assert!(low <= high);
        low + (self.next_u64() % (high - low + 1) as u64) as i64
    }
}

fn pick<T: Copy>(rng: &mut SplitMix64, items: &[T]) -> Option<T> {
    if items.is_empty() {
        None
    } else {
        Some(items[rng.below(items.len())])
    }
}

/// A handle in a slot that no world ever allocates, so it can never address a live row.
fn dead_handle(rng: &mut SplitMix64) -> Handle {
    Handle::new(9_000 + rng.below(64) as u32, 7)
}

/// One deliberately invalid intent with the error key the engine must produce for it.
struct ForcedInvalid {
    intent: CommandIntent,
    key: &'static str,
}

/// Rotate through out-of-range ids in every family that stores a numeric reference. These are
/// the intents that would otherwise index an array with hostile data.
fn forced_invalid(rng: &mut SplitMix64, session: &Session, turn: usize) -> ForcedInvalid {
    let world = &session.world;
    let firms = world.firms.handles_sorted();
    let primary = world
        .investor
        .primary_group
        .or_else(|| firms.first().copied())
        .unwrap_or(Handle::INVALID);
    let dead = dead_handle(rng);
    let reference = session.dataset.reference_price(0);
    match turn % 10 {
        0 => ForcedInvalid {
            intent: CommandIntent::SetPrice {
                firm: primary,
                product: u16::MAX,
                price_cents: reference,
            },
            key: "error.invalid_entity",
        },
        1 => ForcedInvalid {
            intent: CommandIntent::BuildFacility {
                firm: primary,
                region: u8::MAX,
                recipe: 0,
                blocks: 1,
            },
            key: "error.invalid_entity",
        },
        2 => ForcedInvalid {
            intent: CommandIntent::BuildFacility {
                firm: primary,
                region: 0,
                recipe: u16::MAX,
                blocks: 1,
            },
            key: "error.invalid_entity",
        },
        3 => ForcedInvalid {
            intent: CommandIntent::FoundFirm {
                name: "Impossible Holdings".to_owned(),
                region: u8::MAX,
                capital_cents: 30_000_000,
                recipe: 0,
                secondary: None,
            },
            key: "error.invalid_entity",
        },
        4 => ForcedInvalid {
            intent: CommandIntent::PlaceSpotOrder {
                firm: primary,
                product: 0,
                region: u8::MAX,
                quantity_lots: 1,
                max_price_cents: reference,
            },
            key: "error.invalid_entity",
        },
        5 => ForcedInvalid {
            intent: CommandIntent::OfferContract {
                buyer: primary,
                seller: primary,
                product: 0,
                from_region: 0,
                to_region: u8::MAX,
                quantity_lots: 1,
                price_cents: reference,
                weeks: 4,
                penalty_bps: 500,
                escrow_cents: 0,
            },
            key: "error.invalid_entity",
        },
        6 => ForcedInvalid {
            intent: CommandIntent::SetProductionTarget {
                firm: primary,
                target_bps: 20_000,
                lock: false,
            },
            key: "error.arithmetic",
        },
        7 => ForcedInvalid {
            intent: CommandIntent::SetWageOffer {
                firm: primary,
                tier: 9,
                wage_cents: 1_000,
            },
            key: "error.arithmetic",
        },
        8 => ForcedInvalid {
            intent: CommandIntent::SetPrice {
                firm: dead,
                product: 0,
                price_cents: reference,
            },
            key: "error.no_authority",
        },
        _ => ForcedInvalid {
            intent: CommandIntent::IssueShares {
                firm: dead,
                qty: 1_000,
                price_cents: 1_000,
                buyer_outside: 0,
            },
            key: "error.no_authority",
        },
    }
}

/// One random intent drawn mostly from in-range references, so the sequence mixes acceptances
/// with authority and solvency rejections. Nothing here is guaranteed to be accepted.
fn random_intent(rng: &mut SplitMix64, session: &Session) -> CommandIntent {
    let world = &session.world;
    let dataset = &session.dataset;
    let firms = world.firms.handles_sorted();
    let facilities = world.facilities.handles_sorted();
    let loans = world.loans.handles_sorted();
    let deals = world.deals.handles_sorted();
    let policies = world.insurance.handles_sorted();
    let contracts = world.contracts.handles_sorted();
    let primary = world
        .investor
        .primary_group
        .or_else(|| firms.first().copied())
        .unwrap_or(Handle::INVALID);
    let other = pick(rng, &firms).unwrap_or(primary);
    let facility = pick(rng, &facilities).unwrap_or(Handle::INVALID);
    let product = rng.below(dataset.products.len()) as u16;
    let recipe = rng.below(dataset.recipes.len()) as u16;
    let region = rng.below(world.regions.len()) as u8;
    let price = dataset.reference_price(product);
    let weeks = rng.between(1, 52) as u32;

    match rng.below(26) {
        0 if rng.below(2) == 0 => CommandIntent::SetPrice {
            firm: primary,
            product,
            price_cents: price,
        },
        0 => CommandIntent::SetPricePosture {
            firm: primary,
            product,
            mode: rng.below(4) as u8,
            bps: rng.between(-3_000, 12_000) as i32,
        },
        1 => CommandIntent::SetProductionTarget {
            firm: primary,
            target_bps: rng.between(0, 10_000) as i32,
            lock: false,
        },
        2 => CommandIntent::SetMarketingBudget {
            firm: primary,
            amount_cents: rng.between(0, 2_000_000),
        },
        3 => CommandIntent::SetResearchBudget {
            firm: primary,
            amount_cents: rng.between(0, 2_000_000),
        },
        4 => CommandIntent::SetWageOffer {
            firm: primary,
            tier: rng.below(3) as u8,
            wage_cents: rng.between(1, 60_000),
        },
        5 => CommandIntent::BuildFacility {
            firm: primary,
            region,
            recipe,
            blocks: rng.between(1, 2) as u8,
        },
        6 => CommandIntent::ExpandFacility {
            facility,
            blocks: 1,
        },
        7 if rng.below(2) == 0 => CommandIntent::MothballFacility { facility },
        7 => CommandIntent::ReopenFacility { facility },
        8 => CommandIntent::RetoolFacility { facility, recipe },
        9 => CommandIntent::OfferContract {
            buyer: primary,
            seller: other,
            product,
            from_region: region,
            to_region: other_region(rng, world.regions.len(), region),
            quantity_lots: rng.between(1, 4),
            price_cents: price,
            weeks,
            penalty_bps: 500,
            escrow_cents: 0,
        },
        10 => CommandIntent::PlaceSpotOrder {
            firm: primary,
            product,
            region,
            quantity_lots: 1,
            max_price_cents: price,
        },
        11 => CommandIntent::MakeIntercompanyLoan {
            lender: primary,
            borrower: other,
            principal_cents: rng.between(1, 1_000_000),
            rate_bps: 500,
            weeks,
        },
        12 => CommandIntent::SetTransferContract {
            buyer: primary,
            seller: other,
            product,
            quantity_lots: 1,
            price_cents: price,
            weeks,
        },
        13 => CommandIntent::GrantGuarantee {
            guarantor: primary,
            beneficiary: other,
            claim_cents: rng.between(1, 100_000),
            max_cents: rng.between(100_000, 1_000_000),
        },
        14 => CommandIntent::PlaceShareOrder {
            investor: true,
            firm: other,
            buy: rng.below(2) == 0,
            qty: rng.between(1, 100),
            limit_price_cents: price,
        },
        15 => CommandIntent::IssueShares {
            firm: primary,
            qty: rng.between(1_000, 10_000),
            price_cents: price,
            buyer_outside: rng.below(8) as u32,
        },
        16 => CommandIntent::RepurchaseShares {
            firm: primary,
            qty: rng.between(1, 100),
        },
        17 => CommandIntent::PayDividend {
            firm: primary,
            amount_cents: rng.between(0, 100_000),
        },
        18 => CommandIntent::CallMeeting { firm: primary },
        19 => CommandIntent::ProposeBoardAction {
            firm: primary,
            subject: format!("motion {}", rng.below(8)),
        },
        20 => CommandIntent::CastVote {
            firm: primary,
            vote_id: rng.below(4) as u32,
            approve: rng.below(2) == 0,
        },
        21 => CommandIntent::RequestLoan {
            firm: primary,
            amount_cents: rng.between(1, 10_000_000),
        },
        22 => CommandIntent::RepayLoan {
            loan: pick(rng, &loans).unwrap_or(Handle::INVALID),
            amount_cents: rng.between(0, 500_000),
        },
        23 => CommandIntent::AcceptContract {
            contract: pick(rng, &contracts).unwrap_or(Handle::INVALID),
        },
        24 => CommandIntent::FundTender {
            deal: pick(rng, &deals).unwrap_or(Handle::INVALID),
        },
        25 => CommandIntent::CancelPolicy {
            policy: pick(rng, &policies).unwrap_or(Handle::INVALID),
        },
        _ => CommandIntent::SetDepositRate {
            bank: primary,
            rate_bps: rng.between(0, 1_500) as i32,
        },
    }
}

fn other_region(rng: &mut SplitMix64, count: usize, region: u8) -> u8 {
    if count <= 1 {
        return region;
    }
    let mut drawn = rng.below(count) as u8;
    if drawn == region {
        drawn = (drawn + 1) % count as u8;
    }
    drawn
}

/// Ledger and physical invariants every week must keep.
fn assert_invariants(session: &Session) {
    let world = &session.world;
    for (handle, firm) in world.firms.iter() {
        assert!(
            firm.book.is_balanced(),
            "firm {} ({}) ledger does not sum to zero",
            firm.name,
            handle.0
        );
        for ((product, region), lot) in &firm.inventory {
            assert!(
                lot.qty.0 >= 0,
                "firm {} ({}) holds negative stock of product {} in region {}",
                firm.name,
                handle.0,
                product,
                region
            );
        }
    }
    assert!(world.investor.book.is_balanced(), "investor ledger");
    assert!(
        world.investor.cash_cents >= 0,
        "investor cash went negative"
    );
    assert!(world.government.book.is_balanced(), "government ledger");
    assert!(world.outside.advocacy_book.is_balanced(), "advocacy ledger");
    assert!(
        world.outside.legal_services_book.is_balanced(),
        "legal services ledger"
    );
    assert!(world.outside.gateway_book.is_balanced(), "gateway ledger");
    assert!(
        world.accepted_commands.len() <= 512,
        "accepted command log grew past its bound"
    );
    assert!(
        world.command_nonces.len() <= 4096,
        "command nonce table grew past its bound"
    );
}

#[test]
fn a_mixed_valid_and_invalid_sequence_keeps_every_invariant() {
    const COMMANDS: usize = 540;
    const BATCH: usize = 27;

    let mut session = session(0x0BAD_5EED, ScenarioKind::Guided);
    let mut rng = SplitMix64::new(0x5EED_0BAD_1234_5678);
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut weeks = 0u32;
    let mut keys: BTreeMap<&'static str, usize> = BTreeMap::new();

    for index in 0..COMMANDS {
        let (intent, expected_key) = if index % 3 == 0 {
            let forced = forced_invalid(&mut rng, &session, index / 3);
            (forced.intent, Some(forced.key))
        } else {
            (random_intent(&mut rng, &session), None)
        };
        let command = 0x00AD_BEEF_0000_0000u128 | index as u128;
        let before_revision = session.revision();
        let before_hash = session.canonical_hash();
        let before_log = session.world.accepted_commands.len();
        let before_last_sequence = session
            .world
            .accepted_commands
            .last()
            .map(|row| row.sequence);
        let request = envelope(&session, command, intent);

        match session.submit(request) {
            Ok(receipt) => {
                accepted += 1;
                assert!(
                    expected_key.is_none(),
                    "command {index} should have been rejected with {}",
                    expected_key.unwrap_or_default()
                );
                assert_eq!(
                    session.revision(),
                    before_revision + 1,
                    "command {index}: an acceptance advances the revision exactly once"
                );
                assert_eq!(receipt.revision, session.revision());
                assert_eq!(receipt.accepted_tick, session.world.tick.0);
                assert_eq!(
                    session.world.accepted_commands.len(),
                    (before_log + 1).min(512),
                    "command {index}: exactly one accepted row"
                );
                let row = session
                    .world
                    .accepted_commands
                    .last()
                    .expect("accepted row");
                assert_eq!(row.sequence, receipt.sequence);
                assert_eq!(row.revision, receipt.revision);
                assert!(
                    session.world.command_nonces.contains_key(&command),
                    "command {index}: an accepted id is remembered"
                );
                assert_ne!(
                    session.canonical_hash(),
                    before_hash,
                    "command {index}: the accepted-command log is state, so acceptance moves the hash"
                );
            }
            Err(error) => {
                rejected += 1;
                if let Some(expected) = expected_key {
                    assert_eq!(error.key(), expected, "command {index}: {error:?}");
                }
                *keys.entry(error.key()).or_insert(0) += 1;
                assert!(
                    !matches!(error, DomainError::InternalFault { .. }),
                    "command {index} produced an internal fault: {error:?}"
                );
                assert_eq!(
                    session.revision(),
                    before_revision,
                    "command {index}: a rejection must not advance the revision"
                );
                assert_eq!(
                    session.world.accepted_commands.len(),
                    before_log,
                    "command {index}: a rejection must not append to the accepted log"
                );
                assert_eq!(
                    session
                        .world
                        .accepted_commands
                        .last()
                        .map(|row| row.sequence),
                    before_last_sequence,
                    "command {index}: a rejection must not evict history either"
                );
                assert!(
                    !session.world.command_nonces.contains_key(&command),
                    "command {index}: a rejected id must not be remembered as applied"
                );
                assert_eq!(
                    session.canonical_hash(),
                    before_hash,
                    "command {index}: a rejected command must leave the canonical hash unchanged"
                );
            }
        }

        if (index + 1) % BATCH == 0 {
            answer_mandatory(&mut session);
            let mut result = session.advance(1);
            if result.weeks_advanced == 0 {
                // Advisory pauses (grace, strike, board, deal) are cleared; mandatory cards are
                // answered through the command path, as the app does.
                session.dismiss_pause();
                answer_mandatory(&mut session);
                result = session.advance(1);
            }
            assert_eq!(
                result.weeks_advanced, 1,
                "week {} did not advance; pause {:?}",
                weeks, result.pause
            );
            weeks += 1;
            assert_invariants(&session);
        }
    }

    assert_eq!(weeks, 20, "the sequence must span twenty weeks");
    assert_eq!(session.world.tick.0, 20);
    assert_eq!(accepted + rejected, COMMANDS);
    assert!(
        accepted >= 40,
        "only {accepted} commands were accepted; the mixed sequence is not exercising success"
    );
    assert!(
        rejected >= 40,
        "only {rejected} commands were rejected; the sequence is not adversarial enough"
    );
    // Rejections must arrive as typed domain errors, including every deliberately invalid id.
    for key in [
        "error.invalid_entity",
        "error.arithmetic",
        "error.no_authority",
        "error.solvency_restriction",
        "error.insufficient_cash",
        "error.insufficient_stock",
    ] {
        assert!(
            keys.contains_key(key),
            "no rejection used {key}; observed {keys:?}"
        );
    }
    assert_invariants(&session);
}

#[test]
fn duplicate_command_ids_return_the_first_outcome_and_change_nothing() {
    let mut session = session(0x00D0_0001, ScenarioKind::Guided);
    let primary = session.world.investor.primary_group.expect("guided firm");
    let request = envelope(
        &session,
        0x00D0_0001_0000_0007,
        CommandIntent::SetProductionTarget {
            firm: primary,
            target_bps: 9_000,
            lock: false,
        },
    );

    let first = session.submit(request.clone()).expect("first submission");
    let hash = session.canonical_hash();
    let revision = session.revision();
    let log_len = session.world.accepted_commands.len();
    assert!(log_len > 0);
    assert_eq!(first.revision, revision);

    // The id is remembered, so a replay of the same envelope is a no-op that reports the
    // original acceptance tick. Sequence zero means "no new command was applied".
    let second = session
        .submit(request.clone())
        .expect("duplicate is idempotent");
    assert_eq!(second.sequence, 0);
    assert_eq!(second.accepted_tick, first.accepted_tick);
    assert_eq!(second.revision, revision);
    assert_eq!(session.revision(), revision);
    assert_eq!(session.world.accepted_commands.len(), log_len);
    assert_eq!(
        session.canonical_hash(),
        hash,
        "a duplicate must not change state"
    );

    // The nonce table is part of the world, so deduplication survives export and import.
    let bytes = session.export_save().expect("export");
    let mut restored = Session::import_save(&bytes).expect("import");
    let restored_hash = restored.canonical_hash();
    assert_eq!(restored_hash, hash);
    let third = restored
        .submit(request.clone())
        .expect("duplicate survives a restart");
    assert_eq!(third.sequence, 0);
    assert_eq!(restored.canonical_hash(), restored_hash);

    // Inside the fifty-two week window the original receipt stands; after it, reuse of the same
    // id is refused outright rather than silently reapplied.
    let mut advanced = 0u32;
    while advanced < 53 {
        let result = restored.advance((53 - advanced).min(16));
        if result.weeks_advanced == 0 {
            restored.dismiss_pause();
            continue;
        }
        advanced += result.weeks_advanced;
    }
    let stale_hash = restored.canonical_hash();
    let reused = restored.submit(request);
    assert!(
        matches!(reused, Err(DomainError::DuplicateCommand { .. })),
        "expected a typed duplicate rejection, got {reused:?}"
    );
    assert_eq!(restored.canonical_hash(), stale_hash);
}

#[test]
fn a_stale_revision_is_rejected_without_mutation() {
    let mut session = session(0x0057_A1E0, ScenarioKind::Guided);
    let primary = session.world.investor.primary_group.expect("guided firm");
    let intent = CommandIntent::SetProductionTarget {
        firm: primary,
        target_bps: 5_000,
        lock: false,
    };

    let hash = session.canonical_hash();
    let revision = session.revision();

    // A command prepared against a revision the session has not reached is refused. (There is no
    // older revision than zero; that direction is checked below, after an acceptance.)
    let mut request = envelope(&session, 0x0057_A1E0_0000_0001, intent.clone());
    request.expected_revision = revision + 1;
    match session.submit(request) {
        Err(DomainError::StaleRevision {
            expected: got,
            actual,
        }) => {
            assert_eq!(got, revision + 1);
            assert_eq!(actual, revision);
        }
        other => panic!("expected a stale revision rejection, got {other:?}"),
    }
    assert_eq!(session.revision(), revision);
    assert_eq!(session.canonical_hash(), hash);

    // The same guard applies after a real acceptance, not only at revision zero.
    session
        .submit(envelope(&session, 0x0057_A1E0_0000_0002, intent.clone()))
        .expect("fresh envelope at the current revision");
    let accepted_hash = session.canonical_hash();
    let accepted_revision = session.revision();
    assert_eq!(accepted_revision, revision + 1);
    let mut stale = envelope(&session, 0x0057_A1E0_0000_0003, intent);
    stale.expected_revision = revision;
    assert!(matches!(
        session.submit(stale),
        Err(DomainError::StaleRevision { .. })
    ));
    assert_eq!(session.revision(), accepted_revision);
    assert_eq!(session.canonical_hash(), accepted_hash);
}

/// Byte offsets of the documented save envelope, used only to aim corruption at a field.
struct Layout {
    payload_start: usize,
    declared_at: usize,
    checksum_at: usize,
    declared: usize,
}

fn layout(bytes: &[u8]) -> Layout {
    assert_eq!(&bytes[..4], b"CWSV");
    let mut cursor = 4 + 4 + 4;
    for _ in 0..2 {
        let length = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2 + length;
    }
    cursor += 8; // career id
    cursor += 4; // tick
    cursor += 8; // revision
    let declared_at = cursor;
    let declared = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap()) as usize;
    cursor += 8;
    let checksum_at = cursor;
    cursor += 32;
    Layout {
        payload_start: cursor,
        declared_at,
        checksum_at,
        declared,
    }
}

fn expect_save_fault(outcome: Result<Session, DomainError>, expected: SaveFault, note: &str) {
    match outcome {
        Err(DomainError::InvalidSave { detail }) => {
            assert_eq!(detail, expected, "{note}");
        }
        Err(other) => panic!("{note}: expected InvalidSave({expected:?}), got {other:?}"),
        Ok(_) => panic!("{note}: a corrupt save was accepted"),
    }
}

#[test]
fn corrupt_truncated_mis_sized_and_future_saves_are_typed_rejections() {
    let mut session = session(0x5A7E_0001, ScenarioKind::Guided);
    session.advance(3);
    let exported = session.export_save().expect("export");
    let live_hash = session.canonical_hash();
    let at = layout(&exported);
    // The world payload is what these cuts probe: drop the optional freight read-out trailer
    // that follows it (its own corruption is covered in the save module tests).
    let bytes = exported[..at.payload_start + at.declared].to_vec();
    assert!(at.declared > 128, "fixture payload is too small to cut");

    // Truncation: every length in the last sixty-four bytes, plus interior cuts across the
    // header and the payload.
    for cut in 1..=64 {
        let truncated = &bytes[..bytes.len() - cut];
        expect_save_fault(
            Session::import_save(truncated),
            SaveFault::Truncated,
            &format!("truncated by {cut} bytes"),
        );
    }
    let interior = [
        0,
        1,
        2,
        3,
        4,
        7,
        8,
        11,
        12,
        16,
        24,
        at.declared_at - 1,
        at.declared_at,
        at.checksum_at,
        at.payload_start - 1,
        at.payload_start,
        at.payload_start + 1,
        at.payload_start + 17,
        bytes.len() / 2,
        bytes.len() * 3 / 4,
        bytes.len().saturating_sub(65),
        bytes.len().saturating_sub(101),
    ];
    for cut in interior {
        let truncated = &bytes[..cut];
        if cut < 4 {
            expect_save_fault(
                Session::import_save(truncated),
                SaveFault::BadMagic,
                &format!("truncated to {cut} bytes"),
            );
        } else {
            expect_save_fault(
                Session::import_save(truncated),
                SaveFault::Truncated,
                &format!("truncated to {cut} bytes"),
            );
        }
    }

    // Bit flips: any payload or checksum byte is covered by the BLAKE3 digest, and the magic is
    // checked before anything is parsed.
    for offset in [
        0,
        1,
        2,
        3,
        31,
        32,
        100,
        at.declared / 2,
        at.declared.saturating_sub(33),
        at.declared - 2,
        at.declared - 1,
    ] {
        let mut corrupt = bytes.clone();
        corrupt[at.payload_start + offset] ^= 0x01;
        expect_save_fault(
            Session::import_save(&corrupt),
            SaveFault::Checksum,
            &format!("payload bit flip at {offset}"),
        );
    }
    let mut corrupt = bytes.clone();
    corrupt[at.checksum_at] ^= 0x80;
    expect_save_fault(
        Session::import_save(&corrupt),
        SaveFault::Checksum,
        "checksum bit flip",
    );
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 0xFF;
    expect_save_fault(
        Session::import_save(&corrupt),
        SaveFault::BadMagic,
        "magic bit flip",
    );

    // A lie about the declared payload length: shorter cannot match the checksum, longer cannot
    // fit, and beyond the import cap is refused before allocation.
    for (declared, expected) in [
        ((at.declared - 1) as u64, SaveFault::Checksum),
        (0u64, SaveFault::Checksum),
        ((at.declared + 1) as u64, SaveFault::Truncated),
        (MAX_BYTES as u64 + 1, SaveFault::Oversize),
    ] {
        let mut corrupt = bytes.clone();
        corrupt[at.declared_at..at.declared_at + 8].copy_from_slice(&declared.to_le_bytes());
        expect_save_fault(
            Session::import_save(&corrupt),
            expected,
            &format!("declared payload length {declared}"),
        );
    }

    // A schema from the future, and a byte-level cap beyond the import bound.
    let mut corrupt = bytes.clone();
    corrupt[4..8].copy_from_slice(&(SCHEMA_VERSION + 1).to_le_bytes());
    match Session::import_save(&corrupt) {
        Err(DomainError::UnsupportedVersion { found, supported }) => {
            assert_eq!(found, SCHEMA_VERSION + 1);
            assert_eq!(supported, SCHEMA_VERSION);
        }
        Err(other) => panic!("future schema: expected UnsupportedVersion, got {other:?}"),
        Ok(_) => panic!("future schema: a future save was accepted"),
    }
    let oversized = vec![0u8; MAX_BYTES + 1];
    match Session::import_save(&oversized) {
        Err(DomainError::ResourceLimit { limit, requested }) => {
            assert_eq!(limit, MAX_BYTES as u64);
            assert_eq!(requested, MAX_BYTES as u64 + 1);
        }
        Err(other) => panic!("oversized save: expected ResourceLimit, got {other:?}"),
        Ok(_) => panic!("oversized save: an oversized buffer was imported"),
    }

    // None of the failures above damaged the live session, and the untouched bytes still import.
    assert_eq!(session.canonical_hash(), live_hash);
    let restored = Session::import_save(&bytes).expect("the undamaged save still imports");
    assert_eq!(restored.canonical_hash(), live_hash);
}

#[test]
fn an_unreleased_schema_is_rejected_rather_than_guessed() {
    // Recorded engine behaviour for the migration fixture: schema 0 was never released, so this
    // build has no migration for it. The documented rule is to accept only schemas the engine
    // can read and to migrate older released ones explicitly; with the first release at schema
    // 1, anything below 1 is an unknown format and is rejected instead of being parsed as v1.
    // Schema 2 (1.8 preview 3) migrates schema 1 saves; see tests/save_compat.rs.
    assert_eq!(MIN_SCHEMA_VERSION, 1);
    assert_eq!(SCHEMA_VERSION, 2);

    let mut session = session(0x5C4E_0001, ScenarioKind::Founder);
    session.advance(2);
    let bytes = session.export_save().expect("export");
    let live_hash = session.canonical_hash();

    let mut patched = bytes.clone();
    patched[4..8].copy_from_slice(&0u32.to_le_bytes());
    match Session::import_save(&patched) {
        Err(DomainError::UnsupportedVersion { found, supported }) => {
            assert_eq!(found, 0);
            assert_eq!(supported, SCHEMA_VERSION);
        }
        Err(other) => panic!("schema 0: expected UnsupportedVersion, got {other:?}"),
        Ok(_) => panic!("schema 0: an unreleased schema was imported"),
    }

    // The bytes are otherwise intact: restoring the released schema value imports the same
    // world, so the rejection above is the schema rule and not collateral damage.
    patched[4..8].copy_from_slice(&SCHEMA_VERSION.to_le_bytes());
    let restored = Session::import_save(&patched).expect("released schema imports");
    assert_eq!(restored.canonical_hash(), live_hash);

    // A future schema is refused with the same typed envelope error.
    patched[4..8].copy_from_slice(&(SCHEMA_VERSION + 1).to_le_bytes());
    assert!(matches!(
        Session::import_save(&patched),
        Err(DomainError::UnsupportedVersion { .. })
    ));
}

#[test]
fn save_import_and_advance_is_idempotent() {
    let mut original = session(0x1D0E_0001, ScenarioKind::Founder);
    original.advance(3);
    let bytes = original.export_save().expect("export");

    let mut restored = Session::import_save(&bytes).expect("import");
    assert_eq!(
        restored.canonical_hash(),
        original.canonical_hash(),
        "import must reproduce the exported state exactly"
    );
    assert_eq!(restored.world.tick.0, original.world.tick.0);
    assert_eq!(restored.revision(), original.revision());

    // Continuing both handles one week must reach the same authoritative state, including the
    // rival commands the session applies on the same schedule.
    let left = original.advance(1);
    let right = restored.advance(1);
    assert_eq!(left.weeks_advanced, 1);
    assert_eq!(right.weeks_advanced, 1);
    assert_eq!(
        original.canonical_hash(),
        restored.canonical_hash(),
        "a resumed save diverged from the uninterrupted run"
    );

    // A second import of the same bytes is equally deterministic.
    let mut again = Session::import_save(&bytes).expect("second import");
    again.advance(1);
    assert_eq!(again.canonical_hash(), original.canonical_hash());
}

#[test]
fn long_careers_keep_history_bounded_and_lifetime_figures_consistent() {
    // 110 weeks crosses eight quarterly report boundaries and two full journal windows, so the
    // posting journal, sales history and weekly closes are all pruning by the end.
    const WEEKS: u32 = 110;
    // Observed envelope: the busiest firm holds about 1,665 postings here, thirteen weeks at
    // roughly 128 postings a week now that rivals grow to more plants and trade on the corridor
    // exchange. Without pruning the same firm would pass ten thousand by week 110, so this still
    // catches an accidental removal of compaction.
    // The steel firm now opens with six plants in four regions (1.7 preview 2) and its postings
    // per week grow with its plants, so the bound moved to 3,600; unpruned it still passes ten
    // thousand.
    const JOURNAL_ENVELOPE: usize = 3_600;

    let mut session = session(0x00C0_5A7E, ScenarioKind::Founder);
    let mut totals: BTreeMap<u64, (i64, i64)> = BTreeMap::new();
    let mut peak_journal = 0usize;

    for week in 1..=WEEKS {
        let before: BTreeMap<u64, (i64, i64)> = session
            .world
            .firms
            .iter()
            .map(|(handle, firm)| {
                let lifetime = firm.book.lifetime();
                (handle.0, (lifetime.revenue, lifetime.net_income))
            })
            .collect();
        let result = session.advance(1);
        assert_eq!(
            result.weeks_advanced, 1,
            "week {week} did not advance; pause {:?}",
            result.pause
        );
        assert_eq!(session.world.tick.0, week);

        for (handle, firm) in session.world.firms.iter() {
            let lifetime = firm.book.lifetime();
            let prior = before.get(&handle.0).copied().unwrap_or((0, 0));
            let close = firm
                .book
                .recent_weeks()
                .last()
                .copied()
                .expect("every week closes the books");
            assert_eq!(close.week.0, week, "close week for firm {}", handle.0);
            // Independent oracle: lifetime totals are exactly the sum of the reported weekly
            // closes, and revenue never falls because weekly revenue is non-negative. Net income
            // legitimately falls in a loss week, so it is reconciled rather than asserted
            // monotonic.
            assert_eq!(
                lifetime.revenue - prior.0,
                close.revenue,
                "firm {} lifetime revenue did not move by the reported close",
                handle.0
            );
            assert_eq!(
                lifetime.net_income - prior.1,
                close.net_income,
                "firm {} lifetime net income did not move by the reported close",
                handle.0
            );
            assert!(
                lifetime.revenue >= prior.0,
                "firm {} lifetime revenue fell in week {week}",
                handle.0
            );
            assert!(firm.book.is_balanced(), "firm {} ledger", handle.0);
            for ((product, region), lot) in &firm.inventory {
                assert!(
                    lot.qty.0 >= 0,
                    "firm {} negative stock {}:{}",
                    handle.0,
                    product,
                    region
                );
            }
            // Bounded history: detailed postings never reach past the thirteen-week window, and
            // both rolling histories stay capped.
            for posting in firm.book.journal() {
                assert!(
                    posting.week.0 + 12 >= week,
                    "firm {} retained a posting from week {} at week {week}",
                    handle.0,
                    posting.week.0
                );
            }
            assert!(firm.book.recent_weeks().len() <= 13);
            assert!(firm.sales_history.len() <= 13);
            peak_journal = peak_journal.max(firm.book.journal_len());

            let entry = totals.entry(handle.0).or_insert((0, 0));
            entry.0 += close.revenue;
            entry.1 += close.net_income;
        }
    }

    assert!(
        peak_journal <= JOURNAL_ENVELOPE,
        "journal grew to {peak_journal} postings, past the {JOURNAL_ENVELOPE} envelope"
    );
    assert!(
        session.world.accepted_commands.len() <= 512,
        "accepted command log is unbounded"
    );

    // Lifetime figures reconcile with the independently accumulated weekly closes.
    let mut revenue_total = 0i64;
    for (handle, firm) in session.world.firms.iter() {
        let lifetime = firm.book.lifetime();
        let accumulated = totals.get(&handle.0).copied().unwrap_or((0, 0));
        assert_eq!(
            accumulated.0, lifetime.revenue,
            "firm {} lifetime revenue does not reconcile",
            handle.0
        );
        assert_eq!(
            accumulated.1, lifetime.net_income,
            "firm {} lifetime net income does not reconcile",
            handle.0
        );
        for close in firm.book.recent_weeks() {
            assert_eq!(
                close.net_income,
                close.revenue
                    - close.cost_of_goods
                    - close.wages
                    - close.operating_expense
                    - close.depreciation
                    - close.interest_expense
                    + close.interest_income
                    - close.tax
                    + close.other,
                "firm {} weekly profit and loss bridge",
                handle.0
            );
        }
        revenue_total += lifetime.revenue;
    }
    assert!(
        revenue_total > 0,
        "a 110-week career produced no revenue; the fixture is not exercising the economy"
    );
    // Compaction must not break the save: the round trip reproduces the hash, and the envelope
    // stays small because old postings are folded into the chained hash.
    let bytes = session.export_save().expect("export after compaction");
    // The history archive rides in its own trailer with fixed caps (ten years of closes, a year
    // of weekly and forty years of quarterly statements), so it is bounded on its own and left
    // out of the compaction envelope.
    let archive = postcard::to_allocvec(&session.world.archive)
        .expect("archive encodes")
        .len();
    assert!(
        archive < 1024 * 1024,
        "the history archive grew to {archive} bytes in 110 weeks"
    );
    // The systems history is capped at five years of weeks; at 110 weeks it must stay small
    // enough that 260 weeks fits in about two megabytes.
    let systems = postcard::to_allocvec(&session.world.systems)
        .expect("systems history encodes")
        .len();
    assert!(
        systems * 260 / 110 < 2 * 1024 * 1024,
        "the systems history grew to {systems} bytes in 110 weeks"
    );
    let envelope = bytes.len() - archive - systems;
    assert!(
        // The corridor keeps forty-plus renewable supply contracts and thirteen weeks of sales
        // history per firm, so the observed envelope is larger than the original empty-world
        // fixture. The real bound is that it stays far below the 64 MB import cap and does not
        // grow with career length.
        envelope < 2 * 1024 * 1024,
        "compacted save grew to {envelope} bytes"
    );
    let restored = Session::import_save(&bytes).expect("import after compaction");
    assert_eq!(restored.canonical_hash(), session.canonical_hash());
}
