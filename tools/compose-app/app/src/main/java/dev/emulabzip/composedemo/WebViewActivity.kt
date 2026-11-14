package dev.emulabzip.composedemo

import android.annotation.SuppressLint
import android.app.Activity
import android.os.Bundle
import android.webkit.WebView

/**
 * A WebView that can be inspected: web debugging is switched on, so EmuLabZip lists its page under Frameworks → Web
 * pages, with the console, the requests and a place to run JavaScript.
 *
 * It is also what the deep links of this app open (see the manifest): `https://emulabzip.luispeixoto.dev/demo` as an
 * app link, and `emulabzip-demo://open/...`. The page says which link it was opened with.
 */
class WebViewActivity : Activity() {
    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Without this a WebView cannot be inspected from outside. Leave it to debug builds in a real app.
        WebView.setWebContentsDebuggingEnabled(true)
        val web = WebView(this)
        web.settings.javaScriptEnabled = true
        setContentView(web)
        val link = intent?.data?.toString() ?: "the Open the web page button"
        val html = assets.open("web/index.html").bufferedReader().use { it.readText() }
            .replace("__LINK__", link.replace("&", "&amp;").replace("<", "&lt;"))
        // The page takes the site's origin, so it can fetch from it.
        web.loadDataWithBaseURL("https://emulabzip.luispeixoto.dev/demo/", html, "text/html", "utf-8", null)
    }
}
