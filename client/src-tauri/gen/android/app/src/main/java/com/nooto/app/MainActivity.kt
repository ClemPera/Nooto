package com.nooto.app

import android.graphics.Color
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Workaround for android webview <144 that doesn't support env(safe-area-inset-*) (see #106)
    window.statusBarColor = Color.TRANSPARENT
    window.navigationBarColor = Color.TRANSPARENT
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) { //Android 10+
      window.isStatusBarContrastEnforced = false
      window.isNavigationBarContrastEnforced = false
    }
    WindowCompat.getInsetsController(window, window.decorView).apply {
      isAppearanceLightStatusBars = false
      isAppearanceLightNavigationBars = false
    }
    window.decorView.setBackgroundColor(0xFF0F172A.toInt())
    ViewCompat.setOnApplyWindowInsetsListener(findViewById(android.R.id.content)) { view, insets ->
      val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
      val navbar = insets.getInsets(WindowInsetsCompat.Type.navigationBars())
      val statusbar = insets.getInsets(WindowInsetsCompat.Type.statusBars())
      view.setPadding(0, statusbar.top, 0, maxOf(navbar.bottom, keyboard.bottom))
      insets
    }
  }
}