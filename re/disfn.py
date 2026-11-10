"""usage: python re/disfn.py <rva-hex> [...]  -- disassemble whole function(s) with string annotations"""
import sys, re as _re; sys.path.insert(0, 're')
from pe import Bin
b = Bin()
names = b.names()
def ann(ins):
    m = _re.search(r'\[rip ([+-]) 0x([0-9a-f]+)\]', ins.op_str)
    extra = ''
    if m:
        d = int(m.group(2), 16) * (1 if m.group(1) == '+' else -1)
        t = ins.address + ins.size + d - b.base
        s = b.cstr(t, 80)
        if len(s) >= 3 and all(32 <= c < 127 for c in s): extra = f'  ; "{s.decode()}"'
        else: extra = f'  ; {hex(t)}' + (f' {names[t]}' if t in names else '')
    if ins.mnemonic in ('call', 'jmp') and ins.op_str.startswith('0x'):
        t = int(ins.op_str, 16) - b.base
        extra = f'  ; {names.get(t, "")}' + ('' if b.func_of(t) is None else f' [fn {hex(b.func_of(t)[0])}]')
    return extra
for a in sys.argv[1:]:
    f = b.func_of(int(a, 16))
    print(f'==== fn {hex(f[0])}-{hex(f[1])} {names.get(f[0], "")}')
    for ins in b.dis(f[0], f[1]):
        print(f'{ins.address - b.base:8x}: {ins.mnemonic:6} {ins.op_str}{ann(ins)}')
