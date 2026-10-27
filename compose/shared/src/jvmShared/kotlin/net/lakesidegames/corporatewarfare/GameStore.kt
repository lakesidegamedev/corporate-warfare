package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.cw_ffi.BuildQuoteDto
import uniffi.cw_ffi.CommodityDto
import uniffi.cw_ffi.DivisionDto
import uniffi.cw_ffi.OwnershipDto
import uniffi.cw_ffi.PeriodDto
import uniffi.cw_ffi.RaidRecordDto
import uniffi.cw_ffi.MapDivisionDto
import uniffi.cw_ffi.StockDto
import uniffi.cw_ffi.StrategyDto
import uniffi.cw_ffi.strategies
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
import uniffi.cw_ffi.products

/** What a form's pickers can choose from, loaded for one company when a form opens. */
data class Pickers(
    val loans: List<uniffi.cw_ffi.LoanDto> = emptyList(),
    val contracts: List<uniffi.cw_ffi.ContractDto> = emptyList(),
    val policies: List<uniffi.cw_ffi.InsuranceDto> = emptyList(),
    val votes: List<uniffi.cw_ffi.BoardVoteDto> = emptyList(),
    val deals: List<uniffi.cw_ffi.DealDto> = emptyList(),
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
    var events by mutableStateOf<List<EventDto>>(emptyList()); private set
    var boards by mutableStateOf<List<RegionBoardDto>>(emptyList()); private set
    var stocks by mutableStateOf<List<StockDto>>(emptyList()); private set
    /** The selected company's four-week results, oldest first, up to five years. */
    var history by mutableStateOf<List<PeriodDto>>(emptyList()); private set
    var ownership by mutableStateOf<OwnershipDto?>(null); private set
    /** Recent raids anywhere, newest first, for the rival-activity feed. */
    var recentRaids by mutableStateOf<List<RaidRecordDto>>(emptyList()); private set
    var commodities by mutableStateOf<List<CommodityDto>>(emptyList()); private set
    var mapDivisions by mutableStateOf<List<MapDivisionDto>>(emptyList()); private set
    val strategyCatalogue: List<StrategyDto> by lazy { strategies() }
    /** Every player action with its form, in Command Center order. */
    val actions: List<uniffi.cw_ffi.ActionDto> by lazy { uniffi.cw_ffi.actionCatalogue() }
    val productCatalogue: List<ProductDto> by lazy { products() }
    /** The economy, central bank and government. */
    var economy by mutableStateOf<uniffi.cw_ffi.EconomyDto?>(null); private set
    /** The selected company's research position. */
    var research by mutableStateOf<uniffi.cw_ffi.ResearchDto?>(null); private set
    var marketRegion by mutableStateOf<String?>(null)
    var marketRows by mutableStateOf<List<MarketRow>>(emptyList()); private set

    val selectedFirmDto: FirmDto? get() = firms.firstOrNull { it.handle == selectedFirm }
    val hasAutosave: Boolean get() = Saves.list(saveDir).isNotEmpty()

    fun saveSlots(): List<SaveSlot> = Saves.list(saveDir)

    private val autosave get() = File(saveDir, "autosave.cwsave")

    fun newCareer(seed: Long, guided: Boolean, fiveYear: Boolean = false) = perform {
        val handle = withContext(engine) { SessionHandle(seed.toULong(), guided, fiveYear) }
        session = handle
        selectedFirm = withContext(engine) { handle.overview().primaryFirm }
        reload()
        ready = true
    }

    /** A new career from the new-game screen. */
    fun startCareer(options: uniffi.cw_ffi.NewGameDto) = perform {
        val handle = withContext(engine) { SessionHandle.start(options) }
        session = handle
        selectedFirm = withContext(engine) { handle.overview().primaryFirm }
        reload()
        ready = true
    }

    /** Open a save from the load screen. */
    fun loadSave(file: File) = perform {
        val bytes = withContext(engine) { file.readBytes() }
        val handle = withContext(engine) { importSave(bytes) }
        session = handle
        selectedFirm = withContext(engine) { handle.overview().primaryFirm }
        reload()
        ready = true
    }

    fun continueCareer() = perform {
        val latest = Saves.list(saveDir).firstOrNull()?.file ?: autosave
        val bytes = withContext(engine) { latest.readBytes() }
        val handle = withContext(engine) { importSave(bytes) }
        session = handle
        selectedFirm = withContext(engine) { handle.overview().primaryFirm }
        reload()
        ready = true
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
                if (overview?.pause != null || events.any { it.mandatory }) break
                delay(1_000L / speed)
            }
            running = false
            runJob = null
        }
    }

    private suspend fun step() {
        val handle = session ?: return
        val result = withContext(engine) { handle.advance(1u) }
        status = "Advanced to week ${result.finalWeek}."
        reload()
        if ((overview?.tick ?: 0u) % 4u == 0u) save()
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

    /** What a build would cost, computed by the engine without changing anything. */
    fun quote(region: UInt, recipe: UInt, blocks: UInt, onResult: (BuildQuoteDto?) -> Unit) {
        val handle = session ?: return
        val firm = selectedFirm ?: return
        scope.launch {
            onResult(withContext(engine) { handle.buildQuote(firm, region, recipe, blocks) })
        }
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
                )
            }
            onResult(result)
        }
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
            )
        }.let { parts ->
            @Suppress("UNCHECKED_CAST")
            run {
                overview = parts[0] as OverviewDto
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
                research = parts[15] as uniffi.cw_ffi.ResearchDto?
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
            products().map { product -> MarketRow(product, handle.market(board.index, product.index)) }
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
