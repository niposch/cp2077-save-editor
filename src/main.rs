use anyhow::{bail, Context, Result};
use cp2077_save::{facts::parse_fact_key, package::{dump_package, PackageNode}, SaveFile};
use std::path::Path;
use std::fs;

fn load(p: &str) -> Result<SaveFile> {
    SaveFile::parse(&fs::read(p).with_context(|| format!("reading {p}"))?)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a = |i: usize| args.get(i).map(String::as_str).context("missing argument");
    match a(0)? {
        "info" => {
            let s = load(a(1)?)?;
            println!("{:#?}", s.header);
            println!("data_start={:#x} data_len={:#x} nodes={}", s.data_start, s.data.len(), s.nodes.len());
        }
        "nodes" => {
            let s = load(a(1)?)?;
            fn pr(s: &SaveFile, i: usize, d: usize) {
                let n = &s.nodes[i];
                println!("{:5} {}{} off={:#x} size={:#x}", i, "  ".repeat(d), n.name, n.offset, n.size);
                for c in s.children(i) { pr(s, c, d + 1); }
            }
            for r in s.roots() { pr(&s, r, 0); }
        }
        "dump" => {
            let s = load(a(1)?)?;
            fs::write(a(2)?, &s.data)?;
        }
        "node" => {
            let s = load(a(1)?)?;
            let i = s.find_node(a(2)?).context("no such node")?;
            fs::write(a(3)?, s.node_bytes(i))?;
        }
        "roundtrip" => {
            let raw = fs::read(a(1)?)?;
            let s = SaveFile::parse(&raw)?;
            let out = s.write()?;
            let s2 = SaveFile::parse(&out)?;
            println!("orig {} bytes, rewritten {} bytes, data equal: {}", raw.len(), out.len(), s.data == s2.data);
            if let Some(o) = args.get(2) { fs::write(o, out)?; }
        }
        "diff" => {
            // compare nodes by name+ordinal between two saves
            let x = load(a(1)?)?;
            let y = load(a(2)?)?;
            let key = |s: &SaveFile| -> Vec<String> {
                let mut seen = std::collections::HashMap::<String, usize>::new();
                s.nodes.iter().map(|n| { let c = seen.entry(n.name.clone()).or_default(); *c += 1; format!("{}#{}", n.name, c) }).collect()
            };
            let (kx, ky) = (key(&x), key(&y));
            let my: std::collections::HashMap<_, _> = ky.iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
            for (i, k) in kx.iter().enumerate() {
                match my.get(k) {
                    None => println!("only in A: {k}"),
                    Some(&j) => {
                        // compare leaf-ish: compare whole node bytes
                        let (bx, by) = (x.node_bytes(i), y.node_bytes(j));
                        if bx != by && x.nodes[i].child < 0 {
                            let nd = bx.iter().zip(by).filter(|(p, q)| p != q).count();
                            println!("DIFF {k}: size {:#x} -> {:#x}, {} differing bytes (aligned)", bx.len(), by.len(), nd);
                        }
                    }
                }
            }
            let mx: std::collections::HashSet<_> = kx.iter().collect();
            for k in &ky { if !mx.contains(k) { println!("only in B: {k}"); } }
        }
        "facts" => {
            let s = load(a(1)?)?;
            for (i, t) in s.facts_tables()?.iter().enumerate() {
                println!("table {i} (node {}): {} facts", t.node_idx, t.facts.len());
                if args.get(2).map(String::as_str) == Some("-v") {
                    for (h, v) in &t.facts { println!("  {h:#010x} = {v}"); }
                }
            }
        }
        "get-fact" => {
            let s = load(a(1)?)?;
            match s.get_fact(parse_fact_key(a(2)?)?)? {
                Some((t, v)) => println!("{} = {v} (table {t})", a(2)?),
                None => println!("{} not set (0)", a(2)?),
            }
        }
        // edit commands operate on save *folders*: <in_dir> <out_dir> ...
        "set-fact" | "remove-fact" | "unlock-reset" => {
            let (src, dst) = (Path::new(a(1)?), Path::new(a(2)?));
            let mut s = load(src.join("sav.dat").to_str().unwrap())?;
            match a(0)? {
                "set-fact" => s.set_fact(parse_fact_key(a(3)?)?, a(4)?.parse()?)?,
                "remove-fact" => { if !s.remove_fact(parse_fact_key(a(3)?)?)? { println!("fact was not present"); } }
                _ => {
                    let h = parse_fact_key("ResetAttributeDisabled")?;
                    match s.get_fact(h)? {
                        Some((_, v)) => println!("ResetAttributeDisabled = {v}, removing"),
                        None => println!("ResetAttributeDisabled not set; reset is already available"),
                    }
                    s.remove_fact(h)?;
                }
            }
            write_save_dir(src, dst, &s)?;
        }
        "pkg-dump" => {
            // pkg-dump <sav.dat> <NodeName> [ChunkType]
            let s = load(a(1)?)?;
            let i = s.find_node(a(2)?).context("no such node")?;
            let pn = PackageNode::parse(s.node_bytes(i))?;
            print!("{}", dump_package(&pn.package, args.get(3).map(String::as_str)));
        }
        "pkg-check" => {
            // parse + re-serialize every package node, require byte-identical output
            let s = load(a(1)?)?;
            let mut ok = true;
            for i in 0..s.nodes.len() {
                let b = s.node_bytes(i);
                if b.len() < 16 || b[8..10] != [4, 0] || !(b[10] == 6 || b[10] == 7) {
                    continue;
                }
                match PackageNode::parse(b) {
                    Ok(pn) => {
                        let w = pn.write()?;
                        let same = w == b;
                        ok &= same;
                        println!("{:40} chunks={:5} names={:5} roundtrip={}", s.nodes[i].name, pn.package.chunks.len(), pn.package.names.len(), if same { "identical" } else { "DIFFERENT" });
                    }
                    Err(e) => { ok = false; println!("{:40} PARSE ERROR: {e:#}", s.nodes[i].name); }
                }
            }
            if !ok { bail!("package check failed"); }
        }
        c => bail!("unknown command {c}"),
    }
    Ok(())
}

/// Write an edited save as a new save folder (copies screenshot, patches metadata fileSize).
fn write_save_dir(src: &Path, dst: &Path, s: &SaveFile) -> Result<()> {
    if dst.exists() {
        bail!("{} already exists; refusing to overwrite", dst.display());
    }
    let bytes = s.write()?;
    // sanity: re-parse what we wrote
    let check = SaveFile::parse(&bytes)?;
    anyhow::ensure!(check.data == s.data, "re-parse mismatch");
    fs::create_dir_all(dst)?;
    fs::write(dst.join("sav.dat"), &bytes)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name();
        let n = name.to_string_lossy();
        if n == "sav.dat" { continue; }
        if n.starts_with("metadata") && n.ends_with(".json") {
            let txt = fs::read_to_string(e.path())?;
            let mut out = String::new();
            for line in txt.split_inclusive('\n') {
                if let Some(i) = line.find("\"fileSize\":") {
                    let tail = if line.trim_end().ends_with(',') { "," } else { "" };
                    let eol = &line[line.trim_end().len()..];
                    out.push_str(&format!("{}\"fileSize\": {}{}{}", &line[..i], bytes.len(), tail, eol));
                } else {
                    out.push_str(line);
                }
            }
            fs::write(dst.join(&name), out)?;
        } else {
            fs::copy(e.path(), dst.join(&name))?;
        }
    }
    println!("wrote {} ({} bytes)", dst.display(), bytes.len());
    Ok(())
}
