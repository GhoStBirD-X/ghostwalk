package id.ghostwalk.app;

import android.app.Activity;
import android.graphics.Color;
import android.os.Build;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;

/**
 * Lets GhostWalk draw behind the status and navigation bars, and tells it how much room those bars
 * (and the keyboard) take, so nothing important ends up underneath them.
 */
public class Screen {
    /** Left, top, right, bottom bars, keyboard; in pixels. */
    private static volatile int[] insets = {0, 0, 0, 0, 0};

    /** Draws under the system bars, with bar icons that suit a dark or light page. */
    public static void edgeToEdge(final Activity activity, final boolean dark) {
        Gps.MAIN.post(new Runnable() {
            public void run() {
                try {
                    Window w = activity.getWindow();
                    w.setStatusBarColor(Color.TRANSPARENT);
                    w.setNavigationBarColor(Color.TRANSPARENT);
                    if (Build.VERSION.SDK_INT >= 29) {
                        w.setNavigationBarContrastEnforced(false);
                        w.setStatusBarContrastEnforced(false);
                    }
                    if (Build.VERSION.SDK_INT >= 30) {
                        w.setDecorFitsSystemWindows(false);
                        WindowInsetsController c = w.getInsetsController();
                        if (c != null) {
                            int light = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
                                    | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
                            c.setSystemBarsAppearance(dark ? 0 : light, light);
                        }
                    } else {
                        int flags = View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                                | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION;
                        if (!dark) {
                            flags |= View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
                        }
                        w.getDecorView().setSystemUiVisibility(flags);
                    }
                    refresh(activity);
                } catch (Exception e) {
                    // Older or unusual phones keep the normal layout.
                }
            }
        });
    }

    /** The latest known room taken by bars and keyboard; asks for a fresh value in the background. */
    public static int[] insets(final Activity activity) {
        Gps.MAIN.post(new Runnable() {
            public void run() {
                refresh(activity);
            }
        });
        return insets;
    }

    private static void refresh(Activity activity) {
        try {
            WindowInsets i = activity.getWindow().getDecorView().getRootWindowInsets();
            if (i == null) {
                android.util.Log.i("GhostWalk", "insets: none yet");
                return;
            }
            int[] fresh;
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets bars = i.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                android.graphics.Insets ime = i.getInsets(WindowInsets.Type.ime());
                fresh = new int[] {bars.left, bars.top, bars.right, bars.bottom, ime.bottom};
            } else {
                int stable = i.getStableInsetBottom();
                int keyboard = Math.max(0, i.getSystemWindowInsetBottom() - stable);
                fresh = new int[] {i.getStableInsetLeft(), i.getStableInsetTop(), i.getStableInsetRight(), stable, keyboard};
            }
            if (!java.util.Arrays.equals(fresh, insets)) {
                android.util.Log.i("GhostWalk", "insets: " + java.util.Arrays.toString(fresh));
            }
            insets = fresh;
        } catch (Exception e) {
            android.util.Log.w("GhostWalk", "insets failed", e);
        }
    }
}
