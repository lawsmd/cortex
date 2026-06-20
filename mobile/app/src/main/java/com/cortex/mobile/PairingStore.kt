package com.cortex.mobile

import android.content.Context
import android.net.Uri

/**
 * Persists which desktop Cortex bridge to connect to (host / port / token), set
 * during [PairingActivity]. Stored in SharedPreferences — the token never leaves
 * the device and is never in the APK.
 */
class PairingStore(context: Context) {

    private val sp = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    val host: String get() = sp.getString("host", "").orEmpty()
    val port: String get() = sp.getString("port", "").orEmpty()
    val token: String get() = sp.getString("token", "").orEmpty()
    val label: String get() = sp.getString("label", "").orEmpty()
    val secure: Boolean get() = sp.getBoolean("secure", false)

    fun isPaired(): Boolean = host.isNotEmpty() && port.isNotEmpty()

    fun save(host: String, port: String, token: String, label: String, secure: Boolean) {
        sp.edit()
            .putString("host", host.trim())
            .putString("port", port.trim())
            .putString("token", token.trim())
            .putString("label", label.trim())
            .putBoolean("secure", secure)
            .apply()
    }

    fun clear() = sp.edit().clear().apply()

    /**
     * The appassets URL the WebView loads. Serving the bundled client over the
     * virtual **https** origin makes it a secure context — so `navigator.wakeLock`
     * and `navigator.clipboard` work natively (they no-op over plain-http Tailscale).
     * host/port/token ride as query params because `window.location` is the asset
     * host under appassets, not the desktop; the client's `seedFromPairingQuery()`
     * reads them.
     */
    fun buildClientUrl(): String {
        val b = Uri.parse(CLIENT_URL).buildUpon()
        b.appendQueryParameter("host", host)
        b.appendQueryParameter("port", port)
        if (token.isNotEmpty()) b.appendQueryParameter("token", token)
        if (label.isNotEmpty()) b.appendQueryParameter("label", label)
        if (secure) b.appendQueryParameter("secure", "1")
        return b.build().toString()
    }

    companion object {
        private const val PREFS = "cortex_pairing"
        private const val CLIENT_URL =
            "https://appassets.androidplatform.net/assets/client.html"
    }
}
