use super::*;
use std::ptr;
use windows_sys::Win32::{
    Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND,
        ERROR_SUCCESS, FILETIME,
    },
    System::Registry::{
        RegCloseKey, RegEnumValueW, RegOpenKeyExW, RegQueryInfoKeyW, HKEY, HKEY_LOCAL_MACHINE,
        KEY_QUERY_VALUE, KEY_WOW64_64KEY, REG_OPTION_OPEN_LINK,
    },
};

struct RegistryKey(HKEY);
impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns a successful RegOpenKeyExW result.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

pub(super) fn read() -> Result<AdministrativePolicyState, AdministrativePolicyError> {
    let path: Vec<u16> = "Software\\Policies\\FBForge\\Suite"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut handle = 0;
    // SAFETY: fixed terminated path and valid output. Open only the machine
    // 64-bit view with query rights. OPEN_LINK avoids following a leaf link;
    // its SymbolicLinkValue/type is rejected by the strict schema.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            path.as_ptr(),
            REG_OPTION_OPEN_LINK,
            KEY_QUERY_VALUE | KEY_WOW64_64KEY,
            &mut handle,
        )
    };
    if matches!(status, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
        return decode_values(None);
    }
    if status != ERROR_SUCCESS || handle == 0 {
        return Err(AdministrativePolicyError::Unavailable);
    }
    let key = RegistryKey(handle);
    let before = stamp(&key)?;
    before.validate()?;
    let mut values = Vec::with_capacity(before.values as usize);
    // One extra bounded call detects entries appearing beyond the count limit.
    for index in 0..=MAX_VALUES as u32 {
        let mut name = [0u16; MAX_NAME_UNITS];
        let mut data = [0u8; MAX_DATA_BYTES];
        let mut name_len = name.len() as u32;
        let mut data_len = data.len() as u32;
        let mut kind = 0;
        // SAFETY: initialized fixed buffers, lengths in UTF-16 units/bytes as
        // required. Reserved is null and no registry-provided length allocates.
        let status = unsafe {
            RegEnumValueW(
                key.0,
                index,
                name.as_mut_ptr(),
                &mut name_len,
                ptr::null(),
                &mut kind,
                data.as_mut_ptr(),
                &mut data_len,
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status == ERROR_MORE_DATA {
            return Err(AdministrativePolicyError::InvalidPolicy);
        }
        if status != ERROR_SUCCESS {
            return Err(AdministrativePolicyError::Unavailable);
        }
        if values.len() == MAX_VALUES
            || name_len as usize >= name.len()
            || data_len as usize > data.len()
            || name[..name_len as usize].contains(&0)
        {
            return Err(AdministrativePolicyError::InvalidPolicy);
        }
        let name = String::from_utf16(&name[..name_len as usize])
            .map_err(|_| AdministrativePolicyError::InvalidPolicy)?;
        values.push(RegistryValue {
            name,
            kind,
            data: data[..data_len as usize].to_vec(),
        });
    }
    // Last-write/count/size equality detects observed edits. It is not a
    // transaction or an exclusion lock against a privileged concurrent writer.
    decode_snapshot(before, &values, stamp(&key)?)
}

fn stamp(key: &RegistryKey) -> Result<KeyStamp, AdministrativePolicyError> {
    let mut result = KeyStamp {
        subkeys: 0,
        values: 0,
        max_name: 0,
        max_data: 0,
        last_write: 0,
    };
    let mut time = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    // SAFETY: valid owned handle and output pointers. Optional unused outputs
    // and reserved are null. This operation never changes the registry.
    let status = unsafe {
        RegQueryInfoKeyW(
            key.0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
            &mut result.subkeys,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut result.values,
            &mut result.max_name,
            &mut result.max_data,
            ptr::null_mut(),
            &mut time,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(AdministrativePolicyError::Unavailable);
    }
    result.last_write = u64::from(time.dwLowDateTime) | (u64::from(time.dwHighDateTime) << 32);
    Ok(result)
}
