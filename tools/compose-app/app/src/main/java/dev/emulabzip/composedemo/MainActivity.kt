package dev.emulabzip.composedemo

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp

/** A small Jetpack Compose screen to try EmuLabZip's framework detection and Layout on. */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { Demo() }
    }
}

@Composable
fun Demo() {
    var count by remember { mutableIntStateOf(0) }
    var enabled by remember { mutableStateOf(true) }
    // Test tags show up as resource ids, which is how a screen reader tool tells composables apart.
    Column(Modifier.fillMaxSize().padding(16.dp).semantics { testTagsAsResourceId = true }) {
        Text("Compose demo", Modifier.testTag("title"))
        Button(onClick = { count++ }, Modifier.testTag("counter")) { Text("Tapped $count times") }
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
