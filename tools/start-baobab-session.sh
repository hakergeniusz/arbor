#!/bin/bash
# start-baobab-session.sh — hermetically isolated observation session.
#
# Every process is launched with `env -i` (empty environment) so NOTHING is
# inherited from the calling shell: no DISPLAY, no XAUTHORITY, no real session
# bus. GUI output can therefore only ever land on the private Xvfb display,
# never on the user's desktop.
#
# Layout:
#   Xvfb :98 (1440x900x24)
#   private dbus session (dbus-run-session)
#   private XDG_RUNTIME_DIR
#   fake HOME (/tmp/ai-agent/fakehome) -> dconf settings and XDG Trash are
#     fully isolated from the real user
#   at-spi a11y bus, mutter WM, then baobab
#
# Exports the live environment to /tmp/ai-agent/baobab-env.sh for follow-up
# commands (xdotool, import screenshots, Atspi queries).

set -euo pipefail

WORK=/tmp/ai-agent
FAKEHOME=$WORK/fakehome
RTDIR=$WORK/xdgrt
ENVFILE=$WORK/baobab-env.sh
DISP=:98
PATHBIN=/usr/bin:/usr/sbin

mkdir -p "$FAKEHOME/.local/share" "$FAKEHOME/.config" "$FAKEHOME/.cache" "$RTDIR"
chmod 700 "$RTDIR"

# kill only MY previous Xvfb on this exact display
pkill -f "Xvfb $DISP" 2>/dev/null || true
sleep 0.5

env -i PATH="$PATHBIN" Xvfb "$DISP" -screen 0 1440x900x24 -nolisten tcp -noreset &
sleep 1.5

env -i PATH="$PATHBIN" HOME="$FAKEHOME" XDG_RUNTIME_DIR="$RTDIR" \
dbus-run-session -- bash -s <<SESSION
set -u
export PATH="$PATHBIN"
export DISPLAY="$DISP"
export HOME="$FAKEHOME"
export XDG_DATA_HOME="$FAKEHOME/.local/share"
export XDG_CONFIG_HOME="$FAKEHOME/.config"
export XDG_CACHE_HOME="$FAKEHOME/.cache"
export XDG_RUNTIME_DIR="$RTDIR"
export LANG=C.UTF-8
export XDG_SESSION_TYPE=x11

{
echo "export PATH=$PATHBIN"
echo "export DISPLAY=$DISP"
echo "export HOME=$FAKEHOME"
echo "export XDG_DATA_HOME=$FAKEHOME/.local/share"
echo "export XDG_CONFIG_HOME=$FAKEHOME/.config"
echo "export XDG_CACHE_HOME=$FAKEHOME/.cache"
echo "export XDG_RUNTIME_DIR=$RTDIR"
echo "export LANG=C.UTF-8"
echo "export DBUS_SESSION_BUS_ADDRESS='\$DBUS_SESSION_BUS_ADDRESS'"
} > "$ENVFILE"
chmod a+r "$ENVFILE"

# a11y stack first so GTK apps register cleanly.
# GTK4 only connects to AT-SPI when org.gnome.desktop.interface
# toolkit-accessibility is true, and the registry needs manual start because
# systemd user activation is unavailable in a private bus session.
gsettings set org.gnome.desktop.interface toolkit-accessibility true
/usr/lib/at-spi-bus-launcher --launch-immediately > "$WORK/atspi.log" 2>&1 &
sleep 1
A11YBUS="unix:path=$RTDIR/at-spi/bus_98"
AT_SPI_BUS_ADDRESS="$A11YBUS" DISPLAY="$DISP" XDG_RUNTIME_DIR="$RTDIR" \
  DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" \
  /usr/lib/at-spi2-registryd > "$WORK/registryd.log" 2>&1 &
sleep 1
echo "export AT_SPI_BUS_ADDRESS='$A11YBUS'" >> "$ENVFILE"

# window manager (mutter 50: no --replace flag; X11 is the default backend)
mutter > "$WORK/mutter.log" 2>&1 &
sleep 3

# the application under observation — explicit env, nothing inherited
env -i PATH="$PATHBIN" DISPLAY="$DISP" HOME="$FAKEHOME" \
  XDG_DATA_HOME="$FAKEHOME/.local/share" XDG_CONFIG_HOME="$FAKEHOME/.config" \
  XDG_CACHE_HOME="$FAKEHOME/.cache" XDG_RUNTIME_DIR="$RTDIR" \
  DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" \
  AT_SPI_BUS_ADDRESS="$A11YBUS" LANG=C.UTF-8 \
  /usr/bin/baobab > "$WORK/baobab.log" 2>&1 &

sleep 999999
SESSION
