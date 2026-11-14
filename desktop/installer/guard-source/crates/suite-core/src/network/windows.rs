//! Synchronous WinHTTP calls run only on a native blocking worker. Microsoft:
//! https://learn.microsoft.com/en-us/windows/win32/api/winhttp/nf-winhttp-winhttpopen
//! https://learn.microsoft.com/en-us/windows/win32/winhttp/option-flags
//! https://learn.microsoft.com/en-us/windows/win32/api/winhttp/nf-winhttp-winhttpsettimeouts
use super::{
    check_operation, copy_body, finish_phase, phase_timeout_ms, AllowedHosts, CancellationToken,
    DownloadLimits, DownloadProgress, DownloadResult, HttpsUrl, NetworkConfig, NetworkError,
    ProxyMode,
};
use std::{
    ffi::c_void,
    io::Write,
    mem::size_of,
    ptr::{null, null_mut},
    time::Instant,
};
use windows_sys::Win32::{Foundation::GetLastError, Networking::WinHttp::*};

struct InternetHandle(*mut c_void);
impl InternetHandle {
    fn new(handle: *mut c_void) -> Result<Self, NetworkError> {
        if handle.is_null() {
            Err(last_error())
        } else {
            Ok(Self(handle))
        }
    }
}
impl Drop for InternetHandle {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns a live WinHTTP handle and only
        // closes it after all dependent requests/connections have been dropped.
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn option(handle: &InternetHandle, name: u32, value: u32) -> Result<(), NetworkError> {
    // SAFETY: WinHTTP consumes a DWORD by value during this synchronous call.
    if unsafe {
        WinHttpSetOption(
            handle.0,
            name,
            (&value as *const u32).cast(),
            size_of::<u32>() as u32,
        )
    } == 0
    {
        return Err(last_error());
    }
    Ok(())
}

fn apply_phase_timeouts(
    handle: &InternetHandle,
    cancel: &CancellationToken,
    deadline: Instant,
) -> Result<(), NetworkError> {
    check_operation(cancel, deadline)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    let resolve = phase_timeout_ms(remaining, 10_000)?;
    let connect = phase_timeout_ms(remaining, 15_000)?;
    let send = phase_timeout_ms(remaining, 15_000)?;
    let receive = phase_timeout_ms(remaining, 30_000)?;
    // SAFETY: session/request handle, strictly positive bounded millisecond
    // arguments. Internal PAC/multiphase work remains an OS runtime proof gap.
    let configured =
        if unsafe { WinHttpSetTimeouts(handle.0, resolve, connect, send, receive) } == 0 {
            Err(last_error())
        } else {
            Ok(())
        };
    finish_phase(configured, cancel, deadline)?;
    let response_timeout = option(
        handle,
        WINHTTP_OPTION_RECEIVE_RESPONSE_TIMEOUT,
        receive as u32,
    );
    finish_phase(response_timeout, cancel, deadline)
}

fn open_session(
    config: &NetworkConfig,
    cancel: &CancellationToken,
    deadline: Instant,
) -> Result<InternetHandle, NetworkError> {
    let agent = wide(concat!("FBForgeSuite/", env!("CARGO_PKG_VERSION")));
    let proxy = match config.proxy() {
        ProxyMode::System => None,
        ProxyMode::Explicit { server } => Some(wide(server)),
    };
    let access = if proxy.is_some() {
        WINHTTP_ACCESS_TYPE_NAMED_PROXY
    } else {
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
    };
    // SAFETY: all strings are terminated and remain alive until WinHTTP copies
    // them. Null bypass means no explicit bypass, not an empty wildcard list.
    // Flags=0 is deliberately synchronous: SECURE_DEFAULTS also enables ASYNC.
    let session = InternetHandle::new(unsafe {
        WinHttpOpen(
            agent.as_ptr(),
            access,
            proxy.as_ref().map_or(null(), |proxy| proxy.as_ptr()),
            null(),
            0,
        )
    })?;
    // System proxy/PAC decisions are left to Windows. We never reopen a failed
    // proxy request with NO_PROXY or add application-controlled bypass entries.
    // TLS1.2 is the explicit initial transport standard, supported by Windows10.
    // Do not use the TLS1.3 bit on unverified older WinHTTP implementations.
    option(
        &session,
        WINHTTP_OPTION_SECURE_PROTOCOLS,
        WINHTTP_FLAG_SECURE_PROTOCOL_TLS1_2,
    )?;
    option(&session, WINHTTP_OPTION_DISABLE_SECURE_PROTOCOL_FALLBACK, 1)?;
    option(&session, WINHTTP_OPTION_CONNECT_RETRIES, 1)?;
    // No IGNORE_CERT* / client certificate / credential / revocation override.
    // TLS certificate validation and enterprise roots use Windows defaults.
    // This transport does not promise an independent TLS revocation retrieval.
    apply_phase_timeouts(&session, cancel, deadline)?;
    Ok(session)
}

fn open_request(
    session: &InternetHandle,
    url: &HttpsUrl,
) -> Result<(InternetHandle, InternetHandle), NetworkError> {
    let host = wide(url.host());
    let target = wide(url.request_target());
    let verb = wide("GET");
    // SAFETY: terminated validated host, only HTTPS port443, no reserved flags.
    let connection =
        InternetHandle::new(unsafe { WinHttpConnect(session.0, host.as_ptr(), 443, 0) })?;
    // SAFETY: terminated validated path/query and method remain alive for the
    // call. No referrer, credentials, cookies or arbitrary caller headers.
    let request = InternetHandle::new(unsafe {
        WinHttpOpenRequest(
            connection.0,
            verb.as_ptr(),
            target.as_ptr(),
            null(),
            null(),
            null(),
            WINHTTP_FLAG_SECURE | WINHTTP_FLAG_ESCAPE_DISABLE | WINHTTP_FLAG_ESCAPE_DISABLE_QUERY,
        )
    })?;
    option(
        &request,
        WINHTTP_OPTION_DISABLE_FEATURE,
        WINHTTP_DISABLE_REDIRECTS | WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_AUTHENTICATION,
    )?;
    option(
        &request,
        WINHTTP_OPTION_AUTOLOGON_POLICY,
        WINHTTP_AUTOLOGON_SECURITY_LEVEL_HIGH,
    )?;
    option(&request, WINHTTP_OPTION_MAX_RESPONSE_HEADER_SIZE, 65_536)?;
    // Tuple order is intentional: download binds connection before request, so
    // Rust drops request first. No handle escapes this synchronous operation.
    Ok((connection, request))
}

pub(super) fn download(
    config: &NetworkConfig,
    url: &HttpsUrl,
    allowed: &AllowedHosts,
    limits: DownloadLimits,
    cancel: &CancellationToken,
    output: &mut impl Write,
    progress: impl FnMut(DownloadProgress),
) -> Result<DownloadResult, NetworkError> {
    let deadline = Instant::now()
        .checked_add(limits.timeout)
        .ok_or(NetworkError::InvalidConfiguration)?;
    check_operation(cancel, deadline)?;
    let opened = open_session(config, cancel, deadline);
    let session = finish_phase(opened, cancel, deadline)?;
    let mut current = url.clone();
    let mut visited = vec![current.clone()];
    for redirects in 0..=5 {
        check_operation(cancel, deadline)?;
        let opened = open_request(&session, &current);
        let (_connection, request) = finish_phase(opened, cancel, deadline)?;
        apply_phase_timeouts(&request, cancel, deadline)?;
        // SAFETY: live synchronous request, no request body/additional headers.
        let sent = if unsafe { WinHttpSendRequest(request.0, null(), 0, null(), 0, 0, 0) } == 0 {
            Err(last_error())
        } else {
            Ok(())
        };
        finish_phase(sent, cancel, deadline)?;
        apply_phase_timeouts(&request, cancel, deadline)?;
        // SAFETY: live sent synchronous request, reserved pointer is null.
        let received = if unsafe { WinHttpReceiveResponse(request.0, null_mut()) } == 0 {
            Err(last_error())
        } else {
            Ok(())
        };
        finish_phase(received, cancel, deadline)?;
        let queried = status_code(&request);
        let status = finish_phase(queried, cancel, deadline)?;
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            let queried = single_header(&request, WINHTTP_QUERY_LOCATION, 32_768);
            let location =
                finish_phase(queried, cancel, deadline)?.ok_or(NetworkError::InvalidResponse)?;
            let next = allowed.redirect(&current, &location, redirects)?;
            if visited.contains(&next) {
                return Err(NetworkError::RedirectRejected);
            }
            visited.push(next.clone());
            current = next;
            continue;
        }
        match status {
            200 => {}
            407 => return Err(NetworkError::ProxyAuthenticationRequired),
            401 => return Err(NetworkError::AuthenticationRequired),
            _ => return Err(NetworkError::HttpStatus(status)),
        }
        let queried = content_length(&request);
        let total = finish_phase(queried, cancel, deadline)?;
        return copy_body(
            |buffer| {
                apply_phase_timeouts(&request, cancel, deadline)?;
                let mut read = 0;
                // SAFETY: writable bounded buffer and DWORD output remain valid
                // for the synchronous read. Buffer length fits a DWORD.
                if unsafe {
                    WinHttpReadData(
                        request.0,
                        buffer.as_mut_ptr().cast(),
                        buffer.len() as u32,
                        &mut read,
                    )
                } == 0
                {
                    return Err(last_error());
                }
                Ok(read as usize)
            },
            output,
            limits.max_bytes,
            total,
            cancel,
            deadline,
            progress,
        );
    }
    Err(NetworkError::RedirectLimit)
}

fn status_code(request: &InternetHandle) -> Result<u16, NetworkError> {
    let mut status = 0u32;
    let mut length = size_of::<u32>() as u32;
    // SAFETY: live response handle and correctly sized DWORD output.
    if unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            (&mut status as *mut u32).cast(),
            &mut length,
            null_mut(),
        )
    } == 0
    {
        return Err(last_error());
    }
    if length != size_of::<u32>() as u32 || !(100..=599).contains(&status) {
        return Err(NetworkError::InvalidResponse);
    }
    Ok(status as u16)
}

fn single_header(
    request: &InternetHandle,
    field: u32,
    max_chars: usize,
) -> Result<Option<String>, NetworkError> {
    let mut buffer = vec![0u16; max_chars + 1];
    let mut length = (buffer.len() * size_of::<u16>()) as u32;
    let mut index = 0;
    // SAFETY: bounded UTF16 output and initialized length/index, valid handle.
    if unsafe {
        WinHttpQueryHeaders(
            request.0,
            field,
            null(),
            buffer.as_mut_ptr().cast(),
            &mut length,
            &mut index,
        )
    } == 0
    {
        // SAFETY: read thread-local error immediately after the failed call.
        return if unsafe { GetLastError() } == ERROR_WINHTTP_HEADER_NOT_FOUND {
            Ok(None)
        } else {
            Err(NetworkError::InvalidResponse)
        };
    }
    if !length.is_multiple_of(2) || length as usize > max_chars * 2 {
        return Err(NetworkError::InvalidResponse);
    }
    let value = String::from_utf16(&buffer[..length as usize / 2])
        .map_err(|_| NetworkError::InvalidResponse)?;
    if value.contains('\0') {
        return Err(NetworkError::InvalidResponse);
    }
    // Security-sensitive singleton headers must not be selected from duplicates.
    length = (buffer.len() * size_of::<u16>()) as u32;
    // SAFETY: same bounded output, index advanced by the first query.
    let duplicate = unsafe {
        WinHttpQueryHeaders(
            request.0,
            field,
            null(),
            buffer.as_mut_ptr().cast(),
            &mut length,
            &mut index,
        )
    };
    // SAFETY: the error is consumed only if the preceding call failed.
    if duplicate != 0 || unsafe { GetLastError() } != ERROR_WINHTTP_HEADER_NOT_FOUND {
        return Err(NetworkError::InvalidResponse);
    }
    Ok(Some(value))
}

fn content_length(request: &InternetHandle) -> Result<Option<u64>, NetworkError> {
    let Some(raw) = single_header(request, WINHTTP_QUERY_CONTENT_LENGTH, 32)? else {
        return Ok(None);
    };
    let length = raw.trim_matches([' ', '\t']);
    if length.is_empty()
        || !length.bytes().all(|byte| byte.is_ascii_digit())
        || single_header(request, WINHTTP_QUERY_TRANSFER_ENCODING, 128)?.is_some()
    {
        return Err(NetworkError::InvalidResponse);
    }
    length
        .parse()
        .map(Some)
        .map_err(|_| NetworkError::InvalidResponse)
}

fn last_error() -> NetworkError {
    // SAFETY: reads the most recent failed WinHTTP call on the current thread.
    match unsafe { GetLastError() } {
        ERROR_WINHTTP_TIMEOUT => NetworkError::Timeout,
        ERROR_WINHTTP_OPERATION_CANCELLED => NetworkError::Cancelled,
        ERROR_WINHTTP_NAME_NOT_RESOLVED
        | ERROR_WINHTTP_CANNOT_CONNECT
        | ERROR_WINHTTP_CONNECTION_ERROR => NetworkError::Offline,
        ERROR_WINHTTP_AUTODETECTION_FAILED
        | ERROR_WINHTTP_AUTO_PROXY_SERVICE_ERROR
        | ERROR_WINHTTP_BAD_AUTO_PROXY_SCRIPT
        | ERROR_WINHTTP_SCRIPT_EXECUTION_ERROR
        | ERROR_WINHTTP_UNABLE_TO_DOWNLOAD_SCRIPT
        | ERROR_WINHTTP_UNHANDLED_SCRIPT_TYPE => NetworkError::ProxyUnavailable,
        ERROR_WINHTTP_LOGIN_FAILURE => NetworkError::AuthenticationRequired,
        ERROR_WINHTTP_SECURE_CERT_CN_INVALID
        | ERROR_WINHTTP_SECURE_CERT_DATE_INVALID
        | ERROR_WINHTTP_SECURE_CERT_REVOKED
        | ERROR_WINHTTP_SECURE_CERT_WRONG_USAGE
        | ERROR_WINHTTP_SECURE_INVALID_CA
        | ERROR_WINHTTP_SECURE_INVALID_CERT => NetworkError::TlsRejected,
        ERROR_WINHTTP_SECURE_CERT_REV_FAILED
        | ERROR_WINHTTP_SECURE_FAILURE
        | ERROR_WINHTTP_SECURE_FAILURE_PROXY
        | ERROR_WINHTTP_SECURE_CHANNEL_ERROR => NetworkError::TlsValidationUnavailable,
        ERROR_WINHTTP_INVALID_SERVER_RESPONSE
        | ERROR_WINHTTP_INVALID_HEADER
        | ERROR_WINHTTP_HEADER_SIZE_OVERFLOW
        | ERROR_WINHTTP_HEADER_COUNT_EXCEEDED
        | ERROR_WINHTTP_RESPONSE_DRAIN_OVERFLOW
        | ERROR_WINHTTP_CHUNKED_ENCODING_HEADER_SIZE_OVERFLOW => NetworkError::InvalidResponse,
        _ => NetworkError::Unavailable,
    }
}
