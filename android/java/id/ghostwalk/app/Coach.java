package id.ghostwalk.app;

import android.app.Activity;
import android.content.Context;
import android.content.pm.PackageManager;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.location.Location;
import android.os.Build;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.speech.tts.TextToSpeech;
import android.util.Log;
import java.util.Locale;

/**
 * Everything that has to happen during a recording even with the screen off: a spoken update every
 * kilometre, auto-pause announcements, interval cues, the step counter and the notification text.
 * All offline; the voice is the phone's own text-to-speech.
 */
public class Coach implements SensorEventListener {
    private static final String TAG = "GhostWalk";
    private static Context app;
    private static TextToSpeech tts;
    private static volatile boolean ttsReady;
    /** Said as soon as the voice is ready, if something came up before that. */
    private static String pending;
    private static volatile boolean active;
    private static volatile boolean paused;
    private static boolean voice;
    private static boolean speedSport;
    private static double minSpeed;
    private static boolean autoPause;
    /** The recording clock: wall time since the start, minus manual pauses (and standing still with auto-pause). */
    private static long startMs;
    private static long pausedMs;
    private static long pauseStartMs;
    private static double stopped;

    private static Location last;
    private static double dist;
    private static double moving;
    private static int nextKm;
    private static double kmStartMoving;
    private static double stillFor;
    private static boolean saidPaused;

    private static SensorManager sensors;
    private static Coach stepListener;
    private static float lastCount = -1;
    private static volatile int steps;

    private static int work;
    private static int rest;
    private static int reps;
    private static int tick;
    /** Every few seconds: refresh the notification, and pick up the step counter once it is allowed. */
    private static final Runnable PULSE = new Runnable() {
        public void run() {
            if (!active) {
                return;
            }
            ensureSteps();
            Tracker.update(String.format(Locale.US, "%.2f km · %s", dist / 1000, clock(elapsed())).replace('.', ','));
            Gps.MAIN.postDelayed(this, 5000);
        }
    };
    private static final Runnable TICK = new Runnable() {
        public void run() {
            if (!active || reps <= 0) {
                return;
            }
            if (!paused) {
                intervalCue(tick);
                tick++;
            }
            Gps.MAIN.postDelayed(this, 1000);
        }
    };

    /** Starts coaching a new recording. `workS`/`restS`/`repCount` describe an interval workout (repCount 0 = none). */
    public static void begin(Activity activity, boolean speak, boolean bySpeed, boolean pauseWhenStill, double minMovingSpeed,
            int workS, int restS, int repCount) {
        end();
        app = activity.getApplicationContext();
        active = true;
        paused = false;
        voice = speak;
        speedSport = bySpeed;
        minSpeed = minMovingSpeed;
        autoPause = pauseWhenStill;
        startMs = System.currentTimeMillis();
        pausedMs = 0;
        stopped = 0;
        last = null;
        dist = 0;
        moving = 0;
        nextKm = 1;
        kmStartMoving = 0;
        stillFor = 0;
        saidPaused = false;
        steps = 0;
        lastCount = -1;
        work = workS;
        rest = restS;
        reps = repCount;
        tick = 0;
        pending = null;
        if (voice) {
            startVoice(reps > 0 ? null : "Rekaman dimulai.");
        }
        ensureSteps();
        Gps.MAIN.postDelayed(PULSE, 1000);
        if (reps > 0) {
            Gps.MAIN.post(TICK);
        }
    }

    public static void setPaused(boolean p) {
        long now = System.currentTimeMillis();
        if (p && !paused) {
            pauseStartMs = now;
        } else if (!p && paused) {
            pausedMs += now - pauseStartMs;
        }
        paused = p;
        last = null;
    }

    private static double elapsed() {
        long now = System.currentTimeMillis();
        long off = pausedMs + (paused ? now - pauseStartMs : 0);
        return Math.max(0, (now - startMs - off) / 1000.0 - (autoPause ? stopped : 0));
    }

    /** Starts the step counter, unless it runs already or is not allowed yet (it is asked for at the start). */
    private static void ensureSteps() {
        if (stepListener != null || app == null) {
            return;
        }
        if (Build.VERSION.SDK_INT >= 29
                && app.checkSelfPermission("android.permission.ACTIVITY_RECOGNITION") != PackageManager.PERMISSION_GRANTED) {
            return;
        }
        try {
            sensors = (SensorManager) app.getSystemService(Context.SENSOR_SERVICE);
            Sensor counter = sensors.getDefaultSensor(Sensor.TYPE_STEP_COUNTER);
            if (counter != null) {
                Coach l = new Coach();
                if (sensors.registerListener(l, counter, SensorManager.SENSOR_DELAY_NORMAL)) {
                    stepListener = l;
                    Log.i(TAG, "step counter on");
                }
            }
        } catch (Exception e) {
            Log.w(TAG, "no step counter", e);
        }
    }

    /** Steps counted since the recording started, leaving out paused time. */
    public static int steps() {
        return steps;
    }

    public static void end() {
        active = false;
        Gps.MAIN.removeCallbacks(TICK);
        Gps.MAIN.removeCallbacks(PULSE);
        if (sensors != null && stepListener != null) {
            try {
                sensors.unregisterListener(stepListener);
            } catch (Exception e) {
                // Already gone.
            }
        }
        stepListener = null;
        if (tts != null) {
            final TextToSpeech t = tts;
            tts = null;
            ttsReady = false;
            // Let a last sentence finish before letting go of the voice.
            Gps.MAIN.postDelayed(new Runnable() {
                public void run() {
                    t.shutdown();
                }
            }, 4000);
        }
    }

    private static void startVoice(final String first) {
        try {
            tts = new TextToSpeech(app, new TextToSpeech.OnInitListener() {
                public void onInit(int status) {
                    if (status != TextToSpeech.SUCCESS || tts == null) {
                        return;
                    }
                    int r = tts.setLanguage(new Locale("id", "ID"));
                    if (r == TextToSpeech.LANG_MISSING_DATA || r == TextToSpeech.LANG_NOT_SUPPORTED) {
                        Log.w(TAG, "no Indonesian voice on this phone; using the default one");
                    }
                    ttsReady = true;
                    if (first != null) {
                        say(first);
                    }
                    if (pending != null) {
                        say(pending);
                        pending = null;
                    }
                }
            });
        } catch (Exception e) {
            Log.w(TAG, "no text-to-speech", e);
        }
    }

    private static void say(String s) {
        if (tts != null && ttsReady) {
            tts.speak(s, TextToSpeech.QUEUE_ADD, null, "gw" + System.nanoTime());
        } else if (tts != null) {
            pending = s;
        }
    }

    private static void buzz(long ms) {
        try {
            Vibrator v = (Vibrator) app.getSystemService(Context.VIBRATOR_SERVICE);
            if (Build.VERSION.SDK_INT >= 26) {
                v.vibrate(VibrationEffect.createOneShot(ms, VibrationEffect.DEFAULT_AMPLITUDE));
            } else {
                v.vibrate(ms);
            }
        } catch (Exception e) {
            // No vibrator.
        }
    }

    private static void intervalCue(int t) {
        int cycle = Math.max(1, work + rest);
        int rep = t / cycle;
        int into = t % cycle;
        String line = null;
        if (t == reps * cycle) {
            buzz(900);
            line = "Interval selesai. Kerja bagus!";
        } else if (rep < reps && into == 0) {
            buzz(400);
            line = rep == 0 ? "Interval dimulai. Lari." : "Lari. Set " + (rep + 1) + " dari " + reps + ".";
        } else if (rep < reps && into == work && rest > 0) {
            buzz(250);
            line = "Jalan santai.";
        }
        if (line != null && voice) {
            say(line);
        }
    }

    /** One GPS position, from Gps. */
    static void onFix(Location l) {
        if (!active || paused || l.getAccuracy() > 35) {
            return;
        }
        if (last != null) {
            double d = last.distanceTo(l);
            double dt = (l.getTime() - last.getTime()) / 1000.0;
            if (dt > 0) {
                dist += d;
                if (d / dt >= minSpeed) {
                    moving += dt;
                    stillFor = 0;
                    if (saidPaused) {
                        saidPaused = false;
                        if (voice) {
                            say("Lanjut.");
                        }
                    }
                } else {
                    stillFor += dt;
                    stopped += dt;
                    if (stillFor >= 8 && !saidPaused) {
                        saidPaused = true;
                        if (voice) {
                            say("Jeda otomatis.");
                        }
                    }
                }
            }
        }
        last = l;
        while (dist >= nextKm * 1000.0) {
            double kmTime = moving - kmStartMoving;
            kmStartMoving = moving;
            if (voice) {
                say(kmSentence(nextKm, kmTime));
            }
            nextKm++;
        }
    }

    private static String kmSentence(int km, double seconds) {
        String head = "Kilometer " + km + ". ";
        if (seconds <= 0) {
            return head;
        }
        if (speedSport) {
            return head + "Kecepatan " + Math.round(3600 / seconds) + " kilometer per jam.";
        }
        long s = Math.round(seconds);
        return head + "Tempo " + (s / 60) + " menit " + (s % 60) + " detik.";
    }

    private static String clock(double seconds) {
        long s = (long) seconds;
        return s >= 3600
                ? String.format(Locale.US, "%d:%02d:%02d", s / 3600, (s / 60) % 60, s % 60)
                : String.format(Locale.US, "%d:%02d", s / 60, s % 60);
    }

    @Override
    public void onSensorChanged(SensorEvent e) {
        float count = e.values[0];
        if (lastCount >= 0 && !paused && count > lastCount) {
            steps += (int) (count - lastCount);
        }
        lastCount = count;
    }

    @Override
    public void onAccuracyChanged(Sensor sensor, int accuracy) {}
}
