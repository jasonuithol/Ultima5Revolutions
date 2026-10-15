#!/usr/bin/env python3
"""Run the original game in headless DOSBox with scripted keys and timed screenshots, for
comparison with the Rust port.  usage: scripted.py OUTPREFIX TOTAL_SECS "t:key,t:key,..."
keys: single chars, or ENTER/ESC/SPACE/UP/DOWN/LEFT/RIGHT/BS/F1..  Screenshots at every key and at the end."""
import os, sys, time, subprocess, signal
from Xlib import display, X, XK
from Xlib.ext import xtest

out, total, spec = sys.argv[1], float(sys.argv[2]), (sys.argv[3] if len(sys.argv) > 3 else '')
os.environ['DISPLAY'] = ':97'
xvfb = subprocess.Popen(['Xvfb', ':97', '-screen', '0', '800x600x24'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1)
conf = os.path.join(os.path.dirname(os.path.abspath(__file__)), os.environ.get('U5CONF','headless.conf'))
gdir = os.environ.get('U5TEST_DIR')
if gdir:
    txt = open(conf).read().replace('mount C "/home/jason/Projects/Ultima 5"', f'mount C "{gdir}"')
    conf = f'/tmp/claude-1000/headless_{os.getpid()}.conf'; open(conf, 'w').write(txt)
dosbox = subprocess.Popen(['dosbox', '-conf', conf, '-noconsole'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
d = display.Display(':97')
root = d.screen().root
names = {'ENTER': 'Return', 'ESC': 'Escape', 'SPACE': 'space', 'UP': 'Up', 'DOWN': 'Down', 'LEFT': 'Left', 'RIGHT': 'Right', 'BS': 'BackSpace', 'TAB': 'Tab'}
def keysym(k):
    if k.upper() in names: return XK.string_to_keysym(names[k.upper()])
    if k.upper().startswith('F') and k[1:].isdigit(): return XK.string_to_keysym(k.upper())
    return XK.string_to_keysym(k)
def focus_dosbox():
    def walk(w):
        for c in w.query_tree().children:
            try:
                n = c.get_wm_name()
            except Exception: n = None
            if n and 'DOSBox' in n: return c
            r = walk(c)
            if r: return r
        return None
    w = walk(root)
    if w: d.set_input_focus(w, X.RevertToParent, X.CurrentTime); d.sync()
    return w
def press(k):
    if '+' in k:
        mods, key = k.rsplit('+', 1)
        codes = [d.keysym_to_keycode(XK.string_to_keysym(m)) for m in mods.split('+')]
        for c in codes: xtest.fake_input(d, X.KeyPress, c)
        d.sync(); time.sleep(0.05)
        press(key)
        for c in reversed(codes): xtest.fake_input(d, X.KeyRelease, c)
        d.sync(); return
    ks = keysym(k)
    code = d.keysym_to_keycode(ks)
    shift = k.isupper() and len(k) == 1
    if shift: xtest.fake_input(d, X.KeyPress, d.keysym_to_keycode(XK.XK_Shift_L))
    xtest.fake_input(d, X.KeyPress, code); d.sync(); time.sleep(float(os.environ.get("HOLD","0.08")))
    xtest.fake_input(d, X.KeyRelease, code); d.sync()
    if shift: xtest.fake_input(d, X.KeyRelease, d.keysym_to_keycode(XK.XK_Shift_L)); d.sync()
def shot(name):
    subprocess.run(['import', '-display', ':97', '-window', 'root', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
events = []
for item in spec.split(','):
    if ':' in item:
        t, k = item.split(':', 1); events.append((float(t) / 1000.0, k.strip()))
events.sort()
start = time.time(); i = 0; n = 0
focused = None
while time.time() - start < total:
    now = time.time() - start
    if focused is None and now > 3: focused = focus_dosbox()
    while i < len(events) and events[i][0] <= now:
        if focused is None: focused = focus_dosbox()
        press(events[i][1]); n += 1
        time.sleep(0.4); shot(f'{out}_k{n}.png')
        i += 1
    time.sleep(0.05)
shot(f'{out}_end.png')
dosbox.send_signal(signal.SIGTERM); xvfb.send_signal(signal.SIGTERM)
print('done', n, 'keys')
