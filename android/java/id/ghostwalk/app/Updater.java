package id.ghostwalk.app;

import android.app.Activity;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageInfo;
import android.content.pm.PackageInstaller;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.net.Uri;
import android.os.Build;
import android.provider.Settings;
import android.util.Log;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.security.MessageDigest;
import java.util.Arrays;

/**
 * Installs a GhostWalk update the app downloaded, only after checking it: its SHA-256 must match the
 * release's SHA256SUMS, and it must be this app (same package, a higher version, the same signing key).
 * Android still asks the person to confirm the installation.
 */
public class Updater {
    private static final String TAG = "GhostWalk";
    private static final String ACTION = "id.ghostwalk.app.UPDATE_STATUS";

    /** 0 idle, 1 checking/copying, 2 needs "install unknown apps", 3 waiting for Android, 4 failed. */
    private static volatile int state;
    private static volatile String message = "";
    private static boolean listening;

    public static int state() {
        return state;
    }

    public static String message() {
        return message;
    }

    private static void fail(String why) {
        message = why;
        state = 4;
    }

    /** Checks the APK at `path` against `sha256` and this app, then hands it to Android's installer. */
    public static void install(final Activity activity, final String path, final String sha256) {
        state = 1;
        message = "";
        new Thread(new Runnable() {
            public void run() {
                try {
                    check(activity, path, sha256);
                    if (state == 4) {
                        return;
                    }
                    if (Build.VERSION.SDK_INT >= 26 && !activity.getPackageManager().canRequestPackageInstalls()) {
                        state = 2;
                        Intent i = new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                                Uri.parse("package:" + activity.getPackageName()));
                        activity.startActivity(i);
                        return;
                    }
                    commit(activity, path);
                } catch (Exception e) {
                    Log.w(TAG, "update failed", e);
                    fail("Gagal memasang: " + e.getMessage());
                }
            }
        }).start();
    }

    private static void check(Activity activity, String path, String sha256) throws Exception {
        MessageDigest md = MessageDigest.getInstance("SHA-256");
        InputStream in = new FileInputStream(path);
        try {
            byte[] buf = new byte[1 << 16];
            int n;
            while ((n = in.read(buf)) > 0) {
                md.update(buf, 0, n);
            }
        } finally {
            in.close();
        }
        StringBuilder hex = new StringBuilder();
        for (byte b : md.digest()) {
            hex.append(String.format("%02x", b));
        }
        if (!hex.toString().equalsIgnoreCase(sha256)) {
            new File(path).delete();
            fail("File update rusak atau berbeda dari rilisnya (SHA-256 tidak cocok).");
            return;
        }
        PackageManager pm = activity.getPackageManager();
        String me = activity.getPackageName();
        int flags = Build.VERSION.SDK_INT >= 28 ? PackageManager.GET_SIGNING_CERTIFICATES : PackageManager.GET_SIGNATURES;
        PackageInfo apk = pm.getPackageArchiveInfo(path, flags);
        PackageInfo installed = pm.getPackageInfo(me, flags);
        if (apk == null || !me.equals(apk.packageName)) {
            fail("File update bukan GhostWalk.");
            return;
        }
        if (versionCode(apk) <= versionCode(installed)) {
            fail("File update tidak lebih baru dari versi terpasang.");
            return;
        }
        if (!Arrays.equals(signer(apk), signer(installed))) {
            fail("File update tidak ditandatangani dengan kunci GhostWalk.");
        }
    }

    @SuppressWarnings("deprecation")
    private static long versionCode(PackageInfo p) {
        return Build.VERSION.SDK_INT >= 28 ? p.getLongVersionCode() : p.versionCode;
    }

    /** The first signing certificate, as bytes. */
    @SuppressWarnings("deprecation")
    private static byte[] signer(PackageInfo p) {
        Signature[] s = null;
        if (Build.VERSION.SDK_INT >= 28 && p.signingInfo != null) {
            s = p.signingInfo.getApkContentsSigners();
        }
        if (s == null || s.length == 0) {
            s = p.signatures;
        }
        return s == null || s.length == 0 ? new byte[0] : s[0].toByteArray();
    }

    private static void commit(final Activity activity, String path) throws Exception {
        Context app = activity.getApplicationContext();
        listen(app);
        PackageInstaller installer = app.getPackageManager().getPackageInstaller();
        PackageInstaller.SessionParams params = new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        params.setAppPackageName(app.getPackageName());
        int id = installer.createSession(params);
        PackageInstaller.Session session = installer.openSession(id);
        try {
            File f = new File(path);
            InputStream in = new FileInputStream(f);
            OutputStream out = session.openWrite("GhostWalk.apk", 0, f.length());
            try {
                byte[] buf = new byte[1 << 16];
                int n;
                while ((n = in.read(buf)) > 0) {
                    out.write(buf, 0, n);
                }
                session.fsync(out);
            } finally {
                in.close();
                out.close();
            }
            Intent status = new Intent(ACTION).setPackage(app.getPackageName());
            int pf = PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0);
            PendingIntent pi = PendingIntent.getBroadcast(app, id, status, pf);
            state = 3;
            session.commit(pi.getIntentSender());
        } finally {
            session.close();
        }
    }

    /** Android answers the installation through a broadcast; asking the person to confirm comes first. */
    private static synchronized void listen(Context app) {
        if (listening) {
            return;
        }
        listening = true;
        BroadcastReceiver r = new BroadcastReceiver() {
            public void onReceive(Context c, Intent i) {
                int s = i.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
                if (s == PackageInstaller.STATUS_PENDING_USER_ACTION) {
                    Intent confirm = (Intent) i.getParcelableExtra(Intent.EXTRA_INTENT);
                    if (confirm != null) {
                        confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                        c.startActivity(confirm);
                    }
                } else if (s == PackageInstaller.STATUS_SUCCESS) {
                    state = 0;
                } else if (s == PackageInstaller.STATUS_FAILURE_ABORTED) {
                    fail("Pemasangan dibatalkan.");
                } else {
                    String m = i.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
                    fail("Android menolak update" + (m != null ? ": " + m : "."));
                }
            }
        };
        IntentFilter f = new IntentFilter(ACTION);
        if (Build.VERSION.SDK_INT >= 33) {
            app.registerReceiver(r, f, Context.RECEIVER_NOT_EXPORTED);
        } else {
            app.registerReceiver(r, f);
        }
    }
}
