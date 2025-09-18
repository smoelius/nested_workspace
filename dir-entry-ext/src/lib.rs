use std::{
    ffi::{OsStr, OsString},
    fs::DirEntry,
};

pub trait DirEntryExt {
    type Output<'a>
    where
        Self: 'a;
    fn extension(&self) -> Option<Self::Output<'_>>;
}

macro_rules! impl_dir_entry_ext {
    ($ty:ty, $output:ty) => {
        impl DirEntryExt for $ty {
            type Output<'a> = $output;
            fn extension(&self) -> Option<Self::Output<'_>> {
                let file_name = self.file_name();
                let (before, after) = rsplit_file_at_dot(&file_name);
                before.and(after).map(Into::into)
            }
        }
    };
}

impl_dir_entry_ext!(DirEntry, OsString);

#[cfg(feature = "walkdir")]
impl_dir_entry_ext!(walkdir::DirEntry, &'a OsStr);

// `rsplit_file_at_dot` was copied from:
// https://github.com/rust-lang/rust/blob/30292bb68ec1a737df074449cbb9f4384065274a/library/std/src/path.rs#L318-L340
fn rsplit_file_at_dot(file: &OsStr) -> (Option<&OsStr>, Option<&OsStr>) {
    if file.as_encoded_bytes() == b".." {
        return (Some(file), None);
    }

    // The unsafety here stems from converting between &OsStr and &[u8]
    // and back. This is safe to do because (1) we only look at ASCII
    // contents of the encoding and (2) new &OsStr values are produced
    // only from ASCII-bounded slices of existing &OsStr values.
    let mut iter = file.as_encoded_bytes().rsplitn(2, |b| *b == b'.');
    let after = iter.next();
    let before = iter.next();
    if before == Some(b"") {
        (Some(file), None)
    } else {
        unsafe {
            (
                before.map(|s| OsStr::from_encoded_bytes_unchecked(s)),
                after.map(|s| OsStr::from_encoded_bytes_unchecked(s)),
            )
        }
    }
}
