#!/usr/bin/env python3
"""Generate compilable Rust stub modules for every not-yet-translated function.
ROOT -> rust/src/engine/rest.rs ; overlays -> rust/src/ovl/<name>.rs. Signatures come from the Ghidra
decompiled definition line (Ghidra parameter order), names from analysis/symbols.json."""
import json, os, re, glob
R = os.path.expanduser('~/Projects/Ultima5Revolutions')
cat = json.load(open(f'{R}/decomp/catalog.json'))['functions']
sym = json.load(open(f'{R}/analysis/symbols.json'))['functions']
NAMES = ['TOWN','MAINOUT','DUNGEON','INTRO','FLAMES','NPC','COMBAT','BLCKTHRN','LOOKOBJ','DNGLOOK','OUTSUBS','SHOPPES','ENDGAME','SJOG','CMDS','CAST','TALK','CAST2','ZSTATS','COMSUBS','SHOPPES2','SHOPPES3','FONT']
# functions already hand-written in the engine foundation
hand = {}
for f in glob.glob(f'{R}/rust/src/engine/*.rs'):
    if os.path.basename(f) == 'rest.rs': continue
    for m in re.finditer(r'pub fn (\w+)\(', open(f).read()):
        hand[m.group(1)] = 'engine::' + os.path.basename(f)[:-3]
for f in glob.glob(f'{R}/rust/src/ovl/*.rs'):
    mod = os.path.basename(f)[:-3]
    if mod == 'mod': continue
    txt = open(f).read()
    if 'TO BE TRANSLATED' in txt or 'STUB MODULE' in txt: continue
    for m in re.finditer(r'pub fn (\w+)\(', txt): hand[m.group(1)] = 'ovl::' + mod

def rust_type(t):
    t = t.strip()
    t = re.sub(r'\b(unsigned|signed)\s+', lambda m: 'u' if m.group(1)=='unsigned' else '', t)
    if t.endswith('*'): return 'u16'   # near pointer = DGROUP offset
    m = {'void':'()','int':'i16','uint':'u16','char':'i8','uchar':'u8','byte':'u8','bool':'bool','short':'i16','ushort':'u16',
         'long':'i32','ulong':'u32','undefined':'i16','undefined1':'u8','undefined2':'u16','undefined4':'u32','undefined8':'u64',
         'code':'u16','ushort':'u16','uint16_t':'u16','int16_t':'i16','float':'f32','double':'f64','size_t':'u16','word':'u16','dword':'u32'}
    return m.get(t, 'u16')

def parse_sig(text):
    """Return (ret, [(type,name)...]) from the decompiled definition line."""
    for line in text.split('\n'):
        m = re.match(r'^\s*([A-Za-z_][\w \*]*?)\s*(?:__\w+\s+)?(\w+)\s*\((.*)\)\s*$', line)
        if m and not line.strip().startswith('//') and not line.strip().startswith('/*'):
            ret, name, params = m.group(1), m.group(2), m.group(3)
            ret = ret.replace('__cdecl16near','').replace('__cdecl16far','').strip()
            ps = []
            if params.strip() and params.strip() != 'void':
                for p in params.split(','):
                    p = p.strip()
                    mm = re.match(r'^(.*?)\s*(\w+)$', p)
                    if mm: ps.append((rust_type(mm.group(1).replace('*','').strip() + ('*' if '*' in mm.group(1) else '')), mm.group(2)))
                    else: ps.append(('i16', 'p%d' % len(ps)))
            return rust_type(ret), ps
    return 'i16', []

decomp = {}
for fn in glob.glob(f'{R}/decomp/*.c'):
    base = os.path.basename(fn)[:-2]
    if base.startswith('ROOT_part'): continue
    txt = open(fn, errors='replace').read()
    for p in re.split(r'(?=\n// ===== FUNCTION )', txt):
        m = re.search(r'FUNCTION (\S+) @ (\S+) size', p)
        if m: decomp[(base, m.group(2).lower())] = p

index = []
def gen_module(mod, outpath, header):
    recs = sorted([r for r in cat.values() if r['module'] == mod], key=lambda r: r['lin'])
    lines = [header, '#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]',
             'use crate::game::Game;', 'use crate::g;', 'use crate::engine::*;', 'use crate::ovl;', '']
    used = set()
    for r in recs:
        addr = r['addr']
        s = sym.get(addr)
        name = s['name'] if s else f"fn_{addr.replace(':','_')}"
        name = re.sub(r'[^A-Za-z0-9_]', '_', name)
        if name in ('main',): name = 'main_entry'
        if name in hand and mod == 'ROOT': index.append((mod, addr, name, hand[name])); continue
        if name in used: name = f"{name}_{addr.split(':')[1]}"
        used.add(name)
        ret, ps = parse_sig(decomp.get((mod, addr), ''))
        # parameter names from the analyst proto (true order) reversed, if counts match
        pnames = [p[1] for p in ps]
        if s and s.get('proto'):
            mm = re.search(r'\((.*)\)', s['proto'])
            if mm:
                names = [re.sub(r'.*?(\w+)\s*$', r'\1', x.strip()) for x in mm.group(1).split(',') if x.strip() and x.strip() != 'void']
                names = [n for n in names if re.match(r'^\w+$', n)]
                if len(names) == len(ps): pnames = list(reversed(names))
        params = ', '.join(f"{n}: {t}" for (t, _), n in zip(ps, pnames))
        rets = '' if ret == '()' else f' -> {ret}'
        purpose = (s.get('purpose') if s else '') or ''
        lines.append(f"/// orig {addr} ({r['size']} bytes). Ghidra parameter order. {purpose[:300]}")
        lines.append(f"pub fn {name}(gm: &mut Game{', ' if params else ''}{params}){rets} {{ unimplemented!(\"{mod} {name} @ {addr}\") }}")
        lines.append('')
        index.append((mod, addr, name, outpath.split('/src/')[1][:-3].replace('/', '::')))
    open(outpath, 'w').write('\n'.join(lines))
    return len(recs)

rest = f'{R}/rust/src/engine/rest.rs'
if not os.path.exists(rest) or 'STUB MODULE' in open(rest).read():
    n = gen_module('ROOT', rest, '//! ROOT functions not yet translated (STUB MODULE, generated by tools/gen_stubs.py).')
    print('ROOT stubs', n)
else:
    print('rest.rs already translated; not regenerated')
mods = []
for ovl in NAMES:
    path = f'{R}/rust/src/ovl/{ovl.lower()}.rs'
    existing = open(path).read() if os.path.exists(path) else ''
    if existing and 'STUB MODULE' not in existing and 'TO BE TRANSLATED' not in existing:
        mods.append(ovl.lower()); continue
    gen_module(ovl, path, f'//! {ovl}.OVL (STUB MODULE, generated by tools/gen_stubs.py; replace bodies when translating).')
    mods.append(ovl.lower())
open(f'{R}/rust/src/ovl/mod.rs', 'w').write('//! Overlay modules (one per .OVL), translated from decomp_named/<NAME>.c.\n' + ''.join(f'pub mod {m};\n' for m in mods))
with open(f'{R}/analysis/fn_index.tsv', 'w') as f:
    f.write('module\taddr\trust_name\trust_module\n')
    for row in sorted(index): f.write('\t'.join(row) + '\n')
print('wrote', len(index), 'index rows')
