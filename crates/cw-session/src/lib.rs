//! Session orchestration.
//!
//! The session owns one mutable world and serialises all mutation: commands, AI proposals and
//! weekly advances. It exposes immutable read models, a versioned save envelope and a replay log.
//! Swift keeps file I/O and lifecycle; it never computes a financial identity.

pub mod dto;
pub mod save;

use cw_core::error::DomainError;
use cw_data::{Dataset, Difficulty};
use cw_sim::commands::{CommandEnvelope, CommandIntent, CommandReceipt, Preview, apply};
use cw_sim::state::{ScenarioKind, ScenarioMode, World};
use serde::{Deserialize, Serialize};

pub use dto::*;

pub const MAX_ADVANCE_WEEKS: u32 = 16;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub seed: u64,
    pub scenario: ScenarioKind,
    pub mode: ScenarioMode,
    pub dataset_hash: String,
    pub difficulty: Difficulty,
}

#[derive(Clone, Debug)]
pub struct AdvanceResult {
    pub weeks_advanced: u32,
    pub revision: u64,
    pub pause: Option<String>,
    pub final_week: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayLog {
    pub config: ScenarioConfig,
    pub commands: Vec<RecordedCommand>,
    pub advance_calls: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordedCommand {
    pub actor: u64,
    pub high: u64,
    pub low: u64,
    pub expected_revision: u64,
    pub submitted_tick: u32,
    pub intent_key: String,
    pub intent: String,
}

#[derive(Clone, Debug)]
pub enum Query {
    Overview,
    Firms,
    Firm(u64),
    Facilities(u64),
    Facility(u64),
    Regions(Option<String>),
    Finance(u64),
    Ownership(u64),
    Contracts(u64),
    Loans(u64),
    Insurance(u64),
    Deals,
    Events,
    Market(u8, u16),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Detail {
    Overview(Overview),
    Firms(Vec<FirmSummary>),
    Firm(FirmSummary),
    Facilities(Vec<FacilitySummary>),
    Facility(FacilitySummary),
    Regions(Vec<RegionSummary>),
    Finance(FinanceSummary),
    Ownership(OwnershipSummary),
    Contracts(Vec<ContractLine>),
    Loans(Vec<LoanLine>),
    Insurance(Vec<InsuranceLine>),
    Deals(Vec<DealLine>),
    Events(Vec<EventLine>),
    Market(MarketLine),
}

/// Namespace marker for rival proposals. The low half is the persisted command sequence.
pub const AI_COMMAND_HIGH: u64 = 0x00C0_FFEE_0000_0000;

pub struct Session {
    pub dataset: Dataset,
    pub world: World,
    pub career_id: u64,
    pub config: ScenarioConfig,
    log: ReplayLog,
}

impl Session {
    pub fn create(config: ScenarioConfig) -> Result<Session, DomainError> {
        let dataset = Dataset::load().map_err(|_| DomainError::InternalFault { diagnostic: 1 })?;
        if !config.dataset_hash.is_empty() && config.dataset_hash != dataset.hash {
            return Err(DomainError::InvalidSave {
                detail: cw_core::error::SaveFault::MalformedPayload,
            });
        }
        let world =
            cw_sim::economy::seed::build(&dataset, config.seed, config.scenario, config.mode);
        let career_id = cw_core::rng::scramble(config.seed) & 0x00FF_FFFF_FFFF_FFFF;
        let log = ReplayLog {
            config: config.clone(),
            commands: Vec::new(),
            advance_calls: Vec::new(),
        };
        Ok(Session {
            dataset,
            world,
            career_id,
            config,
            log,
        })
    }

    pub fn dataset_hash(&self) -> &str {
        &self.dataset.hash
    }

    pub fn revision(&self) -> u64 {
        self.world.revision
    }

    pub fn canonical_hash(&self) -> String {
        cw_sim::state::canonical_hash(&self.world)
    }

    /// Submit one command. A duplicate command ID returns the original receipt.
    pub fn submit(&mut self, envelope: CommandEnvelope) -> Result<CommandReceipt, DomainError> {
        let key = ((envelope.command_id_high as u128) << 64) | envelope.command_id_low as u128;
        if let Some(tick) = self.world.command_nonces.get(&key) {
            if self.world.tick.0 <= tick.0 + 52 {
                return Ok(CommandReceipt {
                    sequence: 0,
                    accepted_tick: tick.0,
                    revision: self.world.revision,
                });
            }
            return Err(DomainError::DuplicateCommand { command: key });
        }
        let intent_text = serde_json::to_string(&envelope.intent).unwrap_or_default();
        let receipt = apply(&mut self.world, &self.dataset, &envelope)?;
        self.world.command_nonces.insert(key, self.world.tick);
        if self.world.command_nonces.len() > 4096 {
            let cutoff = self.world.tick.0.saturating_sub(52);
            self.world.command_nonces.retain(|_, tick| tick.0 >= cutoff);
        }
        self.log.commands.push(RecordedCommand {
            actor: envelope.actor,
            high: envelope.command_id_high,
            low: envelope.command_id_low,
            expected_revision: envelope.expected_revision,
            submitted_tick: envelope.submitted_tick,
            intent_key: envelope.intent.key().to_owned(),
            intent: intent_text,
        });
        Ok(receipt)
    }

    /// Never reserves or mutates. Submit still validates.
    pub fn preview(&self, actor: u64, intent: &CommandIntent) -> Preview {
        let _ = actor;
        match intent {
            CommandIntent::BuildFacility {
                firm,
                recipe,
                blocks,
                ..
            } => {
                let cash = self
                    .world
                    .firms
                    .get(*firm)
                    .map(|f| f.book.cash())
                    .unwrap_or(0);
                cw_sim::commands::preview_build(&self.dataset, *recipe, *blocks, cash)
            }
            CommandIntent::IssueShares {
                firm,
                qty,
                price_cents,
                ..
            } => cw_sim::commands::preview_issue(&self.world, *firm, *qty, *price_cents),
            CommandIntent::PayDividend {
                firm: _,
                amount_cents,
            } => Preview {
                revision: self.world.revision,
                cash_change_cents: -*amount_cents,
                assumptions: vec!["paid from distributable retained earnings".to_owned()],
                warnings: Vec::new(),
                blocked: None,
            },
            _ => Preview {
                revision: self.world.revision,
                ..Default::default()
            },
        }
    }

    /// Advance the clock, applying rival proposals through the common validator first.
    pub fn advance(&mut self, max_weeks: u32) -> AdvanceResult {
        let weeks = max_weeks.min(MAX_ADVANCE_WEEKS);
        self.log.advance_calls.push(weeks);
        let mut advanced = 0;
        let mut pause = None;
        for _ in 0..weeks {
            if let Some(reason) = self.world.pending_pause.clone() {
                pause = Some(reason);
                break;
            }
            self.run_ai();
            let outcome = cw_sim::tick::advance(&mut self.world, &self.dataset);
            advanced += 1;
            if let Some(reason) = outcome.pause {
                pause = Some(reason);
                break;
            }
        }
        AdvanceResult {
            weeks_advanced: advanced,
            revision: self.world.revision,
            pause,
            final_week: self.world.tick.0,
        }
    }

    fn run_ai(&mut self) {
        let proposals = cw_ai::propose(&self.world, &self.dataset);
        for proposal in proposals {
            // The command identity derives from the persisted sequence counter, never a
            // session-local counter: a resumed save must issue the same ids, and the same
            // revision, as an uninterrupted run. A rejected proposal consumes no sequence,
            // and it is never recorded, so reusing that id is harmless.
            let envelope = CommandEnvelope {
                command_id_high: AI_COMMAND_HIGH,
                command_id_low: self.world.next_sequence,
                actor: proposal.actor,
                expected_revision: self.world.revision,
                submitted_tick: self.world.tick.0,
                intent: proposal.intent,
            };
            let _ = apply(&mut self.world, &self.dataset, &envelope);
        }
    }

    pub fn overview(&self) -> Overview {
        let world = &self.world;
        let net_worth = cw_sim::economy::score::personal_net_worth(world);
        let primary = world
            .investor
            .primary_group
            .filter(|handle| world.firms.contains(*handle));
        let (name, cash, forecast, ebit) = match primary {
            Some(handle) => {
                let firm = world.firms.get(handle).unwrap();
                let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, handle);
                let revenue: i64 = firm
                    .book
                    .recent_weeks()
                    .iter()
                    .map(|week| week.revenue)
                    .sum::<i64>()
                    / firm.book.recent_weeks().len().max(1) as i64;
                let ebit = firm
                    .book
                    .recent_weeks()
                    .last()
                    .map(|week| week.ebit())
                    .unwrap_or(0);
                (
                    firm.name.clone(),
                    firm.book.cash(),
                    firm.book.cash() - outflow * 4 + revenue,
                    ebit,
                )
            }
            None => (String::new(), 0, 0, 0),
        };
        let mut alerts = Vec::new();
        for (handle, firm) in world.firms.iter() {
            if firm.shares.qty(cw_sim::state::HolderId::Investor) == 0 {
                continue;
            }
            if matches!(
                firm.status,
                cw_sim::state::FirmStatus::PastDue { .. }
                    | cw_sim::state::FirmStatus::Restructuring { .. }
            ) {
                alerts.push(Alert {
                    key: "alert.past_due".to_owned(),
                    subject: firm.name.clone(),
                    severity: 3,
                });
            }
            if firm.payout_freeze {
                alerts.push(Alert {
                    key: "alert.payout_frozen".to_owned(),
                    subject: firm.name.clone(),
                    severity: 2,
                });
            }
            let _ = handle;
        }
        for event in dto::events(world, &self.dataset) {
            if event.mandatory {
                alerts.push(Alert {
                    key: "alert.decision".to_owned(),
                    subject: event.title,
                    severity: 2,
                });
            }
        }
        Overview {
            tick: world.tick.0,
            week_label: world.tick.label(),
            revision: world.revision,
            investor_cash_cents: world.investor.cash_cents,
            investor_net_worth_cents: net_worth,
            primary_firm: primary.map(|handle| handle.0),
            primary_name: name,
            firm_cash_cents: cash,
            forecast_four_week_cents: forecast,
            last_week_ebit_cents: ebit,
            alerts,
            pause: world.pending_pause.clone(),
        }
    }

    pub fn query(&self, query: Query) -> Detail {
        let world = &self.world;
        match query {
            Query::Overview => Detail::Overview(self.overview()),
            Query::Firms => Detail::Firms(
                world
                    .firms
                    .handles_sorted()
                    .into_iter()
                    .filter_map(|handle| dto::firm_summary(world, &self.dataset, handle.0))
                    .collect(),
            ),
            Query::Firm(handle) => Detail::Firm(
                dto::firm_summary(world, &self.dataset, handle)
                    .unwrap_or_else(|| empty_firm(handle)),
            ),
            Query::Facilities(firm) => Detail::Facilities(
                world
                    .firms
                    .get(cw_core::ids::Handle(firm))
                    .map(|entity| {
                        entity
                            .facilities
                            .iter()
                            .filter_map(|handle| {
                                dto::facility_summary(world, &self.dataset, handle.0)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            Query::Facility(handle) => Detail::Facility(
                dto::facility_summary(world, &self.dataset, handle)
                    .unwrap_or_else(|| empty_facility(handle)),
            ),
            Query::Regions(selected) => {
                let from = selected
                    .and_then(|code| world.region_index(&code))
                    .map(|index| index as usize);
                Detail::Regions(
                    (0..world.regions.len())
                        .map(|index| dto::region_summary(world, &self.dataset, index, from))
                        .collect(),
                )
            }
            Query::Finance(handle) => Detail::Finance(
                dto::finance_summary(world, handle).unwrap_or_else(|| empty_finance(handle)),
            ),
            Query::Ownership(handle) => Detail::Ownership(
                dto::ownership_summary(world, handle).unwrap_or_else(|| empty_ownership(handle)),
            ),
            Query::Contracts(firm) => Detail::Contracts(dto::contracts(world, &self.dataset, firm)),
            Query::Loans(firm) => Detail::Loans(dto::loans(world, firm)),
            Query::Insurance(firm) => Detail::Insurance(dto::insurance(world, firm)),
            Query::Deals => Detail::Deals(dto::deals(world)),
            Query::Events => Detail::Events(dto::events(world, &self.dataset)),
            Query::Market(region, product) => {
                Detail::Market(dto::market(world, &self.dataset, region, product))
            }
        }
    }

    pub fn export_save(&self) -> Result<Vec<u8>, DomainError> {
        save::encode(&self.world, self.career_id)
    }

    /// Import into a new session. The current session is untouched on failure.
    pub fn import_save(bytes: &[u8]) -> Result<Session, DomainError> {
        if bytes.len() > save::MAX_BYTES {
            return Err(DomainError::ResourceLimit {
                limit: save::MAX_BYTES as u64,
                requested: bytes.len() as u64,
            });
        }
        let (world, career_id) = save::decode(bytes)?;
        let dataset = Dataset::load().map_err(|_| DomainError::InternalFault { diagnostic: 2 })?;
        if world.dataset_hash != dataset.hash {
            return Err(DomainError::InvalidSave {
                detail: cw_core::error::SaveFault::MalformedPayload,
            });
        }
        let config = ScenarioConfig {
            seed: world.seed,
            scenario: world.scenario,
            mode: world.mode,
            dataset_hash: world.dataset_hash.clone(),
            difficulty: world.difficulty,
        };
        let log = ReplayLog {
            config: config.clone(),
            commands: Vec::new(),
            advance_calls: Vec::new(),
        };
        Ok(Session {
            dataset,
            world,
            career_id,
            config,
            log,
        })
    }

    pub fn export_replay(&self) -> String {
        serde_json::to_string(&self.log).unwrap_or_default()
    }

    /// Replay a recorded log from a fresh scenario and return the canonical hash.
    pub fn replay(log: &ReplayLog) -> Result<String, DomainError> {
        let mut session = Session::create(log.config.clone())?;
        for command in &log.commands {
            let intent = decode_intent(&command.intent).ok_or(DomainError::InvalidSave {
                detail: cw_core::error::SaveFault::MalformedPayload,
            })?;
            let envelope = CommandEnvelope {
                command_id_high: command.high,
                command_id_low: command.low,
                actor: command.actor,
                expected_revision: command.expected_revision,
                submitted_tick: command.submitted_tick,
                intent,
            };
            session.submit(envelope)?;
        }
        for weeks in &log.advance_calls {
            session.advance(*weeks);
        }
        Ok(session.canonical_hash())
    }
}

fn decode_intent(text: &str) -> Option<CommandIntent> {
    serde_json::from_str(text).ok()
}

fn empty_firm(handle: u64) -> FirmSummary {
    FirmSummary {
        handle,
        name: String::new(),
        industry: String::new(),
        region: String::new(),
        cash_cents: 0,
        equity_cents: 0,
        fair_value_cents: 0,
        mark_cents: 0,
        grade: "D".to_owned(),
        status: "unknown".to_owned(),
        investor_votes_bps: 0,
        controlled: false,
        ceo_is_investor: false,
        facilities: 0,
    }
}

fn empty_facility(handle: u64) -> FacilitySummary {
    FacilitySummary {
        handle,
        firm: 0,
        recipe: String::new(),
        blocks: 0,
        region: String::new(),
        status: "unknown".to_owned(),
        capacity: 0,
        last_output: 0,
        workers: [0; 3],
        bottleneck: String::new(),
        deposit_remaining: 0,
        target_bps: 0,
    }
}

fn empty_finance(handle: u64) -> FinanceSummary {
    FinanceSummary {
        firm: handle,
        cash_cents: 0,
        restricted_cents: 0,
        inventory_cents: 0,
        assets_cents: 0,
        liabilities_cents: 0,
        equity_cents: 0,
        revenue_trailing_cents: 0,
        ebitda_trailing_cents: 0,
        net_income_trailing_cents: 0,
        cash_bridge: CashBridge::default(),
        recent_weeks: Vec::new(),
    }
}

fn empty_ownership(handle: u64) -> OwnershipSummary {
    OwnershipSummary {
        firm: handle,
        issued: 0,
        treasury: 0,
        eligible_votes: 0,
        controller: None,
        holders: Vec::new(),
        board_seats: std::array::from_fn(|_| None),
        next_meeting: 0,
        ceo: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(seed: u64) -> ScenarioConfig {
        ScenarioConfig {
            seed,
            scenario: ScenarioKind::Guided,
            mode: ScenarioMode::FiveYear,
            dataset_hash: String::new(),
            difficulty: Difficulty::Standard,
        }
    }

    #[test]
    fn a_new_session_has_a_real_overview() {
        let session = Session::create(config(171)).unwrap();
        let overview = session.overview();
        assert_eq!(overview.tick, 0);
        assert!(overview.investor_cash_cents > 0);
        assert!(!overview.primary_name.is_empty());
    }

    #[test]
    fn advancing_produces_a_new_week_and_a_receipt() {
        let mut session = Session::create(config(172)).unwrap();
        let result = session.advance(4);
        assert_eq!(result.weeks_advanced, 4);
        assert_eq!(session.world.tick.0, 4);
    }

    #[test]
    fn a_save_round_trip_keeps_the_hash() {
        let mut session = Session::create(config(173)).unwrap();
        session.advance(3);
        let bytes = session.export_save().unwrap();
        let restored = Session::import_save(&bytes).unwrap();
        assert_eq!(session.canonical_hash(), restored.canonical_hash());
        assert_eq!(restored.world.tick.0, 3);
    }

    #[test]
    fn a_corrupt_save_does_not_destroy_the_session() {
        let mut session = Session::create(config(174)).unwrap();
        session.advance(2);
        let mut bytes = session.export_save().unwrap();
        let index = bytes.len() - 3;
        bytes[index] ^= 0xFF;
        assert!(Session::import_save(&bytes).is_err());
        assert_eq!(session.world.tick.0, 2);
    }

    #[test]
    fn the_cross_platform_fixture_hash_is_pinned() {
        // This exact sequence runs in the iOS XCTest bridge. Equal hashes on Linux and iOS are
        // the cross-platform determinism evidence for this scenario.
        //
        // 2026-10-25: the value moved from 8d66f18... when rival command ids stopped using a
        // session-local counter and started using the persisted `world.next_sequence`. The old
        // scheme restarted at 1 after a save/import, so a resumed career issued ids that were
        // already in the accepted-command log (see `save_import_and_advance_is_idempotent` in
        // tests/adversarial.rs). The economy is unchanged; only the recorded command identity
        // is. The Swift constant must be updated in the same change and re-verified on the Mac
        // runner before R05 claims cross-platform equality again.
        let mut session = Session::create(config(42)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        let price = session.dataset.reference_price(0);
        session
            .submit(cw_sim::commands::CommandEnvelope {
                command_id_high: 0x5357_4946_5400_0000,
                command_id_low: 0,
                actor: 0,
                expected_revision: session.revision(),
                submitted_tick: 0,
                intent: CommandIntent::SetPrice {
                    firm: primary,
                    product: 0,
                    price_cents: price,
                },
            })
            .unwrap();
        session.advance(2);
        let hash = session.canonical_hash();
        assert_eq!(
            hash,
            "7b781159830184e8928f54e79949e343a1fd762ada357c235e42bd6f1b5fff4c"
        );
    }

    #[test]
    fn batching_does_not_change_the_result() {
        let mut single = Session::create(config(176)).unwrap();
        let mut batched = Session::create(config(176)).unwrap();
        single.advance(1);
        single.advance(1);
        batched.advance(2);
        assert_eq!(single.canonical_hash(), batched.canonical_hash());
    }

    #[test]
    fn a_recorded_log_replays_to_the_same_hash() {
        let mut session = Session::create(config(177)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        let price = session.dataset.reference_price(0);
        session
            .submit(cw_sim::commands::CommandEnvelope {
                command_id_high: 177,
                command_id_low: 1,
                actor: 0,
                expected_revision: session.revision(),
                submitted_tick: 0,
                intent: CommandIntent::SetPrice {
                    firm: primary,
                    product: 0,
                    price_cents: price,
                },
            })
            .unwrap();
        session.advance(3);
        let log_text = session.export_replay();
        let log: ReplayLog = serde_json::from_str(&log_text).unwrap();
        let replayed = Session::replay(&log).unwrap();
        assert_eq!(replayed, session.canonical_hash());
    }

    #[test]
    fn duplicate_command_ids_are_idempotent() {
        let mut session = Session::create(config(175)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        let envelope = CommandEnvelope {
            command_id_high: 7,
            command_id_low: 9,
            actor: 0,
            expected_revision: session.revision(),
            submitted_tick: 0,
            intent: CommandIntent::SetProductionTarget {
                firm: primary,
                target_bps: 9_000,
            },
        };
        session.submit(envelope.clone()).unwrap();
        let before = session.revision();
        let receipt = session.submit(envelope).unwrap();
        assert_eq!(receipt.sequence, 0);
        assert_eq!(session.revision(), before);
    }
}
