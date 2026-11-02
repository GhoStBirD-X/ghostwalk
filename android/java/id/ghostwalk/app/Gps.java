package id.ghostwalk.app;

import android.Manifest;
import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.location.Location;
import android.location.LocationListener;
import android.location.LocationManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;
import android.util.Log;
import android.view.WindowManager;
import java.util.ArrayList;

/** Phone GPS for GhostWalk. Positions stay in memory until the app reads them; nothing leaves the phone. */
public class Gps implements LocationListener {
    private static final String TAG = "GhostWalk";
    /** Our own handle on the main thread: Activity.runOnUiThread has no handler when called from native code. */
    static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static Gps listener;
    private static LocationManager manager;
    private static volatile boolean running;
    /** When the last position arrived (ms), so "GPS aktif" only shows with a real signal. */
    private static volatile long lastFix;
    private static final ArrayList<double[]> fixes = new ArrayList<>();

    /**
     * Asks for everything a recording uses that is not allowed yet: location (Android 12+ ignores
     * precise without approximate), the "recording" notification (13+) and the step counter (10+).
     */
    public static void ask(final Activity activity) {
        final ArrayList<String> wanted = new ArrayList<>();
        String[] all = {
            Manifest.permission.ACCESS_FINE_LOCATION,
            Manifest.permission.ACCESS_COARSE_LOCATION,
            Build.VERSION.SDK_INT >= 33 ? "android.permission.POST_NOTIFICATIONS" : null,
            Build.VERSION.SDK_INT >= 29 ? "android.permission.ACTIVITY_RECOGNITION" : null,
        };
        for (String p : all) {
            if (p != null && activity.checkSelfPermission(p) != PackageManager.PERMISSION_GRANTED) {
                wanted.add(p);
            }
        }
        if (wanted.isEmpty()) {
            return;
        }
        MAIN.post(new Runnable() {
            public void run() {
                try {
                    activity.requestPermissions(wanted.toArray(new String[0]), 7);
                } catch (Exception e) {
                    // Leave the recording running without GPS.
                }
            }
        });
    }

    /** Asks for permission if needed, then starts receiving positions about once a second, also in the background. */
    public static void start(final Activity activity) {
        if (activity.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) != PackageManager.PERMISSION_GRANTED) {
            ask(activity);
            return;
        }
        if (running) {
            return;
        }
        // Keeps the recording alive with the screen off or another app open.
        Tracker.start(activity);
        // Location is allowed; also ask for the notification and step counter if they are still missing.
        ask(activity);
        Log.i(TAG, "gps permission ok, asking for updates");
        running = true;
        manager = (LocationManager) activity.getSystemService(Context.LOCATION_SERVICE);
        if (listener == null) {
            listener = new Gps();
        }
        MAIN.post(new Runnable() {
            public void run() {
                activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                try {
                    manager.requestLocationUpdates(LocationManager.GPS_PROVIDER, 1000L, 0f, listener, Looper.getMainLooper());
                    Log.i(TAG, "gps updates requested");
                } catch (Exception e) {
                    Log.w(TAG, "gps request failed", e);
                    // No permission, or no GPS on this device: keep recording time only.
                    running = false;
                }
            }
        });
    }

    public static void stop(final Activity activity) {
        running = false;
        Tracker.stop(activity);
        Coach.end();
        MAIN.post(new Runnable() {
            public void run() {
                try {
                    activity.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                    if (manager != null && listener != null) {
                        manager.removeUpdates(listener);
                    }
                } catch (Exception e) {
                    // Already stopped.
                }
            }
        });
        synchronized (fixes) {
            fixes.clear();
        }
    }

    /** 0 = no permission yet, 1 = waiting for signal, 2 = receiving, 3 = Location is switched off. */
    public static int state(Activity activity) {
        if (activity.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) != PackageManager.PERMISSION_GRANTED) {
            return 0;
        }
        if (!locationOn(activity)) {
            return 3;
        }
        return running && System.currentTimeMillis() - lastFix < 10000 ? 2 : 1;
    }

    private static boolean locationOn(Activity activity) {
        try {
            LocationManager lm = (LocationManager) activity.getSystemService(Context.LOCATION_SERVICE);
            if (Build.VERSION.SDK_INT >= 28) {
                return lm.isLocationEnabled();
            }
            return lm.isProviderEnabled(LocationManager.GPS_PROVIDER);
        } catch (Exception e) {
            return true;
        }
    }

    /** This app's page in the phone settings, for when the permission popup was turned down before. */
    public static void openAppSettings(Activity activity) {
        open(activity, new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", activity.getPackageName(), null)));
    }

    /** The phone's Location switch. */
    public static void openLocationSettings(Activity activity) {
        open(activity, new Intent(Settings.ACTION_LOCATION_SOURCE_SETTINGS));
    }

    private static void open(final Activity activity, final Intent intent) {
        MAIN.post(new Runnable() {
            public void run() {
                try {
                    activity.startActivity(intent);
                } catch (Exception e) {
                    // No settings screen for this on this phone.
                }
            }
        });
    }

    /** All new positions as [lat, lon, altitude, accuracy m, time ms] groups, then forgets them. */
    public static double[] drain() {
        synchronized (fixes) {
            double[] out = new double[fixes.size() * 5];
            for (int i = 0; i < fixes.size(); i++) {
                System.arraycopy(fixes.get(i), 0, out, i * 5, 5);
            }
            fixes.clear();
            return out;
        }
    }

    /** A folder the person can reach with a file manager, for exports. */
    public static String exportDir(Activity activity) {
        java.io.File f = activity.getExternalFilesDir(null);
        return f == null ? "" : f.getAbsolutePath();
    }

    @Override
    public void onLocationChanged(Location l) {
        if (lastFix == 0) {
            Log.i(TAG, "gps first fix, accuracy " + l.getAccuracy() + " m");
        }
        lastFix = System.currentTimeMillis();
        Coach.onFix(l);
        synchronized (fixes) {
            fixes.add(new double[] {l.getLatitude(), l.getLongitude(), l.getAltitude(), l.getAccuracy(), (double) l.getTime()});
        }
    }

    @Override
    public void onStatusChanged(String provider, int status, Bundle extras) {}

    @Override
    public void onProviderEnabled(String provider) {}

    @Override
    public void onProviderDisabled(String provider) {}
}
