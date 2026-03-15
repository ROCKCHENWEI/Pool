//! Swift FFI Bindings
//!
//! C-compatible functions for Swift interop on macOS.
//! All strings are passed as UTF-8 C strings (null-terminated).
//! Complex data types are serialized to JSON for transfer.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Create a new project and return it as a JSON string.
///
/// # Safety
/// The `name` parameter must be a valid null-terminated C string.
///
/// # Returns
/// A newly allocated C string containing the JSON representation of the project.
/// The caller must free this string using `pool_string_free`.
#[no_mangle]
pub extern "C" fn pool_project_create(name: *const c_char) -> *mut c_char {
    if name.is_null() {
        let error = r#"{"error": "name parameter is null"}"#;
        return CString::new(error).unwrap().into_raw();
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"error": "invalid UTF-8 in name"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let project = crate::models::Project::new(name_str.to_string());

    match serde_json::to_string(&project) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error": "failed to serialize project"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

/// Free a string returned by the library.
///
/// # Safety
/// The `s` parameter must be a pointer previously returned by one of the
/// library's allocation functions, or null. Passing any other pointer
/// results in undefined behavior.
#[no_mangle]
pub extern "C" fn pool_string_free(s: *mut c_char) {
    unsafe {
        if !s.is_null() {
            drop(CString::from_raw(s));
        }
    }
}

/// Get the library version.
///
/// # Returns
/// A newly allocated C string containing the version (e.g., "0.1.0").
/// The caller must free this string using `pool_string_free`.
#[no_mangle]
pub extern "C" fn pool_version() -> *mut c_char {
    CString::new(env!("CARGO_PKG_VERSION")).unwrap().into_raw()
}

/// Create a new shot for a project and return it as a JSON string.
///
/// # Safety
/// The `project_id` and `name` parameters must be valid null-terminated C strings.
///
/// # Returns
/// A newly allocated C string containing the JSON representation of the shot.
/// The caller must free this string using `pool_string_free`.
#[no_mangle]
pub extern "C" fn pool_shot_create(project_id: *const c_char, name: *const c_char) -> *mut c_char {
    if project_id.is_null() || name.is_null() {
        let error = r#"{"error": "parameter is null"}"#;
        return CString::new(error).unwrap().into_raw();
    }

    let project_id_str = match unsafe { CStr::from_ptr(project_id) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"error": "invalid UTF-8 in project_id"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"error": "invalid UTF-8 in name"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let shot = crate::models::Shot::new(name_str.to_string()).with_project(project_id_str.to_string());

    match serde_json::to_string(&shot) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error": "failed to serialize shot"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

/// Create a new workflow and return it as a JSON string.
///
/// # Safety
/// The `name` parameter must be a valid null-terminated C string.
///
/// # Returns
/// A newly allocated C string containing the JSON representation of the workflow.
/// The caller must free this string using `pool_string_free`.
#[no_mangle]
pub extern "C" fn pool_workflow_create(name: *const c_char) -> *mut c_char {
    if name.is_null() {
        let error = r#"{"error": "name parameter is null"}"#;
        return CString::new(error).unwrap().into_raw();
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"error": "invalid UTF-8 in name"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let workflow = crate::models::Workflow::new(name_str.to_string(), String::new());

    match serde_json::to_string(&workflow) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error": "failed to serialize workflow"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_pool_version() {
        let version = pool_version();
        let version_str = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
        assert!(!version_str.is_empty());
        pool_string_free(version);
    }

    #[test]
    fn test_pool_project_create() {
        let name = CString::new("Test Project").unwrap();
        let result = pool_project_create(name.as_ptr());
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(json.contains("Test Project"));
        assert!(json.contains("id"));
        pool_string_free(result);
    }
}
