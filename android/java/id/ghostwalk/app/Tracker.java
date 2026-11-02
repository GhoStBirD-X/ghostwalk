package id.ghostwalk.app;

import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;
import android.util.Log;

/**
 * Keeps a recording going while the screen is off or another app is open, with the
 * "GhostWalk sedang merekam" notification Android requires for that.
 */
public class Tracker extends Service {
    private static final String TAG = "GhostWalk";
    private static final String CHANNEL = "rekam";
    private static final int ID = 1;
    private static Context app;
    private static volatile String text = "Menyiapkan GPS…";

    public static void start(Activity activity) {
        app = activity.getApplicationContext();
        try {
            Intent i = new Intent(activity, Tracker.class);
            if (Build.VERSION.SDK_INT >= 26) {
                activity.startForegroundService(i);
            } else {
                activity.startService(i);
            }
        } catch (Exception e) {
            Log.w(TAG, "could not start background recording", e);
        }
    }

    public static void stop(Activity activity) {
        try {
            activity.stopService(new Intent(activity, Tracker.class));
        } catch (Exception e) {
            // Not running.
        }
    }

    /** Changes the line under the notification title, e.g. "2,40 km · 14:05". */
    static void update(String line) {
        text = line;
        Context c = app;
        if (c == null) {
            return;
        }
        try {
            NotificationManager nm = (NotificationManager) c.getSystemService(Context.NOTIFICATION_SERVICE);
            nm.notify(ID, build(c));
        } catch (Exception e) {
            // Notifications turned off.
        }
    }

    private static Notification build(Context c) {
        Notification.Builder b = Build.VERSION.SDK_INT >= 26 ? new Notification.Builder(c, CHANNEL) : new Notification.Builder(c);
        Intent open = new Intent(c, GhostActivity.class).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent pi = PendingIntent.getActivity(c, 0, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return b.setContentTitle("GhostWalk sedang merekam")
                .setContentText(text)
                .setSmallIcon(c.getApplicationInfo().icon)
                .setContentIntent(pi)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .build();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        app = getApplicationContext();
        try {
            if (Build.VERSION.SDK_INT >= 26) {
                NotificationChannel ch = new NotificationChannel(CHANNEL, "Merekam aktivitas", NotificationManager.IMPORTANCE_LOW);
                ch.setDescription("Tampil selama GhostWalk merekam, agar rekaman tetap jalan saat layar mati.");
                ((NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE)).createNotificationChannel(ch);
            }
            if (Build.VERSION.SDK_INT >= 29) {
                startForeground(ID, build(this), ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION);
            } else {
                startForeground(ID, build(this));
            }
        } catch (Exception e) {
            // For example no location permission yet; the app retries once it has it.
            Log.w(TAG, "background recording refused", e);
            stopSelf();
        }
        return START_NOT_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
