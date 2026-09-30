//! Embeds the server's half of the catalog: `/locales/<language>/server/*.ftl`.
//!
//! The web app reads `/locales/<language>/web/*.ftl` itself. Both halves are plain Fluent files,
//! so a translator works in one place and the build picks the files up; adding a language is
//! adding a directory, with no code change.

use std::{
    env,
    error::Error,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

const LOCALES_DIR: &str = "../../../locales";
const DEFAULT_LOCALE: &str = "en";

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join(LOCALES_DIR);
    let root = root
        .canonicalize()
        .map_err(|err| format!("the catalog is expected in {}: {err}", root.display()))?;
    // The directory catches added languages and files, each file an edit.
    println!("cargo:rerun-if-changed={LOCALES_DIR}");

    let mut languages = Vec::new();
    for entry in fs::read_dir(&root)? {
        let path = entry?.path();
        if path.is_dir() {
            let name = file_name(&path)?;
            println!("cargo:rerun-if-changed={LOCALES_DIR}/{name}/server");
            let files = ftl_files(&path.join("server"))?;
            // A language the web app has and the API has not (yet) is not an API language.
            if !files.is_empty() {
                languages.push((name, files));
            }
        }
    }
    languages.sort();
    if !languages.iter().any(|(name, _)| name == DEFAULT_LOCALE) {
        return Err(format!("{}/{DEFAULT_LOCALE} is missing", root.display()).into());
    }

    let mut code = String::from("pub(crate) const EMBEDDED: &[(&str, &[(&str, &str)])] = &[\n");
    for (language, files) in &languages {
        writeln!(code, "    ({language:?}, &[")?;
        for (name, path) in files {
            println!("cargo:rerun-if-changed={}", path.display());
            writeln!(
                code,
                "        ({name:?}, include_str!({:?})),",
                path.display().to_string()
            )?;
        }
        writeln!(code, "    ]),")?;
    }
    code.push_str("];\n");
    fs::write(PathBuf::from(env::var("OUT_DIR")?).join("catalog.rs"), code)?;
    Ok(())
}

fn ftl_files(dir: &Path) -> Result<Vec<(String, PathBuf)>, Box<dyn Error>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "ftl") {
                files.push((file_name(&path)?, path));
            }
        }
    }
    files.sort();
    Ok(files)
}

fn file_name(path: &Path) -> Result<String, Box<dyn Error>> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("{} is not a UTF-8 name", path.display()).into())
}
