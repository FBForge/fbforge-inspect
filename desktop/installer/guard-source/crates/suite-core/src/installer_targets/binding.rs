use super::TargetError;

pub(super) fn bind_registered_target(
    registered: &[String],
    discovered: &str,
    journal: Option<&str>,
    ordinal_equal: impl Fn(&[u16], &[u16]) -> Result<bool, TargetError>,
) -> Result<String, TargetError> {
    let selected = journal.unwrap_or(discovered);
    let selected_units = location_units(selected)?;
    if journal.is_some() && !ordinal_equal(&location_units(discovered)?, &selected_units)? {
        return Err(TargetError::RecoveryRequired);
    }
    for raw in registered {
        // Do not authorize a fresh transaction through an alias which cannot
        // be resolved after the original directory has moved into backup.
        if !ordinal_equal(&location_units(raw)?, &selected_units)? {
            return Err(TargetError::ManualMigrationRequired);
        }
    }
    // Keep the journal's authoritative spelling on recovery, not the missing
    // target's unresolved registry spelling. Caller checks Parent ID first.
    Ok(selected.to_string())
}

fn location_units(value: &str) -> Result<Vec<u16>, TargetError> {
    crate::discovery::validate_windows_location(value)
        .map(|value| value.replace('/', "\\").encode_utf16().collect())
        .map_err(|_| TargetError::UnsafeTarget)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The portable samples are ASCII. Production must use the actual Windows
    // ordinal API, not this deliberately narrow sample comparator.
    fn ascii_sample_equal(a: &[u16], b: &[u16]) -> Result<bool, TargetError> {
        Ok(a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(&a, &b)| a <= 127 && b <= 127 && (a as u8).eq_ignore_ascii_case(&(b as u8))))
    }

    #[test]
    fn canonical_registration_keeps_the_exact_target_for_fresh_and_recovered_work() {
        let canonical = r"C:\Apps\FBForge Connect";
        for journal in [None, Some(canonical)] {
            assert_eq!(
                bind_registered_target(&[canonical.into()], canonical, journal, ascii_sample_equal),
                Ok(canonical.into())
            );
        }
    }

    #[test]
    fn missing_target_case_only_registration_recovers_the_saved_canonical_spelling() {
        let canonical = r"C:\Apps\FBForge Connect";
        let registered = r"C:\Apps\fbforge connect";
        assert_eq!(
            bind_registered_target(
                &[registered.into()],
                registered,
                Some(canonical),
                ascii_sample_equal
            ),
            Ok(canonical.into())
        );
    }

    #[test]
    fn short_name_registration_is_rejected_before_starting_a_new_transaction() {
        assert_eq!(
            bind_registered_target(
                &[r"C:\Apps\FBFORG~1".into()],
                r"C:\Apps\FBForge Connect",
                None,
                ascii_sample_equal,
            ),
            Err(TargetError::ManualMigrationRequired)
        );
    }
}
