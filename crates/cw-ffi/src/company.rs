//! Company pages: logo, chief executive and the investor's view of one firm.
//!
//! What a page shows follows what the investor can know. Listed firms (with shares on the
//! exchange) show their price, accounts, plants and holders; private firms show only what is
//! public: name, logo, chief executive, sector, regions, size band and recent moves; the
//! investor's own firms show everything.

use cw_core::ids::Handle;
use cw_session::{Detail, Query, Session};
use cw_sim::state::{HolderId, World};

use crate::cast::{DossierDto, MoveDto, PersonDto};
use crate::{FinanceDto, HolderDto, NewsDto, StockDto};

/// A generated logo: `rows` use '.' for clear, 'p' plate, 'e' edge and 's' symbol, coloured by
/// the three hex colours.
#[derive(Clone, uniffi::Record)]
pub struct LogoDto {
    pub plate: String,
    pub edge: String,
    pub symbol: String,
    pub rows: Vec<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct RegionPresenceDto {
    pub region_name: String,
    pub plants: u32,
    pub blocks: u32,
}

#[derive(Clone, uniffi::Record)]
pub struct DivisionLineDto {
    pub region_name: String,
    pub industry: String,
    pub strategy_name: String,
    pub plants: u32,
    pub blocks: u32,
}

/// A related company with a plain note: a subsidiary, a parent or a joint venture partner.
#[derive(Clone, uniffi::Record)]
pub struct LinkedFirmDto {
    pub firm: u64,
    pub name: String,
    pub note: String,
}

#[derive(uniffi::Record)]
pub struct CompanyPageDto {
    pub firm: u64,
    pub name: String,
    /// listed, private or yours.
    pub kind: String,
    pub industry: String,
    pub home_region: String,
    /// Small, Mid-sized, Large or Major, from the plants the firm runs.
    pub size_band: String,
    pub ceo: Option<PersonDto>,
    pub ceo_is_you: bool,
    pub ceo_weeks: u32,
    pub style: String,
    pub regions: Vec<RegionPresenceDto>,
    pub stock: Option<StockDto>,
    pub finance: Option<FinanceDto>,
    pub divisions: Vec<DivisionLineDto>,
    pub holders: Vec<HolderDto>,
    pub controller: Option<String>,
    pub parent: Option<LinkedFirmDto>,
    pub subsidiaries: Vec<LinkedFirmDto>,
    pub ventures: Vec<LinkedFirmDto>,
    pub moves: Vec<MoveDto>,
    pub grudges: Vec<String>,
    pub allies: Vec<String>,
    pub news: Vec<NewsDto>,
}

impl CompanyPageDto {
    #[cfg(test)]
    pub(crate) fn rows_empty_logo(&self) -> bool {
        firm_logo(self.name.clone())
            .rows
            .iter()
            .all(|r| !r.contains('s'))
    }
}

#[uniffi::export]
pub fn firm_logo(name: String) -> LogoDto {
    let logo = cw_data::logo::compose(&name);
    LogoDto {
        plate: logo.plate,
        edge: logo.edge,
        symbol: logo.symbol,
        rows: logo.rows,
    }
}

/// The chief executive of a firm, named: a cast member if the engine appointed one, else a
/// generated chief executive picked from the name and the firm's style. Generated ones are
/// spread so no two firms in a world share one. The investor's own firms have none. Letters
/// pick their senders the same way (`cw_session::letters::chief`).
pub(crate) fn resolved_ceo(session: &Session, firm: Handle) -> Option<PersonDto> {
    let p = cw_session::letters::chief(&session.world, &session.dataset, firm)?;
    Some(PersonDto {
        id: p.id,
        name: p.name,
        age: p.age,
        home: p.home,
        background: p.background,
        temperament: p.temperament.key().to_owned(),
        roles: p
            .roles
            .iter()
            .map(|r| {
                r.key().to_owned()
            })
            .collect(),
        stance: p.stance,
        mood: cw_data::Mood::Neutral.key().to_owned(),
        office_portrait: p.office_portrait,
    })
}

/// The firm a line of news names, longest name first.
pub(crate) fn firm_named(world: &World, text: &str) -> Option<(u64, String)> {
    world
        .firms
        .iter()
        .filter(|(_, f)| f.name.len() >= 4 && text.contains(f.name.as_str()))
        .max_by_key(|(h, f)| (f.name.len(), std::cmp::Reverse(h.0)))
        .map(|(h, f)| (h.0, f.name.clone()))
}

fn size_band(blocks: u32) -> &'static str {
    match blocks {
        0..=14 => "Small",
        15..=39 => "Mid-sized",
        40..=89 => "Large",
        _ => "Major",
    }
}

pub(crate) fn page(session: &Session, firm: u64) -> Option<CompanyPageDto> {
    use cw_sim::economy::sectors;
    let world = &session.world;
    let dataset = &session.dataset;
    let handle = Handle(firm);
    let entity = world.firms.get(handle)?;
    let yours = cw_sim::economy::events_engine::player_side(world, handle);
    let listed = entity.on_board();
    let kind = if yours {
        "yours"
    } else if listed {
        "listed"
    } else {
        "private"
    };
    let full = yours || session.can_inspect(firm);
    let dossier: DossierDto = crate::cast::dossier(session, firm)?;
    let divisions = sectors::divisions_of(world, dataset, handle);
    let mut regions: Vec<RegionPresenceDto> = Vec::new();
    for d in &divisions {
        let name = dataset.regions[d.region as usize].name.clone();
        match regions.iter_mut().find(|r| r.region_name == name) {
            Some(r) => {
                r.plants += d.plants.len() as u32;
                r.blocks += d.blocks;
            }
            None => regions.push(RegionPresenceDto {
                region_name: name,
                plants: d.plants.len() as u32,
                blocks: d.blocks,
            }),
        }
    }
    let total_blocks: u32 = divisions.iter().map(|d| d.blocks).sum();
    let stock = if listed {
        cw_session::dto::stock_board(world)
            .into_iter()
            .find(|l| l.firm == firm)
            .map(crate::stock_dto)
    } else {
        None
    };
    let (finance, holders, controller) = if full {
        let finance = match session.query(Query::Finance(firm)) {
            Detail::Finance(row) => Some(crate::finance_dto(row)),
            _ => None,
        };
        match session.query(Query::Ownership(firm)) {
            Detail::Ownership(row) => {
                let o = crate::ownership_dto(row);
                (finance, o.holders, o.controller)
            }
            _ => (finance, Vec::new(), None),
        }
    } else {
        (None, Vec::new(), None)
    };
    let name_of = |h: Handle| {
        world
            .firms
            .get(h)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "a former rival".to_owned())
    };
    let (mut parent, mut subsidiaries, mut ventures) = (None, Vec::new(), Vec::new());
    if full {
        if let Some(HolderId::Firm(p)) = entity.shares.controller() {
            parent = Some(LinkedFirmDto {
                firm: p.0,
                name: name_of(p),
                note: format!(
                    "controls {:.1}% of the votes",
                    f64::from(entity.shares.vote_bps(HolderId::Firm(p))) / 100.0
                ),
            });
        }
        for (h, f) in world.firms.iter() {
            if h == handle || !f.operative() {
                continue;
            }
            if f.shares.controller() == Some(HolderId::Firm(handle))
                && !world.structure.jvs.iter().any(|j| j.firm == h)
            {
                subsidiaries.push(LinkedFirmDto {
                    firm: h.0,
                    name: f.name.clone(),
                    note: format!(
                        "{:.1}% of the votes",
                        f64::from(f.shares.vote_bps(HolderId::Firm(handle))) / 100.0
                    ),
                });
            }
        }
        for j in &world.structure.jvs {
            let other = if j.a == handle {
                j.b
            } else if j.b == handle {
                j.a
            } else {
                continue;
            };
            let Some(v) = world.firms.get(j.firm) else {
                continue;
            };
            ventures.push(LinkedFirmDto {
                firm: j.firm.0,
                name: v.name.clone(),
                note: format!(
                    "with {}, your side holds {:.1}%",
                    name_of(other),
                    f64::from(v.shares.vote_bps(HolderId::Firm(handle))) / 100.0
                ),
            });
        }
    }
    let news: Vec<NewsDto> = world
        .politics
        .news
        .iter()
        .rev()
        .filter(|n| n.text.contains(entity.name.as_str()))
        .take(6)
        .map(|n| NewsDto {
            week: n.week,
            text: n.text.clone(),
            person: n
                .person
                .and_then(|p| dataset.cast.get(p as usize))
                .map(|p| p.id.clone()),
            firm: Some(firm),
            firm_name: Some(entity.name.clone()),
        })
        .collect();
    Some(CompanyPageDto {
        firm,
        name: entity.name.clone(),
        kind: kind.to_owned(),
        industry: cw_sim::state::INDUSTRY_NAMES[entity.industry as usize].to_owned(),
        home_region: world.regions[entity.region as usize].name.clone(),
        size_band: size_band(total_blocks).to_owned(),
        ceo_is_you: yours && entity.ceo.is_none(),
        ceo_weeks: dossier.ceo_weeks,
        style: dossier.style,
        ceo: dossier.ceo,
        regions,
        stock,
        finance,
        divisions: if full {
            divisions
                .iter()
                .map(|d| {
                    let recipe = &dataset.recipes[d.strategy as usize];
                    DivisionLineDto {
                        region_name: dataset.regions[d.region as usize].name.clone(),
                        industry: cw_sim::state::INDUSTRY_NAMES[d.industry as usize].to_owned(),
                        strategy_name: if recipe.name.is_empty() {
                            recipe.id.clone()
                        } else {
                            recipe.name.clone()
                        },
                        plants: d.plants.len() as u32,
                        blocks: d.blocks,
                    }
                })
                .collect()
        } else {
            Vec::new()
        },
        holders,
        controller,
        parent,
        subsidiaries,
        ventures,
        moves: dossier.moves,
        grudges: dossier.grudges,
        allies: dossier.allies,
        news,
    })
}
