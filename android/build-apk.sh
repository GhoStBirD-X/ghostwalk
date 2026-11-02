#!/bin/sh
# Builds the Android installer (APK) for 64-bit ARM phones.
# Needs, under $TOOLS: ndk/ (Android NDK), jdk/ (Java 17), build-tools/, platform/ (android.jar).
set -e
APP="${APP:-$(cd "$(dirname "$0")/.." && pwd -P)}"
TOOLS="${TOOLS:-${XDG_DATA_HOME:-$HOME/.local/share}/toolchain/android}"
STAMP="$(date +%Y%m%d-%H%M%S)"
WORK="${WORK:-${XDG_DATA_HOME:-$HOME/.local/share}/gwbuild/apk}"
OUT="$APP/dist/GhostWalk.apk"
NDK_BIN="$TOOLS/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
BT="$TOOLS/build-tools"
export JAVA_HOME="$TOOLS/jdk" PATH="$TOOLS/jdk/bin:$PATH"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDK_BIN/aarch64-linux-android26-clang"
export CC_aarch64_linux_android="$NDK_BIN/aarch64-linux-android26-clang"
export AR_aarch64_linux_android="$NDK_BIN/llvm-ar"

# The compiler can't work inside some shared folders, so build from a copy.
mkdir -p "$WORK/src" "$WORK/res/drawable" "$WORK/out/$STAMP"
cp -r "$APP/Cargo.toml" "$APP/Cargo.lock" "$APP/src" "$APP/assets" "$WORK/src/"
cd "$WORK/src"
CARGO_TARGET_DIR="$WORK/target" cargo rustc --lib --release --target aarch64-linux-android --crate-type cdylib

O="$WORK/out/$STAMP"
python3 -I "$APP/android/make_icon.py" "$WORK/res/drawable/icon.png" 192
"$BT/aapt2" compile -o "$O/res.zip" "$WORK/res/drawable/icon.png"
"$BT/aapt2" link -o "$O/base.apk" -I "$TOOLS/platform/android.jar" \
    --manifest "$APP/android/AndroidManifest.xml" "$O/res.zip"

# The small Java GPS helper, turned into Android bytecode.
mkdir -p "$O/classes" "$O/dex"
javac -source 8 -target 8 -nowarn -cp "$TOOLS/platform/android.jar" -d "$O/classes" \
    "$APP/android/java/id/ghostwalk/app/"*.java
"$BT/d8" --release --min-api 26 --lib "$TOOLS/platform/android.jar" --output "$O/dex" \
    "$O/classes/id/ghostwalk/app/"*.class

python3 -I - "$O/base.apk" "$WORK/target/aarch64-linux-android/release/libghostwalk.so" "$O/dex/classes.dex" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1], "a", zipfile.ZIP_DEFLATED) as z:
    z.write(sys.argv[2], "lib/arm64-v8a/libghostwalk.so")
    z.write(sys.argv[3], "classes.dex")
PY
"$BT/zipalign" -p 4 "$O/base.apk" "$O/aligned.apk"

KS="$TOOLS/ghostwalk-release.keystore"
if [ ! -f "$KS" ]; then
  keytool -genkeypair -keystore "$KS" -storepass ghostwalk -keypass ghostwalk -alias ghostwalk \
      -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=GhostWalk"
fi
mkdir -p "$APP/dist"
"$BT/apksigner" sign --ks "$KS" --ks-pass pass:ghostwalk --out "$O/signed.apk" "$O/aligned.apk"
"$BT/apksigner" verify "$O/signed.apk"
cp "$O/signed.apk" "$OUT"
echo "APK ready: $OUT"
