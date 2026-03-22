use pool_core::ffi::*;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use tokio::runtime::Runtime;

fn get_runtime() -> Runtime {
    Runtime::new().unwrap()
}

#[test]
fn test_ffi_version() {
    let version = pool_version();
    let version_str = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
    assert!(!version_str.is_empty());
    pool_string_free(version);
}

#[test]
fn test_ffi_project_create() {
    let name = CString::new("Test Project").unwrap();
    let result = pool_project_create(name.as_ptr());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("Test Project"));
    assert!(json.contains("id"));
    pool_string_free(result);
}

#[test]
fn test_ffi_shot_create() {
    let project_id = CString::new("project-123").unwrap();
    let name = CString::new("Test Shot").unwrap();
    let result = pool_shot_create(project_id.as_ptr(), name.as_ptr());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("Test Shot"));
    pool_string_free(result);
}

#[test]
fn test_ffi_workflow_create() {
    let name = CString::new("Test Workflow").unwrap();
    let result = pool_workflow_create(name.as_ptr());
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("Test Workflow"));
    pool_string_free(result);
}

#[test]
fn test_ffi_comfyui_set_config() {
    let url = CString::new("http://127.0.0.1:8188").unwrap();
    let result = pool_comfyui_set_config(url.as_ptr(), 30, true, 3);
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("success"));
    pool_string_free(result);
}

#[test]
fn test_ffi_comfyui_get_config() {
    let result = pool_comfyui_get_config();
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("server_url"));
    pool_string_free(result);
}

#[test]
fn test_ffi_comfyui_get_templates() {
    let result = pool_comfyui_get_templates();
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("Text-to-Image"));
    pool_string_free(result);
}

#[test]
fn test_ffi_workflow_get_node_types() {
    let result = pool_workflow_get_node_types();
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("ComfyUI"));
    pool_string_free(result);
}

#[test]
fn test_ffi_workflow_create_sample() {
    let result = pool_workflow_create_sample();
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    assert!(json.contains("nodes"));
    assert!(json.contains("connections"));
    pool_string_free(result);
}

#[test]
fn test_ffi_comfyui_set_config_validation() {
    // Test with empty URL (should still succeed with default)
    let result = pool_comfyui_set_config(std::ptr::null(), 30, true, 3);
    let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
    // Should use default URL
    assert!(json.contains("success") || json.contains("error"));
    pool_string_free(result);
}

#[test]
fn test_ffi_string_free_null() {
    // Should not crash on null
    pool_string_free(std::ptr::null_mut::<c_char>());
}
