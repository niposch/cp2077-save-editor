import struct, sys
b = open(sys.argv[1], 'rb').read()
node_id, plen = struct.unpack_from('<II', b, 0)
p = b[8:8 + plen]
ver, nsec, ncomp = struct.unpack_from('<HHI', p, 0)
off = 8
if nsec == 7:
    off += 8
name_desc, name_data, chunk_desc, chunk_data = struct.unpack_from('<4I', p, off); off += 16
ncruid = struct.unpack_from('<I', p, off)[0]; off += 4
cruids = struct.unpack_from('<%dQ' % ncruid, p, off); off += 8 * ncruid
base = off
print(f'node={node_id} len={plen:#x} ver={ver} nsec={nsec} ncomp={ncomp} cruids={ncruid} base={base:#x}')
names = []
for i in range((name_data - name_desc) // 4):
    v = struct.unpack_from('<I', p, base + name_desc + 4 * i)[0]
    o, l = v & 0xffffff, v >> 24
    names.append(p[base + o: base + o + l - 1].decode())
print(len(names), names[:12])

chunks = []
nch = (chunk_data - chunk_desc) // 8
for i in range(nch):
    t, o = struct.unpack_from('<II', p, base + chunk_desc + 8 * i)
    chunks.append((names[t], base + o))
ends = [c[1] for c in chunks[1:]] + [len(p)]
PRIM = {'Bool': '<?', 'Int8': '<b', 'Uint8': '<B', 'Int16': '<h', 'Uint16': '<H', 'Int32': '<i', 'Uint32': '<I',
        'Float': '<f', 'Int64': '<q', 'Uint64': '<Q', 'Double': '<d', 'TweakDBID': '<Q', 'CRUID': '<Q', 'EngineTime': '<d'}
unknown = {}
ENUMS = set()
def val(t, pos, end):
    """returns (value, newpos)"""
    if t in PRIM:
        f = PRIM[t]; return struct.unpack_from(f, p, pos)[0], pos + struct.calcsize(f)
    if t == 'CName':
        i = struct.unpack_from('<H', p, pos)[0]; return ('#' + names[i]) if i < len(names) else f'#?{i}', pos + 2
    if t.startswith('handle:') or t.startswith('whandle:'):
        i = struct.unpack_from('<i', p, pos)[0]; return f'->{i}', pos + 4
    if t.startswith('array:'):
        n = struct.unpack_from('<I', p, pos)[0]; pos += 4
        et = t[6:]; out = []
        if (et in ENUMS or (et not in PRIM and not et.startswith(('handle:', 'array:', 'whandle:')) and et not in ('CName', 'NodeRef', 'String') and n and (end - pos) == 2 * n and all(struct.unpack_from('<H', p, pos + 2 * q)[0] for q in range(n)))):
            ENUMS.add(et)
            for _ in range(n):
                out.append(names[struct.unpack_from('<H', p, pos)[0]]); pos += 2
            return out, pos
        for k in range(n):
            v, pos = val(et, pos, end); out.append(v)
        return out, pos
    if t in ('NodeRef', 'String'):
        n = struct.unpack_from('<H', p, pos)[0]; return p[pos + 2:pos + 2 + n].decode('utf-8', 'replace'), pos + 2 + n
    if t in ENUMS or (end - pos == 2 and struct.unpack_from('<H', p, pos)[0] != 0):   # enum: u16 name index (0 = empty struct)
        ENUMS.add(t)
        i = struct.unpack_from('<H', p, pos)[0]; return t + '.' + names[i], pos + 2
    if not looks_like_struct(pos):
        ENUMS.add(t)
        i = struct.unpack_from('<H', p, pos)[0]; return t + '.' + names[i], pos + 2
    return obj(t, pos, end)
def looks_like_struct(pos):
    n = struct.unpack_from('<H', p, pos)[0]
    if n == 0: return True
    if n > 200 or pos + 2 + 8 * n > len(p): return False
    prev = 0
    for k in range(n):
        ni, ti, o = struct.unpack_from('<HHI', p, pos + 2 + 8 * k)
        if ni >= len(names) or ti >= len(names) or o < prev: return False
        if k == 0 and o != 2 + 8 * n: return False
        prev = o
    return True
def obj(t, pos, end):
    start = pos
    n = struct.unpack_from('<H', p, pos)[0]; pos += 2
    props = []
    for k in range(n):
        ni, ti, o = struct.unpack_from('<HHI', p, pos); pos += 8
        props.append((names[ni], names[ti], start + o))
    out = {}
    for k, (nm, ty, o) in enumerate(props):
        e = props[k + 1][2] if k + 1 < len(props) else end
        v, np_ = val(ty, o, e)
        if np_ != e and k + 1 < len(props):
            unknown[ty] = unknown.get(ty, 0) + 1
        out[f'{nm}:{ty}'] = v
    # for nested structs inside arrays the end is unknown; compute real end from last prop
    return out, (np_ if props else pos)
import json
def run():
  global parsed, bad
  parsed = {}
  bad = 0
  for i, (t, o) in enumerate(chunks):
    try:
        v, e = obj(t, o, ends[i])
        if e != ends[i]: bad += 1; parsed[i] = (t, f'ERR end {e:#x} != {ends[i]:#x}')
        else: parsed[i] = (t, v)
    except Exception as ex:
        bad += 1; parsed[i] = (t, f'ERR {ex}')
for _ in range(3):
    n0 = len(ENUMS); run()
    if len(ENUMS) == n0: break
print('chunks', len(chunks), 'bad', bad, 'unknown-size types', unknown)
want = sys.argv[2] if len(sys.argv) > 2 else 'PlayerDevelopmentData'
for i, (t, v) in parsed.items():
    if t == want:
        print(i, t, json.dumps(v, indent=1, default=str)[:6000])
