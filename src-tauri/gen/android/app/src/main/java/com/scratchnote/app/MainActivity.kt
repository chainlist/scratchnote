package com.scratchnote.app

import android.graphics.Rect
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
    claimLeftEdge(webView)
  }

  // On a phone a swipe in from the left edge draws out the ribbon
  // (RibbonDrawer.svelte), but with gesture navigation that edge is the
  // system's Back. Android lets an app take back at most 200dp of each edge:
  // the middle of the left one goes to the page. A screen wide enough for the
  // ribbon (40rem, 640dp at the default text size) keeps all of its Back.
  private fun claimLeftEdge(webView: WebView) {
    webView.addOnLayoutChangeListener { view, _, top, _, bottom, _, _, _, _ ->
      val density = resources.displayMetrics.density
      val phone = view.width / density < 640
      val band = (200 * density).toInt()
      val middle = (bottom - top) / 2
      ViewCompat.setSystemGestureExclusionRects(
        view,
        if (phone) listOf(Rect(0, middle - band / 2, (32 * density).toInt(), middle + band / 2))
        else emptyList()
      )
    }
  }
}
