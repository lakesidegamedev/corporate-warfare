package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.key.Key

/**
 * The rebindable keys of the desktop game: one map from an action to a key name, kept in the settings file as
 * `key.<action>`. An action with no entry uses its default. Only single keys can be bound (a letter, a digit, Space or
 * F1 to F12); Esc and the Ctrl chords stay fixed. A key can serve one action only.
 */
object KeyBindings {
    class Action(val id: String, val label: String, val default: String)

    /** The keys that can be bound, by the name shown to the player. */
    val keys: List<Pair<String, Key>> = buildList {
        add("Space" to Key.Spacebar)
        val letters = listOf(Key.A, Key.B, Key.C, Key.D, Key.E, Key.F, Key.G, Key.H, Key.I, Key.J, Key.K, Key.L, Key.M, Key.N, Key.O, Key.P, Key.Q, Key.R, Key.S, Key.T, Key.U, Key.V, Key.W, Key.X, Key.Y, Key.Z)
        letters.forEachIndexed { i, k -> add(('A' + i).toString() to k) }
        val digits = listOf(Key.Zero, Key.One, Key.Two, Key.Three, Key.Four, Key.Five, Key.Six, Key.Seven, Key.Eight, Key.Nine)
        digits.forEachIndexed { i, k -> add(i.toString() to k) }
        val fKeys = listOf(Key.F1, Key.F2, Key.F3, Key.F4, Key.F5, Key.F6, Key.F7, Key.F8, Key.F9, Key.F10, Key.F11, Key.F12)
        fKeys.forEachIndexed { i, k -> add("F${i + 1}" to k) }
    }

    /** The actions, in the order the settings list them: the clock first, then the places that have a key. */
    val actions: List<Action> = listOf(
        Action("PlayPause", "Play or pause time", "Space"),
        Action("AdvanceWeek", "Next week", "N"),
        Action("Speed1", "Time at normal speed", "1"),
        Action("Speed2", "Time at fast speed", "2"),
        Action("Speed3", "Time at fastest speed", "3"),
    ) + Place.entries.filter { it.key.isNotEmpty() }
        .sortedBy { it.key.drop(1).toIntOrNull() ?: 99 }
        .map { Action("place.${it.name}", "Open ${it.title}", it.key) }

    private var overrides by mutableStateOf<Map<String, String>>(emptyMap())

    fun nameOf(key: Key): String? = keys.firstOrNull { it.second == key }?.first
    private fun known(name: String) = keys.any { it.first == name }

    /** The key name an action answers to now. */
    fun keyOf(id: String): String = overrides[id] ?: actions.firstOrNull { it.id == id }?.default.orEmpty()

    /** The action a key name is bound to, or null. */
    fun actionFor(name: String): String? = actions.firstOrNull { keyOf(it.id) == name }?.id

    /** The key shown beside a place in the navigation, empty when it has none. */
    fun placeKey(place: Place): String = if (place.key.isEmpty()) "" else keyOf("place.${place.name}")

    /** Bind an action to a key. Null when done, else the reason in plain words (nothing changes). */
    fun bind(id: String, name: String): String? {
        val action = actions.firstOrNull { it.id == id } ?: return "That action has no key"
        if (!known(name)) return "That key cannot be used. Pick a letter, a digit, Space or F1 to F12."
        val holder = actions.firstOrNull { it.id != id && keyOf(it.id) == name }
        if (holder != null) return "$name is already used to ${holder.label.replaceFirstChar { it.lowercase() }}. Change that one first."
        overrides = if (name == action.default) overrides - id else overrides + (id to name)
        return null
    }

    fun reset() { overrides = emptyMap() }

    /** The changed bindings, for saving. */
    fun changed(): Map<String, String> = overrides

    /** Load saved bindings, dropping any that no longer make sense (unknown action or key, or a clash). */
    fun load(saved: Map<String, String>) {
        overrides = emptyMap()
        for ((id, name) in saved) bind(id, name)
    }
}
