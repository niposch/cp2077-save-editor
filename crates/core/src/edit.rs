//! Path-based reading/editing of package values.
//!
//! Path syntax (segments separated by `.`):
//! ```text
//! PlayerDevelopmentData.attributes[attributeName=Cool].value
//! PlayerDevelopmentData.devPoints[type=Primary].unspent
//! PlayerDevelopmentData.devPoints[type=].unspent      empty value = field absent (default enum value)
//! #135.highestCompletedMinigameLevel                   chunk by index
//! Vendor[3].stock                                      4th chunk of type Vendor
//! foo.bar:Int32                                        force type when inserting an omitted field
//! ```
//! The first segment selects a chunk (by type, optionally `[n]`-th of that type, or `#index`).
//! Following segments select struct fields; `[i]` / `[field=value]` select array elements.
//!
//! Setting a field that is absent (because it held its default value) inserts it. Its type and
//! position come from other structs of the same type in the package.

use crate::package::{array_elem, Package, Prop, Struct, Value};
use anyhow::{bail, ensure, Context, Result};

#[derive(Debug, Clone)]
enum Sel {
    Index(usize),
    Match(String, String),
}

#[derive(Debug, Clone)]
struct Seg {
    name: String,
    ty: Option<String>,
    sel: Option<Sel>,
}

fn parse_path(path: &str) -> Result<Vec<Seg>> {
    let mut segs = Vec::new();
    // split on '.' outside brackets
    let mut depth = 0;
    let mut cur = String::new();
    let mut parts = Vec::new();
    for c in path.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            '.' if depth == 0 => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    parts.push(cur);
    for p in parts {
        ensure!(!p.is_empty(), "empty path segment in {path:?}");
        let (head, sel) = match p.find('[') {
            Some(i) => {
                ensure!(p.ends_with(']'), "bad selector in {p:?}");
                let inner = &p[i + 1..p.len() - 1];
                let sel = match inner.split_once('=') {
                    Some((f, v)) => Sel::Match(f.to_string(), v.to_string()),
                    None => Sel::Index(inner.parse().with_context(|| format!("bad index {inner:?}"))?),
                };
                (&p[..i], Some(sel))
            }
            None => (p.as_str(), None),
        };
        let (name, ty) = match head.split_once(':') {
            Some((n, t)) => (n.to_string(), Some(t.to_string())),
            None => (head.to_string(), None),
        };
        segs.push(Seg { name, ty, sel });
    }
    Ok(segs)
}

/// Short scalar rendering used for `[field=value]` matching and for printing.
pub fn scalar_str(v: &Value) -> Option<String> {
    use Value::*;
    Some(match v {
        Bool(b) => b.to_string(),
        I8(x) => x.to_string(),
        U8(x) => x.to_string(),
        I16(x) => x.to_string(),
        U16(x) => x.to_string(),
        I32(x) => x.to_string(),
        U32(x) => x.to_string(),
        I64(x) => x.to_string(),
        U64(x) => x.to_string(),
        F32(x) => x.to_string(),
        F64(x) => x.to_string(),
        CName(s) | Enum(s) => s.clone(),
        TweakDBID(t) => format!("{t:#x}"),
        Handle(h) => h.to_string(),
        Str(s) => String::from_utf8_lossy(s).into_owned(),
        _ => return None,
    })
}

/// `field` may be dotted (`ownerID.hash`). An empty `want` matches an absent (default) field.
fn element_matches(e: &Value, field: &str, want: &str) -> bool {
    let mut cur = e;
    for part in field.split('.') {
        let Value::Struct(s) = cur else { return false };
        match s.props.iter().find(|p| p.name == part) {
            Some(p) => cur = &p.value,
            None => return want.is_empty(),
        }
    }
    scalar_str(cur).map_or(false, |v| v == want)
}

fn select_index(arr: &[Value], sel: &Sel) -> Result<usize> {
    match sel {
        Sel::Index(i) => {
            ensure!(*i < arr.len(), "index {i} out of range (len {})", arr.len());
            Ok(*i)
        }
        Sel::Match(f, v) => {
            let hits: Vec<usize> = (0..arr.len()).filter(|&i| element_matches(&arr[i], f, v)).collect();
            match hits.len() {
                1 => Ok(hits[0]),
                0 => bail!("no array element with {f}={v}"),
                n => bail!("{n} array elements match {f}={v}; use an index"),
            }
        }
    }
}

fn select_chunk(pkg: &Package, seg: &Seg) -> Result<usize> {
    if let Some(i) = seg.name.strip_prefix('#') {
        let i: usize = i.parse()?;
        ensure!(i < pkg.chunks.len(), "chunk #{i} out of range");
        return Ok(i);
    }
    let all = pkg.find(&seg.name);
    ensure!(!all.is_empty(), "no chunk of type {}", seg.name);
    match &seg.sel {
        None => {
            ensure!(all.len() == 1, "{} chunks of type {}; use {}[n]", all.len(), seg.name, seg.name);
            Ok(all[0])
        }
        Some(Sel::Index(n)) => all.get(*n).copied().with_context(|| format!("only {} {} chunks", all.len(), seg.name)),
        Some(Sel::Match(f, v)) => {
            let hits: Vec<usize> = all
                .into_iter()
                .filter(|&i| element_matches(&Value::Struct(pkg.chunks[i].data.clone()), f, v))
                .collect();
            ensure!(hits.len() == 1, "{} {} chunks match {f}={v}", hits.len(), seg.name);
            Ok(hits[0])
        }
    }
}

/// Type name of the struct reached by `prop_ty` (stripping `array:`).
fn struct_type_of(prop_ty: &str, through_index: bool) -> String {
    if through_index {
        array_elem(prop_ty).unwrap_or(prop_ty).to_string()
    } else {
        prop_ty.to_string()
    }
}

/// Template for a field of a struct type, learned from the rest of the package:
/// (field type, sample value, canonical field order of that struct type).
fn field_template(pkg: &Package, struct_ty: &str, field: &str) -> Option<(String, Value, Vec<String>)> {
    fn visit(s: &Struct, sty: &str, want_ty: &str, field: &str, best: &mut Option<(String, Value, Vec<String>)>) {
        if sty == want_ty {
            if let Some(p) = s.props.iter().find(|p| p.name == field) {
                let order: Vec<String> = s.props.iter().map(|p| p.name.clone()).collect();
                if best.as_ref().map_or(true, |b| order.len() > b.2.len()) {
                    *best = Some((p.ty.clone(), p.value.clone(), order));
                }
            }
        }
        for p in &s.props {
            visit_val(&p.value, &p.ty, want_ty, field, best);
        }
    }
    fn visit_val(v: &Value, ty: &str, want_ty: &str, field: &str, best: &mut Option<(String, Value, Vec<String>)>) {
        match v {
            Value::Struct(s) => visit(s, ty, want_ty, field, best),
            Value::Array(a) => {
                let et = array_elem(ty).unwrap_or(ty);
                for e in a {
                    visit_val(e, et, want_ty, field, best);
                }
            }
            _ => {}
        }
    }
    let mut best = None;
    for c in &pkg.chunks {
        visit(&c.data, &c.ty, struct_ty, field, &mut best);
    }
    best
}

/// Parse `s` into a value of the same kind as `sample` (type name `ty`).
pub fn parse_like(sample: &Value, ty: &str, s: &str) -> Result<Value> {
    use Value::*;
    let bad = || format!("cannot parse {s:?} as {ty}");
    Ok(match sample {
        Bool(_) => Bool(match s {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => bail!(bad()),
        }),
        I8(_) => I8(s.parse().with_context(bad)?),
        U8(_) => U8(s.parse().with_context(bad)?),
        I16(_) => I16(s.parse().with_context(bad)?),
        U16(_) => U16(s.parse().with_context(bad)?),
        I32(_) => I32(s.parse().with_context(bad)?),
        U32(_) => U32(s.parse().with_context(bad)?),
        I64(_) => I64(s.parse().with_context(bad)?),
        U64(_) => U64(s.parse().with_context(bad)?),
        F32(_) => F32(s.parse().with_context(bad)?),
        F64(_) => F64(s.parse().with_context(bad)?),
        CName(_) => CName(s.to_string()),
        Enum(_) => Enum(s.to_string()),
        TweakDBID(_) => TweakDBID(parse_tweakdbid(s)?),
        Handle(_) => Handle(s.parse().with_context(bad)?),
        Str(_) => Str(s.as_bytes().to_vec()),
        Buffer(_) | Array(_) | Struct(_) => bail!("{ty} is not a scalar; set one of its fields instead"),
    })
}

/// A sample value for a primitive type name (used when inserting with an explicit `:Type`).
fn sample_for_type(ty: &str) -> Option<Value> {
    use Value::*;
    Some(match ty {
        "Bool" => Bool(false),
        "Int8" => I8(0),
        "Uint8" => U8(0),
        "Int16" => I16(0),
        "Uint16" => U16(0),
        "Int32" => I32(0),
        "Uint32" => U32(0),
        "Int64" => I64(0),
        "Uint64" | "CRUID" => U64(0),
        "Float" => F32(0.0),
        "Double" | "EngineTime" => F64(0.0),
        "CName" => CName(String::new()),
        "TweakDBID" => TweakDBID(0),
        "NodeRef" | "String" => Str(vec![]),
        t if t.starts_with("handle:") || t.starts_with("whandle:") => Handle(-1),
        _ => return None,
    })
}

/// TweakDBID from `0x...` or a record path such as `Items.Preset_Lexington_Default`.
pub fn parse_tweakdbid(s: &str) -> Result<u64> {
    if let Some(h) = s.strip_prefix("0x") {
        return Ok(u64::from_str_radix(h, 16)?);
    }
    ensure!(s.len() < 256, "TweakDB path too long");
    Ok(crc32fast::hash(s.as_bytes()) as u64 | ((s.len() as u64) << 32))
}

pub enum SetResult {
    Changed { old: Value },
    Inserted,
}

impl Package {
    /// Remove a struct field, making it fall back to its default value.
    /// Returns the removed value (None if it was already absent).
    pub fn remove_path(&mut self, path: &str) -> Result<Option<Value>> {
        let segs = parse_path(path)?;
        ensure!(segs.len() >= 2, "path must name a field inside a chunk");
        let last = segs.last().unwrap().clone();
        ensure!(last.sel.is_none(), "can only remove whole fields, not array elements");
        let ci = select_chunk(self, &segs[0])?;
        let mut cur: &mut Struct = &mut self.chunks[ci].data;
        for seg in &segs[1..segs.len() - 1] {
            let p = cur.props.iter_mut().find(|p| p.name == seg.name).with_context(|| format!("field {} absent", seg.name))?;
            let mut v = &mut p.value;
            if let Some(sel) = &seg.sel {
                let Value::Array(a) = v else { bail!("{} is not an array", seg.name) };
                let i = select_index(a, sel)?;
                v = &mut a[i];
            }
            let Value::Struct(s) = v else { bail!("{} is not a struct", seg.name) };
            cur = s;
        }
        Ok(cur.props.iter().position(|p| p.name == last.name).map(|i| cur.props.remove(i).value))
    }

    /// Read the value at `path` (None if the field is absent, i.e. default).
    pub fn get_path(&self, path: &str) -> Result<Option<&Value>> {
        let segs = parse_path(path)?;
        let ci = select_chunk(self, &segs[0])?;
        let mut cur: &Struct = &self.chunks[ci].data;
        for (k, seg) in segs.iter().enumerate().skip(1) {
            let last = k == segs.len() - 1;
            let Some(p) = cur.props.iter().find(|p| p.name == seg.name) else {
                ensure!(last && seg.sel.is_none(), "field {} absent (default)", seg.name);
                return Ok(None);
            };
            let mut v = &p.value;
            if let Some(sel) = &seg.sel {
                let Value::Array(a) = v else { bail!("{} is not an array", seg.name) };
                v = &a[select_index(a, sel)?];
            }
            if last {
                return Ok(Some(v));
            }
            let Value::Struct(s) = v else { bail!("{} is not a struct", seg.name) };
            cur = s;
        }
        bail!("path selects a whole chunk; add a field")
    }

    /// Set a scalar at `path` from its string form. Inserts the field if it is absent.
    pub fn set_path(&mut self, path: &str, value: &str) -> Result<SetResult> {
        let segs = parse_path(path)?;
        ensure!(segs.len() >= 2, "path must name a field inside a chunk");
        let ci = select_chunk(self, &segs[0])?;

        // Immutable pass: find the struct type that owns the final field (for insert templates).
        let mut owner_ty = self.chunks[ci].ty.clone();
        {
            let mut cur: &Struct = &self.chunks[ci].data;
            for seg in &segs[1..segs.len() - 1] {
                let p = cur.props.iter().find(|p| p.name == seg.name).with_context(|| {
                    format!("field {} absent (default); inserting whole structs is not supported", seg.name)
                })?;
                let mut v = &p.value;
                if let Some(sel) = &seg.sel {
                    let Value::Array(a) = v else { bail!("{} is not an array", seg.name) };
                    v = &a[select_index(a, sel)?];
                }
                let Value::Struct(s) = v else { bail!("{} is not a struct", seg.name) };
                owner_ty = struct_type_of(&p.ty, seg.sel.is_some());
                cur = s;
            }
        }
        let last = segs.last().unwrap().clone();
        let template = field_template(self, &owner_ty, &last.name);

        // Mutable pass.
        let mut cur: &mut Struct = &mut self.chunks[ci].data;
        for seg in &segs[1..segs.len() - 1] {
            let p = cur.props.iter_mut().find(|p| p.name == seg.name).unwrap();
            let mut v = &mut p.value;
            if let Some(sel) = &seg.sel {
                let Value::Array(a) = v else { unreachable!() };
                let i = select_index(a, sel)?;
                v = &mut a[i];
            }
            let Value::Struct(s) = v else { unreachable!() };
            cur = s;
        }

        if let Some(p) = cur.props.iter_mut().find(|p| p.name == last.name) {
            let (target, ty): (&mut Value, String) = match &last.sel {
                Some(sel) => {
                    let et = array_elem(&p.ty).context("not an array")?.to_string();
                    let Value::Array(a) = &mut p.value else { bail!("{} is not an array", last.name) };
                    let i = select_index(a, sel)?;
                    (&mut a[i], et)
                }
                None => (&mut p.value, p.ty.clone()),
            };
            let new = parse_like(target, &ty, value)?;
            let old = std::mem::replace(target, new);
            return Ok(SetResult::Changed { old });
        }

        // Insert an omitted field.
        ensure!(last.sel.is_none(), "array field {} is absent; inserting arrays is not supported", last.name);
        let (ty, sample, order) = match (&last.ty, template) {
            (Some(t), tpl) => {
                let sample = match &tpl {
                    Some((tt, s, _)) if tt == t => s.clone(),
                    // Not a primitive and nothing to copy from: a scalar of a named type is an enum.
                    _ => sample_for_type(t).unwrap_or(Value::Enum(String::new())),
                };
                (t.clone(), sample, tpl.map(|t| t.2).unwrap_or_default())
            }
            (None, Some(tpl)) => tpl,
            (None, None) => bail!(
                "field {} is absent and no other {owner_ty} in this package has it; give the type as {}:Type",
                last.name,
                last.name
            ),
        };
        let new = parse_like(&sample, &ty, value)?;
        let rank = |n: &str| order.iter().position(|o| o == n);
        let pos = match rank(&last.name) {
            Some(k) => cur.props.iter().position(|p| rank(&p.name).map_or(false, |r| r > k)).unwrap_or(cur.props.len()),
            None => cur.props.len(),
        };
        cur.props.insert(pos, Prop { name: last.name.clone(), ty, value: new });
        Ok(SetResult::Inserted)
    }
}
