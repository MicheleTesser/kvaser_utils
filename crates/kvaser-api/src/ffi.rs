use std::ffi::CStr;

use libloading::Library;

use crate::error::{KvaserError, Result};

pub(crate) fn load_library(
    label: &'static str,
    candidates: &'static [&'static str],
) -> Result<(Library, &'static str)> {
    let mut tried = Vec::with_capacity(candidates.len());
    let mut last_error = None;

    for candidate in candidates {
        tried.push(*candidate);
        match unsafe { Library::new(candidate) } {
            Ok(library) => return Ok((library, candidate)),
            Err(error) => last_error = Some(error),
        }
    }

    Err(KvaserError::LibraryLoad {
        library: label,
        tried: tried.join(", "),
        source: last_error.expect("at least one Kvaser library candidate"),
    })
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
