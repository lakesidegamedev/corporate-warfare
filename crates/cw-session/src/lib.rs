//! Session orchestration.
//!
//! The session owns one mutable world and serialises all mutation: commands, AI proposals and
//! weekly advances. It exposes immutable read models, a versioned save envelope and a replay log.
//! Swift keeps file I/O and lifecycle; it never computes a financial identity.

pub mod cfo;
pub mod dto;
pub mod save;
pub mod telemetry;

use std::collections::BTreeMap;

use cw_core::error::DomainError;
use cw_core::ids::Handle;
use cw_core::money::round_even_div;
use cw_data::{Dataset, Difficulty};
use cw_sim::commands::{CommandEnvelope, CommandIntent, CommandReceipt, Preview, apply};
use cw_sim::state::{ScenarioKind, ScenarioMode, World};
use serde::{Deserialize, Serialize};

pub use dto::*;

pub const MAX_ADVANCE_WEEKS: u32 = 16;

/// A labelled cash preview for an intent whose settlement price is already explicit.
fn cash_preview(revision: u64, cash_change_cents: i64, assumptions: Vec<String>) -> Preview {
    Preview {
        revision,
        cash_change_cents,
        assumptions,
        warnings: Vec::new(),
        blocked: None,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub seed: u64,
    pub scenario: ScenarioKind,
    pub mode: ScenarioMode,
    pub dataset_hash: String,
    pub difficulty: Difficulty,
    #[serde(default)]
    pub economy_start: cw_sim::state::EconomyStart,
    #[serde(default)]
    pub starting_capital_cents: Option<i64>,
    /// Which scenario's world price paths to use, and how they move.
    #[serde(default)]
    pub prices: PriceSetup,
}

/// World commodity price setup for a new career. An empty scenario id means no world prices
/// (used by engine tests); a scenario with no authored paths runs the neutral random walk.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PriceSetup {
    pub scenario_id: String,
    /// None takes the scenario's default mode.
    pub path: Option<cw_data::PricePath>,
    /// Whether the scenario's dated history plays. None takes the default, Historical.
    #[serde(default)]
    pub timeline: Option<cw_sim::economy::timeline::TimelineMode>,
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

// One overview is built per query and returned by value, so boxing it would only add a hop.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Detail {
    /// The caller has no management right or completed diligence for this entity.
    Restricted,
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
    /// The custom scenario this career started from, kept in saves.
    pub custom: Option<cw_data::CustomScenario>,
    log: ReplayLog,
    /// Rival command outcomes for this process. Diagnostic only: never saved or hashed.
    pub ai_log: AiLog,
    /// The currency the investor's figures are shown in. A display choice: never saved or
    /// hashed, and the numeraire (0) until the player picks another.
    pub reporting_currency: u8,
    /// Which pauses stop the clock. A play preference: never saved or hashed.
    pub pause_policy: dto::PausePolicy,
}

/// Accepted and rejected rival commands by archetype, with rejection reasons. Rejected
/// proposals are logged here and never mutate the world.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AiLog {
    pub by_archetype: BTreeMap<String, ArchetypeActions>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ArchetypeActions {
    pub accepted: u64,
    pub rejected: u64,
    pub accepted_by_command: BTreeMap<String, u64>,
    pub rejected_by_reason: BTreeMap<String, u64>,
}

impl Session {
    pub fn create(config: ScenarioConfig) -> Result<Session, DomainError> {
        Self::create_with(config, None)
    }

    /// A career from a custom scenario. Its difficulty, economy and capital replace the
    /// config's; its rivals, regional levels and opening laws shape the starting world. The
    /// scenario is validated first and refused with a plain-words reason if it cannot be played.
    pub fn create_custom(
        mut config: ScenarioConfig,
        custom: &cw_data::CustomScenario,
    ) -> Result<Session, String> {
        let dataset = Dataset::load_scenario(&custom.base_scenario_id)
            .map_err(|_| "The game data could not be loaded".to_owned())?;
        let errors = custom.validate(&dataset);
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
        config.difficulty = Difficulty::from_key(&custom.difficulty);
        config.economy_start = cw_sim::state::EconomyStart::from_key(&custom.economy);
        config.starting_capital_cents = custom.starting_capital_cents;
        config.prices = PriceSetup {
            scenario_id: custom.base_scenario_id.clone(),
            path: Some(custom.price_path),
            timeline: config.prices.timeline,
        };
        Self::create_with(config, Some(custom)).map_err(|e| format!("{e:?}"))
    }

    /// The dataset a career plays on. A config that names the hash it was recorded on gets the
    /// bundled dataset with that hash; a new career gets the dataset of the scenario its price
    /// setup names, and the default scenario's when it names none.
    fn dataset_for(config: &ScenarioConfig) -> Result<Dataset, DomainError> {
        let fault = || DomainError::InternalFault { diagnostic: 1 };
        if !config.dataset_hash.is_empty() {
            return Dataset::for_hash(&config.dataset_hash).ok_or_else(fault);
        }
        Dataset::load_scenario(&config.prices.scenario_id).map_err(|_| fault())
    }

    fn create_with(
        config: ScenarioConfig,
        custom: Option<&cw_data::CustomScenario>,
    ) -> Result<Session, DomainError> {
        let mut dataset = Self::dataset_for(&config)?;
        if !config.dataset_hash.is_empty() && config.dataset_hash != dataset.hash {
            return Err(DomainError::InvalidSave {
                detail: cw_core::error::SaveFault::MalformedPayload,
            });
        }
        let options = cw_sim::state::StartOptions {
            difficulty: config.difficulty,
            economy: config.economy_start,
            capital_cents: config.starting_capital_cents,
        };
        if let Some(custom) = custom {
            custom.apply_to(&mut dataset);
        }
        let mut world = cw_sim::economy::seed::build_with(
            &dataset,
            config.seed,
            config.scenario,
            config.mode,
            &options,
        );
        if !config.prices.scenario_id.is_empty() {
            let id = &config.prices.scenario_id;
            let path = config.prices.path.unwrap_or_else(|| {
                cw_data::scenarios()
                    .iter()
                    .find(|scenario| &scenario.id == id)
                    .map(|scenario| scenario.price_path())
                    .unwrap_or_default()
            });
            cw_sim::economy::world_prices::init(
                &mut world,
                &dataset,
                &cw_data::price_paths::PricePathFile::load(),
                id,
                path,
                config.seed,
            );
        }
        if !config.prices.scenario_id.is_empty() {
            cw_sim::economy::timeline::init(
                &mut world,
                &dataset,
                &config.prices.scenario_id,
                config
                    .prices
                    .timeline
                    .unwrap_or(cw_sim::economy::timeline::TimelineMode::Historical),
                config.seed,
            );
        }
        if !config.prices.scenario_id.is_empty() {
            // The scenario's own people take the posts they fit.
            cw_sim::economy::cast::assign(&mut world, &dataset, &config.prices.scenario_id);
        }
        if let Some(laws) = custom.and_then(|c| c.laws.as_ref()) {
            world.politics.laws = cw_sim::state::Laws {
                corporate_tax_bps: laws.corporate_tax_bps,
                wage_floor_cents: laws.wage_floor_cents,
                merger_review_bps: laws.merger_review_bps,
                unions: match laws.unions.as_str() {
                    "weak" => cw_sim::state::UnionLaw::Weak,
                    "strong" => cw_sim::state::UnionLaw::Strong,
                    _ => cw_sim::state::UnionLaw::Standard,
                },
                ..world.politics.laws
            };
        }
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
            custom: custom.cloned(),
            log,
            ai_log: AiLog::default(),
            reporting_currency: 0,
            pause_policy: dto::PausePolicy::default(),
        })
    }

    pub fn dataset_hash(&self) -> &str {
        &self.dataset.hash
    }

    /// Continue past a pause that is not a card waiting for an answer. A card pause is cleared
    /// by answering it through `RespondEvent`; a pause whose card is gone, answered or no longer
    /// the player's to answer is cleared here too, so nothing can leave the clock stopped.
    pub fn dismiss_pause(&mut self) {
        self.heal_pause();
        let advisory = self
            .world
            .pending_pause
            .as_deref()
            .map(|reason| !reason.starts_with("event:"))
            .unwrap_or(false);
        if advisory {
            self.world.pending_pause = None;
        }
    }

    /// Choose which pauses stop the clock.
    pub fn set_pause_policy(&mut self, policy: dto::PausePolicy) {
        self.pause_policy = policy;
        self.heal_pause();
    }

    /// Drop a pause that can no longer be acted on: a card pause whose card is answered, retired,
    /// missing from the deck, without choices or now belonging to a rival, and a warning pause the
    /// player has asked not to be stopped for.
    fn heal_pause(&mut self) {
        let Some(reason) = self.world.pending_pause.clone() else {
            return;
        };
        let keep = if let Some(id) = reason.strip_prefix("event:") {
            id.parse::<u64>()
                .map(|id| self.card_pause_is_live(id))
                .unwrap_or(false)
        } else {
            self.pause_policy == dto::PausePolicy::DecisionsAndWarnings
                || !is_warning_pause(&reason)
        };
        if !keep {
            self.world.pending_pause = None;
        }
    }

    /// Whether the player can answer card `id` right now, which is what a card pause needs.
    fn card_pause_is_live(&self, id: u64) -> bool {
        let Some(event) = self
            .world
            .events
            .pending
            .iter()
            .find(|event| event.id == id)
        else {
            return false;
        };
        if event.resolved {
            return false;
        }
        let answerable = self
            .dataset
            .event(&event.card)
            .map(|card| !card.choices.is_empty())
            .unwrap_or(false);
        let players = event
            .subject_firm
            .filter(|firm| self.world.firms.contains(*firm))
            .map(|firm| cw_sim::economy::events_engine::player_side(&self.world, firm))
            .unwrap_or(true);
        answerable && players
    }

    /// The current pause in plain words with the action that resumes play, or `None` when the
    /// clock is running.
    pub fn pause_info(&self) -> Option<dto::PauseInfo> {
        let reason = self.world.pending_pause.clone()?;
        let firm_name = |handle: u64| {
            self.world
                .firms
                .get(cw_core::ids::Handle(handle))
                .map(|firm| firm.name.clone())
                .unwrap_or_else(|| "your company".to_owned())
        };
        let parts: Vec<&str> = reason.split(':').collect();
        let number = |index: usize| parts.get(index).and_then(|text| text.parse::<u64>().ok());
        let proceed =
            |kind: &str, title: String, detail: String, firm: Option<u64>| dto::PauseInfo {
                reason: reason.clone(),
                kind: kind.to_owned(),
                title,
                detail,
                action: "continue".to_owned(),
                action_label: "Continue".to_owned(),
                event_id: None,
                firm,
            };
        let info = match parts[0] {
            "event" => {
                let id = number(1)?;
                let event = self.world.events.pending.iter().find(|event| event.id == id)?;
                let title = self
                    .dataset
                    .event(&event.card)
                    .map(|card| card.title.clone())
                    .unwrap_or_else(|| event.card.clone());
                let subject = event.subject_firm.map(|handle| handle.0);
                dto::PauseInfo {
                    reason: reason.clone(),
                    kind: "decision".to_owned(),
                    detail: match subject {
                        Some(handle) => format!(
                            "{} needs your answer before time can move on.",
                            firm_name(handle)
                        ),
                        None => "This needs your answer before time can move on.".to_owned(),
                    },
                    title,
                    action: "answer".to_owned(),
                    action_label: "Answer".to_owned(),
                    event_id: Some(id),
                    firm: subject,
                }
            }
            "default_grace" => {
                let handle = number(1)?;
                proceed(
                    "warning",
                    "Payment grace is running out".to_owned(),
                    format!(
                        "{} is behind on its bills and the grace period ends soon. Raise cash or cut costs, then continue.",
                        firm_name(handle)
                    ),
                    Some(handle),
                )
            }
            "strike" => {
                let handle = number(1)?;
                proceed(
                    "warning",
                    "Workers walked out".to_owned(),
                    format!(
                        "{} has a strike that began this week. Open the company to settle or wait it out, then continue.",
                        firm_name(handle)
                    ),
                    Some(handle),
                )
            }
            "board_vote" => {
                let handle = number(1)?;
                proceed(
                    "deadline",
                    "A board vote closes next week".to_owned(),
                    format!(
                        "{} has a board vote that closes within a week. Open the company's board to cast it, or continue to let it lapse.",
                        firm_name(handle)
                    ),
                    Some(handle),
                )
            }
            "deal_deadline" => {
                let handle = number(1)?;
                proceed(
                    "deadline",
                    "A deal deadline is close".to_owned(),
                    format!(
                        "A deal for {} reaches its deadline within a week. Open Deals to act, or continue to let it run.",
                        firm_name(handle)
                    ),
                    Some(handle),
                )
            }
            "union_counter_rejected" => proceed(
                "warning",
                "The union rejected your counter".to_owned(),
                "Your counteroffer was turned down. Open the company's labour page to try again, or continue."
                    .to_owned(),
                None,
            ),
            "five_year_score" => proceed(
                "milestone",
                "Five-year score".to_owned(),
                "Your five years are up and the score sheet is ready. Continue to keep playing the same career."
                    .to_owned(),
                None,
            ),
            _ => proceed(
                "warning",
                "The clock is paused".to_owned(),
                "Continue to resume play.".to_owned(),
                None,
            ),
        };
        Some(info)
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
        let revision = self.world.revision;
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
            } => cash_preview(
                revision,
                -*amount_cents,
                vec!["paid from distributable retained earnings".to_owned()],
            ),
            CommandIntent::SetPrice {
                product,
                price_cents,
                ..
            } => {
                let reference = self
                    .dataset
                    .products
                    .get(*product as usize)
                    .map(|entry| entry.reference_price_cents)
                    .unwrap_or(0);
                cash_preview(
                    revision,
                    0,
                    vec![
                        format!("reference price {reference} cents per lot"),
                        format!("your asking price {price_cents} cents per lot"),
                        "no cash moves until a sale settles".to_owned(),
                    ],
                )
            }
            CommandIntent::SetWageOffer {
                firm,
                tier,
                wage_cents,
            } => {
                let prevailing = self
                    .world
                    .firms
                    .get(*firm)
                    .and_then(|entity| self.world.regions.get(entity.region as usize))
                    .and_then(|region| region.labour.get(*tier as usize))
                    .map(|tier| tier.prevailing_wage_cents)
                    .unwrap_or(0);
                cash_preview(
                    revision,
                    0,
                    vec![
                        format!("prevailing weekly wage {prevailing} cents in this region"),
                        format!("your weekly offer {wage_cents} cents per worker"),
                        "payroll settles with the next advanced week".to_owned(),
                    ],
                )
            }
            CommandIntent::RepurchaseShares { firm, qty } => {
                let fair = self
                    .world
                    .firms
                    .get(*firm)
                    .map(|f| f.shares.fair_cents)
                    .unwrap_or(0);
                let mut assumptions = vec![
                    "estimated at the current fair value per share".to_owned(),
                    "the cleared price is set at settlement".to_owned(),
                    "bought shares move to treasury".to_owned(),
                ];
                if *qty <= 0 {
                    assumptions.push("quantity must be positive".to_owned());
                }
                cash_preview(
                    revision,
                    qty.saturating_mul(fair).saturating_neg(),
                    assumptions,
                )
            }
            CommandIntent::RetireTreasury { qty, .. } => {
                let mut assumptions = vec![
                    "cancellation reduces issued and treasury shares together".to_owned(),
                    "no cash moves".to_owned(),
                ];
                if *qty <= 0 {
                    assumptions.push("quantity must be positive".to_owned());
                }
                cash_preview(revision, 0, assumptions)
            }
            CommandIntent::LaunchIPO {
                new_shares,
                price_cents,
                ..
            } => {
                let proceeds = new_shares.saturating_mul(*price_cents);
                let fee = round_even_div(proceeds as i128 * 200, 10_000) as i64;
                let fee = fee.max(10_000_000).min(proceeds);
                cash_preview(
                    revision,
                    proceeds - fee,
                    vec![
                        format!("gross proceeds {proceeds} cents"),
                        format!("underwriting fee {fee} cents (2 percent, minimum $100,000)"),
                        "at least ten percent of post-issue shares must be sold".to_owned(),
                    ],
                )
            }
            CommandIntent::RequestLoan { amount_cents, .. } => cash_preview(
                revision,
                *amount_cents,
                vec![
                    "funded by an outside lender over 104 weeks".to_owned(),
                    "the rate is the policy rate plus the grade spread".to_owned(),
                ],
            ),
            CommandIntent::IssueBond {
                firm,
                principal_cents,
                coupon_bps,
                weeks,
            } => {
                let fee = round_even_div(*principal_cents as i128 * 200, 10_000) as i64;
                let quote = cw_sim::economy::structure::bond_quote(
                    &self.world,
                    *firm,
                    *principal_cents,
                    *weeks,
                );
                let mut assumptions = vec![
                    format!("bond issue fee {fee} cents (2 percent)"),
                    format!(
                        "coupon {} basis points: the ten-year yield {} plus a credit spread of {} for leverage, coverage and size",
                        quote.coupon_bps, quote.base_bps, quote.spread_bps
                    ),
                    format!("matures in {weeks} weeks"),
                ];
                if *coupon_bps > 0 && quote.coupon_bps > *coupon_bps {
                    assumptions.push(format!(
                        "refused: the market price {} is above your limit {}",
                        quote.coupon_bps, coupon_bps
                    ));
                }
                cash_preview(revision, principal_cents - fee, assumptions)
            }
            CommandIntent::DepositFunds { amount_cents, .. } => cash_preview(
                revision,
                -*amount_cents,
                vec!["firm cash leaves; a matching bank deposit claim is created".to_owned()],
            ),
            CommandIntent::WithdrawFunds { amount_cents, .. } => cash_preview(
                revision,
                *amount_cents,
                vec!["paid only from actual bank cash and up to the deposit claim".to_owned()],
            ),
            CommandIntent::ApproveLoan { amount_cents, .. } => cash_preview(
                revision,
                -*amount_cents,
                vec![
                    "bank cash falls by the full principal".to_owned(),
                    "the borrower receives the same principal".to_owned(),
                    "liquidity and capital tests apply".to_owned(),
                ],
            ),
            CommandIntent::StartDiligence {
                price_cents,
                max_shares,
                ..
            } => {
                let escrow = cw_sim::economy::deals::escrow_required(*price_cents, *max_shares);
                let fee =
                    round_even_div(*price_cents as i128 * *max_shares as i128 * 50, 10_000) as i64;
                cash_preview(
                    revision,
                    -(escrow + fee),
                    vec![
                        format!("escrow {escrow} cents includes the 2 percent transaction fee"),
                        format!("diligence fee {fee} cents (0.5 percent of bid value)"),
                        "two weeks of diligence, then a four-week offer".to_owned(),
                    ],
                )
            }
            CommandIntent::RespondTender { deal, accept } => {
                let Some(entity) = self.world.deals.get(*deal) else {
                    return cash_preview(revision, 0, vec!["unknown deal".to_owned()]);
                };
                if !*accept {
                    return cash_preview(
                        revision,
                        0,
                        vec!["declining changes nothing and keeps the shares".to_owned()],
                    );
                }
                let qty = self.world.investor_holds(entity.target);
                cash_preview(
                    revision,
                    qty.saturating_mul(entity.offer_price_cents),
                    vec![
                        format!("your {qty} shares at {} cents", entity.offer_price_cents),
                        "tendered shares transfer to the bidder".to_owned(),
                    ],
                )
            }
            CommandIntent::FileLobbyRequest { firm, .. } => {
                let fee = cw_sim::economy::policy::request_fee(&self.world, *firm);
                cash_preview(
                    revision,
                    -fee,
                    vec![
                        format!("request fee {fee} cents"),
                        "the fee is max($25,000, 0.1 percent of group assets)".to_owned(),
                        "decision in four weeks; success is uncertain".to_owned(),
                    ],
                )
            }
            CommandIntent::ProposeSettlement { amount_cents, .. } => cash_preview(
                revision,
                -*amount_cents,
                vec!["the offer must be at least 95 percent of the current demand".to_owned()],
            ),
            CommandIntent::QuotePolicy { limit_cents, .. } => {
                let premium = cw_sim::economy::insurance::weekly_premium(*limit_cents);
                cash_preview(
                    revision,
                    0,
                    vec![
                        format!("weekly premium {premium} cents"),
                        "annual premium = limit x 200 basis points x 1.25".to_owned(),
                    ],
                )
            }
            CommandIntent::RespondBargaining { choice, .. } => cash_preview(
                revision,
                0,
                vec![match choice {
                    cw_sim::commands::BargainingChoice::Accept => {
                        "accepting adopts the union's wage demand".to_owned()
                    }
                    cw_sim::commands::BargainingChoice::Counter => {
                        "a counter sends your weekly offer; a second unresolved week can strike"
                            .to_owned()
                    }
                    cw_sim::commands::BargainingChoice::Reject => {
                        "rejecting risks a strike and lost output".to_owned()
                    }
                }],
            ),
            CommandIntent::AcceptManagementContract => cash_preview(
                revision,
                10_000_000,
                vec!["one-time signing bonus from the distressed outside investor".to_owned()],
            ),
            CommandIntent::FoundFirm {
                capital_cents,
                recipe,
                ..
            } => {
                let fee = self.world.difficulty.formation_fee_cents();
                let capex = self
                    .dataset
                    .recipes
                    .get(*recipe as usize)
                    .map(|template| template.capex_cents)
                    .unwrap_or(0);
                cash_preview(
                    revision,
                    -(capital_cents + fee),
                    vec![
                        format!("paid-in capital {capital_cents} cents becomes the firm's cash"),
                        format!("formation fee {fee} cents to outside legal services"),
                        format!(
                            "template capex {capex} cents is escrowed from the paid-in capital"
                        ),
                        "the investor needs at least the minimum capital plus the fee".to_owned(),
                    ],
                )
            }
            CommandIntent::PlaceShareOrder {
                investor,
                firm,
                buy,
                qty,
                limit_price_cents,
            } => {
                let name = self
                    .world
                    .firms
                    .get(*firm)
                    .map(|entity| entity.name.clone())
                    .unwrap_or_default();
                let reserved = if *buy {
                    qty.saturating_mul(*limit_price_cents).saturating_neg()
                } else {
                    qty.saturating_mul(*limit_price_cents)
                };
                cash_preview(
                    revision,
                    reserved,
                    vec![
                        format!("{qty} shares of {name} at a {limit_price_cents} cent limit"),
                        format!(
                            "reserved from {}",
                            if *investor {
                                "your personal account"
                            } else {
                                "the firm's account"
                            }
                        ),
                        "price impact is 10 percent of quantity over weekly float, capped at 25 percent"
                            .to_owned(),
                        "the quote expires at the next boundary and revalidates cash and ownership"
                            .to_owned(),
                    ],
                )
            }
            _ => Preview {
                revision,
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
            self.heal_pause();
            if let Some(reason) = self.world.pending_pause.clone() {
                pause = Some(reason);
                break;
            }
            cw_sim::ph!("ai+commands", self.run_ai());
            let outcome = cw_sim::tick::advance(&mut self.world, &self.dataset);
            advanced += 1;
            self.heal_pause();
            if outcome.pause.is_some()
                && let Some(reason) = self.world.pending_pause.clone()
            {
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
            // Read the move before it changes the world it is read from.
            let note = cw_ai::note(&self.world, &self.dataset, &proposal);
            let envelope = CommandEnvelope {
                command_id_high: AI_COMMAND_HIGH,
                command_id_low: self.world.next_sequence,
                actor: proposal.actor,
                expected_revision: self.world.revision,
                submitted_tick: self.world.tick.0,
                intent: proposal.intent,
            };
            let archetype = self
                .world
                .firms
                .get(Handle(proposal.actor))
                .and_then(|firm| firm.archetype)
                .map(|archetype| archetype.key())
                .unwrap_or("none")
                .to_owned();
            let command = envelope.intent.key();
            let result = apply(&mut self.world, &self.dataset, &envelope);
            if let (Ok(_), Some(note)) = (&result, note) {
                cw_sim::economy::rivalry::record_move(
                    &mut self.world,
                    Handle(proposal.actor),
                    note.temperament,
                    note.class,
                    note.text,
                    note.grudge,
                );
            }
            let row = self.ai_log.by_archetype.entry(archetype).or_default();
            match result {
                Ok(_) => {
                    row.accepted += 1;
                    *row.accepted_by_command
                        .entry(command.to_owned())
                        .or_insert(0) += 1;
                }
                Err(error) => {
                    row.rejected += 1;
                    let reason = format!("{command}: {}", error.key());
                    *row.rejected_by_reason.entry(reason).or_insert(0) += 1;
                }
            }
        }
    }

    /// Choose the currency the investor's figures are shown in. False for an unknown id.
    pub fn set_reporting_currency(&mut self, id: &str) -> bool {
        let Some(index) = self
            .dataset
            .currencies
            .as_ref()
            .and_then(|set| set.index_of(id))
        else {
            return false;
        };
        self.reporting_currency = index as u8;
        true
    }

    /// Exchange rates, central banks and recorded foreign exchange results; None with one money.
    pub fn fx_view(&self) -> Option<dto::FxView> {
        dto::fx_view(&self.world, &self.dataset, self.reporting_currency)
    }

    pub fn overview(&self) -> Overview {
        let world = &self.world;
        let reporting = self.reporting_currency;
        let net_worth = world.fx.convert(
            cw_sim::economy::score::personal_net_worth(world),
            0,
            reporting,
        );
        let primary = world
            .investor
            .primary_group
            .filter(|handle| world.firms.contains(*handle));
        let (quarter_net, quarter_weeks) = primary
            .and_then(|handle| world.firms.get(handle))
            .map(|firm| {
                let weeks = firm.book.recent_weeks();
                (
                    weeks.iter().map(|week| week.net_income).sum::<i64>(),
                    weeks.len() as u32,
                )
            })
            .unwrap_or((0, 0));
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
        for (_, firm) in world.firms.iter() {
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
                    detail: String::new(),
                });
            }
            if firm.payout_freeze {
                alerts.push(Alert {
                    key: "alert.payout_frozen".to_owned(),
                    subject: firm.name.clone(),
                    severity: 2,
                    detail: String::new(),
                });
            }
            storage_alerts(world, &self.dataset, firm, &mut alerts);
        }
        logistics_alerts(world, &self.dataset, &mut alerts);
        government_alerts(world, &self.dataset, &mut alerts);
        structure_alerts(world, &mut alerts);
        for event in dto::events(world, &self.dataset) {
            if event.mandatory {
                alerts.push(Alert {
                    key: "alert.decision".to_owned(),
                    subject: event.title,
                    severity: 2,
                    detail: String::new(),
                });
            }
        }
        let advice = self.next_action();
        Overview {
            tick: world.tick.0,
            week_label: world.tick.label_from(world.start_year),
            start_year: world.start_year,
            revision: world.revision,
            investor_cash_cents: world.fx.convert(world.investor.cash_cents, 0, reporting),
            investor_net_worth_cents: net_worth,
            reporting_currency: self
                .dataset
                .currencies
                .as_ref()
                .and_then(|set| set.currencies.get(reporting as usize))
                .map(|def| def.id.clone())
                .unwrap_or_default(),
            primary_currency: primary
                .map(|handle| dto::currency_id(world, &self.dataset, handle.0))
                .unwrap_or_default(),
            primary_firm: primary.map(|handle| handle.0),
            primary_name: name,
            firm_cash_cents: cash,
            forecast_four_week_cents: forecast,
            last_week_ebit_cents: ebit,
            quarter_net_income_cents: quarter_net,
            quarter_weeks,
            alerts,
            pause: world.pending_pause.clone(),
            pause_info: self.pause_info(),
            next_action_key: advice.0,
            next_action_reason: advice.1,
        }
    }

    /// One recommended next action with a reason. Deterministic, based only on visible state:
    /// solvency before growth, staffing before price, stock before expansion.
    fn next_action(&self) -> (String, String) {
        let world = &self.world;
        let controlled: Vec<Handle> = world
            .firms
            .iter()
            .filter(|(_, firm)| firm.shares.qty(cw_sim::state::HolderId::Investor) > 0)
            .map(|(handle, _)| handle)
            .collect();
        if controlled.is_empty() {
            return (
                "advice.find_a_firm".to_owned(),
                "You do not control a company. Found one with personal capital, or take the management contract when eligible.".to_owned(),
            );
        }
        for handle in &controlled {
            let firm = world.firm(*handle).unwrap();
            if matches!(
                firm.status,
                cw_sim::state::FirmStatus::PastDue { .. }
                    | cw_sim::state::FirmStatus::Restructuring { .. }
            ) {
                return (
                    "advice.restore_solvency".to_owned(),
                    format!(
                        "{} cannot meet a due payment. Sell unpledged stock, raise funded equity or negotiate before the four-week grace ends.",
                        firm.name
                    ),
                );
            }
        }
        let primary = world
            .investor
            .primary_group
            .filter(|handle| world.firms.contains(*handle));
        let Some(primary) = primary else {
            return (
                "advice.choose_company".to_owned(),
                "Choose the company you want to operate from the portfolio.".to_owned(),
            );
        };
        let firm = world.firm(primary).unwrap();
        let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, primary);
        if outflow > 0 && firm.book.cash() < outflow * 8 {
            return (
                "advice.build_cash_buffer".to_owned(),
                format!(
                    "{} holds less than eight weeks of mandatory payroll and debt service. Raise a price that still clears, cut marketing, or seek legal credit.",
                    firm.name
                ),
            );
        }
        let understaffed = firm
            .facilities
            .iter()
            .filter_map(|handle| world.facilities.get(*handle))
            .find(|facility| {
                facility.status == cw_sim::state::FacilityStatus::Operating
                    && facility.last_staff_ratio_bps < 8_500
            });
        if let Some(facility) = understaffed {
            return (
                "advice.raise_wage".to_owned(),
                format!(
                    "A facility is staffed at {} percent, so capacity is idle. Raise the wage offer toward the regional prevailing wage.",
                    facility.last_staff_ratio_bps / 100
                ),
            );
        }
        for ((region, product), sold) in &firm.last_week_units_sold {
            let sold = sold.lots();
            let stock = firm.stock(*product, *region).lots();
            if sold > 0 && stock > sold.saturating_mul(4) {
                return (
                    "advice.clear_stock".to_owned(),
                    format!(
                        "You hold {} lots of {} against {} sold last week. Lower the price within the allowed band or sell surplus through a contract.",
                        stock, self.dataset.products[*product as usize].name, sold
                    ),
                );
            }
        }
        (
            "advice.operate".to_owned(),
            "Step one week and compare the result with the forecast. Then inspect the facility bottleneck before expanding."
                .to_owned(),
        )
    }

    /// Private firm data needs a management right, public disclosure, or completed diligence.
    pub fn can_inspect(&self, handle: u64) -> bool {
        let firm_handle = Handle(handle);
        let Some(firm) = self.world.firm(firm_handle) else {
            return false;
        };
        if firm.shares.qty(cw_sim::state::HolderId::Investor) > 0
            || self.world.investor.managed_firms.contains(&firm_handle)
        {
            return true;
        }
        if firm.shares.ipo_done {
            return true;
        }
        // A deal the investor's group funded and has already taken through diligence.
        self.world.deals.iter().any(|(_, deal)| {
            deal.target == firm_handle
                && self.world.tick.0 >= deal.diligence_until.0
                && self
                    .world
                    .firm(deal.buyer)
                    .map(|buyer| buyer.shares.qty(cw_sim::state::HolderId::Investor) > 0)
                    .unwrap_or(false)
        })
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
                    .filter_map(|handle| {
                        dto::firm_summary(
                            world,
                            &self.dataset,
                            handle.0,
                            self.can_inspect(handle.0),
                        )
                    })
                    .collect(),
            ),
            Query::Firm(handle) => {
                let summary =
                    dto::firm_summary(world, &self.dataset, handle, self.can_inspect(handle))
                        .unwrap_or_else(|| empty_firm(handle));
                Detail::Firm(summary)
            }
            Query::Facilities(firm) => {
                if !self.can_inspect(firm) {
                    return Detail::Restricted;
                }
                Detail::Facilities(
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
                )
            }
            Query::Facility(handle) => {
                let owner = world
                    .facilities
                    .get(cw_core::ids::Handle(handle))
                    .map(|facility| facility.firm.0)
                    .unwrap_or(0);
                if !self.can_inspect(owner) {
                    return Detail::Restricted;
                }
                Detail::Facility(
                    dto::facility_summary(world, &self.dataset, handle)
                        .unwrap_or_else(|| empty_facility(handle)),
                )
            }
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
            Query::Finance(handle) => {
                if !self.can_inspect(handle) {
                    return Detail::Restricted;
                }
                Detail::Finance(
                    dto::finance_summary(world, handle).unwrap_or_else(|| empty_finance(handle)),
                )
            }
            Query::Ownership(handle) => {
                if !self.can_inspect(handle) {
                    return Detail::Restricted;
                }
                Detail::Ownership(
                    dto::ownership_summary(world, handle)
                        .unwrap_or_else(|| empty_ownership(handle)),
                )
            }
            Query::Contracts(firm) => {
                if !self.can_inspect(firm) {
                    return Detail::Restricted;
                }
                Detail::Contracts(dto::contracts(world, &self.dataset, firm))
            }
            Query::Loans(firm) => {
                if !self.can_inspect(firm) {
                    return Detail::Restricted;
                }
                Detail::Loans(dto::loans(world, firm))
            }
            Query::Insurance(firm) => {
                if !self.can_inspect(firm) {
                    return Detail::Restricted;
                }
                Detail::Insurance(dto::insurance(world, firm))
            }
            Query::Deals => Detail::Deals(dto::deals(world)),
            Query::Events => Detail::Events(dto::events(world, &self.dataset)),
            Query::Market(region, product) => {
                Detail::Market(dto::market(world, &self.dataset, region, product))
            }
        }
    }

    pub fn export_save(&self) -> Result<Vec<u8>, DomainError> {
        save::encode_with(&self.world, self.career_id, self.custom.as_ref())
    }

    /// Import into a new session. The current session is untouched on failure.
    pub fn import_save(bytes: &[u8]) -> Result<Session, DomainError> {
        if bytes.len() > save::MAX_BYTES {
            return Err(DomainError::ResourceLimit {
                limit: save::MAX_BYTES as u64,
                requested: bytes.len() as u64,
            });
        }
        let (world, career_id, custom) = save::decode_with(bytes)?;
        let mut dataset =
            Dataset::for_hash(&world.dataset_hash).ok_or(DomainError::InvalidSave {
                detail: cw_core::error::SaveFault::MalformedPayload,
            })?;
        if let Some(custom) = &custom {
            if !custom.validate(&dataset).is_empty() {
                return Err(DomainError::InvalidSave {
                    detail: cw_core::error::SaveFault::MalformedPayload,
                });
            }
            custom.apply_to(&mut dataset);
        }
        let config = ScenarioConfig {
            seed: world.seed,
            scenario: world.scenario,
            mode: world.mode,
            dataset_hash: world.dataset_hash.clone(),
            difficulty: world.difficulty,
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: Default::default(),
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
            custom,
            log,
            ai_log: AiLog::default(),
            reporting_currency: 0,
            pause_policy: dto::PausePolicy::default(),
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

/// Pauses that only inform. The player may choose not to be stopped for these.
fn is_warning_pause(reason: &str) -> bool {
    reason.starts_with("default_grace:")
        || reason.starts_with("strike:")
        || reason == "union_counter_rejected"
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
        currency: String::new(),
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
        restricted: true,
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
        reopen_cost_cents: 0,
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

/// Structure alerts, only when they need a decision or cost real money: an activist campaign
/// against one of the investor's companies, transfer pricing that is squeezing minority
/// holders enough to draw a challenge, a supplier the market is marking down, and a bond
/// falling due that the company cannot yet cover.
fn structure_alerts(world: &cw_sim::state::World, alerts: &mut Vec<Alert>) {
    use cw_sim::economy::structure as st;
    let mine =
        |firm: cw_core::ids::Handle| cw_sim::economy::events_engine::player_side(world, firm);
    let name = |firm: cw_core::ids::Handle| {
        world
            .firms
            .get(firm)
            .map(|f| f.name.clone())
            .unwrap_or_default()
    };
    for c in &world.structure.campaigns {
        if c.outcome == st::Outcome::Open && mine(c.target) {
            alerts.push(Alert {
                key: "alert.activist".to_owned(),
                subject: name(c.target),
                severity: 2,
                detail: String::new(),
            });
        }
    }
    for (firm, squeeze) in &world.structure.squeeze {
        let equity = world
            .firms
            .get(*firm)
            .map(|f| f.valuation.equity_cents)
            .unwrap_or(0)
            .max(1);
        if mine(*firm) && squeeze * 10_000 / equity >= 200 && st::minority_bps(world, *firm) > 0 {
            alerts.push(Alert {
                key: "alert.squeeze".to_owned(),
                subject: name(*firm),
                severity: 1,
                detail: String::new(),
            });
        }
    }
    for (firm, weeks) in &world.structure.captive_weeks {
        if mine(*firm) && *weeks >= 26 {
            alerts.push(Alert {
                key: "alert.captive_markdown".to_owned(),
                subject: name(*firm),
                severity: 1,
                detail: String::new(),
            });
        }
    }
    for (_, bond) in world.bonds.iter() {
        let left = bond.maturity.0.saturating_sub(world.tick.0);
        if mine(bond.issuer)
            && left <= 13
            && world
                .firms
                .get(bond.issuer)
                .is_some_and(|f| f.book.cash() < bond.principal_cents * 12 / 10)
        {
            alerts.push(Alert {
                key: "alert.bond_due".to_owned(),
                subject: name(bond.issuer),
                severity: 3,
                detail: String::new(),
            });
        }
    }
}

/// A lane's name for alerts: "Columbus to Toledo by rail".
pub fn lane_label(dataset: &cw_data::Dataset, key: cw_sim::economy::logistics::LaneKey) -> String {
    let city = |r: u8| {
        dataset.regions[r as usize]
            .principal_city
            .split(',')
            .next()
            .unwrap_or("")
            .to_owned()
    };
    format!(
        "{} to {} by {}",
        city(key.a),
        city(key.b),
        dataset.logistics.modes[key.mode as usize]
            .name
            .to_lowercase()
    )
}

/// Freight alerts, only when they matter and only for lanes the investor's companies ship on:
/// a lane full for four weeks running, a carrier rate hike of a fifth over four weeks, or a rival
/// reserving a lane in the last three weeks. Each is told once per lane while it holds.
fn logistics_alerts(
    world: &cw_sim::state::World,
    dataset: &cw_data::Dataset,
    alerts: &mut Vec<Alert>,
) {
    use cw_sim::economy::logistics as lg;
    let mine =
        |firm: cw_core::ids::Handle| cw_sim::economy::events_engine::player_side(world, firm);
    let st = &world.logistics;
    for (key, stat) in &st.stats {
        let depends = stat
            .users
            .iter()
            .any(|(firm, last)| mine(*firm) && world.tick.0.saturating_sub(*last) <= 4);
        if !depends {
            continue;
        }
        let label = lane_label(dataset, *key);
        let mine_lots: i32 = st
            .lanes
            .get(key)
            .map(|lane| {
                lane.users
                    .iter()
                    .filter(|(f, _)| mine(*f))
                    .map(|(_, lots)| *lots)
                    .sum()
            })
            .unwrap_or(0);
        if stat.hot_weeks >= lg::SUSTAINED_WEEKS {
            let outside = if stat.moved > 0 {
                stat.outside as i64 * 100 / stat.moved as i64
            } else {
                0
            };
            alerts.push(Alert {
                key: "alert.lane_congested".to_owned(),
                subject: label.clone(),
                severity: 1,
                detail: format!(
                    "{label} has been full for {} weeks and {outside}% of its freight goes to outside carriers at a premium. Your group moves about {mine_lots} lots a week on it. Open the Freight map, tap the lane and reserve lots to lock a price, or build a carrier for it.",
                    stat.hot_weeks
                ),
            });
        }
        if stat.mult_hist.len() >= 5 {
            let now = *stat.mult_hist.last().unwrap_or(&0) as i64;
            let before = stat.mult_hist[stat.mult_hist.len() - 5] as i64;
            if before > 0 && now * 10_000 >= before * (10_000 + lg::HIKE_BPS as i64) {
                let rise = (now * 100 / before) - 100;
                alerts.push(Alert {
                    key: "alert.carrier_hike".to_owned(),
                    subject: label.clone(),
                    severity: 1,
                    detail: format!(
                        "{label}: carriers raised the rate {rise}% in four weeks. Your group moves about {mine_lots} lots a week on it. Open the division's Shipping panel to try another mode, or reserve the lane on the Freight map to lock today's price."
                    ),
                });
            }
        }
    }
    for reservation in &st.reservations {
        if world.tick.0.saturating_sub(reservation.tick) > 3 || mine(reservation.holder) {
            continue;
        }
        let affected = st.stats.get(&reservation.key).is_some_and(|stat| {
            stat.users
                .iter()
                .any(|(firm, last)| mine(*firm) && world.tick.0.saturating_sub(*last) <= 4)
        });
        if affected {
            let label = lane_label(dataset, reservation.key);
            alerts.push(Alert {
                key: "alert.lane_reserved".to_owned(),
                subject: label.clone(),
                severity: 2,
                detail: format!(
                    "{label}: a rival reserved {} lots, which are now closed to you. Open the Freight map, tap the lane and reserve what you need before it fills.",
                    reservation.lots
                ),
            });
        }
    }
}

/// Alerts from laws, the reserve, quotas, state rescue and systemic incidents, only when they
/// touch the investor's companies:
///
/// * a law about carriers announced or just in force, when a company of theirs owns or ships on
///   a carrier lane;
/// * a quota capping a good one of their plants makes;
/// * the state reserve buying or selling a good one of their companies makes;
/// * a rescue law announced or in force, when a company of theirs is a strategic firm, and any
///   rescue of one of their companies in the last month;
/// * a pipeline spill, strike, fire, lane outage or harvest failure that touches one of their
///   companies in the last four weeks.
fn government_alerts(
    world: &cw_sim::state::World,
    dataset: &cw_data::Dataset,
    alerts: &mut Vec<Alert>,
) {
    use cw_sim::economy::{rescue, reserve, systemic};
    let mine: Vec<cw_core::ids::Handle> = world
        .firms
        .iter()
        .filter(|(h, f)| f.operative() && cw_sim::economy::events_engine::player_side(world, *h))
        .map(|(h, _)| h)
        .collect();
    if mine.is_empty() {
        return;
    }
    let now = world.tick.0;
    let p = &world.politics;
    let recent_law = p
        .pending
        .is_some_and(|c| now.saturating_sub(c.announced) <= 4)
        || (p.last_effective > 0 && now.saturating_sub(p.last_effective) <= 4);
    if recent_law {
        let changed = p.pending.map(|c| c.laws).unwrap_or(p.laws);
        let ships = mine.iter().any(|f| {
            world.logistics.carriers.iter().any(|c| c.firm == *f)
                || world
                    .logistics
                    .stats
                    .values()
                    .any(|s| s.users.iter().any(|(u, _)| u == f))
        });
        if ships && changed.common_carrier != world.logistics.common_carrier {
            alerts.push(Alert {
                key: "alert.law_carrier".to_owned(),
                subject: if changed.common_carrier {
                    "Common carrier rules are coming: owned lanes must sell spare capacity at a regulated rate."
                } else {
                    "Common carrier rules are being repealed: owners of lanes can price freely again."
                }
                .to_owned(),
                severity: 1,
                detail: String::new(),
            });
        }
        if mine
            .iter()
            .any(|f| rescue::is_strategic(world, dataset, *f))
            && changed.rescue != world.politics.laws.rescue
        {
            alerts.push(Alert {
                key: "alert.law_rescue".to_owned(),
                subject: match changed.rescue {
                    cw_sim::state::Rescue::Nationalise => {
                        "Failing strategic firms will be nationalised. Yours would get a state loan instead."
                    }
                    cw_sim::state::Rescue::Bailout => {
                        "Failing strategic firms will get state rescue loans."
                    }
                    cw_sim::state::Rescue::None => "State rescue of failing strategic firms ends.",
                }
                .to_owned(),
                severity: 1,
                detail: String::new(),
            });
        }
    }
    for (region, product) in &p.quota_active {
        let touches = world.facilities.iter().any(|(_, plant)| {
            plant.region == *region
                && mine.contains(&plant.firm)
                && dataset
                    .product_index
                    .get(&dataset.recipes[plant.recipe as usize].output)
                    == Some(product)
        });
        if touches {
            alerts.push(Alert {
                key: "alert.quota".to_owned(),
                subject: format!(
                    "{} output in {} is capped at {}% of capacity while prices are soft.",
                    dataset.products[*product as usize].name,
                    dataset.regions[*region as usize].name,
                    p.laws.quota_bps / 100
                ),
                severity: 1,
                detail: String::new(),
            });
        }
    }
    if p.laws.reserve {
        let made: std::collections::BTreeSet<u16> = mine
            .iter()
            .flat_map(|f| cw_sim::economy::pricing::outputs(world, dataset, *f))
            .collect();
        for product in reserve::strategic_goods(dataset) {
            if !made.contains(&product) {
                continue;
            }
            let (verb, sev) = match reserve::phase(world, dataset, product) {
                reserve::Phase::Buying => ("is buying", 0),
                reserve::Phase::Selling if p.reserve.lots_of(product) > 0 => ("is selling", 1),
                _ => continue,
            };
            alerts.push(Alert {
                key: "alert.reserve".to_owned(),
                subject: format!(
                    "The state reserve {verb} {}.",
                    dataset.products[product as usize].name.to_lowercase()
                ),
                severity: sev,
                detail: String::new(),
            });
        }
    }
    for firm in &mine {
        if world
            .firms
            .get(*firm)
            .is_some_and(|f| f.rescued_week > 0 && now.saturating_sub(f.rescued_week) <= 4)
        {
            let name = world
                .firms
                .get(*firm)
                .map(|f| f.name.clone())
                .unwrap_or_default();
            alerts.push(Alert {
                key: "alert.state_rescue".to_owned(),
                subject: format!("{name} received a state rescue loan."),
                severity: 2,
                detail: String::new(),
            });
        }
        for incident in systemic::touching(world, *firm, 4) {
            alerts.push(Alert {
                key: "alert.incident".to_owned(),
                subject: incident.text.clone(),
                severity: 2,
                detail: String::new(),
            });
        }
    }
    alerts.dedup_by(|a, b| a.key == b.key && a.subject == b.subject);
}

/// Storage alerts for one of the investor's companies, only when they matter: space nearly full
/// in a class where stock is substantial, outside storage and holding cost above 3 percent of
/// four weeks of revenue, or spoilage above 2 percent. Each also needs a floor in dollars so
/// a small firm is not nagged.
fn storage_alerts(
    world: &cw_sim::state::World,
    dataset: &cw_data::Dataset,
    firm: &cw_sim::state::Firm,
    alerts: &mut Vec<Alert>,
) {
    const FLOOR_CENTS: i64 = 200_000;
    let revenue_4w: i64 = firm
        .book
        .recent_weeks()
        .iter()
        .rev()
        .take(4)
        .map(|week| week.revenue)
        .sum();
    let recent = firm.storage.recent(4);
    let cost = recent.fee_cents + recent.holding_cents;
    if cost >= FLOOR_CENTS && cost * 100 >= revenue_4w.max(1) * 3 {
        alerts.push(Alert {
            key: "alert.storage_fees".to_owned(),
            subject: firm.name.clone(),
            severity: 1,
            detail: String::new(),
        });
    }
    if recent.spoiled_cents >= FLOOR_CENTS && recent.spoiled_cents * 100 >= revenue_4w.max(1) * 2 {
        alerts.push(Alert {
            key: "alert.spoilage".to_owned(),
            subject: firm.name.clone(),
            severity: 1,
            detail: String::new(),
        });
    }
    let full = cw_sim::economy::storage::uses(world, dataset, firm)
        .into_iter()
        .any(|u| u.used_milli >= 500_000 && u.used_milli * 100 >= u.capacity_milli() * 95);
    if full {
        alerts.push(Alert {
            key: "alert.storage_full".to_owned(),
            subject: firm.name.clone(),
            severity: 1,
            detail: String::new(),
        });
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
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: Default::default(),
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
    fn the_market_board_reports_every_firm_selling_in_a_state() {
        let mut session = Session::create(config(175)).unwrap();
        session.advance(8);
        assert!(session.world.tick.0 >= 4, "the career advanced");
        // A plant whose firm is headquartered in another state still trades where it sits.
        // Move a selling plant's headquarters to another state to prove it.
        let world = &session.world;
        let dataset = &session.dataset;
        let (firm, region, recipe) = world
            .facilities
            .iter()
            .find_map(|(_, facility)| {
                let code = dataset.product_index
                    [dataset.recipes[facility.recipe as usize].output.as_str()];
                let owner = world.firms.get(facility.firm)?;
                let sold = owner
                    .last_closed_sales()
                    .and_then(|week| week.get(&(facility.region, code)).copied())
                    .unwrap_or(0);
                (sold > 0).then_some((facility.firm, facility.region, facility.recipe))
            })
            .expect("some plant sold its output last week");
        let regions = world.regions.len() as u8;
        session.world.firms.get_mut(firm).unwrap().region = (region + 1) % regions;
        let output =
            session.dataset.product_index[session.dataset.recipes[recipe as usize].output.as_str()];
        let line = match session.query(Query::Market(region, output)) {
            Detail::Market(line) => line,
            _ => panic!("market query answers"),
        };
        assert_eq!(
            line.product_id,
            session.dataset.recipes[recipe as usize].output
        );
        assert!(
            line.last_week_sales_lots + line.regional_stock_lots > 0,
            "an out-of-state producer's sales or stock show on the board"
        );
        let selling = (0..regions).any(|region| {
            (0..session.dataset.products.len() as u16).any(|product| {
                matches!(session.query(Query::Market(region, product)),
                    Detail::Market(line) if line.last_week_sales_lots > 0)
            })
        });
        assert!(
            selling,
            "between ticks the board reads the last closed week, not an empty one"
        );
    }

    #[test]
    fn rivals_reprice_from_their_sales_history() {
        let mut session = Session::create(config(177)).unwrap();
        let before: Vec<_> = session
            .world
            .firms
            .iter()
            .map(|(handle, firm)| (handle, firm.policies.prices.clone()))
            .collect();
        session.advance(16);
        let moved = before.iter().any(|(handle, prices)| {
            !cw_ai::player_controlled(&session.world, *handle)
                && session
                    .world
                    .firms
                    .get(*handle)
                    .is_some_and(|firm| firm.policies.prices != *prices)
        });
        assert!(
            moved,
            "some rival changed a price after four months of trading"
        );
    }

    #[test]
    fn sales_in_one_market_accumulate_across_fills() {
        let mut session = Session::create(config(176)).unwrap();
        let (handle, _) = session.world.firms.iter().next().unwrap();
        let firm = session.world.firms.get_mut(handle).unwrap();
        firm.last_week_units_sold.clear();
        firm.record_sale(0, 1, cw_core::quantity::MilliLots::from_lots(3));
        firm.record_sale(0, 1, cw_core::quantity::MilliLots::from_lots(4));
        firm.close_sales_week();
        assert_eq!(firm.last_closed_sales().unwrap().get(&(0, 1)), Some(&7));
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
    fn the_next_action_prioritises_solvency_then_cash_then_operation() {
        let mut session = Session::create(config(211)).unwrap();
        let initial = session.overview();
        assert!(!initial.next_action_key.is_empty());
        assert!(!initial.next_action_reason.is_empty());
        let firm = session.world.investor.primary_group.unwrap();
        // Low cash produces the cash-buffer advice.
        session
            .world
            .firms
            .get_mut(firm)
            .unwrap()
            .book
            .commit(
                session.world.tick,
                cw_core::ledger::Txn::new()
                    .credit(cw_core::ledger::Account::Cash, 90_000_000, "drain")
                    .debit(
                        cw_core::ledger::Account::OperatingExpense,
                        90_000_000,
                        "drain",
                    ),
            )
            .unwrap();
        let low = session.overview();
        assert_eq!(low.next_action_key, "advice.build_cash_buffer");
        // A past-due firm outranks the cash advice.
        cw_sim::economy::insolvency::defer_expense(
            &mut session.world,
            firm,
            1_000_000,
            cw_core::ledger::Account::Wages,
            "unpaid",
        );
        let due = session.overview();
        assert_eq!(due.next_action_key, "advice.restore_solvency");
        assert!(due.next_action_reason.contains("grace"));
    }

    #[test]
    fn private_firm_data_needs_management_public_disclosure_or_diligence() {
        let session = Session::create(config(221)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        assert!(
            session.can_inspect(primary.0),
            "management sees its own firm"
        );
        assert!(matches!(
            session.query(Query::Finance(primary.0)),
            Detail::Finance(_)
        ));
        let private = session
            .world
            .firms
            .iter()
            .find(|(handle, firm)| *handle != primary && !firm.shares.ipo_done)
            .map(|(handle, _)| handle)
            .expect("a private rival");
        assert!(!session.can_inspect(private.0));
        assert!(matches!(
            session.query(Query::Finance(private.0)),
            Detail::Restricted
        ));
        assert!(matches!(
            session.query(Query::Ownership(private.0)),
            Detail::Restricted
        ));
        assert!(matches!(
            session.query(Query::Facilities(private.0)),
            Detail::Restricted
        ));
        if let Detail::Firm(summary) = session.query(Query::Firm(private.0)) {
            assert!(summary.restricted);
            assert_eq!(summary.cash_cents, 0, "private cash is not disclosed");
        } else {
            panic!("firm summary");
        }
        let public = session
            .world
            .firms
            .iter()
            .find(|(handle, firm)| *handle != primary && firm.shares.ipo_done)
            .map(|(handle, _)| handle);
        if let Some(public) = public {
            assert!(
                session.can_inspect(public.0),
                "public statements are disclosed"
            );
        }
    }

    #[test]
    fn completed_diligence_grants_access_to_a_private_target() {
        let mut session = Session::create(config(222)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        let target = session
            .world
            .firms
            .iter()
            .find(|(handle, firm)| *handle != primary && !firm.shares.ipo_done)
            .map(|(handle, _)| handle)
            .unwrap();
        assert!(!session.can_inspect(target.0));
        session.world.deals.insert(cw_sim::state::Deal {
            buyer: primary,
            target,
            kind: cw_sim::state::DealKind::SharePurchase,
            state: cw_sim::state::DealState::Diligence,
            escrow_cents: 0,
            offer_price_cents: 1,
            max_shares: 1,
            committed_shares: 0,
            started: cw_core::calendar::Tick(0),
            diligence_until: cw_core::calendar::Tick(0),
            offer_deadline: cw_core::calendar::Tick(4),
            review_deadline: None,
            frozen_fair_cents: 1,
            review_required: false,
            remedy_share_bps: 0,
            hostile: false,
            diligence_paid: true,
            investor_tendered: false,
        });
        assert!(
            session.can_inspect(target.0),
            "diligence reveals the statements"
        );
    }

    #[test]
    fn the_cross_platform_fixture_hash_is_pinned() {
        // This exact sequence runs in the iOS XCTest bridge. Equal hashes on Linux and iOS are
        // the cross-platform determinism evidence for this scenario.
        //
        // 2026-10-25: the value moved from 8d66f18... when rival command ids stopped using a
        // session-local counter and started using the persisted `world.next_sequence`. It moved
        // again to 3ef82f5... when `production` stopped subtracting weekly output from every
        // facility's `deposit_remaining`, which is only stocked for ore mines; ordinary plants
        // had been drifting to large negative deposits. The economy is unchanged otherwise. On
        // 2026-10-27 it moved to daca9262... when the world gained its world price state (empty
        // here, as this fixture runs without world prices), and again in 1.7 preview 4 when the
        // world gained government laws, the reserve and systemic incidents, and again in 1.7
        // preview 5 when rivals chose moves by temperament and the world gained grudges and
        // alliances. In 1.8 preview 3 it moved from f85add77... when sellers stopped reading
        // their own empty post-trade shelf as scarcity (prices now rise on buyers' unmet input
        // cover and fall on unsold stock), services priced on spare installed capacity and
        // carriers cut below 60 percent lane fill, so week-two asking prices differ. It moved
        // again from 1b265292... in the same preview when facilities gained a cut record,
        // households gained wanted and unmet demand and policies a production lock (new
        // encoded fields), and refineries began shifting their cut. The Swift
        // constant
        // must be updated
        // in the same change and re-verified on the Mac runner before R05 claims cross-platform
        // equality again.
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
            "2d104b6095f5d8996848d9abfddcf531d23ea0aa71aaf7e2fa566b0b36e1124c"
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

    /// Answer every waiting mandatory player card with its first choice, as the inbox would.
    fn answer_cards(session: &mut Session) {
        let waiting: Vec<(u64, String)> = session
            .world
            .events
            .pending
            .iter()
            .filter(|event| !event.resolved && event.mandatory)
            .filter_map(|event| {
                let card = session.dataset.event(&event.card)?;
                Some((event.id, card.choices.first()?.id.clone()))
            })
            .collect();
        for (event, choice) in waiting {
            let _ = session.submit(cw_sim::commands::CommandEnvelope {
                command_id_high: 0xCA2D,
                command_id_low: event,
                actor: 0,
                expected_revision: session.revision(),
                submitted_tick: session.world.tick.0,
                intent: CommandIntent::RespondEvent { event, choice },
            });
        }
    }

    #[test]
    fn the_five_year_score_pauses_and_the_career_continues() {
        let mut session = Session::create(config(201)).unwrap();
        // The five-year mode pauses exactly at week 260 and records a score. Advance is capped
        // at sixteen weeks per call, so a caller loops until the pause.
        let mut result = session.advance(16);
        while result.final_week < 260 && result.pause.as_deref() != Some("five_year_score") {
            if result.pause.is_some() {
                answer_cards(&mut session);
                session.dismiss_pause();
            }
            result = session.advance(16);
        }
        assert_eq!(result.final_week, 260);
        assert_eq!(result.pause.as_deref(), Some("five_year_score"));
        let score = session.world.score.clone().expect("a score is recorded");
        assert_eq!(score.week.0, 260);
        assert!((0..=100).contains(&score.total));
        // Continuing from the score sheet is an ordinary advance, not a new career.
        session.dismiss_pause();
        // An event card can pause the continued career too; answering it is ordinary play.
        let mut weeks = 0;
        for _ in 0..8 {
            weeks += session.advance(4 - weeks).weeks_advanced;
            if weeks >= 4 {
                break;
            }
            answer_cards(&mut session);
            session.dismiss_pause();
        }
        assert_eq!(weeks, 4);
        assert_eq!(session.world.tick.0, 264);
        assert!(session.world.score.is_some(), "the recorded score is kept");
    }

    #[test]
    fn score_net_worth_counts_direct_holdings_once_and_ignores_firm_cash() {
        let session = Session::create(config(202)).unwrap();
        let world = &session.world;
        let mut expected = world.investor.cash_cents - world.investor.personal_loans_cents;
        for (_, firm) in world.firms.iter() {
            expected += firm.shares.qty(cw_sim::state::HolderId::Investor) * firm.shares.fair_cents;
        }
        assert_eq!(
            cw_sim::economy::score::personal_net_worth(world),
            expected.max(0)
        );
        // The guided company's cash is not the investor's cash.
        let primary = world.investor.primary_group.unwrap();
        let firm_cash = world.firm(primary).unwrap().book.cash();
        assert!(firm_cash > 0);
        assert!(
            cw_sim::economy::score::personal_net_worth(world)
                != world.investor.cash_cents + firm_cash
        );
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
                lock: false,
            },
        };
        session.submit(envelope.clone()).unwrap();
        let before = session.revision();
        let receipt = session.submit(envelope).unwrap();
        assert_eq!(receipt.sequence, 0);
        assert_eq!(session.revision(), before);
    }
}
