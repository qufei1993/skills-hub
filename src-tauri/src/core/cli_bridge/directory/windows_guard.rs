use std::io;

pub(super) const fn share_mode() -> u32 {
    1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub attributes: u32,
    pub reparse_tag: u32,
    pub volume: u64,
    pub file_id: [u8; 16],
}

pub(super) fn verify(
    expected: Option<Snapshot>,
    query: impl FnOnce() -> io::Result<Snapshot>,
) -> io::Result<Snapshot> {
    let current = query()?;
    if current.attributes & 0x10 == 0
        || current.attributes & 0x400 != 0
        || current.reparse_tag != 0
        || expected.is_some_and(|previous| {
            previous.volume != current.volume || previous.file_id != current.file_id
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "CLI_BRIDGE_UNSAFE_DIRECTORY",
        ));
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal() -> Snapshot {
        Snapshot {
            attributes: 0x10,
            reparse_tag: 0,
            volume: 7,
            file_id: [91; 16],
        }
    }

    #[test]
    fn cli_bridge_windows_share_mode_denies_write_and_delete_access() {
        assert_eq!(share_mode(), 1);
        assert_eq!(share_mode() & (2 | 4), 0);
    }

    #[test]
    fn cli_bridge_windows_verify_rejects_reparse_and_identity_changes() {
        let original = normal();
        assert_eq!(verify(Some(original), || Ok(original)).unwrap(), original);
        for changed in [
            Snapshot {
                attributes: 0x410,
                ..original
            },
            Snapshot {
                reparse_tag: 0xa0000003,
                ..original
            },
            Snapshot {
                attributes: 0x80,
                ..original
            },
            Snapshot {
                volume: 8,
                ..original
            },
            Snapshot {
                file_id: [92; 16],
                ..original
            },
        ] {
            assert!(verify(Some(original), || Ok(changed)).is_err());
        }
        assert!(verify(None, || Ok(Snapshot {
            attributes: 0x410,
            ..original
        }))
        .is_err());
        assert!(verify(Some(original), || Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "query denied"
        )))
        .is_err());
    }
}
