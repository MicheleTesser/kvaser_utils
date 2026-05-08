use std::collections::HashSet;
use std::env;
use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use libloading::Library;
use libloading::os::unix::Library as UnixLibrary;

use crate::error::{KvaserError, Result};

pub(crate) fn load_library(
    label: &'static str,
    candidates: &'static [&'static str],
) -> Result<(Library, &'static str)> {
    preload_support_libraries();

    let mut tried = Vec::new();
    let mut last_error = None;
    let mut seen = HashSet::new();

    for candidate in candidate_paths(candidates) {
        let display = candidate.display().to_string();
        if !seen.insert(display.clone()) {
            continue;
        }
        tried.push(display);
        match unsafe { Library::new(&candidate) } {
            Ok(library) => return Ok((library, label)),
            Err(error) => last_error = Some(error),
        }
    }

    Err(KvaserError::LibraryLoad {
        library: label,
        tried: tried.join(", "),
        source: last_error.expect("at least one Kvaser library candidate"),
    })
}

fn preload_support_libraries() {
    static PRELOADED: OnceLock<Vec<UnixLibrary>> = OnceLock::new();

    PRELOADED.get_or_init(|| {
        let support_candidates = [
            "libcanlib.so",
            "libcanlib.so.1",
            "libkvadblib.so",
            "libkvadblib.so.1",
            "libkvamemolibxml.so",
            "libkvamemolibxml.so.1",
            "libkvamemolib.so",
            "libkvamemolib.so.1",
            "libkvamemolib0700.so",
            "libkvamemolib0700.so.1",
            "libkvmlib.so",
            "libkvmlib.so.1",
            "libkvlclib.so",
            "libkvlclib.so.1",
        ];

        let mut loaded = Vec::new();
        let mut seen = HashSet::new();
        for dir in library_search_dirs() {
            for candidate in support_candidates {
                let path = dir.join(candidate);
                let display = path.display().to_string();
                if !path.is_file() || !seen.insert(display) {
                    continue;
                }

                if let Ok(library) =
                    unsafe { UnixLibrary::open(Some(&path), libc::RTLD_NOW | libc::RTLD_GLOBAL) }
                {
                    loaded.push(library);
                }
            }
        }

        loaded
    });
}

fn candidate_paths(candidates: &'static [&'static str]) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    for candidate in candidates {
        paths.push(PathBuf::from(candidate));
    }

    for dir in library_search_dirs() {
        for candidate in candidates {
            paths.push(dir.join(candidate));
        }
    }

    paths
}

fn library_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    extend_split_paths(&mut dirs, "KVASER_UTILS_LIB_DIR");
    extend_split_paths(&mut dirs, "KVASER_UTILS_LD_LIBRARY_PATH");

    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            push_dir(&mut dirs, dir);
            push_dir(&mut dirs, dir.join("../lib"));
            push_dir(&mut dirs, dir.join("../lib/kvaser_utils"));
            push_dir(&mut dirs, dir.join("../share/kvaser_utils/lib"));
        }
    }

    if let Ok(cwd) = env::current_dir() {
        push_dir(&mut dirs, cwd.join("third_party/kvaser/linuxcan/canlib"));
        push_dir(
            &mut dirs,
            cwd.join("third_party/kvaser/kvlibsdk/kvadblib/so.ndb"),
        );
        push_dir(
            &mut dirs,
            cwd.join("third_party/kvaser/kvlibsdk/kvamemolibxml/so.ndb"),
        );
        push_dir(
            &mut dirs,
            cwd.join("third_party/kvaser/kvlibsdk/kvmlib/kvmlib/so.ndb"),
        );
        push_dir(
            &mut dirs,
            cwd.join("third_party/kvaser/kvlibsdk/kvmlib/memolib_wrap/so.ndb"),
        );
        push_dir(
            &mut dirs,
            cwd.join("third_party/kvaser/kvlibsdk/kvmlib/memolib_fd/so.ndb"),
        );
        push_dir(&mut dirs, cwd.join("third_party/kvaser/kvlibsdk/kvlclib/so.ndb"));
    }

    push_dir(&mut dirs, "/usr/lib/kvaser_utils");
    push_dir(&mut dirs, "/usr/local/lib/kvaser_utils");
    dirs
}

fn extend_split_paths(dirs: &mut Vec<PathBuf>, key: &str) {
    if let Some(value) = env::var_os(key) {
        for path in env::split_paths(&value) {
            push_dir(dirs, path);
        }
    }
}

fn push_dir(dirs: &mut Vec<PathBuf>, dir: impl AsRef<Path>) {
    let path = dir.as_ref();
    if path.is_dir() {
        dirs.push(path.to_path_buf());
    }
}

pub(crate) unsafe fn load_symbol<T: Copy>(
    library: &Library,
    library_name: &'static str,
    symbol_name: &'static str,
) -> Result<T> {
    let symbol = unsafe { library.get::<T>(symbol_name.as_bytes()) }.map_err(|source| {
        KvaserError::SymbolLoad {
            library: library_name,
            symbol: symbol_name,
            source,
        }
    })?;
    Ok(*symbol)
}

pub(crate) fn nul_terminated_string(bytes: &[u8]) -> String {
    let nul = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..nul]).trim().to_owned()
}

pub(crate) unsafe fn c_buffer_to_string(buf: &[u8]) -> String {
    if buf.first().copied() == Some(0) {
        return String::new();
    }

    match CStr::from_bytes_until_nul(buf) {
        Ok(value) => value.to_string_lossy().trim().to_owned(),
        Err(_) => nul_terminated_string(buf),
    }
}
