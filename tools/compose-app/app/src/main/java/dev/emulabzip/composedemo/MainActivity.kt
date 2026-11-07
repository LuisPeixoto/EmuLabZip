package dev.emulabzip.composedemo

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** A small Jetpack Compose screen to try EmuLabZip's framework detection and Layout on. */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // `--es crash 1` fails on purpose, a few calls deep, so a release build shows what R8 does to a stack.
        if (intent.getStringExtra("crash") != null) Checkout(Cart(listOf(3, 0))).total()
        // `--es data 1` writes what the Data demo keeps; `--es data 2` migrates the database to its second version.
        intent.getStringExtra("data")?.toIntOrNull()?.let { DemoData.write(applicationContext, it) }
        // `--es weak 1` does what an app should not, so the audits have something to find (see WeakDemo.kt).
        val weak = intent.getStringExtra("weak") != null
        if (weak) WeakDemo.run(applicationContext)
        setContent { Demo(weak) }
    }
}

/** Nested calls, so a crash has a few frames of the app's own to read. */
class Cart(private val prices: List<Int>) {
    fun average(): Int = prices.sum() / prices.count { it > 100 }
}

class Checkout(private val cart: Cart) {
    fun total(): Int = discounted(cart.average())
    private fun discounted(value: Int): Int = value - 1
}

/** One plain HTTP request, for the network, replay and timeline demos. The answer is what the server said, or why not. */
fun fetch(address: String): String =
    try {
        val connection = URL(address).openConnection() as HttpURLConnection
        connection.connectTimeout = 8000
        connection.readTimeout = 8000
        val code = connection.responseCode
        if (code == 200) connection.inputStream.bufferedReader().readText().trim().take(120) else "HTTP $code"
    } catch (e: Exception) {
        "Failed: ${e.message}"
    }

@Composable
fun Demo(weak: Boolean = false) {
    var count by remember { mutableIntStateOf(0) }
    var enabled by remember { mutableStateOf(true) }
    var said by remember { mutableStateOf("nothing yet") }
    val scope = rememberCoroutineScope()
    // Test tags show up as resource ids, which is how a screen reader tool tells composables apart.
    Column(Modifier.fillMaxSize().padding(16.dp).semantics { testTagsAsResourceId = true }) {
        Text("Compose demo", Modifier.testTag("title"))
        Button(onClick = { count++ }, Modifier.testTag("counter")) { Text("Tapped $count times") }
        // The server answers with the time in seconds, so a recording and a replay of it are easy to tell apart.
        Row {
            Button(onClick = { scope.launch { said = withContext(Dispatchers.IO) { fetch("http://time.akamai.com/") } } }, Modifier.testTag("fetch")) { Text("Fetch the time") }
            Button(onClick = { scope.launch { said = withContext(Dispatchers.IO) { fetch("http://example.com/").take(40) } } }, Modifier.padding(start = 8.dp).testTag("fetch-other")) { Text("Fetch another page") }
        }
        Text("Server said: $said", Modifier.testTag("said"))
        if (weak) {
            // A label that cannot fit: one line, as wide as the screen, with far more text than that.
            Text(
                "An order confirmation label that is much longer than this row can ever show on one line",
                Modifier.fillMaxWidth().testTag("long-label"),
                maxLines = 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
            )
        }
        val context = androidx.compose.ui.platform.LocalContext.current
        Button(onClick = { scope.launch(Dispatchers.IO) { DemoData.write(context.applicationContext) } }, Modifier.testTag("write-data")) { Text("Write demo data") }
        Row {
            Text("Notifications", Modifier.padding(end = 12.dp))
            Switch(checked = enabled, onCheckedChange = { enabled = it }, modifier = Modifier.testTag("switch"))
        }
        LazyColumn(Modifier.testTag("list")) {
            items(listOf("Apples", "Bananas", "Cherries", "Dates", "Elderberries")) { fruit ->
                Text(fruit, Modifier.padding(vertical = 8.dp).testTag("row-$fruit"))
            }
        }
    }
}
