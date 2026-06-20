package com.cortex.mobile

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import org.json.JSONObject

/**
 * First-run / re-pair screen: capture the desktop bridge host / port / token either
 * by scanning a QR (recommended) or by typing them. On success it persists to
 * [PairingStore] and returns RESULT_OK; [MainActivity] then loads the client.
 *
 * Accepted QR / scan payloads:
 *   - `cortex://pair?host=H&port=P&token=T[&label=L][&secure=1]`
 *   - `http(s)://H:P/?token=T`  (the URL the bridge already serves)
 *   - `{"host":"H","port":"P","token":"T"[,"label":"L"][,"secure":true]}`
 */
class PairingActivity : AppCompatActivity() {

    private lateinit var hostField: EditText
    private lateinit var portField: EditText
    private lateinit var tokenField: EditText
    private lateinit var labelField: EditText

    private val scanLauncher = registerForActivityResult(ScanContract()) { result ->
        val contents = result.contents ?: return@registerForActivityResult
        if (!applyScanned(contents)) {
            Toast.makeText(this, R.string.pair_scan_unrecognized, Toast.LENGTH_LONG).show()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        setContentView(R.layout.activity_pairing)

        hostField = findViewById(R.id.host)
        portField = findViewById(R.id.port)
        tokenField = findViewById(R.id.token)
        labelField = findViewById(R.id.label)

        val store = PairingStore(this)
        if (store.isPaired()) {
            hostField.setText(store.host)
            portField.setText(store.port)
            tokenField.setText(store.token)
            labelField.setText(store.label)
        } else {
            portField.setText("9280")
            labelField.setText(getString(R.string.pair_default_label))
        }

        findViewById<Button>(R.id.scan_btn).setOnClickListener {
            scanLauncher.launch(
                ScanOptions()
                    .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                    .setPrompt(getString(R.string.pair_scan_prompt))
                    .setBeepEnabled(false)
                    .setOrientationLocked(false),
            )
        }
        findViewById<Button>(R.id.save_btn).setOnClickListener { saveManual() }
    }

    /** Parse a scanned payload; on success persist, set RESULT_OK, finish. Returns false if unparseable. */
    private fun applyScanned(raw: String): Boolean {
        val text = raw.trim()
        var host = ""
        var port = ""
        var token = ""
        var label = ""
        var secure = false
        try {
            when {
                text.startsWith("cortex://") -> {
                    val u = Uri.parse(text)
                    host = u.getQueryParameter("host").orEmpty()
                    port = u.getQueryParameter("port").orEmpty()
                    token = u.getQueryParameter("token").orEmpty()
                    label = u.getQueryParameter("label").orEmpty()
                    secure = u.getQueryParameter("secure") == "1"
                }
                text.startsWith("http://") || text.startsWith("https://") -> {
                    val u = Uri.parse(text)
                    host = u.host.orEmpty()
                    port = when {
                        u.port != -1 -> u.port.toString()
                        text.startsWith("https://") -> "443"
                        else -> "80"
                    }
                    token = u.getQueryParameter("token").orEmpty()
                    secure = text.startsWith("https://")
                }
                text.startsWith("{") -> {
                    val j = JSONObject(text)
                    host = j.optString("host")
                    port = j.optString("port")
                    token = j.optString("token")
                    label = j.optString("label")
                    secure = j.optBoolean("secure", false)
                }
                else -> return false
            }
        } catch (_: Exception) {
            return false
        }
        if (host.isEmpty() || port.isEmpty()) return false
        if (label.isEmpty()) label = getString(R.string.pair_default_label)
        PairingStore(this).save(host, port, token, label, secure)
        setResult(RESULT_OK)
        finish()
        return true
    }

    private fun saveManual() {
        val host = hostField.text.toString().trim()
        val port = portField.text.toString().trim()
        if (host.isEmpty() || port.isEmpty()) {
            Toast.makeText(this, R.string.pair_need_host_port, Toast.LENGTH_SHORT).show()
            return
        }
        val label = labelField.text.toString().trim().ifEmpty { getString(R.string.pair_default_label) }
        PairingStore(this).save(host, port, tokenField.text.toString().trim(), label, false)
        setResult(RESULT_OK)
        finish()
    }

    companion object {
        fun intent(context: Context) = Intent(context, PairingActivity::class.java)
    }
}
