//! Conservative admission rules; this is not an arbitrary-token AccessCheck.
use super::ProgramStartFailure as Failure;
use std::time::{Duration, Instant};

pub(super) const MAX_OBJECTS: usize = 8192;
pub(super) const MAX_DEPTH: usize = 32;
#[cfg(windows)]
pub(super) const MAX_ANCESTORS: usize = 128;
pub(super) const MAX_PATH_UNITS: usize = 4 * 1024 * 1024;
pub(super) const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub(super) const MAX_TOTAL_DESCRIPTOR_BYTES: usize = 16 * 1024 * 1024;
#[cfg(windows)]
pub(super) const MAX_ACES: usize = 2048;

#[derive(Clone, Copy)]
pub(super) enum Scope {
    ProgramTree,
    InstallerHost,
    InstallerAncestor,
    PrivateTree,
    Temporary,
    Ancestor,
}

pub(super) fn check_ace(
    scope: Scope,
    kind: u8,
    flags: u8,
    mask: u32,
    trusted: bool,
) -> Result<(), Failure> {
    if matches!(scope, Scope::Temporary) {
        return check_temporary_ace(kind, flags, mask, trusted, false).map(|_| ());
    }
    // SDK ACE types 0/1 are ordinary allow/deny. Callback/object/resource ACEs
    // and unknown rights need a different evaluator and are deliberately denied.
    const KNOWN_FLAGS: u8 = 0x1f;
    const KNOWN_RIGHTS: u32 = 0xf01f_01ff;
    const READ_EXECUTE: u32 = 0xa012_00a9;
    // Ancestors contain the held next component and cannot be emptied by an
    // untrusted SID. MS-FSA rejects setting a reparse point on a nonempty
    // directory; specific CREATE_FILE/CREATE_SUBDIRECTORY may thus add siblings.
    // Delete, ACL/owner and attribute/EA changes can alter the existing path.
    const CHANGE_NAMESPACE: u32 = 0x500d_0150;
    if !matches!(kind, 0 | 1) || flags & !KNOWN_FLAGS != 0 || mask & !KNOWN_RIGHTS != 0 {
        return Err(Failure::UnsupportedInstallation);
    }
    // Denies never justify a separate unsafe allow. Inherit-only entries do not
    // grant rights on this object; every existing child is inspected separately.
    if kind == 1 || flags & 0x08 != 0 || trusted {
        return Ok(());
    }
    let unsafe_grant = match scope {
        Scope::ProgramTree | Scope::InstallerHost | Scope::Temporary => mask & !READ_EXECUTE != 0,
        Scope::PrivateTree => mask != 0,
        Scope::Ancestor | Scope::InstallerAncestor => mask & CHANGE_NAMESPACE != 0,
    };
    if unsafe_grant {
        return Err(Failure::UnsafeInstallation);
    }
    Ok(())
}

// OW is evaluated only against the already validated descriptor owner, never
// added to the global trusted SID set or to program/tree/temp admission.
pub(super) fn allows_owner_rights(
    scope: Scope,
    owner_is_trusted: bool,
    is_owner_rights: bool,
) -> bool {
    matches!(scope, Scope::InstallerHost | Scope::InstallerAncestor)
        && owner_is_trusted
        && is_owner_rights
}

/// Temporary roots must remain safe for subsequently created NSIS children.
/// CreatorOwner is accepted only by the OS adapter after a positive bounded
/// TokenOwner check; it never changes the ordinary trusted-principal set.
pub(super) fn check_temporary_ace(
    kind: u8,
    flags: u8,
    mask: u32,
    trusted: bool,
    creator_owner: bool,
) -> Result<bool, Failure> {
    const READ_EXECUTE: u32 = 0xa012_00a9;
    // Reuse the exact ordinary ACE shape/right checks, independent of trustees.
    check_ace(Scope::ProgramTree, kind, flags, mask, true)?;
    if kind == 1 {
        return Ok(false);
    }
    let mutates = mask & !READ_EXECUTE != 0;
    if flags & 0x08 == 0 && !trusted && mutates {
        return Err(Failure::UnsafeInstallation);
    }
    // Inherit-only and no-propagate do not rescue a foreign future-child write.
    if flags & 0x03 != 0 && !trusted && !creator_owner && mutates {
        return Err(Failure::UnsafeInstallation);
    }
    Ok((trusted || creator_owner) && mask != 0 && flags & 0x03 == 0x03 && flags & 0x04 == 0)
}

pub(super) struct ScanBudget {
    objects: usize,
    path_units: usize,
    descriptor_bytes: usize,
    deadline: Instant,
}
impl ScanBudget {
    pub(super) fn new() -> Self {
        Self {
            objects: 0,
            path_units: 0,
            descriptor_bytes: 0,
            deadline: Instant::now() + Duration::from_secs(30),
        }
    }
    pub(super) fn check_time(&self) -> Result<(), Failure> {
        if Instant::now() >= self.deadline {
            return Err(Failure::UnsupportedInstallation);
        }
        Ok(())
    }
    pub(super) fn charge_object(&mut self, depth: usize, units: usize) -> Result<(), Failure> {
        self.check_time()?;
        self.objects = self
            .objects
            .checked_add(1)
            .ok_or(Failure::UnsupportedInstallation)?;
        self.path_units = self
            .path_units
            .checked_add(units)
            .ok_or(Failure::UnsupportedInstallation)?;
        if self.objects > MAX_OBJECTS || depth > MAX_DEPTH || self.path_units > MAX_PATH_UNITS {
            return Err(Failure::UnsupportedInstallation);
        }
        Ok(())
    }
    pub(super) fn charge_descriptor(&mut self, bytes: usize) -> Result<(), Failure> {
        self.check_time()?;
        self.descriptor_bytes = self
            .descriptor_bytes
            .checked_add(bytes)
            .ok_or(Failure::UnsupportedInstallation)?;
        if bytes > MAX_DESCRIPTOR_BYTES || self.descriptor_bytes > MAX_TOTAL_DESCRIPTOR_BYTES {
            return Err(Failure::UnsupportedInstallation);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_principals_may_read_and_execute_the_program_tree() {
        for mask in [0, 0x0012_00a9, 0x8000_0000, 0x2000_0000, 0xa000_0000] {
            assert!(check_ace(Scope::ProgramTree, 0, 0, mask, false).is_ok());
        }
    }

    #[test]
    fn mutation_scope_rejects_foreign_readers_as_well_as_writers() {
        for mask in [1, 0x20, 0x80, 0x0012_00a9, 0x8000_0000, 0x4000_0000] {
            assert!(check_ace(Scope::PrivateTree, 0, 0, mask, false).is_err());
            assert!(check_ace(Scope::PrivateTree, 0, 0, mask, true).is_ok());
        }
        assert!(check_ace(Scope::ProgramTree, 0, 0, 0x0012_00a9, false).is_ok());
    }

    #[test]
    fn every_other_principal_mutation_right_blocks_the_program_tree() {
        for mask in [
            2,
            4,
            0x10,
            0x40,
            0x100,
            0x1_0000,
            0x4_0000,
            0x8_0000,
            0x4000_0000,
            0x1000_0000,
            0x001f_01ff,
        ] {
            assert!(
                check_ace(Scope::ProgramTree, 0, 0, mask, false).is_err(),
                "{mask:x}"
            );
            assert!(
                check_ace(Scope::ProgramTree, 0, 0, mask, true).is_ok(),
                "{mask:x}"
            );
        }
    }

    #[test]
    fn ancestors_may_allow_sibling_creation_but_not_namespace_mutation() {
        for mask in [2, 4, 0x0012_00a9] {
            assert!(check_ace(Scope::Ancestor, 0, 0, mask, false).is_ok());
        }
        for mask in [
            0x10,
            0x40,
            0x100,
            0x1_0000,
            0x4_0000,
            0x8_0000,
            0x4000_0000,
            0x1000_0000,
            0x001f_01ff,
        ] {
            assert!(check_ace(Scope::Ancestor, 0, 0, mask, false).is_err());
        }
    }

    #[test]
    fn denies_do_not_rescue_a_separate_unsafe_allow() {
        assert!(check_ace(Scope::ProgramTree, 1, 0, 0x001f_01ff, false).is_ok());
        assert!(check_ace(Scope::ProgramTree, 0, 0, 2, false).is_err());
        assert!(check_ace(Scope::ProgramTree, 0, 0x10, 2, false).is_err());
        assert!(check_ace(Scope::ProgramTree, 0, 0x0b, 0x1000_0000, false).is_ok());
    }

    #[test]
    fn unsupported_ace_forms_flags_and_masks_never_grant_admission() {
        for kind in [2, 5, 9, 17, 255] {
            assert!(check_ace(Scope::ProgramTree, kind, 0, 0, true).is_err());
        }
        for flags in [0x20, 0x40, 0x80] {
            assert!(check_ace(Scope::ProgramTree, 0, flags, 0, true).is_err());
        }
        for mask in [0x200, 0x0100_0000, 0x0200_0000, 0x0400_0000] {
            assert!(check_ace(Scope::ProgramTree, 0, 0, mask, true).is_err());
        }
    }

    #[test]
    fn incomplete_or_excessive_tree_checks_cannot_succeed() {
        let mut budget = ScanBudget::new();
        budget.objects = MAX_OBJECTS;
        assert!(budget.charge_object(1, 1).is_err());
        let mut budget = ScanBudget::new();
        assert!(budget.charge_object(MAX_DEPTH + 1, 1).is_err());
        assert!(budget.charge_object(1, MAX_PATH_UNITS + 1).is_err());
        let mut budget = ScanBudget::new();
        assert!(budget.charge_descriptor(MAX_DESCRIPTOR_BYTES + 1).is_err());
        budget.descriptor_bytes = MAX_TOTAL_DESCRIPTOR_BYTES;
        assert!(budget.charge_descriptor(1).is_err());
        budget.deadline = std::time::Instant::now() - std::time::Duration::from_secs(1);
        assert!(budget.check_time().is_err());
    }
}

#[cfg(test)]
mod temporary_tests {
    use super::*;
    #[test]
    fn inherited_foreign_writers_are_rejected_before_inherit_only_skip() {
        for flags in [0x01, 0x02, 0x03, 0x0b, 0x0f] {
            for mask in [
                2,
                4,
                0x10,
                0x40,
                0x100,
                0x1_0000,
                0x4_0000,
                0x8_0000,
                0x4000_0000,
                0x1000_0000,
            ] {
                assert!(check_temporary_ace(0, flags, mask, false, false).is_err());
            }
        }
        assert_eq!(
            check_temporary_ace(0, 0x0b, 0x0012_00a9, false, false),
            Ok(false)
        );
    }
    #[test]
    fn positive_child_propagation_requires_a_nonempty_trusted_allow_without_np() {
        assert_eq!(
            check_temporary_ace(0, 0x03, 0x001f_01ff, true, false),
            Ok(true)
        );
        assert_eq!(
            check_temporary_ace(0, 0x0b, 0x001f_01ff, false, true),
            Ok(true)
        );
        assert!(check_temporary_ace(0, 0x0b, 0x001f_01ff, false, false).is_err());
        for kind in [5, 9, 17] {
            assert_eq!(
                check_temporary_ace(kind, 0x03, 1, true, false),
                Err(Failure::UnsupportedInstallation)
            );
        }
        assert!(check_temporary_ace(0, 0x03, 0x001f_01ff, false, true).is_err());
        for (kind, flags, mask) in [
            (0, 0x01, 1),
            (0, 0x02, 1),
            (0, 0x07, 1),
            (0, 0x0f, 1),
            (0, 0x03, 0),
            (1, 0x03, 1),
        ] {
            assert_eq!(
                check_temporary_ace(kind, flags, mask, true, false),
                Ok(false)
            );
        }
        assert!(check_ace(Scope::Temporary, 0, 0x0b, 2, false).is_err());
    }
}

#[cfg(test)]
mod installer_host_owner_tests {
    use super::*;
    #[test]
    fn installer_owner_rights_require_a_trusted_actual_owner_and_exact_sid() {
        for scope in [Scope::InstallerHost, Scope::InstallerAncestor] {
            assert!(allows_owner_rights(scope, true, true));
            assert!(!allows_owner_rights(scope, false, true));
            assert!(!allows_owner_rights(scope, true, false));
            assert!(check_ace(
                scope,
                0,
                0,
                0x001f_01ff,
                allows_owner_rights(scope, true, true)
            )
            .is_ok());
            assert!(check_ace(
                scope,
                0,
                0,
                0x001f_01ff,
                allows_owner_rights(scope, false, true)
            )
            .is_err());
        }
    }
    #[test]
    fn owner_rights_never_extend_start_private_tree_ancestor_or_temp_scope() {
        for scope in [
            Scope::ProgramTree,
            Scope::PrivateTree,
            Scope::Ancestor,
            Scope::Temporary,
        ] {
            assert!(!allows_owner_rights(scope, true, true));
            assert!(check_ace(scope, 0, 0, 0x001f_01ff, false).is_err());
        }
    }
}
