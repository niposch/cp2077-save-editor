use anyhow::{bail, Context, Result};
use cp2077_save::{
    edit::{scalar_str, SetResult},
    facts::parse_fact_key,
    package::{dump_package, dump_value, PackageNode},
    player::{resolve_attribute, resolve_point_kind},
    SaveFile,
};
mod savedir;
use savedir::{load, write_save_dir};
use std::fs;
use std::path::Path;

const USAGE: &str = "\
cp2077-save <command> ...      (<save> = save folder or sav.dat)

inspect:
  info <save>                          header + sizes
  nodes <save>                         node tree
  node <save> <NodeName> <out.bin>     raw node bytes
  dump <save> <out.bin>                whole decompressed stream
  diff <saveA> <saveB>                 leaf nodes that differ
  roundtrip <save> [out]               decompress/recompress check
  facts <save> [-v] | get-fact <save> <name|0xHASH>
  player <save>                        level, attributes, points
  pkg-dump <save> <Node> [ChunkType]   readable object-package dump
  pkg-get <save> <Node> <path>         one value (see docs/editing.md for paths)
  pkg-check <save>                     parse+rewrite every package, require identical bytes

edit (writes a NEW save folder, never touches the input):
  edit <in_dir> <out_dir> [ops...]
    --attr <body|reflexes|tech|intelligence|cool>=<3..20>
    --points <attribute|perk|relic>=<n>    unspent points
    --unlock-reset                         allow \"Reset Attributes\" again
    --fact <name|0xHASH>=<n>   --remove-fact <name|0xHASH>
    --set <Node>:<path>=<value>            generic package edit
  set-fact / remove-fact / unlock-reset <in_dir> <out_dir> ...   (single-op shortcuts)
";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        print!("{USAGE}");
        return Ok(());
    }
    let a = |i: usize| args.get(i).map(String::as_str).context("missing argument (see --help)");
    let ld = |i: usize| -> Result<SaveFile> { load(Path::new(a(i)?)) };
    match a(0)? {
        "info" => {
            let s = ld(1)?;
            println!("{:#?}", s.header);
            println!("data_start={:#x} data_len={:#x} nodes={}", s.data_start, s.data.len(), s.nodes.len());
        }
        "nodes" => {
            let s = ld(1)?;
            fn pr(s: &SaveFile, i: usize, d: usize) {
                let n = &s.nodes[i];
                println!("{:5} {}{} off={:#x} size={:#x}", i, "  ".repeat(d), n.name, n.offset, n.size);
                for c in s.children(i) {
                    pr(s, c, d + 1);
                }
            }
            for r in s.roots() {
                pr(&s, r, 0);
            }
        }
        "dump" => fs::write(a(2)?, &ld(1)?.data)?,
        "node" => {
            let s = ld(1)?;
            let i = s.find_node(a(2)?).context("no such node")?;
            fs::write(a(3)?, s.node_bytes(i))?;
        }
        "roundtrip" => {
            let s = ld(1)?;
            let out = s.write()?;
            let s2 = SaveFile::parse(&out)?;
            println!("rewritten {} bytes, data equal: {}", out.len(), s.data == s2.data);
            if let Some(o) = args.get(2) {
                fs::write(o, out)?;
            }
        }
        "diff" => diff(&ld(1)?, &ld(2)?),
        "facts" => {
            let s = ld(1)?;
            for (i, t) in s.facts_tables()?.iter().enumerate() {
                println!("table {i} (node {}): {} facts", t.node_idx, t.facts.len());
                if args.get(2).map(String::as_str) == Some("-v") {
                    for (h, v) in &t.facts {
                        println!("  {h:#010x} = {v}");
                    }
                }
            }
        }
        "get-fact" => {
            let s = ld(1)?;
            match s.get_fact(parse_fact_key(a(2)?)?)? {
                Some((t, v)) => println!("{} = {v} (table {t})", a(2)?),
                None => println!("{} not set (0)", a(2)?),
            }
        }
        "player" => {
            let s = ld(1)?;
            let p = s.player_summary()?;
            let o = |v: Option<i64>| v.map_or("-".to_string(), |x| x.to_string());
            println!("level {}   street cred {}", o(p.level), o(p.street_cred));
            println!("attributes (PlayerDevelopmentData / StatsSystem modifier):");
            for (n, v, m) in &p.attributes {
                println!("  {n:18} {:>3} / {}", o(*v), m.map_or("-".into(), |x| x.to_string()));
            }
            println!("points (spent / unspent):");
            for (n, sp, un) in &p.points {
                println!("  {n:26} {:>4} / {}", o(*sp), o(*un));
            }
            let reset = s.get_fact(parse_fact_key("ResetAttributeDisabled")?)?;
            println!("attribute reset: {}", if reset.map_or(false, |r| r.1 != 0) { "USED" } else { "available" });
        }
        "pkg-dump" => {
            let s = ld(1)?;
            let pn = s.package(a(2)?)?;
            print!("{}", dump_package(&pn.package, args.get(3).map(String::as_str)));
        }
        "pkg-get" => {
            let s = ld(1)?;
            let pn = s.package(a(2)?)?;
            match pn.package.get_path(a(3)?)? {
                None => println!("(absent: default value)"),
                Some(v) => {
                    let mut out = String::new();
                    dump_value(&mut out, v, 0);
                    println!("{out}");
                }
            }
        }
        "pkg-check" => pkg_check(&ld(1)?)?,
        "edit" => {
            let ops: Vec<&str> = args[3..].iter().map(String::as_str).collect();
            edit(Path::new(a(1)?), Path::new(a(2)?), &ops)?;
        }
        // single-op shortcuts kept for compatibility
        "set-fact" => {
            let op = format!("{}={}", a(3)?, a(4)?);
            edit(Path::new(a(1)?), Path::new(a(2)?), &["--fact", &op])?;
        }
        "remove-fact" => edit(Path::new(a(1)?), Path::new(a(2)?), &["--remove-fact", a(3)?])?,
        "unlock-reset" => edit(Path::new(a(1)?), Path::new(a(2)?), &["--unlock-reset"])?,
        c => bail!("unknown command {c} (see --help)"),
    }
    Ok(())
}

fn split_kv(s: &str) -> Result<(&str, &str)> {
    s.rsplit_once('=').with_context(|| format!("expected key=value, got {s:?}"))
}

fn edit(src: &Path, dst: &Path, ops: &[&str]) -> Result<()> {
    if ops.is_empty() {
        bail!("no edit operations given (see --help)");
    }
    let mut s = load(src)?;
    let mut meta: Vec<(String, String)> = Vec::new();
    let mut it = ops.iter();
    while let Some(&op) = it.next() {
        let mut arg = || it.next().copied().with_context(|| format!("{op} needs an argument"));
        match op {
            "--attr" => {
                let (k, v) = split_kv(arg()?)?;
                let (stat, meta_key) = resolve_attribute(k)?;
                let v: i32 = v.parse().with_context(|| format!("bad number {v:?}"))?;
                let r = s.set_attribute(stat, v)?;
                if let Some(w) = &r.warning {
                    eprintln!("warning: {w}");
                }
                println!("{stat}: {} -> {v}", r.old.map_or("default".into(), |o| o.to_string()));
                meta.push((meta_key.to_string(), format!("{v}.0")));
            }
            "--points" => {
                let (k, v) = split_kv(arg()?)?;
                let ty = resolve_point_kind(k)?;
                let v: i32 = v.parse().with_context(|| format!("bad number {v:?}"))?;
                let old = s.set_unspent_points(ty, v)?;
                println!("{k} points (unspent): {} -> {v}", old.map_or("0".into(), |o| o.to_string()));
            }
            "--unlock-reset" => {
                let h = parse_fact_key("ResetAttributeDisabled")?;
                match s.get_fact(h)? {
                    Some((_, v)) => println!("ResetAttributeDisabled = {v}, removing"),
                    None => println!("ResetAttributeDisabled not set; reset already available"),
                }
                s.remove_fact(h)?;
            }
            "--fact" => {
                let (k, v) = split_kv(arg()?)?;
                s.set_fact(parse_fact_key(k)?, v.parse()?)?;
                println!("fact {k} = {v}");
            }
            "--remove-fact" => {
                let k = arg()?;
                let removed = s.remove_fact(parse_fact_key(k)?)?;
                println!("fact {k}: {}", if removed { "removed" } else { "was not set" });
            }
            "--set" => {
                let spec = arg()?;
                let (node, rest) = spec.split_once(':').context("--set expects <Node>:<path>=<value>")?;
                let (path, value) = split_kv(rest)?;
                let r = s.edit_package(node, |p| p.set_path(path, value))?;
                match r {
                    SetResult::Changed { old } => {
                        println!("{node}:{path}: {} -> {value}", scalar_str(&old).unwrap_or_else(|| "?".into()))
                    }
                    SetResult::Inserted => println!("{node}:{path}: (default) -> {value} [inserted]"),
                }
            }
            other => bail!("unknown edit op {other:?} (see --help)"),
        }
    }
    write_save_dir(src, dst, &s, &meta)
}

fn pkg_check(s: &SaveFile) -> Result<()> {
    let mut ok = true;
    for i in 0..s.nodes.len() {
        let b = s.node_bytes(i);
        if b.len() < 16 || b[8..10] != [4, 0] || !(b[10] == 6 || b[10] == 7) {
            continue;
        }
        match PackageNode::parse(b) {
            Ok(pn) => {
                let same = pn.write()? == b;
                ok &= same;
                println!(
                    "{:40} chunks={:5} names={:5} roundtrip={}",
                    s.nodes[i].name,
                    pn.package.chunks.len(),
                    pn.package.names.len(),
                    if same { "identical" } else { "DIFFERENT" }
                );
            }
            Err(e) => {
                ok = false;
                println!("{:40} PARSE ERROR: {e:#}", s.nodes[i].name);
            }
        }
    }
    if !ok {
        bail!("package check failed");
    }
    Ok(())
}

fn diff(x: &SaveFile, y: &SaveFile) {
    use std::collections::{HashMap, HashSet};
    let key = |s: &SaveFile| -> Vec<String> {
        let mut seen = HashMap::<String, usize>::new();
        s.nodes
            .iter()
            .map(|n| {
                let c = seen.entry(n.name.clone()).or_default();
                *c += 1;
                format!("{}#{}", n.name, c)
            })
            .collect()
    };
    let (kx, ky) = (key(x), key(y));
    let my: HashMap<_, _> = ky.iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
    for (i, k) in kx.iter().enumerate() {
        match my.get(k) {
            None => println!("only in A: {k}"),
            Some(&j) => {
                let (bx, by) = (x.node_bytes(i), y.node_bytes(j));
                if bx != by && x.nodes[i].child < 0 {
                    let nd = bx.iter().zip(by).filter(|(p, q)| p != q).count();
                    println!("DIFF {k}: size {:#x} -> {:#x}, {} differing bytes (aligned)", bx.len(), by.len(), nd);
                }
            }
        }
    }
    let mx: HashSet<_> = kx.iter().collect();
    for k in &ky {
        if !mx.contains(k) {
            println!("only in B: {k}");
        }
    }
}
