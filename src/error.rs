use std::fmt;

/// Everything that can go wrong during a render.
#[derive(Debug)]
pub enum RenderError {
    InvalidDimension { width: usize, height: usize },
    MissingValue { flag: String },
    InvalidParameter { flag: String, value: String },
    UnknownFlag { flag: String },
    Io(std::io::Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimension { width, height } => write!(
                f,
                "invalid image dimensions {width}x{height}: both must be non-zero"
            ),
            Self::MissingValue { flag } => write!(f, "{flag} requires a value"),
            Self::InvalidParameter { flag, value } => {
                write!(
                    f,
                    "invalid value for {flag}: {value} is not a positive integer"
                )
            }
            Self::UnknownFlag { flag } => write!(f, "unknown option: {flag}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::InvalidDimension { .. }
            | Self::MissingValue { .. }
            | Self::InvalidParameter { .. }
            | Self::UnknownFlag { .. } => None,
        }
    }
}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
