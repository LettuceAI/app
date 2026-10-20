package com.lettuceai.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

class BackgroundWorkService : Service() {
  override fun onCreate() {
    super.onCreate()
    val manager = getSystemService(NotificationManager::class.java)
    manager.createNotificationChannel(
      NotificationChannel(
        CHANNEL_ID,
        getString(R.string.background_work_channel),
        NotificationManager.IMPORTANCE_LOW
      )
    )
    enterForeground()
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    enterForeground()
    return START_NOT_STICKY
  }

  private fun enterForeground() {
    try {
      ServiceCompat.startForeground(
        this,
        NOTIFICATION_ID,
        notification(),
        ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
      )
    } catch (error: IllegalStateException) {
      Log.w(TAG, "the service could not enter the foreground", error)
      stopSelf()
    } catch (error: SecurityException) {
      Log.w(TAG, "the service is not permitted to enter the foreground", error)
      stopSelf()
    }
  }

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onTimeout(startId: Int, fgsType: Int) {
    stopSelf()
  }

  private fun notification() =
    NotificationCompat.Builder(this, CHANNEL_ID)
      .setSmallIcon(R.drawable.ic_notification)
      .setContentTitle(getString(R.string.background_work_title))
      .setContentText(getString(R.string.background_work_body))
      .setCategory(NotificationCompat.CATEGORY_SERVICE)
      .setPriority(NotificationCompat.PRIORITY_LOW)
      .setOngoing(true)
      .setOnlyAlertOnce(true)
      .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
      .setContentIntent(
        PendingIntent.getActivity(
          this,
          0,
          Intent(this, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP),
          PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )
      )
      .build()

  private companion object {
    const val TAG = "LettuceHost"
    const val CHANNEL_ID = "background_work"
    const val NOTIFICATION_ID = 4101
  }
}
