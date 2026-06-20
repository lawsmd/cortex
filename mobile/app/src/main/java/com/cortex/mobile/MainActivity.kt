package com.cortex.mobile

import android.annotation.SuppressLint
import android.os.Bundle
import android.util.Log
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
import androidx.core.view.WindowCompat
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
        // Draw edge-to-edge so the client's CSS safe-area insets (viewport-fit=cover)
        // apply, matching the installed-PWA look.
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
        }
        webView.webChromeClient = object : WebChromeClient() {
            override fun onConsoleMessage(m: ConsoleMessage): Boolean {
                Log.d("CortexWeb", "${m.message()} (${m.sourceId()}:${m.lineNumber()})")
                return true
            }
        }
        webView.addJavascriptInterface(ShellBridge(), "CortexShell")

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
    }
}
