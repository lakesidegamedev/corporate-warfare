package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.cw_ffi.BuildQuoteDto
import uniffi.cw_ffi.CommodityDto
import uniffi.cw_ffi.DivisionDetailDto
import uniffi.cw_ffi.DivisionDto
import uniffi.cw_ffi.FreightSummaryDto
import uniffi.cw_ffi.HeldStockDto
import uniffi.cw_ffi.LaneContractDto
import uniffi.cw_ffi.LaneDto
import uniffi.cw_ffi.StorageSummaryDto
import uniffi.cw_ffi.OwnershipDto
import uniffi.cw_ffi.PeriodDto
import uniffi.cw_ffi.MapPlantDto
import uniffi.cw_ffi.RaidCheckDto
import uniffi.cw_ffi.RaidOptionDto
import uniffi.cw_ffi.RaidQuoteDto
import uniffi.cw_ffi.RaidRecordDto
import uniffi.cw_ffi.RivalMoveFeedDto
import uniffi.cw_ffi.RivalSummaryDto
import uniffi.cw_ffi.RivalTagDto
import uniffi.cw_ffi.MapDivisionDto
import uniffi.cw_ffi.StockDto
import uniffi.cw_ffi.StrategyDto
import uniffi.cw_ffi.EventDto
import uniffi.cw_ffi.FacilityDto
import uniffi.cw_ffi.FinanceDto
import uniffi.cw_ffi.FirmDto
import uniffi.cw_ffi.MarketDto
import uniffi.cw_ffi.OverviewDto
import uniffi.cw_ffi.ProductDto
import uniffi.cw_ffi.ProductLineDto
import uniffi.cw_ffi.RegionBoardDto
import uniffi.cw_ffi.SessionHandle
import uniffi.cw_ffi.importSave

/** What a form's pickers can choose from, loaded for one company when a form opens. */
data class Pickers(
    val loans: List<uniffi.cw_ffi.LoanDto> = emptyList(),
    val contracts: List<uniffi.cw_ffi.ContractDto> = emptyList(),
    val policies: List<uniffi.cw_ffi.InsuranceDto> = emptyList(),
    val votes: List<uniffi.cw_ffi.BoardVoteDto> = emptyList(),
    val deals: List<uniffi.cw_ffi.DealDto> = emptyList(),
    /** Options the engine lists itself: bonds, ventures and open activist campaigns. */
    val extra: Map<String, List<uniffi.cw_ffi.PickDto>> = emptyMap(),
)

/** One market row for the selected region: a product and its local market, if traded. */
data class MarketRow(val product: ProductDto, val market: MarketDto?)

/**
 * Owns the engine session. Every call into Rust runs on one background thread, so the world is
 * never touched concurrently; the UI reads immutable DTOs from Compose state.
 */
class GameStore(private val saveDir: File, private val scope: CoroutineScope) {
    private val engine = Executors.newSingleThreadExecutor { Thread(it, "cw-engine") }.asCoroutineDispatcher()
    private var session: SessionHandle? = null
    private var runJob: Job? = null

    /** The reporting currency the player chose (kept in settings); applied after a career starts or loads. */
    var preferredCurrency: String = ""
    /** Also stop the clock for warnings (kept in settings); applied before every advance. */
    var pauseOnWarnings: Boolean = false
    /** Called when the player picks a reporting currency, so the choice can be saved. */
    var onCurrencyChosen: (String) -> Unit = {}

    /**
     * Receives the market snapshot JSON every five game weeks; null (tests, or analytics off)
     * means none is built. Called off the UI thread, and must not throw.
     */
    var snapshotSink: ((String) -> Unit)? = null

    var ready by mutableStateOf(false); private set
    var busy by mutableStateOf(false); private set
    var running by mutableStateOf(false); private set
    /** Weeks a second while running: 1, 2 or 4. */
    var speed by mutableStateOf(1); private set
    var status by mutableStateOf(""); private set
    var error by mutableStateOf<String?>(null)

    var overview by mutableStateOf<OverviewDto?>(null); private set
    var firms by mutableStateOf<List<FirmDto>>(emptyList()); private set
    var selectedFirm by mutableStateOf<ULong?>(null); private set
    var finance by mutableStateOf<FinanceDto?>(null); private set
    var productLines by mutableStateOf<List<ProductLineDto>>(emptyList()); private set
    var facilities by mutableStateOf<List<FacilityDto>>(emptyList()); private set
    var divisions by mutableStateOf<List<DivisionDto>>(emptyList()); private set
    /** The division whose detail view is open, from the map, a region board, the CEO Office or the company page. */
    var openDivision by mutableStateOf<DivisionDto?>(null); private set
    var events by mutableStateOf<List<EventDto>>(emptyList()); private set
    var boards by mutableStateOf<List<RegionBoardDto>>(emptyList()); private set
    var stocks by mutableStateOf<List<StockDto>>(emptyList()); private set
    /** The selected company's four-week results, oldest first, up to five years. */
    var history by mutableStateOf<List<PeriodDto>>(emptyList()); private set
    var ownership by mutableStateOf<OwnershipDto?>(null); private set
    /** Recent raids anywhere, newest first, for the rival-activity feed. */
    var recentRaids by mutableStateOf<List<RaidRecordDto>>(emptyList()); private set
    /** Every rival group with a plant, threats first, for the CEO Office card. */
    var rivals by mutableStateOf<List<RivalSummaryDto>>(emptyList()); private set
    /** The latest rival moves anywhere, newest first, naming the chief behind each. */
    var rivalFeed by mutableStateOf<List<RivalMoveFeedDto>>(emptyList()); private set
    /** Every plant in the world, so a rival's markets can be listed with a Raid button. */
    var mapPlants by mutableStateOf<List<MapPlantDto>>(emptyList()); private set
    var commodities by mutableStateOf<List<CommodityDto>>(emptyList()); private set
    var mapDivisions by mutableStateOf<List<MapDivisionDto>>(emptyList()); private set
    /** Goods the selected company holds back from sale, with any open buy orders. */
    var held by mutableStateOf<List<HeldStockDto>>(emptyList()); private set
    var storageSummary by mutableStateOf<StorageSummaryDto?>(null); private set
    /** Freight lanes this week, lane contracts, and the corridor's freight in a glance. */
    var lanes by mutableStateOf<List<LaneDto>>(emptyList()); private set
    var laneContracts by mutableStateOf<List<LaneContractDto>>(emptyList()); private set
    var freight by mutableStateOf<FreightSummaryDto?>(null); private set
    /** Groups, transfer prices, reputation, bonds, ventures and activists. */
    var structure by mutableStateOf<uniffi.cw_ffi.StructureViewDto?>(null); private set
    /** The strategies and products with the prices of the career's own scenario, loaded with it. */
    var strategyCatalogue by mutableStateOf<List<StrategyDto>>(emptyList()); private set
    /** Every player action with its form, in Command Center order. */
    val actions: List<uniffi.cw_ffi.ActionDto> by lazy { uniffi.cw_ffi.actionCatalogue() }
    var productCatalogue by mutableStateOf<List<ProductDto>>(emptyList()); private set
    private var catalogueOf: SessionHandle? = null
    /** The economy, central bank and government. */
    var economy by mutableStateOf<uniffi.cw_ffi.EconomyDto?>(null); private set
    /** The selected company's research position. */
    var research by mutableStateOf<uniffi.cw_ffi.ResearchDto?>(null); private set
    /** What the CFO says is costing the selected company money, biggest first. Empty when it is fine. */
    var cfoAdvice by mutableStateOf<List<uniffi.cw_ffi.CfoAdviceDto>>(emptyList()); private set
    var marketRegion by mutableStateOf<String?>(null)
    var marketRows by mutableStateOf<List<MarketRow>>(emptyList()); private set

    val selectedFirmDto: FirmDto? get() = firms.firstOrNull { it.handle == selectedFirm }
    val hasAutosave: Boolean get() = Saves.list(saveDir).isNotEmpty()

    fun saveSlots(): List<SaveSlot> = Saves.list(saveDir)

    private val autosave get() = File(saveDir, "autosave.cwsave")

    fun newCareer(seed: Long, guided: Boolean, fiveYear: Boolean = false) = perform {
        val handle = withContext(engine) { SessionHandle(seed.toULong(), guided, fiveYear) }
        session = handle
        selectedFirm = withContext(engine) {
            if (preferredCurrency.isNotEmpty()) handle.setReportingCurrency(preferredCurrency)
            handle.overview().primaryFirm
        }
        reload()
        ready = true
    }

    /** A new career from the new-game screen. */
    fun startCareer(options: uniffi.cw_ffi.NewGameDto) = perform {
        val handle = withContext(engine) { SessionHandle.start(options) }
        session = handle
        selectedFirm = withContext(engine) {
            if (preferredCurrency.isNotEmpty()) handle.setReportingCurrency(preferredCurrency)
            handle.overview().primaryFirm
        }
        reload()
        ready = true
    }

    /** Where custom scenarios are kept, beside the saves. */
    val customDir: String get() = File(saveDir, "custom-scenarios").path

    /** A new career from an edited scenario. Its own difficulty, economy and capital apply. */
    fun startCustom(scenario: uniffi.cw_ffi.CustomScenarioDto, guided: Boolean, fiveYear: Boolean) = perform {
        val seed = (System.nanoTime() % 9_999_999).toULong()
        val handle = withContext(engine) { SessionHandle.startCustom(seed, guided, fiveYear, scenario) }
        session = handle
        selectedFirm = withContext(engine) {
            if (preferredCurrency.isNotEmpty()) handle.setReportingCurrency(preferredCurrency)
            handle.overview().primaryFirm
        }
        reload()
        ready = true
    }

    /** Open a save from the load screen. */
    fun loadSave(file: File) = perform {
        val bytes = withContext(engine) { file.readBytes() }
        val handle = withContext(engine) { importSave(bytes) }
        session = handle
        selectedFirm = withContext(engine) {
            if (preferredCurrency.isNotEmpty()) handle.setReportingCurrency(preferredCurrency)
            handle.overview().primaryFirm
        }
        reload()
        ready = true
    }

    fun continueCareer() = perform {
        val latest = Saves.list(saveDir).firstOrNull()?.file ?: autosave
        val bytes = withContext(engine) { latest.readBytes() }
        val handle = withContext(engine) { importSave(bytes) }
        session = handle
        selectedFirm = withContext(engine) {
            if (preferredCurrency.isNotEmpty()) handle.setReportingCurrency(preferredCurrency)
            handle.overview().primaryFirm
        }
        reload()
        ready = true
    }

    /** Show the investor's figures in another of the scenario's currencies. */
    fun chooseReportingCurrency(id: String) = perform {
        val handle = session ?: return@perform
        val changed = withContext(engine) { handle.setReportingCurrency(id) }
        if (changed) {
            preferredCurrency = id
            onCurrencyChosen(id)
            reload()
        }
    }

    fun advance() = perform { step() }

    /** Save, stop time and close the career; the main menu offers it again under Continue. */
    fun exitToMenu() = perform {
        runJob?.cancel()
        runJob = null
        running = false
        save()
        session = null
        ready = false
        overview = null
        selectedFirm = null
        status = ""
    }

    fun toggleRun() = playAt(if (runJob == null) 1 else 0)

    /**
     * Time runs at [weeksPerSecond]: 0 pauses, 1, 2 or 4 play. One tick is one week. A running
     * loop picks up a new speed from the next week; ticks never mark the app busy, so buttons
     * stay put while time runs.
     */
    fun playAt(weeksPerSecond: Int) {
        if (weeksPerSecond <= 0) {
            runJob?.cancel()
            runJob = null
            running = false
            return
        }
        speed = weeksPerSecond
        if (runJob != null) return
        running = true
        runJob = scope.launch {
            while (isActive) {
                step()
                if (overview?.pause != null) break
                delay(1_000L / speed)
            }
            running = false
            runJob = null
        }
    }

    private suspend fun step() {
        val handle = session ?: return
        val before = overview?.tick ?: 0u
        val policy = if (pauseOnWarnings) "decisions_and_warnings" else "decisions"
        val result = withContext(engine) {
            handle.setPausePolicy(policy)
            handle.advance(1u)
        }
        status = "Advanced to week ${result.finalWeek}."
        reload()
        if ((overview?.tick ?: 0u) % 4u == 0u) save()
        val week = overview?.tick ?: 0u
        val sink = snapshotSink
        if (sink != null && Telemetry.crossed(before, week)) {
            val json = withContext(engine) { runCatching { handle.marketSnapshotJson() }.getOrDefault("") }
            if (json.isNotEmpty()) scope.launch(Dispatchers.IO) { runCatching { sink(json) } }
        }
    }

    fun selectFirm(handle: ULong) = perform {
        selectedFirm = handle
        reload()
    }

    fun selectRegion(code: String) = perform {
        marketRegion = code
        loadMarkets()
    }

    fun respond(event: ULong, choice: String) =
        submit(CommandJson.command("RespondEvent", "event" to event, "choice" to choice))

    fun setPosture(product: UInt, mode: Int, bps: Int) {
        val firm = selectedFirm ?: return
        submit(CommandJson.command("SetPricePosture", "firm" to firm, "product" to product, "mode" to mode, "bps" to bps))
    }

    /** Every plant of the selected company runs at [bps] of full capacity, 0 to 10,000. */
    fun setTarget(bps: Int) {
        val firm = selectedFirm ?: return
        submit(CommandJson.command("SetProductionTarget", "firm" to firm, "target_bps" to bps))
    }

    /** Every plant in [plants] keeps [weeks] of inputs on hand (zero is the company default). */
    fun setStockTarget(plants: List<ULong>, weeks: Int, seasonal: Boolean) = perform {
        val handle = session ?: return@perform
        var failed = false
        for (plant in plants) {
            val json = CommandJson.command("SetStockTarget", "facility" to plant, "weeks" to weeks, "seasonal" to seasonal)
            val receipt = withContext(engine) { handle.submitJson(json) }
            if (!receipt.accepted) {
                error = Messages.rejection(receipt.errorKey ?: "error.internal_fault", receipt.errorDetail ?: "")
                failed = true
                break
            }
        }
        if (!failed) {
            error = null
            status = "Stock target set."
        }
        reload()
    }

    fun showDivision(division: DivisionDto?) {
        openDivision = division
    }

    /** The engine's detail report for one division of the selected company. */
    fun divisionDetail(division: DivisionDto, onResult: (DivisionDetailDto?) -> Unit) {
        val handle = session ?: return
        val firm = selectedFirm ?: return
        scope.launch { onResult(withContext(engine) { handle.divisionDetail(firm, division.region, division.industry) }) }
    }

    /** A supplying division's transfer price to the rest of its group, 0 at cost, 10,000 at market. */
    fun setTransferPrice(detail: DivisionDetailDto, bps: Int) =
        submit(CommandJson.command("SetTransferPrice", "firm" to detail.firm, "region" to detail.regionIndex, "industry" to detail.industryIndex, "bps" to bps))

    /** The weekly wage the selected company offers in one tier, 0 low to 2 high. */
    fun setWageOffer(tier: Int, wageCents: Long) {
        val firm = selectedFirm ?: return
        submit(CommandJson.command("SetWageOffer", "firm" to firm, "tier" to tier, "wage_cents" to wageCents))
    }

    /** Index of a region in the engine's region list, for commands that take one. */
    fun regionIndex(code: String): UInt = boards.firstOrNull { it.code == code }?.index ?: 0u

    /** Buy [lots] of a good into storage in a region, paying up to [limitBps] of its reference price. */
    fun buyToStore(product: UInt, region: UInt, lots: Long, limitBps: Int) {
        val firm = selectedFirm ?: return
        submit(CommandJson.command("BuyToStore", "firm" to firm, "product" to product, "region" to region, "lots" to lots, "limit_bps" to limitBps))
    }

    /** Release stored goods for sale; zero lots releases everything held there. */
    fun releaseStored(product: UInt, region: UInt, lots: Long) {
        val firm = selectedFirm ?: return
        submit(CommandJson.command("SellFromStore", "firm" to firm, "product" to product, "region" to region, "lots" to lots))
    }

    /** A division's shipping policy: 0 cheapest, 1 fastest, 2 own carriers first, 3 most reliable, and an optional preferred mode. */
    fun setShippingPolicy(plant: ULong, kind: Int, mode: Int?) {
        val fields = mutableListOf<Pair<String, Any>>("facility" to plant, "kind" to kind)
        if (mode != null) fields.add("mode" to mode)
        submit(CommandJson.command("SetShippingPolicy", *fields.toTypedArray()))
    }

    /** One division's (or, with [facility] zero, the whole company's) freight for the week just closed, with policies priced. */
    fun shipping(firm: ULong, facility: ULong, onResult: (uniffi.cw_ffi.ShippingSummaryDto?) -> Unit) {
        val handle = session ?: return
        scope.launch { onResult(withContext(engine) { handle.shippingSummary(firm, facility) }) }
    }

    /** What a lane reservation would cost and save, before any command is sent. */
    fun reserveQuote(lane: LaneDto, lots: Int, weeks: Int, onResult: (uniffi.cw_ffi.ReserveQuoteDto?) -> Unit) {
        val handle = session ?: return
        scope.launch { onResult(withContext(engine) { handle.reserveQuote(lane.a, lane.b, lane.mode, lots, weeks.toUInt()) }) }
    }

    /** Reserve [lots] a week on a lane for [weeks] at the lane's rate, locked for the term. */
    fun reserveLane(from: String, to: String, mode: UInt, lots: Int, weeks: Int) {
        val firm = selectedFirm ?: return
        submit(CommandJson.command("ReserveLane", "firm" to firm, "from" to regionIndex(from), "to" to regionIndex(to), "mode" to mode, "lots" to lots, "weeks" to weeks))
    }

    /** What a build would cost, computed by the engine without changing anything. */
    fun quote(region: UInt, recipe: UInt, blocks: UInt, onResult: (BuildQuoteDto?) -> Unit) {
        val handle = session ?: return
        val firm = selectedFirm ?: return
        scope.launch {
            onResult(withContext(engine) { handle.buildQuote(firm, region, recipe, blocks) })
        }
    }

    /** A company's chief executive and recent moves, read from the engine. */
    fun dossier(firm: ULong, onResult: (uniffi.cw_ffi.DossierDto?) -> Unit) {
        val handle = session ?: return
        scope.launch { onResult(withContext(engine) { handle.dossier(firm) }) }
    }

    /** Read anything from the engine off the main thread; null without a session. */
    suspend fun <T> read(block: (SessionHandle) -> T): T? {
        val handle = session ?: return null
        return withContext(engine) { block(handle) }
    }

    /** A company's full page, with what the investor may see of it. */
    fun companyPage(firm: ULong, onResult: (uniffi.cw_ffi.CompanyPageDto?) -> Unit) {
        val handle = session ?: return
        scope.launch { onResult(withContext(engine) { handle.companyPage(firm) }) }
    }

    /** A rival's chief, temperament and any grudge or tie with the selected company. */
    fun rivalTag(firm: ULong, onResult: (RivalTagDto?) -> Unit) {
        val handle = session ?: return
        val you = selectedFirm ?: return
        scope.launch { onResult(withContext(engine) { handle.rivalTag(firm, you) }) }
    }

    /** Whether the selected company can raid `target` in a market, and why not when it cannot. */
    fun raidCheck(target: ULong, region: String, industry: String, onResult: (RaidCheckDto?) -> Unit) {
        val handle = session ?: return
        val firm = selectedFirm ?: return
        scope.launch { onResult(withContext(engine) { handle.raidCheck(firm, target, region, industry) }) }
    }

    /** The odds and stake of a raid at a chosen budget. */
    fun raidQuote(option: RaidOptionDto, budgetCents: Long, onResult: (RaidQuoteDto?) -> Unit) {
        val handle = session ?: return
        val firm = selectedFirm ?: return
        scope.launch {
            onResult(withContext(engine) { handle.raidQuote(firm, option.target, option.regionIndex, option.industryIndex, budgetCents) })
        }
    }

    /** Launch a market raid; the outcome lands in [recentRaids]. */
    fun raid(option: RaidOptionDto, budgetCents: Long) {
        val firm = selectedFirm ?: return
        submit(CommandJson.command("RaidMarket", "firm" to firm, "target" to option.target, "region" to option.regionIndex, "industry" to option.industryIndex, "budget_cents" to budgetCents))
    }

    fun build(region: UInt, recipe: UInt, blocks: UInt) {
        val firm = selectedFirm ?: return
        submit(CommandJson.command("BuildFacility", "firm" to firm, "region" to region, "recipe" to recipe, "blocks" to blocks))
    }

    /** A personal share order, filled at the weekly close. */
    fun orderShares(firm: ULong, buy: Boolean, quantity: Long, limitCents: Long) =
        submit(CommandJson.command("PlaceShareOrder", "investor" to true, "firm" to firm, "buy" to buy, "qty" to quantity, "limit_price_cents" to limitCents))

    /** What an action would do, computed without changing anything. */
    fun preview(json: String, onResult: (uniffi.cw_ffi.PreviewDto?) -> Unit) {
        val handle = session ?: return onResult(null)
        scope.launch {
            val result = withContext(engine) { runCatching { handle.previewJson(json) }.getOrNull() }
            onResult(result)
        }
    }

    /** Submit an action from a form and report back accepted or the reason, in plain words. */
    fun act(json: String, onResult: (Boolean, String) -> Unit) {
        val handle = session ?: return onResult(false, "No career is open.")
        scope.launch {
            val receipt = withContext(engine) { runCatching { handle.submitJson(json) }.getOrNull() }
            if (receipt == null) {
                onResult(false, "The engine could not read that action.")
                return@launch
            }
            if (receipt.accepted) {
                Tutorial.sent(json.substringAfter("{\"").substringBefore("\""))
                reload()
                onResult(true, "Done.")
            } else {
                onResult(false, Messages.rejection(receipt.errorKey ?: "error.internal_fault", receipt.errorDetail ?: ""))
            }
        }
    }

    /** Loans, contracts, policies and votes of [firm], and every open deal, for form pickers. */
    fun pickers(firm: ULong?, onResult: (Pickers) -> Unit) {
        val handle = session ?: return onResult(Pickers())
        scope.launch {
            val result = withContext(engine) {
                Pickers(
                    loans = firm?.let { handle.loans(it) } ?: emptyList(),
                    contracts = firm?.let { handle.contracts(it) } ?: emptyList(),
                    policies = firm?.let { handle.insurance(it) } ?: emptyList(),
                    votes = firm?.let { handle.boardVotes(it) } ?: emptyList(),
                    deals = handle.deals(),
                    extra = listOf("bond", "my_bond", "my_venture", "open_campaign").associateWith { handle.structurePicks(it) },
                )
            }
            onResult(result)
        }
    }

    /** Press a CFO button: send its commands in order, stopping at the first the engine refuses. */
    fun applyAdvice(advice: uniffi.cw_ffi.CfoAdviceDto) = perform {
        val handle = session ?: return@perform
        for (json in advice.commands) {
            val receipt = withContext(engine) { handle.submitJson(json) }
            if (!receipt.accepted) {
                error = Messages.rejection(receipt.errorKey ?: "error.internal_fault", receipt.errorDetail ?: "")
                reload()
                return@perform
            }
        }
        error = null
        status = "Done: ${advice.actionLabel.lowercase()}."
        reload()
    }

    fun submit(json: String) = perform {
        val handle = session ?: return@perform
        val receipt = withContext(engine) { handle.submitJson(json) }
        if (receipt.accepted) {
            error = null
            status = "Accepted."
            Tutorial.sent(json.substringAfter("{\"").substringBefore("\""))
        } else {
            error = Messages.rejection(receipt.errorKey ?: "error.internal_fault", receipt.errorDetail ?: "")
        }
        reload()
    }

    fun dismissPause() = perform {
        val handle = session ?: return@perform
        withContext(engine) { handle.dismissPause() }
        reload()
    }

    fun saveNow() = perform {
        save(manual = true)
        status = "Saved."
    }

    private suspend fun save(manual: Boolean = false) {
        val handle = session ?: return
        val now = overview
        withContext(engine) {
            val bytes = handle.exportSave()
            val info = SaveInfo(
                company = now?.primaryName.orEmpty(),
                week = now?.weekLabel.orEmpty(),
                tick = now?.tick?.toInt() ?: 0,
                savedAt = System.currentTimeMillis(),
                autosave = !manual,
            )
            Saves.write(saveDir, if (manual) "save-${System.currentTimeMillis()}" else "autosave", bytes, info)
            if (manual) Saves.prune(saveDir)
        }
    }

    private suspend fun reload() {
        val handle = session ?: return
        val firm = selectedFirm
        if (catalogueOf !== handle) {
            val (strategiesNow, productsNow) = withContext(engine) { handle.strategies() to handle.products() }
            strategyCatalogue = strategiesNow
            productCatalogue = productsNow
            catalogueOf = handle
        }
        withContext(engine) {
            val overviewNow = handle.overview()
            val firmsNow = handle.firms()
            val eventsNow = handle.events()
            val boardsNow = handle.regionBoards(firm)
            val financeNow = firm?.let { handle.finance(it) }
            val linesNow = firm?.let { handle.productLines(it) } ?: emptyList()
            val facilitiesNow = firm?.let { handle.facilities(it) } ?: emptyList()
            val divisionsNow = firm?.let { handle.divisions(it) } ?: emptyList()
            listOf(
                overviewNow, firmsNow, eventsNow, boardsNow, financeNow, linesNow, facilitiesNow, divisionsNow,
                handle.stockBoard(), handle.commodityBoard(), handle.mapDivisions(),
                firm?.let { handle.history(it) } ?: emptyList<PeriodDto>(), firm?.let { handle.ownership(it) },
                handle.recentRaids(),
                handle.economy(), firm?.let { handle.research(it) },
                firm?.let { handle.heldStock(it) } ?: emptyList<HeldStockDto>(),
                firm?.let { handle.storageSummary(it) },
                handle.lanes(), handle.laneContracts(), handle.freightSummary(),
                handle.structureView(),
                firm?.let { handle.rivals(it) } ?: emptyList<RivalSummaryDto>(),
                handle.rivalFeed(12u),
                handle.mapPlants(),
                firm?.let { handle.cfoAdvice(it) } ?: emptyList<uniffi.cw_ffi.CfoAdviceDto>(),
            )
        }.let { parts ->
            @Suppress("UNCHECKED_CAST")
            run {
                overview = parts[0] as OverviewDto
                ResultBuckets.startYear = (parts[0] as OverviewDto).startYear
                firms = parts[1] as List<FirmDto>
                events = parts[2] as List<EventDto>
                boards = parts[3] as List<RegionBoardDto>
                finance = parts[4] as FinanceDto?
                productLines = parts[5] as List<ProductLineDto>
                facilities = parts[6] as List<FacilityDto>
                divisions = parts[7] as List<DivisionDto>
                stocks = parts[8] as List<StockDto>
                commodities = parts[9] as List<CommodityDto>
                mapDivisions = parts[10] as List<MapDivisionDto>
                history = parts[11] as List<PeriodDto>
                ownership = parts[12] as OwnershipDto?
                recentRaids = parts[13] as List<RaidRecordDto>
                economy = parts[14] as uniffi.cw_ffi.EconomyDto
                val fx = economy?.fx
                if (fx == null) Format.reset()
                else Format.configure(fx.reportingSymbol, fx.currencies.associate { it.id to it.symbol }, fx.currencies.firstOrNull()?.id.orEmpty())
                research = parts[15] as uniffi.cw_ffi.ResearchDto?
                held = parts[16] as List<HeldStockDto>
                storageSummary = parts[17] as StorageSummaryDto?
                lanes = parts[18] as List<LaneDto>
                laneContracts = parts[19] as List<LaneContractDto>
                freight = parts[20] as FreightSummaryDto?
                structure = parts[21] as uniffi.cw_ffi.StructureViewDto
                rivals = parts[22] as List<RivalSummaryDto>
                rivalFeed = parts[23] as List<RivalMoveFeedDto>
                mapPlants = parts[24] as List<MapPlantDto>
                cfoAdvice = parts[25] as List<uniffi.cw_ffi.CfoAdviceDto>
            }
        }
        loadMarkets()
    }

    private suspend fun loadMarkets() {
        val handle = session ?: return
        val code = marketRegion
            ?: selectedFirmDto?.let { firm -> boards.firstOrNull { it.name == firm.region }?.code }
            ?: boards.firstOrNull()?.code
        marketRegion = code
        val board = boards.firstOrNull { it.code == code } ?: return
        marketRows = withContext(engine) {
            handle.products().map { product -> MarketRow(product, handle.market(board.index, product.index)) }
        }
    }

    private fun perform(block: suspend () -> Unit) {
        scope.launch {
            busy = true
            try {
                block()
            } catch (failure: Exception) {
                error = failure.message ?: failure.toString()
            } finally {
                busy = false
            }
        }
    }
}

/** Builds the engine's command JSON: `{"Variant": {"field": value, ...}}`. */
object CommandJson {
    fun command(variant: String, vararg fields: Pair<String, Any>): String =
        "{" + quote(variant) + ":{" + fields.joinToString(",") { (key, value) -> quote(key) + ":" + encode(value) } + "}}"

    private fun encode(value: Any): String = when (value) {
        is String -> quote(value)
        is Boolean -> value.toString()
        is ULong, is UInt, is Long, is Int, is Short, is Byte -> value.toString()
        else -> quote(value.toString())
    }

    private fun quote(text: String): String =
        "\"" + text.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
}

/** Player-facing copy for engine rejections; the key is stable, the words live here. */
object Messages {
    fun rejection(key: String, detail: String): String = when (key) {
        "error.insufficient_cash" -> "Not enough cash for that."
        "error.no_authority" -> "You do not have the authority for that."
        "error.wrong_phase" -> "That cannot be done right now."
        "error.arithmetic" -> "That amount is out of range."
        else -> if (detail.isNotEmpty()) detail else key
    }
}
