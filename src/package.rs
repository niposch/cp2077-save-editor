//! Embedded RTTI object packages (used by ScriptableSystemsContainer, StatsSystem,
//! StatPoolsSystem, ... nodes). See docs/package-format.md.
//!
//! Node layout: `u32 nodeId, u32 packageLen, package[packageLen], trailing...`
//!
//! Package:
//! ```text
//! u16 version(4) u16 numSections(6|7) u32 numComponents
//! [u32 refPoolDesc, u32 refPoolData]            (only if numSections == 7)
//! u32 nameDesc u32 nameData u32 chunkDesc u32 chunkData   (relative to `base`)
//! u32 numCruids u64 cruids[numCruids]
//! base:
//!   nameDesc:  u32 (offset:24 | len:8) per name, len includes NUL
//!   nameData:  NUL-terminated strings
//!   chunkDesc: (u32 typeNameIdx, u32 offset) per object
//!   chunkData: objects
//! object/struct: u16 n, n x (u16 nameIdx, u16 typeIdx, u32 offsetFromStructStart), values
//! ```
//! Properties holding their default value are omitted entirely.

use anyhow::{ensure, Context, Result};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    /// CName stored as a name-pool index.
    CName(String),
    /// TweakDBID: u32 crc32 of the record path | u8 path length << 32.
    TweakDBID(u64),
    /// Enum value, stored as the name-pool index of the value name.
    Enum(String),
    /// handle:/whandle: — index of the referenced chunk (-1 = null).
    Handle(i32),
    /// u16-length-prefixed byte string (NodeRef, String).
    Str(Vec<u8>),
    /// DataBuffer: u32 length + opaque bytes (nested serialization, not decoded yet).
    Buffer(Vec<u8>),
    Array(Vec<Value>),
    Struct(Struct),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Struct {
    pub props: Vec<Prop>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Prop {
    pub name: String,
    pub ty: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub ty: String,
    pub data: Struct,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub version: u16,
    pub num_sections: u16,
    pub num_components: u32,
    pub ref_pool: Option<(u32, u32)>,
    /// Root CRUID list, present only in some packages.
    pub cruids: Option<Vec<u64>>,
    /// Original name pool order (kept so unmodified packages round-trip byte-exactly).
    pub names: Vec<String>,
    pub chunks: Vec<Chunk>,
}

/// Node wrapper: `u32 nodeId, u32 len, package, trailing`.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageNode {
    pub node_id: u32,
    pub package: Package,
    pub trailing: Vec<u8>,
}

fn prim_size(t: &str) -> Option<usize> {
    Some(match t {
        "Bool" | "Int8" | "Uint8" => 1,
        "Int16" | "Uint16" | "CName" => 2,
        "Int32" | "Uint32" | "Float" => 4,
        "Int64" | "Uint64" | "Double" | "TweakDBID" | "CRUID" | "EngineTime" => 8,
        _ => return None,
    })
}

fn is_ref(t: &str) -> bool {
    t.starts_with("handle:") || t.starts_with("whandle:")
}
fn is_str(t: &str) -> bool {
    matches!(t, "NodeRef" | "String")
}
fn array_elem(t: &str) -> Option<&str> {
    if let Some(e) = t.strip_prefix("array:") {
        return Some(e);
    }
    // static arrays: "[N]Type"
    if t.starts_with('[') {
        return t.find(']').map(|i| &t[i + 1..]);
    }
    None
}

struct Reader<'a> {
    p: &'a [u8],
    names: &'a [String],
    enums: HashSet<String>,
}

impl<'a> Reader<'a> {
    fn u16(&self, at: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(self.p.get(at..at + 2).context("eof")?.try_into()?))
    }
    fn u32(&self, at: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(self.p.get(at..at + 4).context("eof")?.try_into()?))
    }
    fn name(&self, i: u16) -> Result<String> {
        self.names.get(i as usize).cloned().with_context(|| format!("name index {i} out of range"))
    }

    /// Heuristic: does `pos` start a valid struct header?
    fn looks_like_struct(&self, pos: usize) -> bool {
        let Ok(n) = self.u16(pos) else { return false };
        let n = n as usize;
        if n == 0 {
            return true;
        }
        if n > 512 || pos + 2 + 8 * n > self.p.len() {
            return false;
        }
        let mut prev = 0;
        for k in 0..n {
            let e = pos + 2 + 8 * k;
            let (ni, ti, o) = (self.u16(e).unwrap(), self.u16(e + 2).unwrap(), self.u32(e + 4).unwrap() as usize);
            if ni as usize >= self.names.len() || ti as usize >= self.names.len() || o < prev {
                return false;
            }
            if k == 0 && o != 2 + 8 * n {
                return false;
            }
            prev = o;
        }
        true
    }

    /// `end` is the known end of this value if available (exact for all but the
    /// last property of a nested struct).
    fn value(&mut self, t: &str, pos: usize, end: Option<usize>) -> Result<(Value, usize)> {
        use Value::*;
        if let Some(sz) = prim_size(t) {
            let b = self.p.get(pos..pos + sz).context("eof in primitive")?;
            let v = match t {
                "Bool" => Bool(b[0] != 0),
                "Int8" => I8(b[0] as i8),
                "Uint8" => U8(b[0]),
                "Int16" => I16(i16::from_le_bytes(b.try_into()?)),
                "Uint16" => U16(u16::from_le_bytes(b.try_into()?)),
                "CName" => CName(self.name(u16::from_le_bytes(b.try_into()?))?),
                "Int32" => I32(i32::from_le_bytes(b.try_into()?)),
                "Uint32" => U32(u32::from_le_bytes(b.try_into()?)),
                "Float" => F32(f32::from_le_bytes(b.try_into()?)),
                "Int64" => I64(i64::from_le_bytes(b.try_into()?)),
                "Uint64" | "CRUID" => U64(u64::from_le_bytes(b.try_into()?)),
                "TweakDBID" => TweakDBID(u64::from_le_bytes(b.try_into()?)),
                "Double" | "EngineTime" => F64(f64::from_le_bytes(b.try_into()?)),
                _ => unreachable!(),
            };
            return Ok((v, pos + sz));
        }
        if is_ref(t) {
            return Ok((Handle(self.u32(pos)? as i32), pos + 4));
        }
        if is_str(t) {
            let n = self.u16(pos)? as usize;
            let s = self.p.get(pos + 2..pos + 2 + n).context("eof in string")?.to_vec();
            return Ok((Str(s), pos + 2 + n));
        }
        if matches!(t, "DataBuffer" | "serializationDeferredDataBuffer" | "SharedDataBuffer") {
            let n = self.u32(pos)? as usize;
            let b = self.p.get(pos + 4..pos + 4 + n).context("eof in DataBuffer")?.to_vec();
            return Ok((Buffer(b), pos + 4 + n));
        }
        if let Some(et) = array_elem(t) {
            let n = self.u32(pos)? as usize;
            let mut q = pos + 4;
            // array of enums whose type we have not learned yet: exact size match
            let opaque = prim_size(et).is_none() && !is_ref(et) && !is_str(et) && array_elem(et).is_none();
            if opaque && n > 0 && !self.enums.contains(et) {
                if let Some(e) = end {
                    if e - q == 2 * n && (0..n).all(|k| self.u16(q + 2 * k).map_or(false, |v| v != 0)) {
                        self.enums.insert(et.to_string());
                    }
                }
            }
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let (v, nq) = self.value(et, q, None)?;
                out.push(v);
                q = nq;
            }
            return Ok((Array(out), q));
        }
        // enum or struct
        let is_enum = self.enums.contains(t)
            || (end == Some(pos + 2) && self.u16(pos)? != 0)
            || !self.looks_like_struct(pos);
        if is_enum {
            self.enums.insert(t.to_string());
            return Ok((Enum(self.name(self.u16(pos)?)?), pos + 2));
        }
        let (s, e) = self.strukt(pos, end)?;
        Ok((Struct(s), e))
    }

    fn strukt(&mut self, start: usize, end: Option<usize>) -> Result<(Struct, usize)> {
        let n = self.u16(start)? as usize;
        let mut descs = Vec::with_capacity(n);
        for k in 0..n {
            let e = start + 2 + 8 * k;
            descs.push((self.name(self.u16(e)?)?, self.name(self.u16(e + 2)?)?, start + self.u32(e + 4)? as usize));
        }
        let mut props = Vec::with_capacity(n);
        let mut pos = start + 2 + 8 * n;
        for k in 0..n {
            let (name, ty, off) = &descs[k];
            ensure!(*off == pos, "property {name} at {off:#x}, expected {pos:#x}");
            let vend = if k + 1 < n { Some(descs[k + 1].2) } else { end };
            let (value, np) = self.value(ty, *off, vend).with_context(|| format!("in {name}: {ty} @{off:#x}"))?;
            if let Some(ve) = vend {
                ensure!(np == ve, "property {name}: {ty} parsed to {np:#x}, expected {ve:#x}");
            }
            props.push(Prop { name: name.clone(), ty: ty.clone(), value });
            pos = np;
        }
        Ok((Struct { props }, pos))
    }
}

impl Package {
    pub fn parse(p: &[u8]) -> Result<Self> {
        let rd16 = |a: usize| -> Result<u16> { Ok(u16::from_le_bytes(p.get(a..a + 2).context("eof")?.try_into()?)) };
        let rd32 = |a: usize| -> Result<u32> { Ok(u32::from_le_bytes(p.get(a..a + 4).context("eof")?.try_into()?)) };
        let version = rd16(0)?;
        let num_sections = rd16(2)?;
        let num_components = rd32(4)?;
        ensure!(version == 4, "unsupported package version {version}");
        let mut off = 8;
        let ref_pool = if num_sections == 7 {
            off += 8;
            Some((rd32(8)?, rd32(12)?))
        } else {
            ensure!(num_sections == 6, "unexpected section count {num_sections}");
            None
        };
        let (name_desc, name_data, chunk_desc, chunk_data) =
            (rd32(off)? as usize, rd32(off + 4)? as usize, rd32(off + 8)? as usize, rd32(off + 12)? as usize);
        off += 16;
        // Some packages (ScriptableSystemsContainer) carry a root CRUID list, others don't.
        // Disambiguate by checking where the first name-desc entry must point (nameData).
        let first_name_ok = |b: usize| rd32(b).map_or(false, |v| (v & 0xff_ffff) as usize == name_data);
        let cruids = if name_data > 0 && !first_name_ok(off) {
            let ncruid = rd32(off)? as usize;
            off += 4;
            let c = (0..ncruid)
                .map(|i| Ok(u64::from_le_bytes(p.get(off + 8 * i..off + 8 * i + 8).context("cruid list exceeds package")?.try_into()?)))
                .collect::<Result<Vec<_>>>()?;
            off += 8 * ncruid;
            ensure!(first_name_ok(off), "could not locate name table");
            Some(c)
        } else {
            None
        };
        let base = off;
        ensure!(ref_pool.is_none(), "ref pools not supported yet");
        ensure!(name_desc == 0, "unexpected name desc offset");

        let mut names = Vec::new();
        for i in 0..(name_data - name_desc) / 4 {
            let v = rd32(base + name_desc + 4 * i)?;
            let (o, l) = ((v & 0xff_ffff) as usize, (v >> 24) as usize);
            ensure!(l >= 1 && p[base + o + l - 1] == 0, "name {i} not NUL-terminated");
            names.push(String::from_utf8_lossy(&p[base + o..base + o + l - 1]).into_owned());
        }
        let nchunks = (chunk_data - chunk_desc) / 8;
        let mut descs = Vec::with_capacity(nchunks);
        for i in 0..nchunks {
            let e = base + chunk_desc + 8 * i;
            descs.push((rd32(e)? as usize, base + rd32(e + 4)? as usize));
        }

        let mut r = Reader { p, names: &names, enums: HashSet::new() };
        // Enum types are learned from unambiguous occurrences; re-parse until stable.
        let mut last_err = None;
        for _pass in 0..4 {
            let before = r.enums.len();
            let mut chunks = Vec::with_capacity(nchunks);
            last_err = None;
            for (i, &(t, o)) in descs.iter().enumerate() {
                let end = descs.get(i + 1).map_or(p.len(), |d| d.1);
                let ty = names.get(t).cloned().context("chunk type out of range")?;
                match r.strukt(o, Some(end)) {
                    Ok((data, e)) if e == end => chunks.push(Chunk { ty, data }),
                    Ok((_, e)) => {
                        last_err = Some(anyhow::anyhow!("chunk {i} ({ty}) ends at {e:#x}, expected {end:#x}"));
                        break;
                    }
                    Err(err) => {
                        last_err = Some(err.context(format!("chunk {i} ({ty})")));
                        break;
                    }
                }
            }
            if last_err.is_none() && r.enums.len() == before {
                return Ok(Package { version, num_sections, num_components, ref_pool, cruids, names, chunks });
            }
            if r.enums.len() == before {
                break;
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("enum detection did not converge")))
    }

    pub fn write(&self) -> Result<Vec<u8>> {
        let mut w = Writer { names: self.names.clone(), index: HashMap::new() };
        for (i, n) in self.names.iter().enumerate() {
            w.index.entry(n.clone()).or_insert(i);
        }
        // serialize chunk bodies first (this may append new names)
        let mut bodies = Vec::with_capacity(self.chunks.len());
        let mut chunk_types = Vec::with_capacity(self.chunks.len());
        for c in &self.chunks {
            chunk_types.push(w.idx(&c.ty)? as u32);
            let mut b = Vec::new();
            w.strukt(&mut b, &c.data)?;
            bodies.push(b);
        }
        let mut name_desc = Vec::new();
        let mut name_data = Vec::new();
        let names_len = w.names.len();
        let data_start = names_len * 4;
        for n in &w.names {
            let o = data_start + name_data.len();
            ensure!(o < 1 << 24 && n.len() + 1 < 256, "name pool overflow");
            name_desc.extend_from_slice(&((o as u32) | (((n.len() + 1) as u32) << 24)).to_le_bytes());
            name_data.extend_from_slice(n.as_bytes());
            name_data.push(0);
        }
        let chunk_desc_off = data_start + name_data.len();
        let chunk_data_off = chunk_desc_off + bodies.len() * 8;
        let mut chunk_desc = Vec::new();
        let mut o = chunk_data_off;
        for (t, b) in chunk_types.iter().zip(&bodies) {
            chunk_desc.extend_from_slice(&t.to_le_bytes());
            chunk_desc.extend_from_slice(&(o as u32).to_le_bytes());
            o += b.len();
        }

        let mut out = Vec::new();
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&self.num_sections.to_le_bytes());
        out.extend_from_slice(&self.num_components.to_le_bytes());
        for v in [0u32, data_start as u32, chunk_desc_off as u32, chunk_data_off as u32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        if let Some(cr) = &self.cruids {
            out.extend_from_slice(&(cr.len() as u32).to_le_bytes());
            for c in cr {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        out.extend_from_slice(&name_desc);
        out.extend_from_slice(&name_data);
        out.extend_from_slice(&chunk_desc);
        for b in bodies {
            out.extend_from_slice(&b);
        }
        Ok(out)
    }

    /// Indices of all chunks of a given type.
    pub fn find(&self, ty: &str) -> Vec<usize> {
        self.chunks.iter().enumerate().filter(|(_, c)| c.ty == ty).map(|(i, _)| i).collect()
    }
}

struct Writer {
    names: Vec<String>,
    index: HashMap<String, usize>,
}

impl Writer {
    fn idx(&mut self, n: &str) -> Result<u16> {
        let i = match self.index.get(n) {
            Some(&i) => i,
            None => {
                self.names.push(n.to_string());
                self.index.insert(n.to_string(), self.names.len() - 1);
                self.names.len() - 1
            }
        };
        ensure!(i <= u16::MAX as usize, "too many names");
        Ok(i as u16)
    }

    fn strukt(&mut self, out: &mut Vec<u8>, s: &Struct) -> Result<()> {
        let start = out.len();
        out.extend_from_slice(&(s.props.len() as u16).to_le_bytes());
        let desc = out.len();
        out.resize(desc + 8 * s.props.len(), 0);
        for (k, p) in s.props.iter().enumerate() {
            let (ni, ti) = (self.idx(&p.name)?, self.idx(&p.ty)?);
            let off = (out.len() - start) as u32;
            let e = desc + 8 * k;
            out[e..e + 2].copy_from_slice(&ni.to_le_bytes());
            out[e + 2..e + 4].copy_from_slice(&ti.to_le_bytes());
            out[e + 4..e + 8].copy_from_slice(&off.to_le_bytes());
            self.value(out, &p.value)?;
        }
        Ok(())
    }

    fn value(&mut self, out: &mut Vec<u8>, v: &Value) -> Result<()> {
        use Value::*;
        match v {
            Bool(b) => out.push(*b as u8),
            I8(x) => out.push(*x as u8),
            U8(x) => out.push(*x),
            I16(x) => out.extend_from_slice(&x.to_le_bytes()),
            U16(x) => out.extend_from_slice(&x.to_le_bytes()),
            I32(x) => out.extend_from_slice(&x.to_le_bytes()),
            U32(x) => out.extend_from_slice(&x.to_le_bytes()),
            I64(x) => out.extend_from_slice(&x.to_le_bytes()),
            U64(x) | TweakDBID(x) => out.extend_from_slice(&x.to_le_bytes()),
            F32(x) => out.extend_from_slice(&x.to_le_bytes()),
            F64(x) => out.extend_from_slice(&x.to_le_bytes()),
            CName(s) | Enum(s) => {
                let i = self.idx(s)?;
                out.extend_from_slice(&i.to_le_bytes())
            }
            Handle(h) => out.extend_from_slice(&h.to_le_bytes()),
            Str(s) => {
                ensure!(s.len() <= u16::MAX as usize, "string too long");
                out.extend_from_slice(&(s.len() as u16).to_le_bytes());
                out.extend_from_slice(s);
            }
            Buffer(b) => {
                out.extend_from_slice(&(b.len() as u32).to_le_bytes());
                out.extend_from_slice(b);
            }
            Array(a) => {
                out.extend_from_slice(&(a.len() as u32).to_le_bytes());
                for e in a {
                    self.value(out, e)?;
                }
            }
            Struct(s) => self.strukt(out, s)?,
        }
        Ok(())
    }
}

impl PackageNode {
    pub fn parse(node: &[u8]) -> Result<Self> {
        ensure!(node.len() >= 8, "node too small");
        let node_id = u32::from_le_bytes(node[0..4].try_into()?);
        let len = u32::from_le_bytes(node[4..8].try_into()?) as usize;
        ensure!(8 + len <= node.len(), "package length {len:#x} exceeds node");
        let package = Package::parse(&node[8..8 + len])?;
        Ok(PackageNode { node_id, package, trailing: node[8 + len..].to_vec() })
    }

    pub fn write(&self) -> Result<Vec<u8>> {
        let pkg = self.package.write()?;
        let mut out = self.node_id.to_le_bytes().to_vec();
        out.extend_from_slice(&(pkg.len() as u32).to_le_bytes());
        out.extend_from_slice(&pkg);
        out.extend_from_slice(&self.trailing);
        Ok(out)
    }
}

// ---------------------------------------------------------------- text dump

pub fn dump_value(out: &mut String, v: &Value, indent: usize) {
    use Value::*;
    let pad = "  ".repeat(indent);
    match v {
        Struct(s) => {
            if s.props.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            for p in &s.props {
                let _ = write!(out, "{pad}  {}: {} = ", p.name, p.ty);
                dump_value(out, &p.value, indent + 1);
                out.push('\n');
            }
            let _ = write!(out, "{pad}}}");
        }
        Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, e) in a.iter().enumerate() {
                let _ = write!(out, "{pad}  [{i}] ");
                dump_value(out, e, indent + 1);
                out.push('\n');
            }
            let _ = write!(out, "{pad}]");
        }
        Str(s) => {
            let _ = write!(out, "{:?}", String::from_utf8_lossy(s));
        }
        Buffer(b) => {
            let _ = write!(out, "<{} bytes>", b.len());
        }
        CName(s) => {
            let _ = write!(out, "n\"{s}\"");
        }
        Enum(s) => out.push_str(s),
        Handle(h) => {
            let _ = write!(out, "-> chunk {h}");
        }
        TweakDBID(t) => {
            let _ = write!(out, "TDBID({:#010x}, len {})", t & 0xffff_ffff, (t >> 32) & 0xff);
        }
        other => {
            let s = format!("{other:?}");
            // "I32(5)" -> "5"
            let inner = s.find('(').map_or(s.as_str(), |i| &s[i + 1..s.len() - 1]);
            out.push_str(inner);
        }
    }
}

pub fn dump_package(pkg: &Package, filter: Option<&str>) -> String {
    let mut out = String::new();
    for (i, c) in pkg.chunks.iter().enumerate() {
        if filter.map_or(false, |f| c.ty != f) {
            continue;
        }
        let _ = write!(out, "[chunk {i}] {} ", c.ty);
        dump_value(&mut out, &Value::Struct(c.data.clone()), 0);
        out.push('\n');
    }
    out
}

