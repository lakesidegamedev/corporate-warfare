//! Executives for the apps (1.9): the three posts of a company, who holds them, who can be hired
//! and at what cost. The engine keeps the state (`cw_sim::economy::executives`); hiring, dismissing
//! and poaching go through the ordinary command path (`HireExecutive`, `DismissExecutive`,
//! `PoachExecutive`, in the action catalogue).

use cw_core::ids::Handle;
use cw_data::Dataset;
use cw_sim::economy::executives::{self as ex, Candidate, Executive, Post};
use cw_sim::state::World;

use crate::SessionHandle;

/// Someone who can be hired, with what they would do and cost at this company.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CandidateDto {
    /// The number to send with `HireExecutive`.
    pub id: u32,
    pub name: String,
    pub age: u32,
    pub home: String,
    pub background: String,
    /// integrator, raider, financier, speculator, builder or cautious.
    pub temperament: String,
    /// Names the portrait in the generated portrait atlas.
    pub portrait_id: String,
    /// One line on what the person is good at, for the named people for hire; empty otherwise.
    pub hire_note: String,
    /// A named member of the cast rather than a generated person.
    pub named: bool,
    pub level: u32,
    /// What they would change, in plain words with the numbers.
    pub effect: String,
    /// Weekly pay they ask of this company.
    pub ask_cents: i64,
    /// Paid up front, four weeks of pay.
    pub signing_fee_cents: i64,
    /// What the effect is worth to this company over a year, from its own figures.
    pub yearly_value_cents: i64,
    /// Weeks for the effect to earn back the signing fee after pay; zero when it does not.
    pub payback_weeks: u32,
    /// Enough cash for the fee and twenty-six weeks of pay.
    pub affordable: bool,
}

/// Someone holding a post.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ExecutiveDto {
    pub name: String,
    pub age: u32,
    pub home: String,
    pub background: String,
    pub temperament: String,
    pub portrait_id: String,
    pub hire_note: String,
    pub named: bool,
    pub level: u32,
    pub effect: String,
    pub pay_cents: i64,
    pub weeks_left: u32,
    /// What dismissing them would cost now.
    pub severance_cents: i64,
    /// What a rival must offer a week to move them, when they are not yours.
    pub poach_price_cents: i64,
}

/// One post of a company.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ExecutivePostDto {
    /// cfo, coo or logistics.
    pub post: String,
    /// The number to send with the commands.
    pub post_code: u32,
    pub title: String,
    /// What the post does at level five, for an empty post.
    pub blurb: String,
    pub holder: Option<ExecutiveDto>,
}

/// A company's executives and the people it could hire.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ExecutivesDto {
    pub firm: u64,
    pub posts: Vec<ExecutivePostDto>,
    /// Everyone available, best level first, grouped by post in the order of `posts`.
    pub candidates: Vec<CandidatePostDto>,
    /// Weekly pay of the executives held now.
    pub weekly_pay_cents: i64,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CandidatePostDto {
    pub post: String,
    pub post_code: u32,
    pub people: Vec<CandidateDto>,
}

fn candidate_dto(world: &World, ds: &Dataset, firm: Handle, c: &Candidate) -> Option<CandidateDto> {
    let hire = ex::hire_of(c)?;
    let extra = hire.person;
    let ask = ex::ask_cents(world, firm, c);
    let fee = ask * ex::SIGNING_WEEKS;
    let value = ex::yearly_value_cents(world, ds, firm, c.post, c.level);
    let gain = value / 52 - ask;
    let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
    Some(CandidateDto {
        id: c.id as u32,
        name: extra.name.clone(),
        age: extra.age,
        home: extra.home.clone(),
        background: extra.background.clone(),
        temperament: extra.temperament.key().to_owned(),
        portrait_id: extra.id.clone(),
        hire_note: extra.hire_note,
        named: hire.named,
        level: c.level as u32,
        effect: ex::effect_text(c.post, c.level),
        ask_cents: ask,
        signing_fee_cents: fee,
        yearly_value_cents: value,
        payback_weeks: if gain > 0 {
            ((fee + gain - 1) / gain).min(9_999) as u32
        } else {
            0
        },
        affordable: cash >= fee + ask * ex::CASH_COVER_WEEKS,
    })
}

fn executive_dto(world: &World, e: &Executive, poach: bool) -> Option<ExecutiveDto> {
    let c = world.executives.person(e.person)?;
    let hire = ex::hire_of(c)?;
    let extra = hire.person;
    Some(ExecutiveDto {
        name: extra.name.clone(),
        age: extra.age,
        home: extra.home.clone(),
        background: extra.background.clone(),
        temperament: extra.temperament.key().to_owned(),
        portrait_id: extra.id.clone(),
        hire_note: extra.hire_note,
        named: hire.named,
        level: c.level as u32,
        effect: ex::effect_text(e.post, c.level),
        pay_cents: e.pay_cents,
        weeks_left: e.expires.saturating_sub(world.tick.0),
        severance_cents: ex::severance_cents(world, e),
        poach_price_cents: if poach {
            ex::poach_price_cents(world, e.firm, e.post).unwrap_or(0)
        } else {
            0
        },
    })
}

fn blurb(post: Post) -> String {
    format!("At level five: {}.", ex::effect_text(post, 5))
}

/// The posts of a company with whoever holds them. `poach` fills the price a rival would pay.
pub fn posts(world: &World, firm: Handle, poach: bool) -> Vec<ExecutivePostDto> {
    Post::ALL
        .iter()
        .map(|post| ExecutivePostDto {
            post: post.key().to_owned(),
            post_code: post.code() as u32,
            title: post.title().to_owned(),
            blurb: blurb(*post),
            holder: world
                .executives
                .holder(firm, *post)
                .and_then(|e| executive_dto(world, e, poach)),
        })
        .collect()
}

pub fn executives(world: &World, ds: &Dataset, firm: u64) -> ExecutivesDto {
    let handle = Handle(firm);
    let posts = posts(world, handle, false);
    let candidates = Post::ALL
        .iter()
        .map(|post| CandidatePostDto {
            post: post.key().to_owned(),
            post_code: post.code() as u32,
            people: ex::candidates(world, *post)
                .into_iter()
                .filter_map(|c| candidate_dto(world, ds, handle, c))
                .collect(),
        })
        .collect();
    ExecutivesDto {
        firm,
        posts,
        candidates,
        weekly_pay_cents: world.executives.of_firm(handle).map(|e| e.pay_cents).sum(),
    }
}

#[uniffi::export]
impl SessionHandle {
    /// A company's executive posts, who holds them, and the people it could hire with the effect
    /// and cost each would have at that company.
    pub fn executives(&self, firm: u64) -> ExecutivesDto {
        let session = self.lock();
        executives(&session.world, &session.dataset, firm)
    }
}
