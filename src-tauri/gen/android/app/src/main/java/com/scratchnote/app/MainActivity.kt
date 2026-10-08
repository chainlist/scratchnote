package com.scratchnote.app

import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // The interface is laid out for a window, so it keeps clear of the
    // status and navigation bars, the camera cutout and the keyboard.
    ViewCompat.setOnApplyWindowInsetsListener(findViewById(android.R.id.content)) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or
          WindowInsetsCompat.Type.displayCutout() or
          WindowInsetsCompat.Type.ime()
      )
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
    }
  }

  // The WebView scales only the text by the system font size, leaving icons
  // and spacing behind. The page takes the scale into its root font size
  // instead (#lib/appearance.ts), so everything sized in rem grows with it.
  // A font size change recreates the activity, so the scale read here holds.
  override fun onWebViewCreate(webView: WebView) {
    val scale = resources.configuration.fontScale
    webView.settings.textZoom = 100
    webView.addJavascriptInterface(object {
      @JavascriptInterface fun fontScale(): Float = scale
    }, "AndroidText")
  }
}
