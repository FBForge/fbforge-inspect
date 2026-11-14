//! Development-only Unicode NSIS ABI host, not an installer or installation test.
#[cfg(not(windows))]
fn main() {
    eprintln!("The NSIS ABI fixture requires Windows.");
    std::process::exit(2);
}
#[cfg(windows)]
fn main() {
    windows::run();
}

#[cfg(windows)]
mod windows {
    use fbforge_suite_coordination::{Acquisition, AppIdentity, Lease};
    use std::{
        mem::{size_of, transmute},
        path::PathBuf,
        ptr,
        sync::atomic::{AtomicUsize, Ordering},
    };
    use windows_sys::Win32::{
        Foundation::{FreeLibrary, GlobalFree, HMODULE, HWND},
        System::{
            LibraryLoader::{
                GetModuleHandleExW, GetProcAddress, LoadLibraryExW,
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, LOAD_WITH_ALTERED_SEARCH_PATH,
            },
            Memory::{GlobalAlloc, GPTR},
        },
    };
    const CAPACITY: usize = 1024;
    // Independently declared from the pinned NSIS SDK, not imported from the DLL.
    #[repr(C)]
    struct Stack {
        next: *mut Stack,
        text: [u16; 1],
    }
    #[repr(C)]
    struct Flags {
        values: [i32; 14],
    }
    type Callback = unsafe extern "C" fn(i32) -> usize;
    #[repr(C)]
    struct Extra {
        flags: *mut Flags,
        execute: Option<unsafe extern "system" fn(i32, HWND) -> i32>,
        validate: Option<unsafe extern "system" fn(*mut u16)>,
        register: Option<unsafe extern "system" fn(HMODULE, Callback) -> i32>,
    }
    type Export = unsafe extern "C" fn(HWND, i32, *mut u16, *mut *mut Stack, *mut Extra);
    static CALLBACK: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "system" fn register(_: HMODULE, callback: Callback) -> i32 {
        CALLBACK.store(callback as usize, Ordering::Release);
        0
    }
    fn call(function: Export, argument: Option<&str>, version: i32) -> (String, i32) {
        let argument = argument.map(|s| s.encode_utf16().collect::<Vec<_>>());
        call_raw(function, argument.as_deref(), version)
    }
    fn call_raw(function: Export, argument: Option<&[u16]>, version: i32) -> (String, i32) {
        let mut flags = Flags { values: [0; 14] };
        flags.values[7] = version;
        let mut extra = Extra {
            flags: &mut flags,
            execute: None,
            validate: None,
            register: Some(register),
        };
        let mut top = ptr::null_mut();
        if let Some(argument) = argument {
            assert!(argument.len() < CAPACITY);
            // SAFETY: a private SDK-style fixed global allocation, freed by DLL pop.
            let node =
                unsafe { GlobalAlloc(GPTR, size_of::<Stack>() + CAPACITY * 2) }.cast::<Stack>();
            assert!(!node.is_null());
            let chars = argument.iter().copied().chain(Some(0));
            // SAFETY: fixture arguments are bounded well below the allocated capacity.
            let text = unsafe { ptr::addr_of_mut!((*node).text).cast::<u16>() };
            for (index, character) in chars.enumerate() {
                unsafe { *text.add(index) = character };
            }
            top = node;
        }
        // SAFETY: independent SDK layout, live stack/flags, declared cdecl prototype.
        unsafe { function(0, CAPACITY as i32, ptr::null_mut(), &mut top, &mut extra) };
        assert!(
            !top.is_null(),
            "missing reply must never be treated as a grant"
        );
        let reply_node = top;
        let mut text = Vec::new();
        // SAFETY: NSIS returns exactly one valid, null-terminated global stack node.
        let characters = unsafe { ptr::addr_of!((*reply_node).text).cast::<u16>() };
        for index in 0..CAPACITY {
            let character = unsafe { *characters.add(index) };
            if character == 0 {
                break;
            }
            text.push(character);
        }
        assert!(text.len() < CAPACITY);
        let result = String::from_utf16(&text).unwrap();
        // SAFETY: reply ownership was handed to this host using the global heap.
        unsafe {
            top = (*reply_node).next;
            GlobalFree(reply_node.cast());
        }
        assert!(top.is_null(), "argument leak or more than one reply");
        (result, flags.values[2])
    }
    fn call_v2(function: Export, arguments: &[&str], version: i32) -> (String, i32) {
        let mut flags = Flags { values: [0; 14] };
        flags.values[7] = version;
        let mut extra = Extra {
            flags: &mut flags,
            execute: None,
            validate: None,
            register: Some(register),
        };
        let mut top = ptr::null_mut();
        let mut push = |value: &str| {
            let text: Vec<_> = value.encode_utf16().collect();
            assert!(text.len() < CAPACITY);
            // SAFETY: private fixed SDK allocation, large enough for bounded text.
            let node =
                unsafe { GlobalAlloc(GPTR, size_of::<Stack>() + CAPACITY * 2) }.cast::<Stack>();
            assert!(!node.is_null());
            // SAFETY: node has the independently declared SDK layout and capacity.
            unsafe {
                (*node).next = top;
                let output = ptr::addr_of_mut!((*node).text).cast::<u16>();
                for (index, unit) in text.into_iter().chain(Some(0)).enumerate() {
                    *output.add(index) = unit;
                }
            }
            top = node;
        };
        push("v2-caller-sentinel");
        for argument in arguments.iter().rev() {
            push(argument);
        }
        // SAFETY: live Unicode SDK frame and exact export prototype.
        unsafe { function(0, CAPACITY as i32, ptr::null_mut(), &mut top, &mut extra) };
        let mut pop = || {
            assert!(!top.is_null());
            let node = top;
            let mut units = Vec::new();
            // SAFETY: every input/output is a fixed GPTR stack allocation.
            unsafe {
                let input = ptr::addr_of!((*node).text).cast::<u16>();
                for index in 0..CAPACITY {
                    let unit = *input.add(index);
                    if unit == 0 {
                        break;
                    }
                    units.push(unit);
                }
                top = (*node).next;
                GlobalFree(node.cast());
            }
            assert!(units.len() < CAPACITY);
            String::from_utf16(&units).unwrap()
        };
        let reply = pop();
        assert_eq!(pop(), "v2-caller-sentinel");
        assert!(top.is_null());
        (reply, flags.values[2])
    }
    fn reject_raw(acquire: Export, id: &str, mode: &str) {
        let mut flags = Flags { values: [0; 14] };
        flags.values[7] = 0x0001_0000;
        unsafe extern "system" fn refuse(_: HMODULE, _: Callback) -> i32 {
            -1
        }
        let mut extra = Extra {
            flags: if mode == "flags" {
                ptr::null_mut()
            } else {
                &mut flags
            },
            execute: None,
            validate: None,
            register: Some(if mode == "registration" {
                refuse
            } else {
                register
            }),
        };
        // Capacity/null-stack reject before consuming this argument; the other
        // cases receive a valid argument so they cannot mask an Extra failure.
        let input = unsafe { GlobalAlloc(GPTR, size_of::<Stack>() + CAPACITY * 2) }.cast::<Stack>();
        assert!(!input.is_null());
        let text = unsafe { ptr::addr_of_mut!((*input).text).cast::<u16>() };
        for (index, ch) in id.encode_utf16().chain(Some(0)).enumerate() {
            unsafe {
                *text.add(index) = ch;
            }
        }
        let mut top = input;
        let size = match mode {
            "capacity-low" => 128,
            "capacity-high" => 32769,
            _ => CAPACITY as i32,
        };
        unsafe {
            acquire(
                0,
                size,
                ptr::null_mut(),
                if mode == "stack" {
                    ptr::null_mut()
                } else {
                    &mut top
                },
                if mode == "extra" {
                    ptr::null_mut()
                } else {
                    &mut extra
                },
            )
        };
        if matches!(mode, "capacity-low" | "capacity-high" | "stack") {
            assert_eq!(
                top, input,
                "rejected call must not consume the caller argument"
            );
        } else {
            assert!(
                !top.is_null() && top != input,
                "expected a fresh denial response"
            );
            let text = unsafe { ptr::addr_of!((*top).text).cast::<u16>() };
            let expected: Vec<u16> = "v1:unavailable".encode_utf16().chain(Some(0)).collect();
            for (index, ch) in expected.iter().enumerate() {
                assert_eq!(unsafe { *text.add(index) }, *ch);
            }
        }
        assert_eq!(
            flags.values[2],
            i32::from(!matches!(mode, "extra" | "flags"))
        );
        // Bounded cleanup of the one caller node or the one returned reply.
        assert!(!top.is_null());
        assert!(unsafe { (*top).next }.is_null());
        unsafe {
            GlobalFree(top.cast());
        }
    }

    fn expect(function: Export, argument: Option<&str>, expected: &str) {
        let (reply, error) = call(function, argument, 0x0001_0000);
        assert_eq!(reply, expected);
        assert_eq!(
            error,
            i32::from(!matches!(expected, "v1:held" | "v1:released"))
        );
    }
    unsafe fn symbol(module: HMODULE, name: &[u8]) -> Export {
        // SAFETY: these are the exact five-argument SDK exports of this fixture DLL.
        let address = unsafe { GetProcAddress(module, name.as_ptr()) }.expect("undecorated export");
        unsafe { transmute::<unsafe extern "system" fn() -> isize, Export>(address) }
    }
    fn busy(identity: &AppIdentity) {
        assert!(matches!(Lease::try_acquire(identity), Acquisition::Busy));
    }
    fn free(identity: &AppIdentity) {
        assert!(matches!(
            Lease::try_acquire(identity),
            Acquisition::Acquired(_)
        ));
    }
    pub fn run() {
        let arguments: Vec<_> = std::env::args().collect();
        assert_eq!(arguments.len(), 3);
        let path = PathBuf::from(&arguments[1]);
        assert!(path.is_absolute() && path.is_file());
        let mode = arguments[2].as_str();
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: the isolated runner supplies only this freshly built development DLL.
        let module = unsafe { LoadLibraryExW(path.as_ptr(), 0, LOAD_WITH_ALTERED_SEARCH_PATH) };
        assert_ne!(module, 0);
        let acquire = unsafe { symbol(module, b"AcquireV1\0") };
        let status = unsafe { symbol(module, b"StatusV1\0") };
        let release = unsafe { symbol(module, b"ReleaseV1\0") };
        let id = format!("example.nsis.guard.{}.{mode}", std::process::id());
        let identity = AppIdentity::new(&id).unwrap();
        match mode {
            "v2-unsigned"
            | "v2-invalid-tool"
            | "v2-invalid-dir"
            | "v2-invalid-components"
            | "v2-api" => {
                // This host is unsigned and has no NSIS VERSIONINFO assertions.
                // These are ABI/denial proofs, never simulated signed installation.
                let prepare = unsafe { symbol(module, b"PrepareV2\0") };
                let poll = unsafe { symbol(module, b"PollV2\0") };
                let path_v2 = unsafe { symbol(module, b"PathV2\0") };
                let publish = unsafe { symbol(module, b"PublishV2\0") };
                let finalize = unsafe { symbol(module, b"FinalizeV2\0") };
                let release_v2 = unsafe { symbol(module, b"ReleaseV2\0") };
                let valid = ["fbforge-inspect", r"C:\private-abi-fixture", ""];
                let invalid = match mode {
                    "v2-invalid-tool" => ["fbforge-suite", valid[1], ""],
                    "v2-invalid-dir" => [valid[0], r"\\server\share", ""],
                    "v2-invalid-components" => [valid[0], valid[1], "scanner,scanner"],
                    _ => valid,
                };
                let (reply, error) = call_v2(
                    prepare,
                    &invalid,
                    if mode == "v2-api" {
                        0x0002_0000
                    } else {
                        0x0001_0000
                    },
                );
                if matches!(mode, "v2-unsigned" | "v2-invalid-tool") {
                    assert_eq!((reply, error), ("v2:preparing".into(), 0));
                } else {
                    assert_eq!((reply, error), ("v2:unavailable".into(), 1));
                    assert_eq!(
                        call_v2(prepare, &valid, 0x0001_0000),
                        ("v2:unavailable".into(), 1)
                    );
                }
                assert_ne!(unsafe { FreeLibrary(module) }, 0);
                let mut resident = 0;
                assert_ne!(
                    unsafe {
                        GetModuleHandleExW(
                            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                                | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                            poll as *const () as *const u16,
                            &mut resident,
                        )
                    },
                    0
                );
                assert_eq!(resident, module);
                let mut denied = false;
                for _ in 0..200 {
                    let (reply, error) = call_v2(poll, &[], 0x0001_0000);
                    if reply == "v2:unavailable" {
                        assert_eq!(error, 1);
                        denied = true;
                        break;
                    }
                    assert_eq!((reply, error), ("v2:preparing".into(), 0));
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                assert!(
                    denied,
                    "unsigned/unknown host must never get a staging grant"
                );
                assert_eq!(
                    call_v2(prepare, &valid, 0x0001_0000),
                    ("v2:unavailable".into(), 1)
                );
                for kind in ["staging", "target"] {
                    assert_eq!(
                        call_v2(path_v2, &[kind], 0x0001_0000),
                        ("v2:unavailable".into(), 1)
                    );
                }
                for export in [publish, finalize] {
                    assert_eq!(
                        call_v2(export, &[], 0x0001_0000),
                        ("v2:unavailable".into(), 1)
                    );
                }
                let mut released = false;
                for _ in 0..200 {
                    let answer = call_v2(release_v2, &[], 0x0001_0000);
                    if answer == ("v2:released".into(), 0) {
                        released = true;
                        break;
                    }
                    assert_eq!(answer, ("v2:unavailable".into(), 1));
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                assert!(released, "release requires positive owner completion");
            }
            "v1-v2-mix" => {
                expect(acquire, Some(&id), "v1:held");
                let poll = unsafe { symbol(module, b"PollV2\0") };
                assert_eq!(
                    call_v2(poll, &[], 0x0001_0000),
                    ("v2:unavailable".into(), 1)
                );
                busy(&identity);
                expect(status, None, "v1:held");
                expect(release, None, "v1:released");
                free(&identity);
            }
            "held" | "callbacks" => {
                expect(acquire, Some(&id), "v1:held");
                busy(&identity);
                // PIN must survive a host unload attempt while the owner is alive.
                assert_ne!(unsafe { FreeLibrary(module) }, 0);
                let mut resident = 0;
                assert_ne!(
                    unsafe {
                        GetModuleHandleExW(
                            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                                | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                            status as *const () as *const u16,
                            &mut resident,
                        )
                    },
                    0
                );
                assert_eq!(resident, module);
                std::thread::spawn(move || expect(status, None, "v1:held"))
                    .join()
                    .unwrap();
                let address = CALLBACK.load(Ordering::Acquire);
                assert_ne!(address, 0, "callback registration missing");
                let callback: Callback = unsafe { transmute(address) };
                assert_eq!(unsafe { callback(1) }, 0); // GUIUNLOAD is not a release grant.
                busy(&identity);
                if mode == "callbacks" {
                    assert_eq!(unsafe { callback(0) }, 0);
                    busy(&identity); // Final unload is not a mutation-completion proof either.
                }
                std::thread::spawn(move || expect(release, None, "v1:released"))
                    .join()
                    .unwrap();
                free(&identity);
                expect(status, None, "v1:unavailable");
            }
            "busy" => {
                let owner = match Lease::try_acquire(&identity) {
                    Acquisition::Acquired(lease) => lease,
                    result => panic!("{result:?}"),
                };
                expect(acquire, Some(&id), "v1:busy");
                expect(status, None, "v1:unavailable");
                drop(owner);
                expect(release, None, "v1:released");
                free(&identity);
            }
            "abandoned" => {
                let child_id = identity.clone();
                std::thread::spawn(move || {
                    let owner = match Lease::try_acquire(&child_id) {
                        Acquisition::Acquired(lease) => lease,
                        result => panic!("{result:?}"),
                    };
                    // Fixture-only leak preserves the named object while its owner exits.
                    std::mem::forget(owner);
                })
                .join()
                .unwrap();
                expect(acquire, Some(&id), "v1:abandoned");
                expect(status, None, "v1:unavailable");
                busy(&identity);
                expect(release, None, "v1:released");
                free(&identity);
            }
            "missing" | "utf16" | "api" | "invalid-id" | "capacity-low" | "capacity-high"
            | "stack" | "extra" | "flags" | "registration" => {
                match mode {
                    "missing" => expect(acquire, None, "v1:unavailable"),
                    "utf16" => assert_eq!(
                        call_raw(acquire, Some(&[0xd800]), 0x0001_0000),
                        ("v1:unavailable".into(), 1)
                    ),
                    "api" => assert_eq!(
                        call(acquire, Some(&id), 0x0002_0000),
                        ("v1:unavailable".into(), 1)
                    ),
                    "invalid-id" => expect(acquire, Some("../invalid"), "v1:unavailable"),
                    _ => reject_raw(acquire, &id, mode),
                }
                // Fresh process per case: no intervening bad AppIdentity masks
                // the early ABI failure before this immediately valid attempt.
                // NSIS may unload immediately after the FIRST early rejection.
                // No intervening valid Acquire may accidentally supply the PIN.
                // Sticky state
                // must survive the host dropping its loader reference.
                assert_ne!(unsafe { FreeLibrary(module) }, 0);
                let mut resident = 0;
                assert_ne!(
                    unsafe {
                        GetModuleHandleExW(
                            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                                | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                            status as *const () as *const u16,
                            &mut resident,
                        )
                    },
                    0
                );
                assert_eq!(resident, module);
                expect(acquire, Some(&id), "v1:unavailable");
                expect(status, None, "v1:unavailable");
                free(&identity);
            }
            "bounds" => {
                let mut flags = Flags { values: [0; 14] };
                flags.values[7] = 0x0001_0000;
                let mut extra = Extra {
                    flags: &mut flags,
                    execute: None,
                    validate: None,
                    register: Some(register),
                };
                for capacity in [-1, 0, 128, 32769] {
                    let mut top = ptr::null_mut();
                    flags.values[2] = 0;
                    unsafe { status(0, capacity, ptr::null_mut(), &mut top, &mut extra) };
                    assert!(top.is_null());
                    assert_eq!(flags.values[2], 1);
                }
                flags.values[2] = 0;
                unsafe {
                    status(
                        0,
                        CAPACITY as i32,
                        ptr::null_mut(),
                        ptr::null_mut(),
                        &mut extra,
                    )
                };
                assert_eq!(flags.values[2], 1);
                free(&identity);
            }
            _ => panic!("unknown isolated ABI fixture mode"),
        }
        println!("PASS Unicode ABI fixture: {mode}");
    }
    use std::os::windows::ffi::OsStrExt;
}
