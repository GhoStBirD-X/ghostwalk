package id.ghostwalk.app;

import android.app.Activity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.provider.OpenableColumns;
import android.util.Log;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * Opening files the person picks (GPX, map files, backups) through Android's own file picker.
 * Android only lets the app read the one file that was picked, so it is copied into the app's
 * cache where the rest of the app can read it like a normal file.
 */
public class Files {
    private static final String TAG = "GhostWalk";
    private static final int REQUEST = 4141;
    /** null while nothing is ready; "" when cancelled or failed; otherwise the path of the copy. */
    private static volatile String result;

    /** Opens the system file picker. The answer arrives later through take(). */
    public static void pick(final Activity activity) {
        result = null;
        Gps.MAIN.post(new Runnable() {
            public void run() {
                try {
                    Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                    i.addCategory(Intent.CATEGORY_OPENABLE);
                    i.setType("*/*");
                    activity.startActivityForResult(i, REQUEST);
                } catch (Exception e) {
                    Log.w(TAG, "file picker failed", e);
                    result = "";
                }
            }
        });
    }

    /** The picked file once it is ready (then forgets it): a path, "" when cancelled, or null while waiting. */
    public static String take() {
        String r = result;
        result = null;
        return r;
    }

    static void onResult(final Activity activity, int request, int code, Intent data) {
        if (request != REQUEST) {
            return;
        }
        final Uri uri = code == Activity.RESULT_OK && data != null ? data.getData() : null;
        if (uri == null) {
            result = "";
            return;
        }
        // Map files can be large; copy away from the main thread.
        new Thread(new Runnable() {
            public void run() {
                try {
                    result = copy(activity, uri);
                } catch (Exception e) {
                    Log.w(TAG, "could not read the picked file", e);
                    result = "";
                }
            }
        }).start();
    }

    private static String copy(Activity activity, Uri uri) throws IOException {
        String name = "berkas";
        Cursor c = activity.getContentResolver().query(uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null);
        if (c != null) {
            try {
                if (c.moveToFirst() && !c.isNull(0)) {
                    name = c.getString(0);
                }
            } finally {
                c.close();
            }
        }
        File dir = new File(activity.getCacheDir(), "dipilih");
        dir.mkdirs();
        File out = new File(dir, name.replaceAll("[^A-Za-z0-9._-]", "_"));
        InputStream in = activity.getContentResolver().openInputStream(uri);
        if (in == null) {
            throw new IOException("no data");
        }
        OutputStream o = new FileOutputStream(out);
        try {
            byte[] buf = new byte[1 << 16];
            int n;
            while ((n = in.read(buf)) > 0) {
                o.write(buf, 0, n);
            }
        } finally {
            in.close();
            o.close();
        }
        return out.getAbsolutePath();
    }
}
