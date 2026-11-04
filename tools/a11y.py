#!/usr/bin/env python3
# a11y.py — dump the AT-SPI accessibility tree of the running GUI apps on the
# isolated session bus. Screen-reader view: roles, names, states. Run with the
# session env sourced (DBUS_SESSION_BUS_ADDRESS must point at the private bus).
import sys
import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi

SHOW_STATES = '-s' in sys.argv

def dump(obj, depth=0):
    try:
        name = obj.get_name() or ''
        role = obj.get_role_name()
        n = obj.get_child_count()
        line = f'{role} "{name}"'
        if SHOW_STATES:
            st = obj.get_state_set().get_states()
            s = ','.join(x.value_name.replace('ATSPI_STATE_', '').lower() for x in st)
            line += f'  [{s}]'
        txt = ''
        if role in ('text', 'entry', 'label', 'paragraph', 'static'):
            try:
                t = obj.get_text(0, -1)
                if t: txt = f'  text={t!r}'
            except Exception:
                pass
        val = ''
        if role in ('slider', 'progress bar', 'scroll bar'):
            try:
                val = f'  value={obj.get_value(0)!r}'
            except Exception:
                pass
        print('  ' * depth + line + txt + val)
    except Exception as e:
        print('  ' * depth + f'<error: {e}>')
        return
    try:
        for i in range(n):
            ch = obj.get_child_at_index(i)
            if ch:
                dump(ch, depth + 1)
    except Exception:
        pass

desktop = Atspi.get_desktop(0)
for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    print(f'== APP: {app.get_name()}')
    dump(app, 1)
