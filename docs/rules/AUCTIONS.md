# Plant auctions, listings and mergers (2.0)

A plant leaves its owner only when someone pays for it. Engine: `economy/auctions.rs` (lots,
deadlines, the merger log) and `economy/disposal.rs` (how a bidder values a plant). The offer
period is `capacity.plant_auction_weeks` in `content/initial-balance.json` (4); 0 turns auctions
off. It is part of the dataset hash.

## How a bidder values a plant
A bidder (a rival outside the seller's group that could run the plant) bids the least of three
sums: what the blocks it could keep busy earn over the payback horizon it judges a new plant on;
what building the same blocks would cost it today, scaled by the share of the plant's cost not
yet written off; and its cash above eight weeks of mandatory outflow (`BID_RESERVE_WEEKS`). It
reads the same room-to-build signal the Openings board shows (CAPACITY.md). The highest bid wins
and the plant changes hands running, staff and contracts included. The cash comes from the buyer.
Player companies never bid by themselves: buying a plant is the player's choice.

## Break-up auction
When a company goes into liquidation, each of its plants that could still run is offered for
`plant_auction_weeks`. The plants sit idle in the estate. At the deadline each plant goes to the
best bid that reaches the scrap price, 40 percent of its book value (`SCRAP_PERCENT`); with no
such bid it is scrapped for that amount. Proceeds go to the failed company's creditors in the
usual order (lenders first, shareholders last). The company stops paying interest while its
plants are on offer, and its debts are settled when the last plant is sold. Goods still in
transit to it are sold with its stock. Winding a company down does the same for its idle plants,
and its shareholders get everything left, including whatever the plants fetched above scrap.

## Listing
An owner can list one of its own plants (`ListPlant`). It keeps running while bids come in.
At the deadline the bids are put to the owner, best first. The owner accepts the best
(`AcceptPlantBid`) or takes the plant off the market (`WithdrawListing`) within another `plant_auction_weeks`, after which the offer lapses.
With no bids it is withdrawn. Rivals list a plant they have kept closed for half a year, so
the player can bid for those too. Selling a plant therefore always needs a buyer; when nobody
bids the plant is written off for no cash.

## The player's bid
`BidForPlant` names a lot and a price through one of the player's companies. The bid is sealed
until the deadline, stands only if the company can still pay then, and wins a tie. The player
sees the engine's opening figures (book value, scrap value, what the plant would earn the
company a week) but not rival bids until the deadline.

## Mergers
`ProposeMerger` completes a settled deal. `FoldInCompany` folds in any company the buyer holds
at least 90 percent of, however it came to hold it: the last minority holders are paid fair value
in cash, and the buyer takes over the target's plants, debts and contracts. Refused while any deal
between the two went to a competition review or carried a remedy. A company waits 52 weeks
(`MERGER_COOLDOWN_WEEKS`) after one merger before proposing another; a rival folds in at most one
company a year and only with cash to spare.

## Saved state
`World.auctions` (lots, closed lots, totals, the merger log) is outside the postcard payload in its
own save trailer and folded into the canonical hash while not empty, so no save layout moved.
