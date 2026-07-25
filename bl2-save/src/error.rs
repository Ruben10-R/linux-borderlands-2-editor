//! The one error type for the whole crate.

/// Anything that can go wrong loading, editing, or writing a save.
#[derive(thiserror::Error, Debug)]
pub enum SaveError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("file too short to be a BL2 save ({0} bytes)")]
    TooShort(usize),

    #[error("SHA1 mismatch — file is corrupt or not a BL2 save")]
    Sha1Mismatch,

    #[error("WSG magic missing — not a BL2 save")]
    BadMagic,

    #[error("unsupported save version {0} (expected 2)")]
    BadVersion(u32),

    #[error("CRC mismatch: stored {stored:#010x} != computed {computed:#010x}")]
    CrcMismatch { stored: u32, computed: u32 },

    /// A declared size in the file did not match the actual decoded size.
    #[error("size mismatch: {0}")]
    Size(String),

    #[error("LZO: {0}")]
    Lzo(String),

    #[error("protobuf: {0}")]
    Proto(String),

    /// A self-check after re-encoding failed — we refuse to write such a file.
    #[error("self-verify failed after encoding: {0}")]
    SelfVerify(String),
}

pub type Result<T> = std::result::Result<T, SaveError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_display_helpfully() {
        assert!(SaveError::TooShort(3).to_string().contains('3'));
        assert!(SaveError::Proto("boom".into()).to_string().contains("boom"));
        assert!(SaveError::Sha1Mismatch.to_string().to_lowercase().contains("sha1"));
        assert!(SaveError::BadVersion(9).to_string().contains('9'));
    }
}
