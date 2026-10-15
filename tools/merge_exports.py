#!/usr/bin/env python3
"""Merge per-overlay Ghidra exports into a unified function catalog and per-module decompiled C files."""
import re, json, os, sys
from collections import defaultdict
EXP = os.path.expanduser('~/Projects/Ultima5Revolutions/ghidra/export')
OUT = os.path.expanduser('~/Projects/Ultima5Revolutions/decomp')
NAMES = ['TOWN','MAINOUT','DUNGEON','INTRO','FLAMES','NPC','COMBAT','BLCKTHRN','LOOKOBJ','DNGLOOK','OUTSUBS','SHOPPES','ENDGAME','SJOG','CMDS','CAST','TALK','CAST2','ZSTATS','COMSUBS','SHOPPES2','SHOPPES3','FONT']
START = [0x081d,0x081d,0x081d,0x081d,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0bf8,0x0bf8,0x0bf8,0x0bf8,0x0e1e,0x0e1e,0x0e1e,0x0e1e,0x0e1e,0x0e1e]
END   = [0x09a4,0x09e8,0x0a12,0x0a29,0x0a2b,0x0b5c,0x0bf8,0x0af0,0x0b46,0x0b64,0x0ac3,0x0b9c,0x0ad8,0x0e1e,0x0dc9,0x0e0f,0x0d29,0x0f3a,0x0f4f,0x0f64,0x0ed0,0x0ebc,0x0f08]
rng = {n:((s+0x1000)*16,(e+0x1000)*16) for n,s,e in zip(NAMES,START,END)}
OVL_AREA=(0x181d0,0x1f640)

def lin(a):
    s,o=a.split(':'); return int(s,16)*16+int(o,16)

def parse_functions(path):
    fs={}
    for line in open(path):
        m=re.match(r'(\S+) (\S+) size=(\d+) callers=\[(.*?)\] callees=\[(.*?)\](?: proto=(.*))?$',line)
        if not m: continue
        addr,name,size,callers,callees,proto=m.groups()
        fs[addr]=dict(addr=addr,lin=lin(addr),name=name,size=int(size),proto=proto or '',
                      callers=[c.strip() for c in callers.split(',') if c.strip()],
                      callees=[c.strip() for c in callees.split(',') if c.strip()])
    return fs

def parse_decomp(path):
    """return dict name -> (addr, text)"""
    out={}
    cur=None; buf=[]
    for line in open(path, errors='replace'):
        m=re.match(r'// ===== FUNCTION (\S+) @ (\S+) size=(\d+) =====',line)
        if m:
            if cur: out[cur[0]]=(cur[1],''.join(buf))
            cur=(m.group(1),m.group(2)); buf=[line]
        else:
            buf.append(line)
    if cur: out[cur[0]]=(cur[1],''.join(buf))
    return out

def module_of(l, prog):
    if prog!='ROOT' and rng[prog][0] <= l < rng[prog][1]: return prog
    if OVL_AREA[0] <= l < OVL_AREA[1]: return None   # some other overlay's region / uninit
    if l >= 0x1f640: return None  # data/heap bogus
    return 'ROOT'

catalog={}   # id -> record
decomp_text={}
progs=['ROOT']+NAMES
for prog in progs:
    d=os.path.join(EXP,prog)
    fs=parse_functions(os.path.join(d,'ULTIMA_EXE_functions.txt'))
    dc=parse_decomp(os.path.join(d,'ULTIMA_EXE_decompiled.c'))
    for addr,f in fs.items():
        mod=module_of(f['lin'],prog)
        if mod is None: continue
        if f['name'].startswith('vec_'): mod='VEC'
        fid=f"{mod}:{addr}"
        rec=catalog.get(fid)
        if rec is None:
            rec=dict(id=fid,module=mod,addr=addr,lin=f['lin'],name=f['name'],size=f['size'],proto=f['proto'],
                     callers=set(),callees=set(),seen_in=[])
            catalog[fid]=rec
            decomp_text[fid]=dc.get(f['name'],(None,''))[1]
        else:
            if prog=='ROOT' or (rec['size']<f['size'] and mod=='ROOT'):
                pass
            if f['size']>rec['size'] and mod=='ROOT':
                rec['size']=f['size']; decomp_text[fid]=dc.get(f['name'],(None,''))[1]
        rec['seen_in'].append(prog)
        # resolve callees/callers names to ids within this program
        def resolve(nm):
            # find function with that name in this program
            for a2,f2 in fs.items():
                if f2['name']==nm:
                    m2=module_of(f2['lin'],prog)
                    if f2['name'].startswith('vec_'): m2='VEC'
                    if m2 is None: return None
                    return f"{m2}:{a2}"
            return None
        for c in f['callees']:
            r=resolve(c)
            if r: rec['callees'].add(r)
        for c in f['callers']:
            r=resolve(c)
            if r: rec['callers'].add(r)

# vector -> target resolution
vec_target={}
for fid,rec in catalog.items():
    if rec['module']=='VEC':
        m=re.match(r'vec_([A-Z0-9]+)_([0-9a-f]{4})',rec['name'])
        if m:
            ovl,off=m.groups()
            tid=f"{ovl}:1000:{off}"
            if tid in catalog: vec_target[fid]=tid
            else:
                # overlay function might be at that address but with different id formatting
                for cid,crec in catalog.items():
                    if crec['module']==ovl and crec['lin']==0x10000+int(off,16):
                        vec_target[fid]=cid; break
# add resolved edges
for fid,rec in catalog.items():
    newc=set()
    for c in rec['callees']:
        if c in vec_target:
            newc.add(vec_target[c]); catalog[vec_target[c]]['callers'].add(fid)
        newc.add(c)
    rec['callees']=newc

os.makedirs(OUT,exist_ok=True)
for rec in catalog.values():
    rec['callers']=sorted(rec['callers']); rec['callees']=sorted(rec['callees'])
json.dump(dict(functions=catalog,vec_target=vec_target),open(os.path.join(OUT,'catalog.json'),'w'),indent=1)
# per-module C files
bymod=defaultdict(list)
for fid,rec in catalog.items(): bymod[rec['module']].append(rec)
for mod,recs in bymod.items():
    if mod=='VEC': continue
    recs.sort(key=lambda r:r['lin'])
    with open(os.path.join(OUT,f'{mod}.c'),'w') as f:
        for r in recs:
            f.write(decomp_text[r['id']])
    print(f"{mod:10s} {len(recs):4d} funcs {sum(r['size'] for r in recs):6d} bytes")
print('vectors',len(vec_target),'of',len(bymod['VEC']))
