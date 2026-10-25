package com.lettuceai.app

import android.Manifest
import android.app.Activity
import android.content.ComponentCallbacks2
import android.content.res.Configuration
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.content.IntentCompat
import androidx.lifecycle.Lifecycle
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class SubscribeArgs {
  lateinit var handler: Channel
}

@InvokeArg
class BackgroundWorkArgs {
  var active: Boolean = false
}

@TauriPlugin
class HostPlugin(private val activity: Activity) : Plugin(activity) {
  private var channel: Channel? = null
  private val pending = ArrayList<JSObject>()
  private var launchIntentRead = false
  private var notificationsRequested = false
  private var workActive = false
  private var memoryWarningPending = false
  private val memoryCallbacks = object : ComponentCallbacks2 {
    override fun onConfigurationChanged(configuration: Configuration) {}
    override fun onLowMemory() { memoryWarning() }
    override fun onTrimMemory(level: Int) {
      if (level == ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW ||
          level == ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL ||
          level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND) memoryWarning()
    }
  }

  init { activity.application.registerComponentCallbacks(memoryCallbacks) }

  override fun onDestroy(activity: AppCompatActivity) {
    activity.application.unregisterComponentCallbacks(memoryCallbacks)
  }

  @Synchronized
  private fun memoryWarning() {
    memoryWarningPending = true
    flush()
  }

  @Command
  @Synchronized
  fun subscribe(invoke: Invoke) {
    val args = invoke.parseArgs(SubscribeArgs::class.java)
    channel = args.handler
    if (!launchIntentRead) {
      launchIntentRead = true
      pending.addAll(itemsOf(activity.intent))
    }
    flush()
    invoke.resolve()
  }

  @Command
  fun setBackgroundWork(invoke: Invoke) {
    val args = invoke.parseArgs(BackgroundWorkArgs::class.java)
    activity.runOnUiThread {
      workActive = args.active
      if (args.active) startWork() else stopWork()
    }
    invoke.resolve()
  }

  @Synchronized
  override fun onNewIntent(intent: Intent) {
    pending.addAll(itemsOf(intent))
    flush()
  }

  private fun flush() {
    val target = channel ?: return
    if (memoryWarningPending) {
      memoryWarningPending = false
      target.send(JSObject().put("memoryPressure", true))
    }
    if (pending.isEmpty()) return
    val items = JSArray()
    pending.forEach { items.put(it) }
    pending.clear()
    target.send(JSObject().put("items", items))
  }

  private fun itemsOf(intent: Intent?): List<JSObject> {
    if (intent == null) return emptyList()
    if (intent.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY != 0) return emptyList()
    return when (intent.action) {
      Intent.ACTION_VIEW -> viewItems(intent.data)
      Intent.ACTION_SEND -> listOfNotNull(
        IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
      ).map(::fileItem)
      Intent.ACTION_SEND_MULTIPLE -> (
        IntentCompat.getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
          ?: emptyList<Uri>()
      ).map(::fileItem)
      else -> emptyList()
    }
  }

  private fun viewItems(data: Uri?): List<JSObject> {
    if (data == null) return emptyList()
    if (data.scheme != NAVIGATION_SCHEME) return listOf(fileItem(data))
    if (data.host != NAVIGATION_HOST) return emptyList()
    return listOf(JSObject().put("kind", "navigate").put("path", data.path ?: ""))
  }

  private fun fileItem(uri: Uri): JSObject =
    JSObject()
      .put("kind", "file")
      .put("source", JSObject().put("uri", uri.toString()))

  private fun startWork() {
    requestNotificationPermission()
    try {
      ContextCompat.startForegroundService(
        activity.applicationContext,
        Intent(activity.applicationContext, BackgroundWorkService::class.java)
      )
    } catch (error: IllegalStateException) {
      Log.w(TAG, "the background work service could not be started", error)
    } catch (error: SecurityException) {
      Log.w(TAG, "the background work service is not permitted", error)
    }
  }

  private fun stopWork() {
    activity.applicationContext.stopService(
      Intent(activity.applicationContext, BackgroundWorkService::class.java)
    )
  }

  override fun onResume(activity: AppCompatActivity) {
    if (workActive) activity.window.decorView.post { requestNotificationPermission() }
  }

  private fun requestNotificationPermission() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
    if (notificationsRequested) return
    if (ContextCompat.checkSelfPermission(activity, Manifest.permission.POST_NOTIFICATIONS)
      == PackageManager.PERMISSION_GRANTED
    ) return
    val appCompat = activity as? AppCompatActivity ?: return
    if (!appCompat.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) return
    notificationsRequested = true
    ActivityCompat.requestPermissions(
      activity,
      arrayOf(Manifest.permission.POST_NOTIFICATIONS),
      NOTIFICATION_REQUEST_CODE
    )
  }

  private companion object {
    const val TAG = "LettuceHost"
    const val NAVIGATION_SCHEME = "lettuceai"
    const val NAVIGATION_HOST = "open"
    const val NOTIFICATION_REQUEST_CODE = 4101
  }
}
