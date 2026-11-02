package id.ghostwalk.app;

import android.app.NativeActivity;
import android.content.Intent;

/** The app's screen: Android's NativeActivity, plus receiving files chosen in the system file picker. */
public class GhostActivity extends NativeActivity {
    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        Files.onResult(this, requestCode, resultCode, data);
    }
}
