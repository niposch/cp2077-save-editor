//! `metadata.9.json` helpers (pure string transforms, no I/O).

/// Replace the numeric value of `"key": <number>` lines in the game's pretty-printed JSON,
/// leaving everything else byte-for-byte intact.
pub fn patch_json_numbers(txt: &str, patches: &[(String, String)]) -> String {
    let mut out = String::with_capacity(txt.len());
    for line in txt.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let hit = patches.iter().find(|(k, _)| trimmed.starts_with(&format!("\"{k}\":")));
        match hit {
            Some((k, v)) => {
                let indent = &line[..line.len() - trimmed.len()];
                let body = line.trim_end();
                let comma = if body.ends_with(',') { "," } else { "" };
                let eol = &line[body.len()..];
                out.push_str(&format!("{indent}\"{k}\": {v}{comma}{eol}"));
            }
            None => out.push_str(line),
        }
    }
    out
}

/// Metadata patches for an edited save: new `fileSize` plus any extra numeric fields.
pub fn patch_for_save(txt: &str, file_size: usize, extra: &[(String, String)]) -> String {
    let mut p = vec![("fileSize".to_string(), file_size.to_string())];
    p.extend_from_slice(extra);
    patch_json_numbers(txt, &p)
}
