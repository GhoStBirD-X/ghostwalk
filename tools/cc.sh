#!/bin/sh
# Linker helper. Inside the Craft Anything sandbox there is no C compiler, so
# Zig's bundled clang links the app; elsewhere the normal system compiler is used.
ZIG="${ZIG:-${XDG_DATA_HOME:-$HOME/.local/share}/toolchain/zig/zig}"
if [ ! -x "$ZIG" ]; then
  exec cc "$@"
fi
args=""
for a in "$@"; do
  case "$a" in
    -fuse-ld=*|-B*|-Wl,-znostart-stop-gc|--target=*) ;;
    *) args="$args
$a" ;;
  esac
done
IFS='
'
exec "$ZIG" cc -target x86_64-linux-gnu.2.28 $args
