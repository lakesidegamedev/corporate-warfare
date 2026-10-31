//! The weekly paper for the apps: a front page written by the engine from facts, back issues and
//! a ticker of the top headlines.

use cw_session::paper::{self, Classified, GoodMove, Markets, Paper, PaperLink, ShareMove, Story};

use crate::SessionHandle;

/// What a story opens: company, region, good, deal, event or none.
#[derive(Clone, uniffi::Record)]
pub struct PaperLinkDto {
    pub kind: String,
    /// Company handle, region index, good code or deal handle.
    pub id: u64,
    /// Timeline event id, for event links.
    pub key: String,
}

#[derive(Clone, uniffi::Record)]
pub struct StoryDto {
    pub id: String,
    pub headline: String,
    pub body: String,
    pub byline: String,
    pub link: PaperLinkDto,
    pub week: u32,
    /// The player is party to the story.
    pub yours: bool,
    /// The journalist's portrait id, empty when the scenario has none.
    pub author: String,
    pub author_name: String,
    /// neutral, pleased or displeased: which face of the journalist to draw.
    pub mood: String,
}

#[derive(Clone, uniffi::Record)]
pub struct ShareMoveDto {
    pub firm: u64,
    pub name: String,
    pub price_cents: i64,
    pub change_bps: i64,
}

#[derive(Clone, uniffi::Record)]
pub struct GoodMoveDto {
    pub product: u32,
    pub name: String,
    pub price_cents: i64,
    pub change_bps: i64,
    /// The market screen's top reason for the price now; empty for a back issue.
    pub reason: String,
}

#[derive(Clone, uniffi::Record)]
pub struct MarketsDto {
    pub open: bool,
    pub index_change_bps: i64,
    pub shares: Vec<ShareMoveDto>,
    pub goods: Vec<GoodMoveDto>,
}

#[derive(Clone, uniffi::Record)]
pub struct ClassifiedDto {
    /// bid, restructuring or liquidation.
    pub kind: String,
    pub text: String,
    pub link: PaperLinkDto,
}

#[derive(Clone, uniffi::Record)]
pub struct PaperDto {
    pub week: u32,
    pub masthead: String,
    pub date_line: String,
    pub lead: Option<StoryDto>,
    pub briefs: Vec<StoryDto>,
    pub markets: MarketsDto,
    pub classifieds: Vec<ClassifiedDto>,
}

fn link_dto(link: PaperLink) -> PaperLinkDto {
    PaperLinkDto {
        kind: link.kind,
        id: link.id,
        key: link.key,
    }
}

fn story_dto(story: Story) -> StoryDto {
    StoryDto {
        id: story.id,
        headline: story.headline,
        body: story.body,
        byline: story.byline,
        link: link_dto(story.link),
        week: story.week,
        yours: story.yours,
        author: story.author,
        author_name: story.author_name,
        mood: story.mood.key().to_owned(),
    }
}

fn share_dto(m: ShareMove) -> ShareMoveDto {
    ShareMoveDto {
        firm: m.firm,
        name: m.name,
        price_cents: m.price_cents,
        change_bps: m.change_bps,
    }
}

fn good_dto(m: GoodMove) -> GoodMoveDto {
    GoodMoveDto {
        product: m.product as u32,
        name: m.name,
        price_cents: m.price_cents,
        change_bps: m.change_bps,
        reason: m.reason,
    }
}

fn markets_dto(m: Markets) -> MarketsDto {
    MarketsDto {
        open: m.open,
        index_change_bps: m.index_change_bps,
        shares: m.shares.into_iter().map(share_dto).collect(),
        goods: m.goods.into_iter().map(good_dto).collect(),
    }
}

fn classified_dto(c: Classified) -> ClassifiedDto {
    ClassifiedDto {
        kind: c.kind,
        text: c.text,
        link: link_dto(c.link),
    }
}

fn paper_dto(p: Paper) -> PaperDto {
    PaperDto {
        week: p.week,
        masthead: p.masthead,
        date_line: p.date_line,
        lead: p.lead.map(story_dto),
        briefs: p.briefs.into_iter().map(story_dto).collect(),
        markets: markets_dto(p.markets),
        classifieds: p.classifieds.into_iter().map(classified_dto).collect(),
    }
}

#[uniffi::export]
impl SessionHandle {
    /// The paper for a week; a week past today is today's. A good's reason is the market
    /// screen's strongest driver, which describes the market as it stands, so only today's
    /// issue carries reasons.
    pub fn paper(&self, week: u32) -> PaperDto {
        let (mut dto, current) = {
            let session = self.lock();
            let now = session.world.tick.0;
            let built = paper::weekly_paper(&session.world, &session.dataset, week);
            (paper_dto(built), now.max(1))
        };
        if dto.week == current {
            for good in &mut dto.markets.goods {
                if let Some(depth) = self.good_depth(good.product) {
                    let rising = good.change_bps > 0;
                    good.reason = depth
                        .drivers
                        .iter()
                        .filter(|d| d.push == 0 || (d.push > 0) == rising)
                        .max_by_key(|d| d.push.abs())
                        .or_else(|| depth.drivers.first())
                        .map(|d| d.text.clone())
                        .unwrap_or_default();
                }
            }
        }
        dto
    }

    /// The weeks with a back issue, newest first.
    pub fn paper_archive(&self) -> Vec<u32> {
        paper::archive_weeks(&self.lock().world)
    }

    /// The top three headlines of today's paper, for the ticker.
    pub fn ticker(&self) -> Vec<String> {
        let session = self.lock();
        let week = session.world.tick.0;
        paper::ticker(&paper::weekly_paper(&session.world, &session.dataset, week))
    }
}
