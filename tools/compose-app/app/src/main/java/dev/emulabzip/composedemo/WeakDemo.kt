package dev.emulabzip.composedemo

import android.content.BroadcastReceiver
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.database.sqlite.SQLiteDatabase
import android.util.Base64
import android.util.Log
import java.net.HttpURLConnection
import java.net.URL

/**
 * What an app should not do, on purpose, so EmuLabZip's audits have something to find. `--es weak 1` runs it: a bearer token and an
 * email address in the log, a session token in preferences, a password column in a plain database, a token in a URL, and analytics
 * calls to Segment and Mixpanel over plain HTTP. The manifest adds an exported receiver, and the screen a label that does not fit.
 * Never copy any of this into an app you ship.
 */
object WeakDemo {
    fun run(context: Context) {
        Log.d("Auth", "Authorization: Bearer abcdef0123456789abcdef0123456789")
        Log.i("User", "signed in as ana@example.com")

        context.getSharedPreferences("auth", Context.MODE_PRIVATE).edit()
            .putString("session_token", "s3cr3t-session-token-9f8e7d")
            .putString("email", "ana@example.com")
            .putString("theme", "dark")
            .commit()

        val db = SQLiteDatabase.openOrCreateDatabase(context.getDatabasePath("accounts.db"), null)
        db.execSQL("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, email TEXT, password_hash TEXT, api_token TEXT)")
        db.insert("users", null, ContentValues().apply { put("email", "ana@example.com"); put("password_hash", "5f4dcc3b5aa765d61d8327deb882cf99"); put("api_token", "tok_live_1234567890abcdef") })
        db.close()

        Thread {
            // A token in the address, and the events of two analytics services, all in the clear.
            post("http://example.com/?access_token=abcdef1234567890", "text/plain", "")
            post(
                "http://api.segment.io/v1/track",
                "application/json",
                """{"type":"track","event":"Order Completed","userId":"ana-1","properties":{"total":42.5,"email":"ana@example.com"}}""",
            )
            val mixpanel = Base64.encodeToString("""[{"event":"Signed Up","properties":{"distinct_id":"ana-1","plan":"free"}}]""".toByteArray(), Base64.NO_WRAP)
            post("http://api.mixpanel.com/track", "application/x-www-form-urlencoded", "data=" + java.net.URLEncoder.encode(mixpanel, "UTF-8"))
        }.start()
    }

    private fun post(address: String, type: String, body: String) {
        try {
            val c = URL(address).openConnection() as HttpURLConnection
            c.connectTimeout = 4000
            c.readTimeout = 4000
            c.requestMethod = "POST"
            c.doOutput = true
            c.setRequestProperty("Content-Type", type)
            c.outputStream.use { it.write(body.toByteArray()) }
            c.responseCode
        } catch (_: Exception) {
        }
    }
}

/** Reachable by any app that knows its action, and it checks nothing: an exposed component for the audit to find. */
class PingReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        Log.d("Ping", "pinged by ${intent.getStringExtra("from")}")
    }
}
