#!/usr/bin/env python3
"""Merge analysis JSON files into analysis/symbols.json (functions + globals)."""
import json, glob, os, re, sys
from collections import Counter, defaultdict
A = os.path.expanduser('~/Projects/Ultima5Revolutions/analysis')
cat = json.load(open(os.path.expanduser('~/Projects/Ultima5Revolutions/decomp/catalog.json')))['functions']

functions = {}   # addr -> rec
globals_votes = defaultdict(list)  # offset -> [(name,type,meaning,source)]
seed = {}
if os.path.exists(f'{A}/globals_seed.jsonl'):
    for line in open(f'{A}/globals_seed.jsonl'):
        line = line.strip()
        if not line: continue
        rec = json.loads(line)
        off = '0x%04x' % int(rec['off'], 16)
        seed[off] = rec
        globals_votes[off].append((rec.get('name'), rec.get('type'), rec.get('meaning'), 'seed'))

files = sorted(glob.glob(f'{A}/root_part*.json*')) + sorted(glob.glob(f'{A}/ovl_*.json*'))
for fn in files:
    try:
        if fn.endswith('.jsonl'):
            arr = []
            for line in open(fn):
                line = line.strip()
                if not line: continue
                try: arr.append(json.loads(line))
                except Exception as e: print('bad line in', fn, e)
        else:
            arr = json.load(open(fn))
    except Exception as e:
        print('BAD JSON', fn, e); continue
    if isinstance(arr, dict) and 'functions' in arr: arr = arr['functions']
    for rec in arr:
        addr = rec.get('addr')
        if not addr: continue
        addr = addr.lower()
        functions[addr] = dict(rec, src=os.path.basename(fn))
        for off, g in (rec.get('globals') or {}).items():
            if isinstance(g, str): g = {'name': g}
            o = off.lower()
            if not o.startswith('0x'): o = '0x' + o
            o = '0x%04x' % int(o, 16)
            globals_votes[o].append((g.get('name'), g.get('type'), g.get('meaning'), os.path.basename(fn)))

# choose global names: seed wins, else most common proposed name
globs = {}
for off, votes in globals_votes.items():
    seedv = [v for v in votes if v[3] == 'seed' and v[0]]
    codev = [v for v in votes if v[3] != 'seed' and v[0]]
    if seedv and not codev:
        name, typ, meaning, _ = seedv[0]
    else:
        names = Counter(v[0] for v in votes if v[0])
        name = names.most_common(1)[0][0] if names else f'g_{off[2:]}'
        typ = next((v[1] for v in votes if v[0] == name and v[1]), None)
        meaning = next((v[2] for v in votes if v[0] == name and v[2]), None)
    if not name.startswith('g_'): name = 'g_' + name
    sd = seed.get(off)
    if sd and not meaning: meaning = sd.get('meaning')
    globs[off] = dict(name=name, type=typ, meaning=meaning, size=(sd or {}).get('size'),
                      doc=(sd or {}).get('name'), alt=[v[0] for v in votes if v[0] and v[0] != name][:4])
# uniquify global names
seen = Counter(g['name'] for g in globs.values())
for off, g in globs.items():
    if seen[g['name']] > 1: g['name'] = f"{g['name']}_{off[2:]}"
# uniquify function names and attach module from catalog
byname = Counter()
for addr, f in functions.items():
    nm = f.get('name') or f"fn_{addr.replace(':','_')}"
    nm = re.sub(r'[^A-Za-z0-9_]', '_', nm)
    f['name'] = nm; byname[nm] += 1
for addr, f in functions.items():
    if byname[f['name']] > 1: f['name'] = f"{f['name']}_{addr.split(':')[1]}"
    cid = next((k for k, r in cat.items() if r['addr'] == addr and r['module'] != 'VEC'), None)
    f['module'] = cat[cid]['module'] if cid else None
    f['id'] = cid
out = dict(functions=functions, globals=globs)
json.dump(out, open(f'{A}/symbols.json', 'w'), indent=1)
print('functions', len(functions), 'globals', len(globs), 'from', [os.path.basename(x) for x in files])
