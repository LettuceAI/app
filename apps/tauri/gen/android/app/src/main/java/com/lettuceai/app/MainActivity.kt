package com.lettuceai.app

import android.os.Bundle
import android.view.View
import android.view.ViewTreeObserver
import androidx.activity.enableEdgeToEdge
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  // The backend stays ready when Android recreates the activity in this process.
  private companion object {
    var startupFinished = false
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    val appContent = findViewById<View>(android.R.id.content)
    val initialBottomPadding = appContent.paddingBottom
    ViewCompat.setOnApplyWindowInsetsListener(appContent) { view, insets ->
      // Handle IME insets natively so the layout viewport also shrinks.
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(view.paddingLeft, view.paddingTop, view.paddingRight, initialBottomPadding + ime.bottom)
      val remaining = WindowInsetsCompat.Builder(insets)
        .setInsets(WindowInsetsCompat.Type.ime(), Insets.NONE)
      if (ime.bottom > 0) {
        val navigation = insets.getInsets(WindowInsetsCompat.Type.navigationBars())
        remaining.setInsets(WindowInsetsCompat.Type.navigationBars(), Insets.of(navigation.left, navigation.top, navigation.right, 0))
      }
      remaining.build()
    }
    ViewCompat.requestApplyInsets(appContent)
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
