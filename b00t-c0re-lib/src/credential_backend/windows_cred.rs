//! Windows Credential Manager backend.
//!
//! Uses direct Win32 API calls (`CredWriteW`, `CredReadW`, `CredDeleteW`)
//! instead of the `keyring` crate. This removes the cross-platform dependency
//! and gives direct control over Win32 credential types.
//!
//! # Safety
//! All Win32 API calls are `unsafe` but well-formed with correct pointer types.
//! `CredFree` is always called on read buffers to prevent memory leaks.
//! Null-pointer checks guard against unexpected API failures.

use anyhow::{Context, Result};

use super::CredentialBackend;

use windows::Win32::Foundation::{ERROR_NOT_FOUND, HRESULT};
use windows::Win32::Security::Credentials::*;

/// Windows Credential Manager backend.
pub struct WindowsCred;

impl CredentialBackend for WindowsCred {
    fn set_password(service: &str, user: &str, password: &str) -> Result<()> {
        // Build wide-char strings (UTF-16 with null terminator)
        let target_name = format!("{}/{}", service, user);
        let target_name_wide: Vec<u16> =
            target_name.encode_utf16().chain(std::iter::once(0)).collect();
        let user_wide: Vec<u16> =
            user.encode_utf16().chain(std::iter::once(0)).collect();
        let password_bytes: Vec<u8> = password.as_bytes().to_vec();

        // Build the CREDENTIALW struct with our data.
        // Pointers reference local Vecs — safe because CredWriteW copies internally.
        let cred = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target_name_wide.as_ptr() as *mut u16),
            Comment: PWSTR(std::ptr::null_mut()),
            LastWritten: Default::default(),
            CredentialBlobSize: password_bytes.len() as u32,
            CredentialBlob: password_bytes.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: PWSTR(std::ptr::null_mut()),
            UserName: PWSTR(user_wide.as_ptr() as *mut u16),
        };

        unsafe {
            CredWriteW(&cred as *const CREDENTIALW, 0)
                .context("CredWriteW failed")?;
        }

        Ok(())
    }

    fn get_password(service: &str, user: &str) -> Result<Option<String>> {
        let target_name = format!("{}/{}", service, user);
        let target_name_wide: Vec<u16> =
            target_name.encode_utf16().chain(std::iter::once(0)).collect();

        let mut cred_ptr: *mut CREDENTIALW = std::ptr::null_mut();

        unsafe {
            let result = CredReadW(
                PWSTR(target_name_wide.as_ptr() as *mut u16),
                CRED_TYPE_GENERIC,
                Some(0),
                &mut cred_ptr as *mut *mut CREDENTIALW,
            );

            match result {
                Ok(()) => {
                    // CredReadW succeeded — extract password from the allocated blob
                    let cred = &*cred_ptr;
                    let password = if cred.CredentialBlobSize > 0 {
                        let slice = std::slice::from_raw_parts(
                            cred.CredentialBlob as *const u8,
                            cred.CredentialBlobSize as usize,
                        );
                        Some(
                            String::from_utf8(slice.to_vec())
                                .context("credential blob is not valid UTF-8")?,
                        )
                    } else {
                        None
                    };
                    // Must free the buffer allocated by CredReadW
                    CredFree(cred_ptr as *const core::ffi::c_void);
                    Ok(password)
                }
                Err(e) => {
                    // Free if CredReadW partially allocated before failing
                    if !cred_ptr.is_null() {
                        CredFree(cred_ptr as *const core::ffi::c_void);
                    }
                    // ERROR_NOT_FOUND means no credential exists — return None, not an error
                    if e.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) {
                        Ok(None)
                    } else {
                        Err(e).context("CredReadW failed")
                    }
                }
            }
        }
    }

    fn delete_password(service: &str, user: &str) -> Result<()> {
        let target_name = format!("{}/{}", service, user);
        let target_name_wide: Vec<u16> =
            target_name.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            CredDeleteW(
                PWSTR(target_name_wide.as_ptr() as *mut u16),
                CRED_TYPE_GENERIC,
                Some(0),
            )
            .context("CredDeleteW failed")?;
        }

        Ok(())
    }
}
