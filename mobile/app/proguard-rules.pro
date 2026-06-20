# The web client calls into ShellBridge via @JavascriptInterface; R8 must not
# rename or strip it (release builds aren't minified today, but keep this so a
# future minified build doesn't silently break re-pairing).
-keepclassmembers class com.cortex.mobile.MainActivity$ShellBridge {
    @android.webkit.JavascriptInterface <methods>;
}
