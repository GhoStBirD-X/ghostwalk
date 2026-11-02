package id.ghostwalk.app;

import android.app.Activity;
import android.content.ContentValues;
import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.provider.MediaStore;
import android.util.Log;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * Puts a file the app made where the person can find it (pictures in the gallery, everything else in
 * Download/GhostWalk) and can open the phone's share sheet for it. Nothing is sent by the app itself.
 */
public class Share {
    private static final String TAG = "GhostWalk";

    /** Returns where the file went, for telling the person, or "" if it could not be saved. */
    public static String publish(final Activity activity, String path, String mime, boolean share) {
        if (Build.VERSION.SDK_INT < 29) {
            return "";
        }
        File f = new File(path);
        boolean image = mime.startsWith("image/");
        try {
            ContentValues v = new ContentValues();
            v.put(MediaStore.MediaColumns.DISPLAY_NAME, f.getName());
            v.put(MediaStore.MediaColumns.MIME_TYPE, mime);
            v.put(MediaStore.MediaColumns.RELATIVE_PATH,
                    (image ? Environment.DIRECTORY_PICTURES : Environment.DIRECTORY_DOWNLOADS) + "/GhostWalk");
            Uri place = image ? MediaStore.Images.Media.EXTERNAL_CONTENT_URI : MediaStore.Downloads.EXTERNAL_CONTENT_URI;
            final Uri uri = activity.getContentResolver().insert(place, v);
            if (uri == null) {
                return "";
            }
            InputStream in = new FileInputStream(f);
            OutputStream out = activity.getContentResolver().openOutputStream(uri);
            try {
                byte[] buf = new byte[1 << 16];
                int n;
                while ((n = in.read(buf)) > 0) {
                    out.write(buf, 0, n);
                }
            } finally {
                in.close();
                out.close();
            }
            if (share) {
                final Intent send = new Intent(Intent.ACTION_SEND)
                        .setType(mime)
                        .putExtra(Intent.EXTRA_STREAM, uri)
                        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                Gps.MAIN.post(new Runnable() {
                    public void run() {
                        try {
                            activity.startActivity(Intent.createChooser(send, "Bagikan"));
                        } catch (Exception e) {
                            Log.w(TAG, "no share sheet", e);
                        }
                    }
                });
            }
            return (image ? "Galeri > GhostWalk/" : "Download/GhostWalk/") + f.getName();
        } catch (Exception e) {
            Log.w(TAG, "could not save " + path, e);
            return "";
        }
    }
}
