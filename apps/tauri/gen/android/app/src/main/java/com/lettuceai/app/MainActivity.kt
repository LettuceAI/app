package com.lettuceai.app

import android.os.Bundle
import android.view.ViewTreeObserver
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  // The backend stays ready when Android recreates the activity in this process.
  private companion object {
    var startupFinished = false
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    val content = window.decorView
    content.viewTreeObserver.addOnPreDrawListener(object : ViewTreeObserver.OnPreDrawListener {
      override fun onPreDraw(): Boolean {
        if (!startupFinished) return false
        content.viewTreeObserver.removeOnPreDrawListener(this)
        return true
      }
    })
  }

  fun finishStartup() {
    runOnUiThread {
      startupFinished = true
      window.decorView.invalidate()
    }
  }
}
