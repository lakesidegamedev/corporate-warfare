package net.lakesidegames.corporatewarfare

import com.sun.jna.Library
import com.sun.jna.Memory
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.Structure
import com.sun.jna.WString
import com.sun.jna.ptr.PointerByReference
import java.util.concurrent.TimeUnit

/**
 * Where the Lakeside sign-in is kept: Windows Credential Manager, or the Linux Secret Service
 * through the standard `secret-tool` program. Anywhere else, or when either is missing or fails,
 * there is no store and the sign-in lasts the session only. Never a plain file; never logged.
 */
fun desktopTokenStore(): TokenStore {
    val os = System.getProperty("os.name").lowercase()
    return when {
        os.contains("win") -> WindowsCredentialStore()
        os.contains("linux") -> SecretServiceStore()
        else -> NoTokenStore
    }
}

@Structure.FieldOrder(
    "Flags", "Type", "TargetName", "Comment", "LastWrittenLow", "LastWrittenHigh", "CredentialBlobSize",
    "CredentialBlob", "Persist", "AttributeCount", "Attributes", "TargetAlias", "UserName",
)
class WinCredential : Structure {
    constructor() : super()
    constructor(p: Pointer) : super(p)

    @JvmField var Flags: Int = 0
    @JvmField var Type: Int = 0
    @JvmField var TargetName: WString? = null
    @JvmField var Comment: WString? = null
    @JvmField var LastWrittenLow: Int = 0
    @JvmField var LastWrittenHigh: Int = 0
    @JvmField var CredentialBlobSize: Int = 0
    @JvmField var CredentialBlob: Pointer? = null
    @JvmField var Persist: Int = 0
    @JvmField var AttributeCount: Int = 0
    @JvmField var Attributes: Pointer? = null
    @JvmField var TargetAlias: WString? = null
    @JvmField var UserName: WString? = null
}

interface WinAdvapi32 : Library {
    fun CredWriteW(credential: WinCredential, flags: Int): Boolean
    fun CredReadW(target: WString, type: Int, flags: Int, out: PointerByReference): Boolean
    fun CredDeleteW(target: WString, type: Int, flags: Int): Boolean
    fun CredFree(buffer: Pointer)
}

/** Windows Credential Manager, a generic credential kept on this machine for this Windows user. */
class WindowsCredentialStore(private val target: String = "CorporateWarfare/LakesideSignIn") : TokenStore {
    private val api: WinAdvapi32? by lazy { runCatching { Native.load("Advapi32", WinAdvapi32::class.java) }.getOrNull() }

    override fun load(): String? = runCatching {
        val a = api ?: return null
        val out = PointerByReference()
        if (!a.CredReadW(WString(target), GENERIC, 0, out)) return null
        val cred = WinCredential(out.value)
        cred.read()
        val bytes = cred.CredentialBlob?.getByteArray(0, cred.CredentialBlobSize)
        a.CredFree(out.value)
        bytes?.toString(Charsets.UTF_8)
    }.getOrNull()

    override fun save(secret: String): Boolean = runCatching {
        val a = api ?: return false
        val bytes = secret.toByteArray(Charsets.UTF_8)
        val blob = Memory(bytes.size.toLong()).also { it.write(0, bytes, 0, bytes.size) }
        val cred = WinCredential().apply {
            Type = GENERIC
            TargetName = WString(target)
            CredentialBlobSize = bytes.size
            CredentialBlob = blob
            Persist = PERSIST_LOCAL_MACHINE
            UserName = WString("lakeside")
        }
        a.CredWriteW(cred, 0)
    }.getOrDefault(false)

    override fun delete() {
        runCatching { api?.CredDeleteW(WString(target), GENERIC, 0) }
    }

    private companion object {
        const val GENERIC = 1
        const val PERSIST_LOCAL_MACHINE = 2
    }
}

/**
 * The Linux Secret Service (GNOME Keyring, KWallet's bridge) through `secret-tool`. The secret goes
 * in on standard input, never on the command line. Without the program or a running keyring every
 * call quietly does nothing and the sign-in stays in memory.
 */
class SecretServiceStore : TokenStore {
    private val attrs = listOf("service", "corporate-warfare", "account", "lakeside-sign-in")

    private fun run(args: List<String>, input: String? = null): Pair<Int, String>? = runCatching {
        val process = ProcessBuilder(listOf("secret-tool") + args).redirectErrorStream(false).start()
        if (input != null) process.outputStream.use { it.write(input.toByteArray(Charsets.UTF_8)) } else process.outputStream.close()
        if (!process.waitFor(10, TimeUnit.SECONDS)) {
            process.destroyForcibly()
            return null
        }
        process.exitValue() to String(process.inputStream.readNBytes(64 * 1024), Charsets.UTF_8)
    }.getOrNull()

    override fun load(): String? = run(listOf("lookup") + attrs)?.takeIf { it.first == 0 }?.second?.trim()?.takeIf { it.isNotEmpty() }

    override fun save(secret: String): Boolean =
        run(listOf("store", "--label=Corporate Warfare Lakeside sign-in") + attrs, secret)?.first == 0

    override fun delete() {
        run(listOf("clear") + attrs)
    }
}
