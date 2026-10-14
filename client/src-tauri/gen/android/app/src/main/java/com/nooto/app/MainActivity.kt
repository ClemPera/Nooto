package com.nooto.app

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    ViewCompat.setOnApplyWindowInsetsListener(findViewById(android.R.id.content)) { view, insets ->
      val navbar = insets.getInsets(WindowInsetsCompat.Type.navigationBars())
      val statusbar = insets.getInsets(WindowInsetsCompat.Type.statusBars())
      view.setPadding(0, statusbar.top, 0, navbar.bottom)
      insets
    }
  }
}