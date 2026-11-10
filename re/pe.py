"""Small static-analysis helpers over the copied Cyberpunk2077.exe (no game files are touched)."""
import pefile, re, bisect, struct, json, os, capstone
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(ROOT, 'gamefiles', 'Cyberpunk2077.exe')

class Bin:
    def __init__(self, path=EXE):
        self.pe = pefile.PE(path, fast_load=True)
        self.pe.parse_data_directories(directories=[pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_EXCEPTION']])
        self.base = self.pe.OPTIONAL_HEADER.ImageBase
        self.secs = {s.Name.rstrip(b'\0').decode(): s for s in self.pe.sections}
        self.img = self.pe.get_memory_mapped_image()
        t = self.secs['.text']
        self.text_rva, self.text = t.VirtualAddress, self.img[t.VirtualAddress:t.VirtualAddress + t.Misc_VirtualSize]
        self.funcs = sorted((e.struct.BeginAddress, e.struct.EndAddress) for e in self.pe.DIRECTORY_ENTRY_EXCEPTION)
        self.fstarts = [f[0] for f in self.funcs]
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
        self.md.detail = False
        self._names = None

    def func_of(self, rva):
        i = bisect.bisect_right(self.fstarts, rva) - 1
        if i >= 0 and self.funcs[i][0] <= rva < self.funcs[i][1]:
            return self.funcs[i]
        return None

    def find_bytes(self, needle, section=None):
        out = []
        for name, s in self.secs.items():
            if section and name != section: continue
            data = self.img[s.VirtualAddress:s.VirtualAddress + s.Misc_VirtualSize]
            out += [s.VirtualAddress + m.start() for m in re.finditer(re.escape(needle), data)]
        return out

    def rip_refs(self, target_rva):
        """All instructions in .text with a [rip+disp32] operand pointing at target (lea/mov/cmp...)."""
        hits = []
        t = self.text
        for m in re.finditer(rb'[\x48\x4c][\x8d\x8b][\x05\x0d\x15\x1d\x25\x2d\x35\x3d]', t):
            p = m.start()
            disp = struct.unpack_from('<i', t, p + 3)[0]
            if self.text_rva + p + 7 + disp == target_rva:
                hits.append(self.text_rva + p)
        return hits

    def dis(self, start, end=None, n=None):
        if end is None:
            f = self.func_of(start); end = f[1] if f else start + 0x200
        code = self.img[start:end]
        out = []
        for ins in self.md.disasm(code, self.base + start):
            out.append(ins)
            if n and len(out) >= n: break
        return out

    def cstr(self, rva, maxlen=200):
        b = self.img[rva:rva + maxlen]
        return b.split(b'\0')[0]

    def names(self):
        if self._names is None:
            syms = json.load(open(os.path.join(ROOT, 'work', 'syms.json')))
            self._names = {}
            for s, off, h in syms:
                sec, o = off.split(':')
                rva = 0x1000 + int(o, 16) if sec == '0001' else None
                if rva is not None:
                    self._names[rva] = s or ('h_' + h)
        return self._names

def _callers(self, target_rva):
    """Direct `call rel32` / `jmp rel32` sites targeting target_rva."""
    import numpy as np
    if not hasattr(self, '_np'):
        self._np = np.frombuffer(self.text, dtype=np.uint8)
    t = self._np
    n = len(t) - 5
    out = []
    for op in (0xE8, 0xE9):
        idx = np.nonzero(t[:n] == op)[0]
        d = (t[idx + 1].astype(np.int64) | (t[idx + 2].astype(np.int64) << 8) |
             (t[idx + 3].astype(np.int64) << 16) | (t[idx + 4].astype(np.int64) << 24))
        d = np.where(d >= 1 << 31, d - (1 << 32), d)
        tgt = self.text_rva + idx + 5 + d
        out += [int(self.text_rva + i) for i in idx[tgt == target_rva]]
    return sorted(out)
Bin.callers = _callers
