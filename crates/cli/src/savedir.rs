//! Writing an edited save as a new save folder.

use cp2077_save::{metadata::patch_for_save, SaveFile};
use anyhow::{bail, ensure, Result};
use std::fs;
use std::path::Path;

/// Load `sav.dat` from a save folder or a direct file path.
pub fn load(path: &Path) -> Result<SaveFile> {
    let p = if path.is_dir() { path.join("sav.dat") } else { path.to_path_buf() };
    let bytes = fs::read(&p).map_err(|e| anyhow::anyhow!("reading {}: {e}", p.display()))?;
    SaveFile::parse(&bytes)
}

/// Write `s` as a new save folder `dst`, copying the other files from `src`.
/// `metadata.*.json` gets `fileSize` updated plus any extra numeric `meta_patches`.
pub fn write_save_dir(src: &Path, dst: &Path, s: &SaveFile, meta_patches: &[(String, String)]) -> Result<()> {
    if dst.exists() {
        bail!("{} already exists; refusing to overwrite", dst.display());
    }
    ensure!(src.is_dir(), "{} is not a save folder", src.display());
    let bytes = s.write()?;
    let check = SaveFile::parse(&bytes)?;
    ensure!(check.data == s.data, "re-parse mismatch");
    fs::create_dir_all(dst)?;
    fs::write(dst.join("sav.dat"), &bytes)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name();
        let n = name.to_string_lossy();
        if n == "sav.dat" {
            continue;
        }
        if n.starts_with("metadata") && n.ends_with(".json") {
            let txt = fs::read_to_string(e.path())?;
            fs::write(dst.join(&name), patch_for_save(&txt, bytes.len(), meta_patches))?;
        } else {
            fs::copy(e.path(), dst.join(&name))?;
        }
    }
    println!("wrote {} ({} bytes)", dst.display(), bytes.len());
    Ok(())
}
