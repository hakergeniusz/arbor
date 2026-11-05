#!/usr/bin/env python3
# a11y-focus.py — print the currently focused accessible object and its role
import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi

def find(obj, path=''):
    try:
        st = obj.get_state_set()
        if st.contains(Atspi.StateType.FOCUSED) or st.contains(Atspi.StateType.ACTIVE):
            print(f'{path} role={obj.get_role_name()} name={obj.get_name()!r}')
    except Exception:
        return
    try:
        n = obj.get_child_count()
        for i in range(n):
            ch = obj.get_child_at_index(i)
            if ch:
                find(ch, path + '/' + obj.get_role_name())
    except Exception:
        pass

d = Atspi.get_desktop(0)
for i in range(d.get_child_count()):
    find(d.get_child_at_index(i), '')
