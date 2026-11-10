//! Desktop shell: the UI (React + the wasm core) is identical to the website; this crate only
//! adds direct access to the game's save folder.
//!
//! The file commands are intentionally narrow:
//! - reads are limited to files inside a save folder (a directory containing `sav.dat`);
//! - writes only create a *new* save folder and new files inside folders created this session;
//!   nothing is ever overwritten or deleted.

use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::State;

#[derive(Default)]
struct Created(Mutex<HashSet<PathBuf>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveListing {
    folder: String,
    path: String,
    metadata: Option<String>,
    has_screenshot: bool,
}

fn is_metadata(name: &str) -> bool {
    name.starts_with("metadata") && name.ends_with(".json")
}

/// `%USERPROFILE%\Saved Games\CD Projekt Red\Cyberpunk 2077`
#[tauri::command]
fn default_saves_dir() -> Result<String, String> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map_err(|e| e.to_string())?;
    Ok(Path::new(&home)
        .join("Saved Games")
        .join("CD Projekt Red")
        .join("Cyberpunk 2077")
        .to_string_lossy()
        .replace('\\', "/"))
}

/// Save folders (subdirectories containing `sav.dat`) with their metadata text.
#[tauri::command]
fn list_saves(dir: String) -> Result<Vec<SaveListing>, String> {
    let mut out = Vec::new();
    for e in fs::read_dir(&dir).map_err(|e| format!("{dir}: {e}"))? {
        let Ok(e) = e else { continue };
        let p = e.path();
        if !p.join("sav.dat").is_file() {
            continue;
        }
        let names: Vec<String> = fs::read_dir(&p)
            .map(|rd| rd.filter_map(|x| x.ok()).map(|x| x.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        let metadata = names.iter().find(|n| is_metadata(n)).and_then(|n| fs::read_to_string(p.join(n)).ok());
        out.push(SaveListing {
            folder: e.file_name().to_string_lossy().into_owned(),
            path: p.to_string_lossy().replace('\\', "/"),
            metadata,
            has_screenshot: names.iter().any(|n| n.eq_ignore_ascii_case("screenshot.png")),
        });
    }
    Ok(out)
}

/// File names directly inside a save folder.
#[tauri::command]
fn list_files(dir: String) -> Result<Vec<String>, String> {
    let d = Path::new(&dir);
    if !d.join("sav.dat").is_file() {
        return Err(format!("{dir} is not a save folder"));
    }
    Ok(fs::read_dir(d)
        .map_err(|e| e.to_string())?
        .filter_map(|x| x.ok())
        .filter(|x| x.path().is_file())
        .map(|x| x.file_name().to_string_lossy().into_owned())
        .collect())
}

/// Raw bytes of a file inside a save folder (returned as an ArrayBuffer, not JSON).
#[tauri::command]
fn read_file(path: String) -> Result<Response, String> {
    let p = Path::new(&path);
    let parent = p.parent().ok_or("no parent directory")?;
    if !parent.join("sav.dat").is_file() {
        return Err(format!("{path} is not inside a save folder"));
    }
    fs::read(p).map(Response::new).map_err(|e| format!("{path}: {e}"))
}

fn check_name(s: &str) -> Result<(), String> {
    let ok = !s.is_empty()
        && s.len() <= 120
        && s.chars().all(|c| c.is_ascii_alphanumeric() || " _.-".contains(c))
        && s != "."
        && s != "..";
    if ok { Ok(()) } else { Err(format!("invalid name {s:?}")) }
}

/// Create `<parent>/<folder>`; fails if it already exists.
#[tauri::command]
fn create_save_dir(parent: String, folder: String, created: State<Created>) -> Result<String, String> {
    check_name(&folder)?;
    let dir = Path::new(&parent).join(&folder);
    fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    created.0.lock().unwrap().insert(dir.clone());
    Ok(dir.to_string_lossy().replace('\\', "/"))
}

/// Write a new file into a folder created by `create_save_dir` this session.
/// Body = raw bytes; headers `x-parent`, `x-folder`, `x-name` (URI-encoded).
#[tauri::command]
fn write_new_file(request: Request, created: State<Created>) -> Result<(), String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw bytes".into());
    };
    let header = |k: &str| -> Result<String, String> {
        let v = request.headers().get(k).ok_or(format!("missing {k}"))?.to_str().map_err(|e| e.to_string())?;
        Ok(percent_decode(v))
    };
    let (parent, folder, name) = (header("x-parent")?, header("x-folder")?, header("x-name")?);
    check_name(&name)?;
    let dir = Path::new(&parent).join(&folder);
    if !created.0.lock().unwrap().contains(&dir) {
        return Err("can only write into a save folder created by this app".into());
    }
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(&name))
        .map_err(|e| format!("{name}: {e}"))?;
    f.write_all(bytes).map_err(|e| e.to_string())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = hex {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Created::default())
        .invoke_handler(tauri::generate_handler![
            default_saves_dir,
            list_saves,
            list_files,
            read_file,
            create_save_dir,
            write_new_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running the save editor");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("C%3A%2FUsers%2Fx%20y"), "C:/Users/x y");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("bad%zz"), "bad%zz");
    }

    #[test]
    fn rejects_bad_names() {
        assert!(check_name("ManualSave-19").is_ok());
        assert!(check_name("..").is_err());
        assert!(check_name("a/b").is_err());
        assert!(check_name("a\\b").is_err());
        assert!(check_name("").is_err());
    }

    #[test]
    fn lists_and_guards_reads() {
        let t = tempfile::tempdir().unwrap();
        let save = t.path().join("ManualSave-1");
        fs::create_dir(&save).unwrap();
        fs::write(save.join("sav.dat"), b"x").unwrap();
        fs::write(save.join("metadata.9.json"), b"{}").unwrap();
        fs::create_dir(t.path().join("NotASave")).unwrap();
        let l = list_saves(t.path().to_string_lossy().into()).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].metadata.as_deref(), Some("{}"));
        assert!(read_file(save.join("sav.dat").to_string_lossy().into()).is_ok());
        assert!(read_file(t.path().join("NotASave").join("x").to_string_lossy().into()).is_err());
    }
}
