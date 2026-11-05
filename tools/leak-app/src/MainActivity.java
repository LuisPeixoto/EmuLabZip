package dev.emulabzip.leaky;

import android.app.Activity;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.widget.TextView;

/**
 * Leaks itself on purpose, to try the leak finder. Start it with `--es mode leak`: it stores itself in a static field
 * and finishes a moment later, so a destroyed activity stays reachable. Without a mode it just shows a screen, and
 * leaks nothing.
 */
public class MainActivity extends Activity {
    /** The leak: a static field outlives every activity. */
    static Activity kept;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        TextView text = new TextView(this);
        text.setText("EmuLabZip Leaky");
        text.setTextSize(24);
        setContentView(text);
        if ("leak".equals(getIntent().getStringExtra("mode"))) {
            kept = this;
            new Handler(Looper.getMainLooper()).postDelayed(this::finish, 800);
        }
    }
}
