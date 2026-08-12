package com.lettuceai.app

import android.app.ActivityManager
import android.app.Service
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.MediaStore
import android.util.Base64
import org.json.JSONObject
import java.io.File
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class CrashMonitorService : Service() {
  private lateinit var monitorThread: HandlerThread
  private lateinit var monitorHandler: Handler
  private lateinit var repository: CrashMonitorRepository
  private var lastReportedHangSessionId: String? = null

  private val monitorRunnable = object : Runnable {
    override fun run() {
      try {
        checkHeartbeat()
      } finally {
        monitorHandler.postDelayed(this, HEARTBEAT_INTERVAL_MS)
      }
    }
  }

  override fun onCreate() {
    super.onCreate()
    repository = CrashMonitorRepository(applicationContext)
    repository.prepare()
    repository.harvestHistoricalExitReasons()
    repository.reportArchivedUncleanShutdownIfNeeded()
    repository.reportUncleanShutdownIfNeeded()

    monitorThread = HandlerThread("CrashMonitorThread")
    monitorThread.start()
    monitorHandler = Handler(monitorThread.looper)
    monitorHandler.post(monitorRunnable)
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    return START_STICKY
  }

  override fun onDestroy() {
    monitorHandler.removeCallbacksAndMessages(null)
    monitorThread.quitSafely()
    super.onDestroy()
  }

  override fun onBind(intent: Intent?): IBinder? = null

  private fun checkHeartbeat() {
    val session = repository.readCurrentSession() ?: return
    if (session.cleanExit) {
      lastReportedHangSessionId = null
      return
    }

    val heartbeatAgeMs = System.currentTimeMillis() - session.lastHeartbeatMs
    if (heartbeatAgeMs < HANG_THRESHOLD_MS) {
      return
    }

    if (lastReportedHangSessionId == session.sessionId) {
      return
    }

    repository.writeHangLog(session, heartbeatAgeMs)
    lastReportedHangSessionId = session.sessionId
  }

  companion object {
    private const val HEARTBEAT_INTERVAL_MS = 2_000L
    private const val HANG_THRESHOLD_MS = 15_000L
  }
}

private data class MonitorSession(
  val sessionId: String,
  val appVersion: String,
  val processId: Int,
  val startedAt: String,
  val startedAtMs: Long,
  val lastHeartbeatAt: String,
  val lastHeartbeatMs: Long,
  val route: String?,
  val cleanExit: Boolean,
)

private data class SelectedLogDir(
  val topTreeUri: String,
  val relativeTerms: List<String>,
  val displayName: String,
)

private data class CrashContext(
  val sessionId: String,
  val recordedAt: String,
  val reason: String,
  val details: String,
  val route: String?,
)

private class CrashMonitorRepository(private val context: Context) {
  private val prefs = context.getSharedPreferences("crash_monitor", Context.MODE_PRIVATE)
  private val monitorDir = File(context.filesDir, "monitor")
  private val crashDir = File(monitorDir, "crashes")
  private val sessionFile = File(monitorDir, "current-session.json")
  private val previousSessionFile = File(monitorDir, "previous-session.json")
  private val crashContextFile = File(monitorDir, "crash-context.json")
  private val breadcrumbsFile = File(monitorDir, "breadcrumbs.jsonl")
  private val cleanExitMarker = File(monitorDir, "clean-exit.marker")
  private val exportConfigFile = File(monitorDir, "log-export-dir.json")
  private val logsDir = File(context.filesDir, "logs")
  private val formatter = SimpleDateFormat("yyyy-MM-dd'T'HH-mm-ss", Locale.US)

  fun prepare() {
    crashDir.mkdirs()
  }

  fun readCurrentSession(): MonitorSession? {
    if (!sessionFile.exists()) return null
    return runCatching {
      val json = JSONObject(sessionFile.readText(StandardCharsets.UTF_8))
      MonitorSession(
        sessionId = json.optString("session_id"),
        appVersion = json.optString("app_version"),
        processId = json.optInt("process_id"),
        startedAt = json.optString("started_at"),
        startedAtMs = json.optLong("started_at_ms"),
        lastHeartbeatAt = json.optString("last_heartbeat_at"),
        lastHeartbeatMs = json.optLong("last_heartbeat_ms"),
        route = json.optString("route").takeIf { it.isNotBlank() },
        cleanExit = json.optBoolean("clean_exit", false),
      )
    }.getOrNull()
  }

  fun reportUncleanShutdownIfNeeded() {
    val session = readCurrentSession() ?: return
    if (session.cleanExit || cleanExitMarker.exists()) return

    val heartbeatAgeMs = System.currentTimeMillis() - session.lastHeartbeatMs
    if (heartbeatAgeMs < 15_000L) return

    val lastReportedSessionId = prefs.getString(KEY_LAST_UNCLEAN_SESSION_ID, null)
    if (lastReportedSessionId == session.sessionId) return

    val content = buildString {
      appendLine("type=unclean-shutdown")
      appendLine("recorded_at=${isoNow()}")
      appendLine("session_id=${session.sessionId}")
      appendLine("app_version=${session.appVersion}")
      appendLine("process_id=${session.processId}")
      appendLine("started_at=${session.startedAt}")
      appendLine("last_heartbeat_at=${session.lastHeartbeatAt}")
      appendLine("heartbeat_age_ms=$heartbeatAgeMs")
      appendLine("route=${session.route ?: "unknown"}")
      appendCrashContext(this, session.sessionId)
      appendBreadcrumbTail(this, session.sessionId)
      appendErrorLogTail(this)
      appendAppLogTail(this)
    }

    writeCrashLog("unclean", session.lastHeartbeatMs, content)
    prefs.edit().putString(KEY_LAST_UNCLEAN_SESSION_ID, session.sessionId).apply()
  }

  fun reportArchivedUncleanShutdownIfNeeded() {
    if (!previousSessionFile.exists()) return

    val session = runCatching {
      val json = JSONObject(previousSessionFile.readText(StandardCharsets.UTF_8))
      MonitorSession(
        sessionId = json.optString("session_id"),
        appVersion = json.optString("app_version"),
        processId = json.optInt("process_id"),
        startedAt = json.optString("started_at"),
        startedAtMs = json.optLong("started_at_ms"),
        lastHeartbeatAt = json.optString("last_heartbeat_at"),
        lastHeartbeatMs = json.optLong("last_heartbeat_ms"),
        route = json.optString("route").takeIf { it.isNotBlank() },
        cleanExit = json.optBoolean("clean_exit", false),
      )
    }.getOrNull()

    if (session == null || session.cleanExit) {
      previousSessionFile.delete()
      return
    }

    val lastReportedSessionId = prefs.getString(KEY_LAST_UNCLEAN_SESSION_ID, null)
    if (lastReportedSessionId == session.sessionId) {
      previousSessionFile.delete()
      return
    }

    val content = buildString {
      appendLine("type=unclean-shutdown")
      appendLine("recorded_at=${isoNow()}")
      appendLine("session_id=${session.sessionId}")
      appendLine("app_version=${session.appVersion}")
      appendLine("process_id=${session.processId}")
      appendLine("started_at=${session.startedAt}")
      appendLine("last_heartbeat_at=${session.lastHeartbeatAt}")
      appendLine("heartbeat_age_ms=${System.currentTimeMillis() - session.lastHeartbeatMs}")
      appendLine("route=${session.route ?: "unknown"}")
      appendLine("source=archived-session")
      appendCrashContext(this, session.sessionId)
      appendBreadcrumbTail(this, session.sessionId)
      appendErrorLogTail(this)
      appendAppLogTail(this)
    }

    writeCrashLog("unclean", session.lastHeartbeatMs, content)
    prefs.edit().putString(KEY_LAST_UNCLEAN_SESSION_ID, session.sessionId).apply()
    previousSessionFile.delete()
  }

  fun writeHangLog(session: MonitorSession, heartbeatAgeMs: Long) {
    val content = buildString {
      appendLine("type=hang")
      appendLine("recorded_at=${isoNow()}")
      appendLine("session_id=${session.sessionId}")
      appendLine("app_version=${session.appVersion}")
      appendLine("process_id=${session.processId}")
      appendLine("started_at=${session.startedAt}")
      appendLine("last_heartbeat_at=${session.lastHeartbeatAt}")
      appendLine("heartbeat_age_ms=$heartbeatAgeMs")
      appendLine("route=${session.route ?: "unknown"}")
      appendCrashContext(this, session.sessionId)
      appendBreadcrumbTail(this, session.sessionId)
      appendErrorLogTail(this)
      appendAppLogTail(this)
    }

    writeCrashLog("hang", System.currentTimeMillis(), content)
  }

  fun harvestHistoricalExitReasons() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return

    val activityManager = context.getSystemService(ActivityManager::class.java) ?: return
    val lastProcessedTimestamp = prefs.getLong(KEY_LAST_EXIT_TIMESTAMP, 0L)
    var newestTimestamp = lastProcessedTimestamp

    val exitReasons = runCatching {
      activityManager.getHistoricalProcessExitReasons(null, 0, 16)
    }.getOrDefault(emptyList())

    exitReasons
      .asSequence()
      .filter { it.processName != null && !it.processName.endsWith(":monitor") }
      .filter { it.timestamp > lastProcessedTimestamp }
      .filter { shouldReportExitReason(it.reason) }
      .sortedBy { it.timestamp }
      .forEach { info ->
        val traceText = runCatching {
          info.traceInputStream?.use { stream -> formatTracePayload(stream.readBytes()) }
        }.getOrNull()

        val content = buildString {
          appendLine("type=process-exit")
          appendLine("recorded_at=${isoNow()}")
          appendLine("reason=${reasonName(info.reason)}")
          appendLine("reason_code=${info.reason}")
          appendLine("timestamp=${info.timestamp}")
          appendLine("process_name=${info.processName}")
          appendLine("importance=${info.importance}")
          appendLine("status=${info.status}")
          appendLine("description=${info.description ?: ""}")
          appendLine("pss_kb=${info.pss}")
          appendLine("rss_kb=${info.rss}")
          appendLine("cause_hint=${buildCauseHint(info.reason, info.description)}")
          appendCrashContext(this, activeOrPreviousSessionId())
          appendBreadcrumbTail(this, activeOrPreviousSessionId())
          appendErrorLogTail(this)
          if (!traceText.isNullOrBlank()) {
            appendLine()
            appendLine("trace_begin")
            appendLine(traceText.trimEnd())
            appendLine("trace_end")
          }
          appendAppLogTail(this)
        }

        writeCrashLog(reasonName(info.reason).lowercase(Locale.US), info.timestamp, content)
        if (info.timestamp > newestTimestamp) {
          newestTimestamp = info.timestamp
        }
      }

    if (newestTimestamp > lastProcessedTimestamp) {
      prefs.edit().putLong(KEY_LAST_EXIT_TIMESTAMP, newestTimestamp).apply()
      }
  }

  private fun activeOrPreviousSessionId(): String? {
    return runCatching {
      val json = JSONObject(previousSessionFile.readText(StandardCharsets.UTF_8))
      json.optString("session_id").takeIf { it.isNotBlank() }
    }.getOrNull() ?: readCurrentSession()?.sessionId
  }

  private fun readCrashContextForSession(sessionId: String?): CrashContext? {
    if (sessionId.isNullOrBlank() || !crashContextFile.exists()) return null
    return runCatching {
      val json = JSONObject(crashContextFile.readText(StandardCharsets.UTF_8))
      val fileSessionId = json.optString("session_id")
      if (fileSessionId == sessionId) {
        CrashContext(
          sessionId = fileSessionId,
          recordedAt = json.optString("recorded_at"),
          reason = json.optString("reason"),
          details = json.optString("details"),
          route = json.optString("route").takeIf { it.isNotBlank() },
        )
      } else {
        null
      }
    }.getOrNull()
  }

  private fun appendCrashContext(builder: StringBuilder, sessionId: String?) {
    val context = readCrashContextForSession(sessionId) ?: return
    builder.appendLine()
    builder.appendLine("crash_context_begin")
    builder.appendLine("recorded_at=${context.recordedAt}")
    builder.appendLine("reason=${context.reason}")
    builder.appendLine("details=${context.details}")
    builder.appendLine("route=${context.route ?: "unknown"}")
    builder.appendLine("crash_context_end")
  }

  private fun appendBreadcrumbTail(builder: StringBuilder, sessionId: String?) {
    if (sessionId.isNullOrBlank() || !breadcrumbsFile.exists()) return
    val tail = runCatching {
      breadcrumbsFile.readLines(StandardCharsets.UTF_8)
        .mapNotNull { line ->
          val json = JSONObject(line)
          if (json.optString("session_id") != sessionId) {
            null
          } else {
            "${json.optString("timestamp")} ${json.optString("category")}: ${json.optString("message")} (route=${json.optString("route").ifBlank { "unknown" }})"
          }
        }
        .takeLast(50)
        .joinToString(separator = "\n")
    }.getOrNull()

    if (tail.isNullOrBlank()) return

    builder.appendLine()
    builder.appendLine("breadcrumbs_begin")
    builder.appendLine(tail)
    builder.appendLine("breadcrumbs_end")
  }

  private fun appendAppLogTail(builder: StringBuilder) {
    val latestLog = logsDir
      .takeIf { it.exists() }
      ?.listFiles()
      ?.filter { it.isFile && it.extension.equals("log", ignoreCase = true) }
      ?.maxByOrNull { it.lastModified() }
      ?: return

    val tail = runCatching {
      latestLog.readLines(StandardCharsets.UTF_8).takeLast(200).joinToString(separator = "\n")
    }.getOrNull() ?: return

    builder.appendLine()
    builder.appendLine("app_log_tail_begin")
    builder.appendLine(tail)
    builder.appendLine("app_log_tail_end")
  }

  private fun appendErrorLogTail(builder: StringBuilder) {
    val latestLog = logsDir
      .takeIf { it.exists() }
      ?.listFiles()
      ?.filter { it.isFile && it.extension.equals("log", ignoreCase = true) }
      ?.maxByOrNull { it.lastModified() }
      ?: return

    val tail = runCatching {
      latestLog.readLines(StandardCharsets.UTF_8)
        .filter { line -> line.contains(" ERROR ") || line.contains(" WARN ") }
        .takeLast(100)
        .joinToString(separator = "\n")
    }.getOrNull()

    if (tail.isNullOrBlank()) return

    builder.appendLine()
    builder.appendLine("error_log_tail_begin")
    builder.appendLine(tail)
    builder.appendLine("error_log_tail_end")
  }

  private fun writeCrashLog(prefix: String, timestampMs: Long, content: String) {
    val filename = "${prefix}-${formatter.format(Date(timestampMs))}.log"
    runCatching {
      File(crashDir, filename).writeText(content, StandardCharsets.UTF_8)
    }
    runCatching {
      writeExternalLog(filename, content)
    }
  }

  private fun readSelectedLogDir(): SelectedLogDir? {
    if (!exportConfigFile.exists()) return null
    return runCatching {
      val root = JSONObject(exportConfigFile.readText(StandardCharsets.UTF_8))
      val dirJson = JSONObject(root.getString("dir_path_json"))
      val termsJson = dirJson.optJSONArray("relativeTerms")
      val relativeTerms = buildList {
        if (termsJson != null) {
          for (index in 0 until termsJson.length()) {
            add(termsJson.getString(index))
          }
        }
      }

      SelectedLogDir(
        topTreeUri = dirJson.getString("topTreeUri"),
        relativeTerms = relativeTerms,
        displayName = root.optString("display_name", "Selected folder"),
      )
    }.getOrNull()
  }

  private fun writeExternalLog(filename: String, content: String) {
    val selectedDir = readSelectedLogDir()
    if (selectedDir != null) {
      writeSelectedDirLog(selectedDir, filename, content)
      return
    }

    writeDownloadsLog(filename, content)
  }

  private fun writeSelectedDirLog(selectedDir: SelectedLogDir, filename: String, content: String) {
    val topTreeUri = Uri.parse(selectedDir.topTreeUri)
    val parentId = selectedDir.relativeTerms.lastOrNull()
      ?: DocumentsContract.getTreeDocumentId(topTreeUri)
    val parentUri = DocumentsContract.buildDocumentUriUsingTree(topTreeUri, parentId)
    val fileUri = DocumentsContract.createDocument(
      context.contentResolver,
      parentUri,
      "text/plain",
      filename,
    ) ?: throw IllegalStateException("Failed to create external crash log document")

    context.contentResolver.openOutputStream(fileUri, "w")?.use { stream ->
      stream.write(content.toByteArray(StandardCharsets.UTF_8))
      stream.flush()
    } ?: throw IllegalStateException("Failed to open external crash log output stream")
  }

  private fun writeDownloadsLog(filename: String, content: String) {
    val values = ContentValues().apply {
      put(MediaStore.MediaColumns.DISPLAY_NAME, filename)
      put(MediaStore.MediaColumns.MIME_TYPE, "text/plain")
      put(
        MediaStore.MediaColumns.RELATIVE_PATH,
        "${Environment.DIRECTORY_DOWNLOADS}/lettuceai/logs",
      )
    }

    val fileUri = context.contentResolver.insert(
      MediaStore.Downloads.EXTERNAL_CONTENT_URI,
      values,
    ) ?: throw IllegalStateException("Failed to create Downloads crash log document")

    context.contentResolver.openOutputStream(fileUri, "w")?.use { stream ->
      stream.write(content.toByteArray(StandardCharsets.UTF_8))
      stream.flush()
    } ?: throw IllegalStateException("Failed to open Downloads crash log output stream")
  }

  private fun reasonName(reason: Int): String {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return "UNKNOWN"
    return when (reason) {
      android.app.ApplicationExitInfo.REASON_UNKNOWN -> "UNKNOWN"
      android.app.ApplicationExitInfo.REASON_ANR -> "ANR"
      android.app.ApplicationExitInfo.REASON_CRASH -> "CRASH"
      android.app.ApplicationExitInfo.REASON_CRASH_NATIVE -> "CRASH_NATIVE"
      android.app.ApplicationExitInfo.REASON_DEPENDENCY_DIED -> "DEPENDENCY_DIED"
      android.app.ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE -> "EXCESSIVE_RESOURCE_USAGE"
      android.app.ApplicationExitInfo.REASON_EXIT_SELF -> "EXIT_SELF"
      android.app.ApplicationExitInfo.REASON_INITIALIZATION_FAILURE -> "INITIALIZATION_FAILURE"
      android.app.ApplicationExitInfo.REASON_LOW_MEMORY -> "LOW_MEMORY"
      android.app.ApplicationExitInfo.REASON_OTHER -> "OTHER"
      android.app.ApplicationExitInfo.REASON_PERMISSION_CHANGE -> "PERMISSION_CHANGE"
      android.app.ApplicationExitInfo.REASON_SIGNALED -> "SIGNALED"
      android.app.ApplicationExitInfo.REASON_USER_REQUESTED -> "USER_REQUESTED"
      android.app.ApplicationExitInfo.REASON_USER_STOPPED -> "USER_STOPPED"
      else -> "UNKNOWN"
    }
  }

  private fun buildCauseHint(reason: Int, description: String?): String {
    val crashContext = readCrashContextForSession(activeOrPreviousSessionId())
    if (crashContext != null) {
      return "${crashContext.reason}: ${crashContext.details}"
    }

    return when (reason) {
      android.app.ApplicationExitInfo.REASON_ANR -> "Main app process stopped responding"
      android.app.ApplicationExitInfo.REASON_CRASH -> "Java/Kotlin process crash"
      android.app.ApplicationExitInfo.REASON_CRASH_NATIVE -> "Native crash in Rust, WebView, or native dependency"
      android.app.ApplicationExitInfo.REASON_SIGNALED -> description ?: "Process terminated by signal"
      else -> description ?: "No explicit cause available"
    }
  }

  private fun formatTracePayload(bytes: ByteArray): String? {
    if (bytes.isEmpty()) return null

    val decoded = decodeUtf8Strict(bytes)
    if (decoded != null && looksLikeReadableText(decoded)) {
      return decoded.trimEnd()
    }

    val previewHex = bytes
      .take(32)
      .joinToString(separator = " ") { byte -> "%02x".format(byte.toInt() and 0xff) }
    val base64 = Base64.encodeToString(bytes, Base64.NO_WRAP)

    return buildString {
      appendLine("[binary trace payload]")
      appendLine("trace_encoding=base64")
      appendLine("trace_size_bytes=${bytes.size}")
      appendLine("trace_preview_hex=$previewHex")
      append(base64)
    }
  }

  private fun decodeUtf8Strict(bytes: ByteArray): String? {
    val decoder = StandardCharsets.UTF_8
      .newDecoder()
      .onMalformedInput(CodingErrorAction.REPORT)
      .onUnmappableCharacter(CodingErrorAction.REPORT)

    return try {
      decoder.decode(ByteBuffer.wrap(bytes)).toString()
    } catch (_: CharacterCodingException) {
      null
    }
  }

  private fun looksLikeReadableText(value: String): Boolean {
    if (value.isBlank()) return false

    var suspicious = 0
    for (char in value) {
      if (char == '\uFFFD') return false
      if (char == '\n' || char == '\r' || char == '\t') continue
      if (Character.isISOControl(char)) suspicious++
    }

    return suspicious * 20 <= value.length
  }

  private fun shouldReportExitReason(reason: Int): Boolean {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return false
    return when (reason) {
      android.app.ApplicationExitInfo.REASON_ANR,
      android.app.ApplicationExitInfo.REASON_CRASH,
      android.app.ApplicationExitInfo.REASON_CRASH_NATIVE,
      android.app.ApplicationExitInfo.REASON_DEPENDENCY_DIED,
      android.app.ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE,
      android.app.ApplicationExitInfo.REASON_INITIALIZATION_FAILURE,
      android.app.ApplicationExitInfo.REASON_LOW_MEMORY,
      android.app.ApplicationExitInfo.REASON_SIGNALED -> true
      else -> false
    }
  }

  private fun isoNow(): String {
    return SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSSXXX", Locale.US).format(Date())
  }

  companion object {
    private const val KEY_LAST_EXIT_TIMESTAMP = "last_exit_timestamp"
    private const val KEY_LAST_UNCLEAN_SESSION_ID = "last_unclean_session_id"
  }
}
