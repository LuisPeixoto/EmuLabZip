package dev.emulabzip.crashy;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.system.Os;
import android.system.OsConstants;

/** Does one bad thing on purpose, chosen by the `mode` intent extra: crash, native or anr. */
public class MainActivity extends Activity {
    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        run(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        run(intent);
    }

    private void run(Intent intent) {
        String mode = intent.getStringExtra("mode");
        if (mode == null) return;
        switch (mode) {
            case "crash":
                crash();
                break;
            case "native":
                // A fatal signal leaves a tombstone, like a crash in native code does.
                try {
                    Os.kill(Os.getpid(), OsConstants.SIGSEGV);
                } catch (Exception e) {
                    throw new RuntimeException(e);
                }
                break;
            case "anr":
                // Blocks the main thread once the window is up. Android reports an ANR when an input
                // event arrives and gets no answer for five seconds, so tap the screen after
                // starting this.
                new Handler(Looper.getMainLooper()).postDelayed(() -> {
                    try {
                        Thread.sleep(60_000);
                    } catch (InterruptedException ignored) {
                    }
                }, 2_000);
                break;
            default:
                break;
        }
    }

    private void crash() {
        try {
            Integer.parseInt("not a number");
        } catch (NumberFormatException cause) {
            throw new IllegalStateException("EmuLabZip test crash", cause);
        }
    }
}
