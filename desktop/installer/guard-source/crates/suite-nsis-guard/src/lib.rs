//! Development-only resident Unicode NSIS coordination guard.
//! A held lease is not package, process, installation-health or execution evidence.
#[cfg(windows)]
mod abi;
#[cfg(any(windows, test))]
mod session;

#[cfg(any(windows, test))]
mod mutation;
