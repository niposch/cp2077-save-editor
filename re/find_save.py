import sys, struct; sys.path.insert(0, 're')
from pe import Bin
b = Bin()
print('base', hex(b.base), 'funcs', len(b.funcs), {k: hex(v.VirtualAddress) for k, v in b.secs.items()})
for tag in [b'VASC', b'FZLC', b'EDON', b'ENOD', b'4ZLX']:
    hits = b.find_bytes(tag, '.text')
    print(tag, [(hex(h), hex(b.func_of(h)[0]) if b.func_of(h) else None) for h in hits[:12]], len(hits))
for s in [b'FactsTable\0', b'ScriptableSystemsContainer\0', b'FactsDB\0', b'PlayerDevelopmentData\0', b'sav.dat\0']:
    locs = b.find_bytes(s)
    print(s, [hex(l) for l in locs[:5]])
    for l in locs[:3]:
        refs = b.rip_refs(l)
        print('   refs', [(hex(r), hex(b.func_of(r)[0])) for r in refs[:10]])
