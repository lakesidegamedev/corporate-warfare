import Foundation
import XCTest
@testable import CorporateWarfare

/// The 2.0 mechanics on the app's existing screens: the words, the actions and a real career.
final class Mechanics2Tests: XCTestCase {
    override func tearDown() {
        Format.reset()
        super.tearDown()
    }

    private func raid(role: String, weeksLeft: UInt32) -> RaidCampaignDto {
        RaidCampaignDto(
            role: role, attacker: 3, attackerName: "Permian Drilling", defender: 9, defenderName: "Lone Star Oil",
            region: 2, regionName: "Houston", industry: 1, industryName: "Energy", budgetCents: 2_000_000,
            attackCents: 2_000_000, defenceCents: 1_000_000, chanceBps: 1_450, blocksAtStake: 2, divisionBlocks: 5,
            priceCents: 500_000, announcedWeek: 40, resolvesWeek: 44, weeksLeft: weeksLeft
        )
    }

    private func lot(route: String, yours: Bool, bids: [PlantBidDto] = []) -> PlantLotDto {
        PlantLotDto(
            lot: 7, facility: 11, route: route, seller: 4, sellerName: "Failed Steel", plantName: "Blast furnace",
            region: 2, regionName: "Gary", blocks: 3, bookCents: 120_000_000, reserveCents: 48_000_000, openedWeek: 40,
            closesWeek: 44, weeksLeft: 2, weeklyCents: 150_000, blocksUsed: 3, earningsCents: 900_000,
            replacementCents: 200_000_000, playerBidCents: 0, playerBidFirm: 0, yours: yours, bids: bids, answerByWeek: 48
        )
    }

    func testRaidWordsSayWhoIsRaidingWhom() {
        XCTAssertEqual(RaidText.title(raid(role: "against_player", weeksLeft: 3)), "Permian Drilling is raiding you")
        XCTAssertEqual(RaidText.title(raid(role: "by_player", weeksLeft: 3)), "You are raiding Lone Star Oil")
        XCTAssertEqual(RaidText.title(raid(role: "between_rivals", weeksLeft: 3)), "Permian Drilling is raiding Lone Star Oil")
        XCTAssertEqual(RaidText.decided(raid(role: "by_player", weeksLeft: 3)), "Decided in 3 weeks, in week 44")
        XCTAssertEqual(RaidText.decided(raid(role: "by_player", weeksLeft: 1)), "Decided in 1 week, in week 44")
        XCTAssertEqual(RaidText.decided(raid(role: "by_player", weeksLeft: 0)), "Decided this week")
        XCTAssertEqual(RaidText.stake(raid(role: "by_player", weeksLeft: 3)), "2 of 5 plants")
        XCTAssertEqual(Format.percentBps(raid(role: "by_player", weeksLeft: 3).chanceBps), "14.5%")
    }

    func testPlantLotWordsNameTheRouteAndTheClock() {
        XCTAssertEqual(PlantLotText.route(lot(route: "break_up", yours: false)), "Break-up auction of Failed Steel")
        XCTAssertEqual(PlantLotText.route(lot(route: "listing", yours: false)), "Listed by Failed Steel")
        XCTAssertEqual(PlantLotText.weeks(0), "closes this week")
        XCTAssertEqual(PlantLotText.weeks(1), "1 week left")
        XCTAssertEqual(PlantLotText.weeks(4), "4 weeks left")
        XCTAssertTrue(PlantLotText.pickLabel(lot(route: "break_up", yours: false)).hasSuffix("2 weeks left"))
        XCTAssertTrue(PlantLotText.pickLabel(lot(route: "listing", yours: true)).hasSuffix("no bids yet"))
        let bid = PlantBidDto(bidder: 5, bidderName: "Rival", priceCents: 1, weeklyCents: 1, blocksUsed: 3)
        XCTAssertTrue(PlantLotText.pickLabel(lot(route: "listing", yours: true, bids: [bid])).hasSuffix("1 bid"))
    }

    func testCareerWords() {
        XCTAssertEqual(CareerText.ambition(nil), "No ambition")
        XCTAssertEqual(CareerText.ambition("richest"), "Richest")
        XCTAssertEqual(CareerText.medal("silver"), "Silver")
        XCTAssertEqual(CareerText.medal("none"), "None yet")
        XCTAssertEqual(CareerText.growth(23_000), "2.3 times")
        XCTAssertEqual(CareerText.growth(10_000), "1.0 times")
        XCTAssertEqual(CareerText.timeLeft(weeks: 0), "none")
        XCTAssertEqual(CareerText.timeLeft(weeks: 9), "9 weeks")
        XCTAssertEqual(CareerText.timeLeft(weeks: 52), "1 year")
        XCTAssertEqual(CareerText.timeLeft(weeks: 3 * 52 + 12), "3 years 12 weeks")
    }

    func testTheFortuneLineSaysWhenItWasScaled() {
        let same = HelmOfferDto(
            firm: 1, name: "Acme", industry: "energy", region: "Texas", rank: 4, valueCents: 1, revenueYearCents: 0,
            plants: 3, workers: 100, sharesNeeded: 10, markCents: 5, priceCents: 50, standardFortuneCents: 500_000_000,
            fortuneCents: 500_000_000
        )
        XCTAssertTrue(CareerText.fortuneNote(same).contains("the scenario's own"))
        let scaled = HelmOfferDto(
            firm: 1, name: "Acme", industry: "energy", region: "Texas", rank: 4, valueCents: 1, revenueYearCents: 0,
            plants: 3, workers: 100, sharesNeeded: 10, markCents: 5, priceCents: 50, standardFortuneCents: 500_000_000,
            fortuneCents: 900_000_000
        )
        XCTAssertTrue(CareerText.fortuneNote(scaled).contains("instead of the scenario's usual"))
        XCTAssertTrue(CareerText.fortuneNote(scaled).contains("scaled up"))
    }

    func testTheEndingTitlesAndTheStartLine() {
        func summary(_ ending: String, lost: Bool, start: String = "founder", company: String? = nil, scaled: Bool = false) -> CareerSummaryDto {
            CareerSummaryDto(
                start: start, helmCompany: company, helmFortuneScaled: scaled, ambition: "biggest", years: 10, endWeek: 520,
                standing: "none", ended: true, ending: ending, lost: lost, medal: "none", sentence: "", boardWarned: false,
                startWorthCents: 100, netWorthCents: 200, growthBps: 20_000, yearsLog: [], events: []
            )
        }
        XCTAssertEqual(CareerText.ending(summary("won", lost: false)), "You won")
        XCTAssertEqual(CareerText.ending(summary("removed", lost: true)), "Removed by the board")
        XCTAssertEqual(CareerText.ending(summary("taken_over", lost: true)), "Taken over")
        XCTAssertEqual(CareerText.ending(summary("no_company", lost: true)), "No company left")
        XCTAssertEqual(CareerText.start(summary("won", lost: false)), "Started as a founder.")
        XCTAssertEqual(
            CareerText.start(summary("won", lost: false, start: "helm", company: "Acme", scaled: true)),
            "Took the helm of Acme. The starting fortune was scaled up to pay for control."
        )
    }

    func testDefaultChoicesStartTheGameAsBefore() {
        var choice = CareerChoice()
        XCTAssertTrue(choice.isDefault)
        choice.ambition = "richest"
        XCTAssertFalse(choice.isDefault)
        XCTAssertEqual(choice.options().start, "founder")
        XCTAssertNil(choice.options().helmFirm)
        choice.helm = true
        choice.helmFirm = 42
        choice.years = 10
        XCTAssertEqual(choice.options().start, "helm")
        XCTAssertEqual(choice.options().helmFirm, 42)
        XCTAssertEqual(choice.options().years, 10)
    }

    func testTheNewActionsAreInTheCatalogueForTheActionsScreen() {
        let commands = Set(ActionCatalogue.all.map(\.command))
        for name in ["BidForPlant", "ListPlant", "AcceptPlantBid", "WithdrawListing", "FoldInCompany"] {
            XCTAssertTrue(commands.contains(name), "\(name) is missing from the action catalogue")
        }
        let bid = ActionCatalogue.all.first { $0.command == "BidForPlant" }
        XCTAssertEqual(bid?.fields.first { $0.key == "lot" }?.kind, "plant_lot")
        let accept = ActionCatalogue.all.first { $0.command == "AcceptPlantBid" }
        XCTAssertEqual(accept?.fields.first { $0.key == "lot" }?.kind, "my_listing")
    }

    func testRaidRejectionsReadInPlainWords() {
        let launch = Remedy.phase("WrongPhase { expected: \"no_campaign\", actual: \"campaign_running\" }")
        XCTAssertEqual(launch?.title, "Campaign still running")
        let abandon = Remedy.phase("WrongPhase { expected: \"campaign_running\", actual: \"no_campaign\" }")
        XCTAssertEqual(abandon?.title, "No raid to give up")
        XCTAssertEqual(Remedy.phase("opening_grace")?.title, "Too new to raid")
    }

    /// A real helm career through the generated bindings: the offers, the guide, the standing and a
    /// raid and a plant lot read back without a crash.
    func testARealHelmCareerReadsBackThroughTheNewCalls() throws {
        let options = NewGameDto(
            seed: 7, scenarioId: "great-lakes-2000", guided: false, fiveYear: false, difficulty: "standard",
            economy: "historic", startingCapitalCents: nil, pricePath: "", timeline: ""
        )
        let offers = try helmOffers(options: options)
        let pick = try XCTUnwrap(offers.first)
        XCTAssertLessThanOrEqual(offers.count, 3)
        let guide = ambitionGuide(start: "helm", years: 10)
        XCTAssertEqual(Set(guide.map(\.key)), ["richest", "biggest", "dominant", "survivor"])
        XCTAssertEqual(careerLengths().years, [5, 10, 20])
        let choice = CareerChoice(helm: true, helmFirm: pick.firm, ambition: "biggest", years: 10)
        let handle = try SessionHandle.startCareer(options: options, career: choice.options())
        let standing = try XCTUnwrap(handle.careerStanding())
        XCTAssertEqual(standing.ambition, "biggest")
        XCTAssertFalse(standing.ended)
        _ = handle.advance(weeks: 4)
        XCTAssertNotNil(handle.careerSummary())
        _ = handle.raidCampaigns()
        _ = handle.plantLots()
        let openings = handle.capacityOpenings(limit: 5)
        XCTAssertLessThanOrEqual(openings.count, 5)
        let product = try XCTUnwrap(products().first { $0.referencePriceCents > 0 })
        let depth = try XCTUnwrap(handle.goodDepth(product: UInt32(product.index)))
        XCTAssertFalse(depth.priceReason.isEmpty)
    }
}
