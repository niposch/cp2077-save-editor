//! Cyberpunk 2077 `sav.dat` container reader/writer.
//!
//! Layout (save version 277 / game 2.5):
//! ```text
//! "VASC" u32 saveVersion u32 gameVersion  str gameDef  u64 timestamp  u32 archiveVersion
//! "FZLC" u32 chunkCount  [N x (u32 offset, u32 compSize, u32 decompSize)]
//! chunks: "4ZLX" u32 decompSize  <lz4 block>        (compSize includes the 8-byte header)
//! "EDON" vlq nodeCount  nodes: str name, i32 next, i32 child, i32 offset, i32 size
//! u32 nodeTableOffset  "ENOD"
//! ```
//! Node offsets are *virtual*: the decompressed stream is treated as if it
//! started at `data_start` (the offset of the first chunk).

use anyhow::{bail, ensure, Context, Result};

pub mod facts;
pub mod package;
pub mod edit;
pub mod player;
pub mod savedir;

pub const CHUNK_SIZE: usize = 0x40000;

#[derive(Debug, Clone)]
pub struct Header {
    pub save_version: u32,
    pub game_version: u32,
    pub game_def: String,
    pub timestamp: u64,
    pub archive_version: u32,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub next: i32,
    pub child: i32,
    pub offset: i32,
    pub size: i32,
}

#[derive(Debug, Clone)]
pub struct SaveFile {
    pub header: Header,
    /// Virtual offset of the first decompressed byte (== offset of first chunk in the file).
    pub data_start: usize,
    /// Number of slots in the chunk table (512 on 2.5, 256 on older saves).
    pub table_capacity: usize,
    /// Fully decompressed payload.
    pub data: Vec<u8>,
    pub nodes: Vec<Node>,
}

// ---------------------------------------------------------------- primitives

pub fn read_vlq(b: &[u8], pos: &mut usize) -> Result<i64> {
    // CR2W packed int: byte0 = [sign][cont][6 bits], following = [cont][7 bits]
    let b0 = *b.get(*pos).context("eof in vlq")?;
    *pos += 1;
    let neg = b0 & 0x80 != 0;
    let mut val = (b0 & 0x3f) as i64;
    let mut shift = 6;
    let mut cont = b0 & 0x40 != 0;
    while cont {
        let bx = *b.get(*pos).context("eof in vlq")?;
        *pos += 1;
        val |= ((bx & 0x7f) as i64) << shift;
        shift += 7;
        cont = bx & 0x80 != 0;
    }
    Ok(if neg { -val } else { val })
}

pub fn write_vlq(out: &mut Vec<u8>, v: i64) {
    let neg = v < 0;
    let mut a = v.unsigned_abs();
    let mut b0 = (a & 0x3f) as u8;
    a >>= 6;
    if neg {
        b0 |= 0x80;
    }
    if a != 0 {
        b0 |= 0x40;
    }
    out.push(b0);
    while a != 0 {
        let mut bx = (a & 0x7f) as u8;
        a >>= 7;
        if a != 0 {
            bx |= 0x80;
        }
        out.push(bx);
    }
}

/// Length-prefixed string: negative length = 1-byte chars, positive = UTF-16.
pub fn read_str(b: &[u8], pos: &mut usize) -> Result<String> {
    let len = read_vlq(b, pos)?;
    if len <= 0 {
        let n = (-len) as usize;
        let s = b.get(*pos..*pos + n).context("eof in str")?;
        *pos += n;
        Ok(String::from_utf8_lossy(s).into_owned())
    } else {
        let n = len as usize * 2;
        let s = b.get(*pos..*pos + n).context("eof in wstr")?;
        *pos += n;
        let u: Vec<u16> = s.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Ok(String::from_utf16_lossy(&u))
    }
}

pub fn write_str(out: &mut Vec<u8>, s: &str) {
    if s.is_ascii() {
        write_vlq(out, -(s.len() as i64));
        out.extend_from_slice(s.as_bytes());
    } else {
        let u: Vec<u16> = s.encode_utf16().collect();
        write_vlq(out, u.len() as i64);
        for c in u {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
}

fn u32_at(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(b.get(p..p + 4).context("eof")?.try_into()?))
}
fn i32_at(b: &[u8], p: usize) -> Result<i32> {
    Ok(u32_at(b, p)? as i32)
}

// ---------------------------------------------------------------- container

impl SaveFile {
    pub fn parse(file: &[u8]) -> Result<Self> {
        ensure!(&file[0..4] == b"VASC", "not a CP2077 save (bad magic)");
        let save_version = u32_at(file, 4)?;
        let game_version = u32_at(file, 8)?;
        let mut p = 12;
        let game_def = read_str(file, &mut p)?;
        let timestamp = u64::from_le_bytes(file[p..p + 8].try_into()?);
        p += 8;
        let archive_version = u32_at(file, p)?;
        p += 4;

        ensure!(&file[p..p + 4] == b"FZLC", "missing FZLC chunk table at {p:#x}");
        p += 4;
        let count = u32_at(file, p)? as usize;
        p += 4;
        let table_start = p;
        let mut chunks = Vec::with_capacity(count);
        for i in 0..count {
            let e = table_start + i * 12;
            chunks.push((u32_at(file, e)? as usize, u32_at(file, e + 4)? as usize, u32_at(file, e + 8)? as usize));
        }
        let data_start = chunks.first().map(|c| c.0).context("no chunks")?;
        ensure!(
            (data_start - table_start) % 12 == 0,
            "unexpected chunk table layout (data at {data_start:#x})"
        );
        let table_capacity = (data_start - table_start) / 12;

        let total: usize = chunks.iter().map(|c| c.2).sum();
        let mut data = Vec::with_capacity(total);
        for (i, &(off, csize, dsize)) in chunks.iter().enumerate() {
            let c = &file[off..off + csize];
            ensure!(&c[0..4] == b"4ZLX", "chunk {i} bad magic");
            ensure!(u32_at(c, 4)? as usize == dsize, "chunk {i} size mismatch");
            let d = lz4_flex::block::decompress(&c[8..], dsize)
                .with_context(|| format!("lz4 chunk {i}"))?;
            ensure!(d.len() == dsize);
            data.extend_from_slice(&d);
        }

        // footer
        let n = file.len();
        ensure!(&file[n - 4..] == b"ENOD", "missing DONE footer");
        let node_off = u32_at(file, n - 8)? as usize;
        ensure!(&file[node_off..node_off + 4] == b"EDON", "missing NODE table");
        let mut p = node_off + 4;
        let ncount = read_vlq(file, &mut p)? as usize;
        let mut nodes = Vec::with_capacity(ncount);
        for _ in 0..ncount {
            let name = read_str(file, &mut p)?;
            let next = i32_at(file, p)?;
            let child = i32_at(file, p + 4)?;
            let offset = i32_at(file, p + 8)?;
            let size = i32_at(file, p + 12)?;
            p += 16;
            nodes.push(Node { name, next, child, offset, size });
        }
        ensure!(p == n - 8, "trailing bytes after node table");

        Ok(SaveFile {
            header: Header { save_version, game_version, game_def, timestamp, archive_version },
            data_start,
            table_capacity,
            data,
            nodes,
        })
    }

    pub fn write(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(self.data.len() / 3);
        out.extend_from_slice(b"VASC");
        out.extend_from_slice(&self.header.save_version.to_le_bytes());
        out.extend_from_slice(&self.header.game_version.to_le_bytes());
        write_str(&mut out, &self.header.game_def);
        out.extend_from_slice(&self.header.timestamp.to_le_bytes());
        out.extend_from_slice(&self.header.archive_version.to_le_bytes());
        out.extend_from_slice(b"FZLC");
        let chunks: Vec<&[u8]> = self.data.chunks(CHUNK_SIZE).collect();
        ensure!(chunks.len() <= self.table_capacity, "too many chunks");
        out.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
        let table_pos = out.len();
        out.resize(table_pos + self.table_capacity * 12, 0);
        ensure!(out.len() == self.data_start, "header size changed; node offsets would shift");

        for (i, c) in chunks.iter().enumerate() {
            let off = out.len();
            out.extend_from_slice(b"4ZLX");
            out.extend_from_slice(&(c.len() as u32).to_le_bytes());
            out.extend_from_slice(&lz4_flex::block::compress(c));
            let csize = out.len() - off;
            let e = table_pos + i * 12;
            out[e..e + 4].copy_from_slice(&(off as u32).to_le_bytes());
            out[e + 4..e + 8].copy_from_slice(&(csize as u32).to_le_bytes());
            out[e + 8..e + 12].copy_from_slice(&(c.len() as u32).to_le_bytes());
        }

        let node_off = out.len();
        out.extend_from_slice(b"EDON");
        write_vlq(&mut out, self.nodes.len() as i64);
        for n in &self.nodes {
            write_str(&mut out, &n.name);
            for v in [n.next, n.child, n.offset, n.size] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&(node_off as u32).to_le_bytes());
        out.extend_from_slice(b"ENOD");
        Ok(out)
    }

    /// Raw bytes of a node (including its children), sliced from the decompressed data.
    pub fn node_bytes(&self, idx: usize) -> &[u8] {
        let n = &self.nodes[idx];
        let s = n.offset as usize - self.data_start;
        &self.data[s..s + n.size as usize]
    }

    pub fn node_bytes_mut(&mut self, idx: usize) -> &mut [u8] {
        let n = &self.nodes[idx];
        let s = n.offset as usize - self.data_start;
        &mut self.data[s..s + n.size as usize]
    }

    /// Replace the full contents of node `idx` (which must have no children) and
    /// fix up offsets of every following node and sizes of every ancestor.
    pub fn replace_node(&mut self, idx: usize, new: Vec<u8>) -> Result<()> {
        ensure!(self.nodes[idx].child < 0, "replace_node only supports leaf nodes");
        let (off, size) = (self.nodes[idx].offset as i64, self.nodes[idx].size as i64);
        let end = off + size;
        let delta = new.len() as i64 - size;
        let s = off as usize - self.data_start;
        self.data.splice(s..s + size as usize, new);
        for (i, n) in self.nodes.iter_mut().enumerate() {
            if i == idx {
                n.size = (size + delta) as i32;
            } else if n.offset as i64 > off {
                n.offset = (n.offset as i64 + delta) as i32;
            } else if (n.offset as i64) < off && n.offset as i64 + n.size as i64 >= end {
                n.size = (n.size as i64 + delta) as i32; // ancestor
            }
        }
        Ok(())
    }

    pub fn find_node(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    /// Indices of the direct children of node `idx`.
    pub fn children(&self, idx: usize) -> Vec<usize> {
        let mut out = vec![];
        let mut c = self.nodes[idx].child;
        while c >= 0 {
            out.push(c as usize);
            c = self.nodes[c as usize].next;
        }
        out
    }

    /// Root node indices (nodes that are nobody's child).
    pub fn roots(&self) -> Vec<usize> {
        let mut is_child = vec![false; self.nodes.len()];
        for n in &self.nodes {
            if n.child >= 0 {
                let mut c = n.child;
                while c >= 0 {
                    is_child[c as usize] = true;
                    c = self.nodes[c as usize].next;
                }
            }
        }
        (0..self.nodes.len()).filter(|&i| !is_child[i]).collect()
    }
}

pub fn check_magic(b: &[u8], m: &[u8]) -> Result<()> {
    if !b.starts_with(m) {
        bail!("expected magic {:?}", String::from_utf8_lossy(m));
    }
    Ok(())
}
