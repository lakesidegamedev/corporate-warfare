//! Every player action, described once for every app: its category, a plain explanation, and
//! the fields its form needs. The apps render one searchable Command Center and generate each
//! form from this list, then submit the action as JSON through `submit_json` and preview it with
//! `preview_json`. A new engine command needs only a new entry here to appear on every platform.

/// One option in a choice field.
#[derive(uniffi::Record, Clone)]
pub struct OptionDto {
    pub value: String,
    pub label: String,
}

/// One input of an action's form.
///
/// `kind` tells the app which control to draw and where its options come from:
/// `my_firm` (a company the player controls, defaulting to the selected one), `any_firm`,
/// `bank`, `insurer`, `region`, `product`, `recipe`, `sector`, `my_facility`, `my_loan`,
/// `my_contract`, `my_deal`, `my_policy`, `vote`, `money` (cents), `lots`, `shares`, `bps`
/// (a percentage in basis points), `weeks`, `count`, `bool`, `choice` and `text`.
/// `encoding` is how the value goes into the JSON: `number`, `string`, `bool` or `optional`
/// (a number, or null when left empty).
#[derive(uniffi::Record, Clone)]
pub struct FieldDto {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub encoding: String,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub default_value: i64,
    pub help: String,
    pub choices: Vec<OptionDto>,
}

#[derive(uniffi::Record, Clone)]
pub struct ActionDto {
    /// The engine command, for example `LaunchIPO`.
    pub command: String,
    pub category: String,
    pub title: String,
    pub summary: String,
    /// Search words beyond the title.
    pub keywords: String,
    pub fields: Vec<FieldDto>,
}

fn f(key: &str, label: &str, kind: &str) -> FieldDto {
    let encoding = match kind {
        "bool" => "bool",
        "text" => "string",
        _ => "number",
    };
    FieldDto {
        key: key.to_owned(),
        label: label.to_owned(),
        kind: kind.to_owned(),
        encoding: encoding.to_owned(),
        min: 0,
        max: 0,
        step: 1,
        default_value: 0,
        help: String::new(),
        choices: Vec::new(),
    }
}

impl FieldDto {
    fn range(mut self, min: i64, max: i64, step: i64, default: i64) -> Self {
        self.min = min;
        self.max = max;
        self.step = step;
        self.default_value = default;
        self
    }

    fn help(mut self, text: &str) -> Self {
        self.help = text.to_owned();
        self
    }

    fn choices(mut self, encoding: &str, items: &[(&str, &str)]) -> Self {
        self.encoding = encoding.to_owned();
        self.choices = items
            .iter()
            .map(|(value, label)| OptionDto {
                value: (*value).to_owned(),
                label: (*label).to_owned(),
            })
            .collect();
        self
    }

    fn optional(mut self) -> Self {
        self.encoding = "optional".to_owned();
        self
    }
}

fn a(
    command: &str,
    category: &str,
    title: &str,
    summary: &str,
    keywords: &str,
    fields: Vec<FieldDto>,
) -> ActionDto {
    ActionDto {
        command: command.to_owned(),
        category: category.to_owned(),
        title: title.to_owned(),
        summary: summary.to_owned(),
        keywords: keywords.to_owned(),
        fields,
    }
}

fn firm() -> FieldDto {
    f("firm", "Company", "my_firm")
}

fn money(key: &str, label: &str) -> FieldDto {
    f(key, label, "money")
}

/// The full catalogue, in the order the Command Center lists it.
#[uniffi::export]
pub fn action_catalogue() -> Vec<ActionDto> {
    let tiers = [("0", "Low skill"), ("1", "Mid skill"), ("2", "High skill")];
    vec![
        // Company
        a(
            "FoundFirm",
            "Company",
            "Found a company",
            "Start a new company with your own capital: pick its headquarters, first strategy and how much to put in.",
            "start new business create incorporate",
            vec![
                f("name", "Name", "text"),
                f("region", "Headquarters", "region"),
                f("recipe", "First strategy", "recipe"),
                money("capital_cents", "Capital to invest")
                    .range(100_000_000, 0, 500_000, 250_000_000)
                    .help("Moves from your own cash into the company."),
                f("secondary", "Second sector (optional)", "sector").optional(),
            ],
        ),
        a(
            "SetPrimaryGroup",
            "Company",
            "Make this your main company",
            "The main company is the one the map, HUD and objectives follow.",
            "primary main headline",
            vec![firm()],
        ),
        a(
            "SetSectorFocus",
            "Company",
            "Change sector focus",
            "Set the primary sector and an optional second one. Changing focus takes a transition period.",
            "focus refocus diversify",
            vec![
                firm(),
                f("primary", "Primary sector", "sector"),
                f("secondary", "Second sector (optional)", "sector").optional(),
            ],
        ),
        // Operations
        a(
            "BuildFacility",
            "Operations",
            "Build plants",
            "Build new plants of a strategy in a region. They open after the strategy's build time.",
            "construct factory plant capacity expand new",
            vec![
                firm(),
                f("region", "Region", "region"),
                f("recipe", "Strategy", "recipe"),
                f("blocks", "Plants", "count").range(1, 20, 1, 1),
            ],
        ),
        a(
            "ExpandFacility",
            "Operations",
            "Expand a site",
            "Add plants to an existing site of the same strategy.",
            "grow capacity add",
            vec![
                f("facility", "Site", "my_facility"),
                f("blocks", "Plants to add", "count").range(1, 20, 1, 1),
            ],
        ),
        a(
            "RetoolFacility",
            "Operations",
            "Retool a site",
            "Switch a site to another strategy of its sector for a quarter of the new capex.",
            "convert switch strategy",
            vec![
                f("facility", "Site", "my_facility"),
                f("recipe", "New strategy", "recipe"),
            ],
        ),
        a(
            "MothballFacility",
            "Operations",
            "Mothball a site",
            "Stop a site and release its staff until you reopen it.",
            "close pause idle shut",
            vec![f("facility", "Site", "my_facility")],
        ),
        a(
            "ReopenFacility",
            "Operations",
            "Reopen a mothballed site",
            "Restart a mothballed site; staff are hired again over the following weeks.",
            "restart",
            vec![f("facility", "Site", "my_facility")],
        ),
        a(
            "DisposeFacility",
            "Operations",
            "Sell off a site",
            "Dispose of a site for its salvage value.",
            "sell scrap divest close",
            vec![f("facility", "Site", "my_facility")],
        ),
        a(
            "SetProductionTarget",
            "Operations",
            "Production target",
            "How hard every plant runs, as a share of full capacity.",
            "output utilisation run rate",
            vec![
                firm(),
                f("target_bps", "Target", "bps").range(0, 10_000, 100, 8_500),
            ],
        ),
        a(
            "SetAutoProduction",
            "Operations",
            "Automatic production planning",
            "Let the weekly sell-through rule set the production target.",
            "auto plan",
            vec![firm(), f("enabled", "On", "bool")],
        ),
        a(
            "SetStockTarget",
            "Supply",
            "Stock target",
            "Weeks of inputs a site keeps on hand. Zero uses the company's default cover. Seasonal build buys grain ahead in the harvest quarter.",
            "inventory cover buffer safety stock season harvest",
            vec![
                f("facility", "Site", "my_facility"),
                f("weeks", "Weeks of inputs", "count").range(0, 26, 1, 0),
                f("seasonal", "Seasonal build", "bool").range(0, 1, 1, 1),
            ],
        ),
        a(
            "BuyToStore",
            "Supply",
            "Buy to store",
            "Buy a good on the exchange into storage to sell later. Fills over the next weeks at no more than your price limit. Stored goods are not for sale until you release them.",
            "speculate stockpile hoard commodity buy inventory",
            vec![
                firm(),
                f("product", "Good", "product"),
                f("region", "Store in", "region"),
                f("lots", "Lots", "count").range(1, 5_000, 10, 100),
                f("limit_bps", "Price limit (percent of reference)", "bps")
                    .range(5_000, 20_000, 500, 10_000),
            ],
        ),
        a(
            "SellFromStore",
            "Supply",
            "Sell from store",
            "Release stored goods for sale at your asking price. Zero lots releases everything held there and cancels its open buy order.",
            "release speculate unload stockpile",
            vec![
                firm(),
                f("product", "Good", "product"),
                f("region", "Stored in", "region"),
                f("lots", "Lots (0 for all)", "count").range(0, 20_000, 10, 0),
            ],
        ),
        a(
            "SetLogisticsBudget",
            "Operations",
            "Logistics programme",
            "Weekly spend that builds logistics strength and cuts freight costs over time.",
            "freight shipping supply chain",
            vec![
                firm(),
                money("amount_cents", "Weekly budget").range(0, 5_000_000, 50_000, 0),
            ],
        ),
        // Pricing and sales
        a(
            "SetPricePosture",
            "Sales",
            "Pricing posture",
            "Price a good automatically, against the market, or as a markup over your cost.",
            "price discount markup undercut",
            vec![
                firm(),
                f("product", "Good", "product"),
                f("mode", "How", "choice").choices(
                    "number",
                    &[
                        ("0", "Automatic"),
                        ("1", "Against the market"),
                        ("2", "Over your cost"),
                    ],
                ),
                f("bps", "Percent", "bps")
                    .range(-5_000, 10_000, 100, 0)
                    .help("Against the market: -50 to +50 percent. Over cost: 0 to 100 percent."),
            ],
        ),
        a(
            "SetPrice",
            "Sales",
            "Set an exact price",
            "Fix a good's price per lot until you change it.",
            "price fixed manual",
            vec![
                firm(),
                f("product", "Good", "product"),
                money("price_cents", "Price per lot").range(100, 0, 100, 10_000),
            ],
        ),
        a(
            "SetMarketingBudget",
            "Sales",
            "Marketing budget",
            "Weekly brand spend that lifts demand for your goods in your regions.",
            "advertising brand campaign",
            vec![
                firm(),
                money("amount_cents", "Weekly budget").range(0, 5_000_000, 50_000, 0),
            ],
        ),
        a(
            "SetQualityBudget",
            "Sales",
            "Quality budget",
            "Weekly spend on one good's quality; better goods win buyers at the same price.",
            "quality improve product",
            vec![
                firm(),
                f("product", "Good", "product"),
                money("amount_cents", "Weekly budget").range(0, 2_000_000, 25_000, 0),
            ],
        ),
        a(
            "RaidMarket",
            "Sales",
            "Raid a rival's market",
            "Spend on a campaign against a rival division in a region and sector you both operate in, to take some of its plants.",
            "attack steal share campaign",
            vec![
                firm(),
                f("target", "Rival", "any_firm"),
                f("region", "Region", "region"),
                f("industry", "Sector", "sector"),
                money("budget_cents", "Campaign budget").range(0, 20_000_000, 100_000, 2_000_000),
            ],
        ),
        // Research
        a(
            "SetResearchBudget",
            "Research",
            "Research budget",
            "Weekly research spend. Each new technology level makes plants use fewer inputs and workers and lifts quality.",
            "tech technology r&d innovation",
            vec![
                firm(),
                money("amount_cents", "Weekly budget").range(0, 5_000_000, 50_000, 0),
            ],
        ),
        // Workforce
        a(
            "SetWageOffer",
            "Workforce",
            "Wage offer",
            "Weekly pay offered to one skill tier. Pay under the market slows hiring and builds union support.",
            "pay salary hire staff",
            vec![
                firm(),
                f("tier", "Skill tier", "choice").choices("number", &tiers),
                money("wage_cents", "Weekly wage").range(10_000, 400_000, 1_000, 40_000),
            ],
        ),
        a(
            "SetTrainingBudget",
            "Workforce",
            "Training budget",
            "Weekly training that moves workers up a skill tier. A regional grant covers a fifth.",
            "skills train upskill",
            vec![
                firm(),
                money("amount_cents", "Weekly budget").range(0, 2_000_000, 25_000, 0),
            ],
        ),
        a(
            "RespondBargaining",
            "Workforce",
            "Answer a union demand",
            "Accept, counter or reject the union's demand.",
            "union strike negotiate",
            vec![
                firm(),
                f("choice", "Answer", "choice").choices(
                    "string",
                    &[
                        ("Accept", "Accept"),
                        ("Counter", "Counter"),
                        ("Reject", "Reject"),
                    ],
                ),
                money("offer_cents", "Counter offer (weekly)").range(0, 400_000, 1_000, 0),
            ],
        ),
        a(
            "ProposeSettlement",
            "Workforce",
            "Offer a strike settlement",
            "A one-off payment to end a strike.",
            "strike end pay",
            vec![
                firm(),
                money("amount_cents", "Settlement").range(0, 20_000_000, 100_000, 1_000_000),
            ],
        ),
        // Supply
        a(
            "OfferContract",
            "Supply",
            "Offer a supply contract",
            "A weekly delivery of a good between two companies at a set price for a number of weeks.",
            "deal supplier buyer delivery",
            vec![
                f("buyer", "Buyer", "any_firm"),
                f("seller", "Seller", "any_firm"),
                f("product", "Good", "product"),
                f("from_region", "From", "region"),
                f("to_region", "To", "region"),
                f("quantity_lots", "Lots a week", "lots").range(1, 10_000, 1, 50),
                money("price_cents", "Price per lot").range(100, 0, 100, 10_000),
                f("weeks", "Weeks", "weeks").range(1, 260, 1, 26),
                f("penalty_bps", "Shortfall penalty", "bps").range(0, 5_000, 100, 500),
                money("escrow_cents", "Escrow").range(0, 0, 10_000, 0),
            ],
        ),
        a(
            "AcceptContract",
            "Supply",
            "Accept a contract",
            "Accept a contract offered to you.",
            "agree sign",
            vec![f("contract", "Contract", "my_contract")],
        ),
        a(
            "CancelContract",
            "Supply",
            "Cancel a contract",
            "End a contract early; penalties may apply.",
            "terminate end",
            vec![f("contract", "Contract", "my_contract")],
        ),
        a(
            "PlaceSpotOrder",
            "Supply",
            "Buy on the spot market",
            "Buy a good once, up to a maximum price.",
            "purchase inputs buy now",
            vec![
                firm(),
                f("product", "Good", "product"),
                f("region", "Region", "region"),
                f("quantity_lots", "Lots", "lots").range(1, 10_000, 1, 50),
                money("max_price_cents", "Highest price per lot").range(100, 0, 100, 10_000),
            ],
        ),
        // Finance
        a(
            "RequestLoan",
            "Finance",
            "Ask for a loan",
            "Ask the banks for a quote; the rate follows the central bank rate, the economy and your credit grade.",
            "borrow debt credit bank",
            vec![
                firm(),
                money("amount_cents", "Amount").range(1_000_000, 0, 500_000, 50_000_000),
            ],
        ),
        a(
            "AcceptLoanQuote",
            "Finance",
            "Take a quoted loan",
            "Accept a loan at the quoted rate and term.",
            "borrow accept",
            vec![
                firm(),
                money("amount_cents", "Amount").range(1_000_000, 0, 500_000, 50_000_000),
                f("rate_bps", "Rate", "bps").range(0, 3_000, 5, 800),
                f("weeks", "Term", "weeks").range(13, 520, 13, 260),
            ],
        ),
        a(
            "RepayLoan",
            "Finance",
            "Repay a loan",
            "Pay down a loan early.",
            "pay back debt",
            vec![
                f("loan", "Loan", "my_loan"),
                money("amount_cents", "Amount").range(0, 0, 100_000, 10_000_000),
            ],
        ),
        a(
            "IssueBond",
            "Finance",
            "Issue a bond",
            "Borrow from the market at a fixed coupon for a fixed term.",
            "debt borrow coupon",
            vec![
                firm(),
                money("principal_cents", "Amount").range(10_000_000, 0, 1_000_000, 100_000_000),
                f("coupon_bps", "Coupon", "bps").range(100, 2_000, 5, 700),
                f("weeks", "Term", "weeks").range(52, 1_040, 52, 260),
            ],
        ),
        a(
            "ProposeRestructuring",
            "Finance",
            "Propose a restructuring",
            "Ask lenders to restructure a company's debts when it cannot pay.",
            "default bankrupt debt",
            vec![firm()],
        ),
        a(
            "MakeIntercompanyLoan",
            "Finance",
            "Lend between your companies",
            "Move cash from one of your companies to another as a loan.",
            "group transfer fund",
            vec![
                f("lender", "Lender", "any_firm"),
                f("borrower", "Borrower", "any_firm"),
                money("principal_cents", "Amount").range(0, 0, 100_000, 10_000_000),
                f("rate_bps", "Rate", "bps").range(0, 2_000, 5, 500),
                f("weeks", "Term", "weeks").range(4, 520, 4, 52),
            ],
        ),
        a(
            "GrantGuarantee",
            "Finance",
            "Guarantee a group company",
            "Stand behind another company's debts up to a limit.",
            "guarantee backstop",
            vec![
                f("guarantor", "Guarantor", "any_firm"),
                f("beneficiary", "Beneficiary", "any_firm"),
                money("claim_cents", "Claim").range(0, 0, 100_000, 0),
                money("max_cents", "Limit").range(0, 0, 100_000, 10_000_000),
            ],
        ),
        // Shares
        a(
            "LaunchIPO",
            "Shares",
            "Launch an IPO",
            "List a private company on the exchange by selling new shares to the public.",
            "list public float flotation stock market",
            vec![
                firm(),
                f("new_shares", "New shares", "shares").range(10_000, 10_000_000, 10_000, 250_000),
                money("price_cents", "Price per share").range(10, 0, 5, 500),
            ],
        ),
        a(
            "IssueShares",
            "Shares",
            "Issue new shares",
            "Raise equity by selling new shares; existing holders are diluted.",
            "raise capital equity dilution",
            vec![
                firm(),
                f("qty", "Shares", "shares").range(1_000, 10_000_000, 1_000, 100_000),
                money("price_cents", "Price per share").range(10, 0, 5, 500),
                f("buyer_outside", "Buyer", "count")
                    .range(0, 9, 1, 0)
                    .help("Which outside investor takes the shares."),
            ],
        ),
        a(
            "RepurchaseShares",
            "Shares",
            "Buy back shares",
            "The company buys its own shares into treasury.",
            "buyback repurchase",
            vec![
                firm(),
                f("qty", "Shares", "shares").range(1_000, 10_000_000, 1_000, 50_000),
            ],
        ),
        a(
            "RetireTreasury",
            "Shares",
            "Retire treasury shares",
            "Cancel shares held in treasury.",
            "cancel shares",
            vec![
                firm(),
                f("qty", "Shares", "shares").range(1_000, 10_000_000, 1_000, 50_000),
            ],
        ),
        a(
            "PayDividend",
            "Shares",
            "Pay a dividend",
            "Pay cash to every shareholder in proportion to their holding.",
            "payout distribution",
            vec![
                firm(),
                money("amount_cents", "Total dividend").range(0, 0, 100_000, 5_000_000),
            ],
        ),
        a(
            "PlaceShareOrder",
            "Shares",
            "Buy or sell shares",
            "Place an order on the exchange for any listed company.",
            "trade stock buy sell invest",
            vec![
                f("investor", "As yourself", "bool").range(0, 1, 1, 1),
                f("firm", "Company", "any_firm"),
                f("buy", "Buy", "bool").range(0, 1, 1, 1),
                f("qty", "Shares", "shares").range(100, 10_000_000, 100, 10_000),
                money("limit_price_cents", "Limit price").range(1, 0, 1, 500),
            ],
        ),
        // Deals
        a(
            "StartDiligence",
            "Deals",
            "Make a takeover approach",
            "Start due diligence on a target for a share purchase, an asset purchase or a merger.",
            "acquire takeover buy merger m&a",
            vec![
                f("buyer", "Buyer", "my_firm"),
                f("target", "Target", "any_firm"),
                f("kind", "Deal", "choice").choices(
                    "string",
                    &[
                        ("SharePurchase", "Buy shares"),
                        ("AssetPurchase", "Buy assets"),
                        ("Merger", "Merge"),
                    ],
                ),
                money("price_cents", "Offer per share").range(1, 0, 1, 500),
                f("max_shares", "Most shares", "shares").range(0, 10_000_000, 1_000, 500_000),
            ],
        ),
        a(
            "FundTender",
            "Deals",
            "Fund the tender offer",
            "Put the cash in escrow and open the offer to shareholders.",
            "tender offer escrow",
            vec![f("deal", "Deal", "my_deal")],
        ),
        a(
            "RespondTender",
            "Deals",
            "Answer a tender offer",
            "Tender your shares into someone's offer, or decline.",
            "accept reject offer",
            vec![
                f("deal", "Deal", "my_deal"),
                f("accept", "Tender my shares", "bool").range(0, 1, 1, 1),
            ],
        ),
        a(
            "ProposeMerger",
            "Deals",
            "Put the merger to a vote",
            "Ask shareholders to approve the merger.",
            "vote merge",
            vec![f("deal", "Deal", "my_deal")],
        ),
        a(
            "SubmitRemedy",
            "Deals",
            "Offer a merger remedy",
            "Offer to divest share so the merger clears review.",
            "antitrust divest regulator",
            vec![
                f("deal", "Deal", "my_deal"),
                f("divest_share_bps", "Share to divest", "bps").range(100, 5_000, 50, 500),
            ],
        ),
        a(
            "WithdrawOffer",
            "Deals",
            "Withdraw an offer",
            "Walk away from a deal.",
            "cancel abandon",
            vec![f("deal", "Deal", "my_deal")],
        ),
        // Board
        a(
            "CallMeeting",
            "Board",
            "Call a shareholder meeting",
            "Call an extraordinary meeting of shareholders.",
            "agm egm vote",
            vec![firm()],
        ),
        a(
            "NominateCEO",
            "Board",
            "Nominate a CEO",
            "Put yourself or a professional manager up as chief executive.",
            "chief executive management",
            vec![
                f("firm", "Company", "any_firm"),
                f("investor", "Nominate yourself", "bool").range(0, 1, 1, 1),
            ],
        ),
        a(
            "CastVote",
            "Board",
            "Vote on a resolution",
            "Vote your shares on an open resolution.",
            "ballot approve reject",
            vec![
                f("firm", "Company", "any_firm"),
                f("vote_id", "Resolution", "vote"),
                f("approve", "Approve", "bool").range(0, 1, 1, 1),
            ],
        ),
        a(
            "ProposeBoardAction",
            "Board",
            "Propose a board action",
            "Put a proposal to the board.",
            "motion resolution",
            vec![
                f("firm", "Company", "my_firm"),
                f("subject", "Proposal", "choice").choices(
                    "string",
                    &[
                        ("merger_authority", "Authorise a merger"),
                        ("large_issuance", "Authorise a large issuance"),
                        ("debt_limit", "Raise the debt limit"),
                        ("conflict_transfer", "Approve a conflict transfer"),
                    ],
                ),
            ],
        ),
        // Banking and insurance (for banks and insurers the player controls)
        a(
            "SetDepositRate",
            "Banking",
            "Set your bank's deposit rate",
            "The rate your bank pays depositors.",
            "savings interest bank",
            vec![
                f("bank", "Bank", "bank"),
                f("rate_bps", "Rate", "bps").range(0, 1_500, 5, 300),
            ],
        ),
        a(
            "DepositFunds",
            "Banking",
            "Deposit at a bank",
            "Move company cash into a bank deposit that earns interest.",
            "save interest",
            vec![
                f("bank", "Bank", "bank"),
                firm(),
                money("amount_cents", "Amount").range(0, 0, 100_000, 10_000_000),
            ],
        ),
        a(
            "WithdrawFunds",
            "Banking",
            "Withdraw a deposit",
            "Take cash back out of a bank deposit.",
            "cash out",
            vec![
                f("bank", "Bank", "bank"),
                firm(),
                money("amount_cents", "Amount").range(0, 0, 100_000, 10_000_000),
            ],
        ),
        a(
            "ApproveLoan",
            "Banking",
            "Lend from your bank",
            "Approve a loan from your bank to a borrower.",
            "lend credit",
            vec![
                f("bank", "Bank", "bank"),
                f("borrower", "Borrower", "any_firm"),
                money("amount_cents", "Amount").range(0, 0, 100_000, 10_000_000),
            ],
        ),
        a(
            "QuotePolicy",
            "Insurance",
            "Get an insurance quote",
            "Ask an insurer to price cover for one site.",
            "insure cover premium",
            vec![
                f("insurer", "Insurer", "insurer"),
                f("facility", "Site", "my_facility"),
                money("limit_cents", "Cover limit").range(0, 0, 100_000, 50_000_000),
                money("deductible_cents", "Deductible").range(0, 0, 50_000, 1_000_000),
            ],
        ),
        a(
            "BindPolicy",
            "Insurance",
            "Insure a site",
            "Buy cover for one site.",
            "insure protect",
            vec![
                f("insurer", "Insurer", "insurer"),
                f("insured", "Insured company", "my_firm"),
                f("facility", "Site", "my_facility"),
                money("limit_cents", "Cover limit").range(0, 0, 100_000, 50_000_000),
                money("deductible_cents", "Deductible").range(0, 0, 50_000, 1_000_000),
            ],
        ),
        a(
            "CancelPolicy",
            "Insurance",
            "Cancel a policy",
            "End an insurance policy.",
            "stop cover",
            vec![f("policy", "Policy", "my_policy")],
        ),
        a(
            "SetUnderwritingLimit",
            "Insurance",
            "Set your insurer's risk appetite",
            "Specialist underwriting takes bigger, rarer risks.",
            "underwrite",
            vec![
                f("insurer", "Insurer", "insurer"),
                f("specialist", "Specialist", "bool").range(0, 1, 1, 0),
            ],
        ),
        // Government
        a(
            "FileLobbyRequest",
            "Government",
            "Lobby the government",
            "Ask a region's government for faster permits, a training grant or a tax break.",
            "lobby policy politics permit grant tax",
            vec![
                firm(),
                f("region", "Region", "region"),
                f("request", "Ask for", "choice").choices(
                    "string",
                    &[
                        ("PermitAcceleration", "Faster permits"),
                        ("TrainingGrant", "A training grant"),
                        ("TaxAdjustment", "A tax break"),
                    ],
                ),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalogue_action_is_an_engine_command_with_unique_fields() {
        let catalogue = action_catalogue();
        assert!(catalogue.len() >= 55);
        for action in &catalogue {
            let mut keys: Vec<&str> = action.fields.iter().map(|f| f.key.as_str()).collect();
            keys.sort();
            keys.dedup();
            assert_eq!(
                keys.len(),
                action.fields.len(),
                "{} has duplicate fields",
                action.command
            );
            // Build the action's JSON from its defaults and check the engine can read it.
            let fields: Vec<String> = action
                .fields
                .iter()
                .map(|field| {
                    let value = match field.encoding.as_str() {
                        "bool" => (field.default_value != 0).to_string(),
                        "string" => format!(
                            "\"{}\"",
                            field
                                .choices
                                .first()
                                .map(|c| c.value.clone())
                                .unwrap_or_else(|| "Test".to_owned())
                        ),
                        "optional" => "null".to_owned(),
                        _ => field
                            .choices
                            .first()
                            .map(|c| c.value.clone())
                            .unwrap_or_else(|| field.default_value.max(field.min).to_string()),
                    };
                    format!("\"{}\":{}", field.key, value)
                })
                .collect();
            let json = format!("{{\"{}\":{{{}}}}}", action.command, fields.join(","));
            let parsed: Result<cw_sim::commands::CommandIntent, _> = serde_json::from_str(&json);
            assert!(
                parsed.is_ok(),
                "{} does not parse: {json} {:?}",
                action.command,
                parsed.err()
            );
        }
    }
}
