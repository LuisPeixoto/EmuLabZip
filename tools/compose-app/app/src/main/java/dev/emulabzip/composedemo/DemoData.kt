package dev.emulabzip.composedemo

import android.content.ContentValues
import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteOpenHelper
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.doublePreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.core.stringSetPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.tencent.mmkv.MMKV
import kotlinx.coroutines.runBlocking

private val Context.demoStore by preferencesDataStore(name = "demo")

/**
 * What an app keeps, for EmuLabZip's Storage to read, change, save and compare: a SQLite database (with a migration from
 * version 1 to 2, to try the schema diff), SharedPreferences, a Preferences DataStore and MMKV.
 */
object DemoData {
    /** The shop database. Version 2 adds a column, an index and a table. */
    private class Shop(context: Context, version: Int) : SQLiteOpenHelper(context, "shop.db", null, version) {
        override fun onCreate(db: SQLiteDatabase) {
            db.execSQL("CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT, vip INTEGER DEFAULT 0)")
            db.execSQL("CREATE TABLE cart (id INTEGER PRIMARY KEY, sku TEXT NOT NULL, qty INTEGER NOT NULL, price REAL, customer_id INTEGER REFERENCES customers(id))")
            val customers = listOf(Triple("Ana", "ana@example.com", 1), Triple("Bo", "bo@example.com", 0), Triple("Cy", null, 0))
            for ((name, email, vip) in customers) {
                db.insert("customers", null, ContentValues().apply { put("name", name); put("email", email); put("vip", vip) })
            }
            val items = listOf(Triple("APPLE", 3, 0.5), Triple("PEAR", 1, 0.75), Triple("MELON", 2, 3.2))
            for ((i, item) in items.withIndex()) {
                db.insert("cart", null, ContentValues().apply { put("sku", item.first); put("qty", item.second); put("price", item.third); put("customer_id", i % 2 + 1) })
            }
        }

        override fun onUpgrade(db: SQLiteDatabase, oldVersion: Int, newVersion: Int) {
            if (oldVersion < 2) {
                db.execSQL("ALTER TABLE cart ADD COLUMN note TEXT")
                db.execSQL("CREATE INDEX idx_cart_sku ON cart (sku)")
                db.execSQL("CREATE TABLE orders (id INTEGER PRIMARY KEY, cart_id INTEGER NOT NULL, placed_at INTEGER NOT NULL)")
            }
        }
    }

    /** Writes the demo data. `version` is the database's: 1 creates it, 2 migrates it (when it is at 1 already). */
    fun write(context: Context, version: Int = 1) {
        Shop(context, version).writableDatabase.close()

        context.getSharedPreferences("settings", Context.MODE_PRIVATE).edit()
            .putString("theme", "dark")
            .putInt("launches", 3)
            .putLong("last_sync", 1_792_773_580_000L)
            .putFloat("ratio", 0.5f)
            .putBoolean("onboarded", true)
            .putStringSet("tags", linkedSetOf("beta", "vip"))
            .commit()

        runBlocking {
            context.demoStore.edit {
                it[stringPreferencesKey("language")] = "pt-BR"
                it[intPreferencesKey("font_scale")] = 110
                it[longPreferencesKey("installed_at")] = 1_792_700_000_000L
                it[doublePreferencesKey("volume")] = 0.8
                it[booleanPreferencesKey("notifications")] = true
                it[stringSetPreferencesKey("favourites")] = setOf("apples", "pears")
            }
        }

        MMKV.initialize(context)
        MMKV.defaultMMKV().apply {
            encode("session_token", "tok_live_123")
            encode("visits", 7)
            encode("beta", true)
            encode("ratio", 1.25)
            encode("big", 5_000_000_000L)
            encode("tags", setOf("x", "y"))
            encode("blob", byteArrayOf(1, 2, 3, 4))
        }
    }
}
