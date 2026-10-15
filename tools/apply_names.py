#!/usr/bin/env python3
"""Produce decomp_named/*.c: decompiled C with FUN_/DAT_ identifiers replaced by chosen names and a
purpose comment above each function."""
import json, os, re, glob
R = os.path.expanduser('~/Projects/Ultima5Revolutions')
sym = json.load(open(f'{R}/analysis/symbols.json'))
cat = json.load(open(f'{R}/decomp/catalog.json'))
funcs = sym['functions']; globs = sym['globals']
vec_target = cat['vec_target']
# name maps
fmap = {}
for addr, f in funcs.items():
    seg, off = addr.split(':')
    fmap[f'FUN_{seg}_{off}'] = f['name']
    for cid, rec in cat['functions'].items():
        if rec['addr'] == addr and rec['module'] != 'VEC':
            fmap[rec['name']] = f['name']   # e.g. TOWN_93c0 -> name
            break
# vectors -> module::name
for vid, tid in vec_target.items():
    vrec = cat['functions'][vid]; trec = cat['functions'][tid]
    tname = funcs.get(trec['addr'], {}).get('name', trec['name'])
    fmap[vrec['name']] = f"{trec['module'].lower()}::{tname}"
gmap = {}
for off, g in globs.items():
    gmap[f'DAT_1f64_{off[2:]}'] = g['name']
pat = re.compile(r'\b(FUN_[0-9a-f]{4}_[0-9a-f]{4}|vec_[A-Z0-9]+_[0-9a-f]{4}|[A-Z0-9]+_[0-9a-f]{4}|DAT_1f64_[0-9a-f]{4})\b')
def sub(m):
    t = m.group(1)
    if t in fmap: return fmap[t]
    if t in gmap: return gmap[t]
    return t
os.makedirs(f'{R}/decomp_named', exist_ok=True)
for fn in sorted(glob.glob(f'{R}/decomp/*.c')):
    base = os.path.basename(fn)
    if base.startswith('ROOT_part'): continue
    text = open(fn, errors='replace').read()
    parts = re.split(r'(?=\n// ===== FUNCTION )', text)
    out = []
    for p in parts:
        m = re.search(r'// ===== FUNCTION (\S+) @ (\S+) size=(\d+)', p)
        if m:
            addr = m.group(2).lower()
            f = funcs.get(addr)
            hdr = ''
            if f:
                hdr = f"\n/* [{f.get('module')}] {f['name']}  (orig {addr})\n   proto: {f.get('proto')}\n   purpose: {f.get('purpose')}\n"
                if f.get('libc'): hdr += f"   libc: {f['libc']}\n"
                if f.get('notes'): hdr += f"   notes: {f['notes']}\n"
                hdr += "*/\n"
            p = hdr + p
        out.append(pat.sub(sub, p))
    open(f'{R}/decomp_named/{base}', 'w').write(''.join(out))
# globals header
with open(f'{R}/decomp_named/globals.txt', 'w') as f:
    for off in sorted(globs, key=lambda o: int(o, 16)):
        g = globs[off]
        f.write(f"{off}\t{g['name']}\t{g.get('type')}\t{g.get('meaning')}\n")
print('named files written')
