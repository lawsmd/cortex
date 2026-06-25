package com.cortex.mobile

import android.annotation.SuppressLint
import android.os.Bundle
import android.util.Log
import android.view.MotionEvent
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.webkit.ConsoleMessage
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.WebViewAssetLoader

/**
 * The whole app: a single full-screen WebView hosting the Cortex mobile web client
 * (the same `client.html` the desktop bridge serves, bundled here as an asset). On
 * first launch — or when re-pairing — it routes to [PairingActivity] to capture the
 * desktop host/port/token (QR scan or manual), then loads the client over the
 * appassets **https** origin so it runs as a secure context.
 */
class MainActivity : AppCompatActivity() {

    private lateinit var webView: WebView
    private lateinit var assetLoader: WebViewAssetLoader
    private var clientLoaded = false

    // Latest window insets (status/nav bars + soft-keyboard), stringified for the web
    // layer to pull via CortexShell.env() and turn into CSS vars. Updated by the inset
    // listener / post. The `ime=` field is the keyboard height *above* the nav bar.
    @Volatile
    private var lastInsets = "n/a"

    private val pairingLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult(),
    ) {
        if (PairingStore(this).isPaired()) {
            loadClient()
        } else if (!clientLoaded) {
            finish() // backed out of first-run pairing with nothing stored — nothing to show
        }
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Edge-to-edge (forced on API 35 regardless): the window draws behind the
        // system bars. We re-inset the WebView ourselves below (setOnApplyWindowInsets
        // → setPadding) so the page content lands in the safe area.
        WindowCompat.setDecorFitsSystemWindows(window, false)
        setContentView(R.layout.activity_main)
        webView = findViewById(R.id.webview)

        assetLoader = WebViewAssetLoader.Builder()
            .addPathHandler("/assets/", WebViewAssetLoader.AssetsPathHandler(this))
            .build()

        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true // localStorage holds the connection profiles
            mediaPlaybackRequiresUserGesture = false
            // The page is a secure https (appassets) origin, but the bridge speaks plain
            // ws:// over the (already WireGuard-encrypted) tailnet — allow that mix.
            mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
            // The web client implements its own pinch-zoom (xterm fontSize); disable the
            // WebView's so the two don't fight.
            setSupportZoom(false)
            builtInZoomControls = false
            textZoom = 100
        }

        webView.webViewClient = object : WebViewClient() {
            override fun shouldInterceptRequest(
                view: WebView,
                request: WebResourceRequest,
            ): WebResourceResponse? = assetLoader.shouldInterceptRequest(request.url)

            // Pre-focus the WebView the moment the page is up so the user's FIRST tap on
            // the compose box reaches the field and raises the keyboard. A fresh WebView
            // holds no focus, so otherwise the first tap is spent focusing the container
            // and only the second tap opens the IME. Focusing the container never focuses
            // an <input>, so this does NOT auto-show the keyboard on launch.
            override fun onPageFinished(view: WebView, url: String) {
                view.requestFocus()
            }
        }
        webView.webChromeClient = object : WebChromeClient() {
            override fun onConsoleMessage(m: ConsoleMessage): Boolean {
                Log.d("CortexWeb", "${m.message()} (${m.sourceId()}:${m.lineNumber()})")
                return true
            }
        }
        webView.addJavascriptInterface(ShellBridge(), "CortexShell")

        // Some WebViews "eat" the first tap focusing the WebView itself, so a tap in an
        // HTML <input> (the compose box) only opens the keyboard on the SECOND tap.
        // Pre-focus the WebView on touch-down so the first tap reaches the field.
        // Returns false → the WebView still handles the gesture (xterm pan/pinch, button
        // taps) normally.
        webView.isFocusableInTouchMode = true
        @Suppress("ClickableViewAccessibility")
        webView.setOnTouchListener { view, event ->
            if (event.actionMasked == MotionEvent.ACTION_DOWN && !view.hasFocus()) {
                view.requestFocus()
            }
            false
        }

        // API 35 forces edge-to-edge: the WebView draws behind the status/nav bars,
        // and Android WebView — unlike iOS Safari — does NOT surface those insets to
        // the page's CSS env(safe-area-inset-*). Padding the WebView itself was a
        // no-op (the HTML viewport never reflowed; confirmed via the on-screen
        // diagnostic: nativeInsets t=161 but topbar padTop=0, viewport = full screen).
        // So we DON'T touch the WebView's layout here — we just capture the true
        // system-bar insets (getRootWindowInsets, immune to AppCompat consuming the
        // passed-in value) AND the soft-keyboard (IME) inset into `lastInsets`. The page
        // PULLS them via CortexShell.env() and sets its own CSS --cortex-sat/--cortex-sab/
        // --cortex-ime vars (the topbar/drawer/soft-keys pad from --sat/--sab; the bottom
        // bars lift by --ime so they clear the keyboard). Fires on attach / rotation /
        // nav-mode change / keyboard show-hide, and applyBarInsets nudges the page to
        // re-pull on each change. (Plain-browser / installed-PWA has no native shell, so
        // the page's env() path covers the safe area; it has no keyboard-occlusion issue
        // because the browser resizes the viewport itself.)
        ViewCompat.setOnApplyWindowInsetsListener(webView) { v, insets ->
            applyBarInsets(v, insets)
            insets
        }
        ViewCompat.requestApplyInsets(webView)
        // Belt-and-suspenders: re-capture after first layout, when getRootWindowInsets
        // is guaranteed non-null (the listener can fire before attach with nothing).
        webView.post { applyBarInsets(webView, null) }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                // The client pushes a history entry per open overlay (drawer / settings /
                // loading), so the system back button closes the topmost overlay; with
                // none open, exit the app.
                if (webView.canGoBack()) webView.goBack() else finish()
            }
        })

        if (PairingStore(this).isPaired()) loadClient()
        else pairingLauncher.launch(PairingActivity.intent(this))
    }

    private fun loadClient() {
        clientLoaded = true
        webView.loadUrl(PairingStore(this).buildClientUrl())
        // Hold focus on the WebView before the first tap (belt-and-suspenders with
        // onPageFinished) so tapping the compose box opens the keyboard on the first try.
        webView.requestFocus()
    }

    /**
     * Capture the current status/nav-bar **and** soft-keyboard (IME) insets into
     * [lastInsets] for the web layer to pull via `CortexShell.env()`. System-bar sizes
     * come from the ROOT insets (getRootWindowInsets), immune to intermediate
     * consumption. The IME size comes from the listener's live `passed` value — it
     * carries the keyboard height during the show/hide animation, which the root insets
     * can lag. We report the keyboard height *above the nav bar* (`ime - navBottom`):
     * the page already pads its bottom bar by the nav-bar inset, so lifting by the full
     * IME inset would double-count it. Edge-to-edge consumes the IME inset rather than
     * resizing the window (so `adjustResize` is inert and the page fires no JS resize),
     * so on any change we also nudge the page to re-pull and lift its compose bar /
     * soft-keys clear of the keyboard. Values are physical px; the density `d` rides
     * along so the page can convert to CSS px (px / d). Does NOT pad the WebView —
     * padding it never reflowed the HTML viewport.
     */
    private fun applyBarInsets(v: View, passed: WindowInsetsCompat?) {
        val root = ViewCompat.getRootWindowInsets(v)
        val src = root ?: passed ?: return
        val bars = src.getInsets(WindowInsetsCompat.Type.systemBars())
        val imeBottom =
            (passed ?: root)?.getInsets(WindowInsetsCompat.Type.ime())?.bottom ?: 0
        val imeAboveNav = (imeBottom - bars.bottom).coerceAtLeast(0)
        val next =
            "t=${bars.top} b=${bars.bottom} l=${bars.left} r=${bars.right} ime=$imeAboveNav d=${resources.displayMetrics.density}"
        if (next == lastInsets) return
        lastInsets = next
        Log.d("CortexInsets", "insets $lastInsets")
        // No JS resize fires when the keyboard animates (the window doesn't shrink), so
        // nudge the page to re-pull the insets and lift its bottom bars above the IME.
        v.post {
            webView.evaluateJavascript(
                "window.applyNativeInsets && window.applyNativeInsets()", null,
            )
        }
    }

    override fun onDestroy() {
        webView.destroy()
        super.onDestroy()
    }

    /** Exposed to the web client as `window.CortexShell`. */
    inner class ShellBridge {
        /** Let the in-page settings offer "re-pair" — relaunch the native pairing flow. */
        @JavascriptInterface
        fun rescan() {
            runOnUiThread { pairingLauncher.launch(PairingActivity.intent(this@MainActivity)) }
        }

        /** Diagnostic: the latest system-bar insets, for the on-screen debug box. */
        @JavascriptInterface
        fun env(): String = lastInsets

        /** Web client asks for the soft keyboard when the compose box gains focus. */
        @JavascriptInterface
        fun showKeyboard() {
            runOnUiThread {
                // The JS focus event can fire a hair before the WebView's input
                // connection to the freshly-focused <textarea> is live — the cold-start
                // race that makes the keyboard skip the FIRST tap. Make sure the WebView
                // holds focus, then post + one short delayed retry so showSoftInput lands
                // once the connection exists. SHOW_IMPLICIT is a no-op when a hardware
                // keyboard is attached, so it never pops the on-screen keyboard wrongly.
                val imm = getSystemService(InputMethodManager::class.java)
                    ?: return@runOnUiThread
                if (!webView.hasFocus()) webView.requestFocus()
                val show = Runnable { imm.showSoftInput(webView, InputMethodManager.SHOW_IMPLICIT) }
                webView.post(show)
                webView.postDelayed(show, 60)
            }
        }
    }
}
