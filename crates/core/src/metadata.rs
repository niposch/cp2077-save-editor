//! `metadata.9.json` helpers (pure string transforms, no I/O).

/// Replace the value of `"key": <value>` lines in the game's pretty-printed JSON with a raw
/// JSON token (`48.0`, `"Nomad"`), leaving everything else byte-for-byte intact.
pub fn patch_json_values(txt: &str, patches: &[(String, String)]) -> String {
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

/// Metadata patches for an edited save: new `fileSize` plus extra fields (raw JSON values).
pub fn patch_for_save(txt: &str, file_size: usize, extra: &[(String, String)]) -> String {
    let mut p = vec![("fileSize".to_string(), file_size.to_string())];
    p.extend_from_slice(extra);
    patch_json_values(txt, &p)
}

/// Quote a string as a JSON string token.
pub fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_numbers_and_strings_in_place() {
        let src = "{\n    \"level\": 48.0,\n    \"lifePath\": \"Corporate\",\n    \"fileSize\": 1\n}\n";
        let out = patch_for_save(src, 99, &[("level".into(), "50.0".into()), ("lifePath".into(), json_string("Nomad"))]);
        assert_eq!(out, "{\n    \"level\": 50.0,\n    \"lifePath\": \"Nomad\",\n    \"fileSize\": 99\n}\n");
    }

    #[test]
    fn quotes_json() {
        assert_eq!(json_string("a\"b\\c"), r#""a\"b\\c""#);
    }
}
