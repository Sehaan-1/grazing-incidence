use std::fmt;

/// Everything that can go wrong during a render.
#[derive(Debug)]
pub enum RenderError {
    /// Dimensions the renderer will not allocate. Three things land here: an
    /// axis below 2, a `width * height` that overflows `usize`, and a product
    /// above `image::MAX_PIXELS`. All three are "asked for more image than a
    /// render can hold", so they share a variant and a message.
    InvalidDimension {
        width: usize,
        height: usize,
    },
    MissingValue {
        flag: String,
    },
    InvalidParameter {
        flag: String,
        value: String,
    },
    UnknownFlag {
        flag: String,
    },
    /// A `--scene` name that resolves to nothing. The name is all there is to
    /// say; `Display` prints the valid ones alongside it.
    UnknownScene(String),
    Io(std::io::Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimension { width, height } => write!(
                f,
                "invalid image dimensions {width}x{height}: each axis must be at least 2 \
                 and the product must be at most {} pixels",
                crate::image::MAX_PIXELS
            ),
            Self::MissingValue { flag } => write!(f, "{flag} requires a value"),
            Self::InvalidParameter { flag, value } => {
                write!(
                    f,
                    "invalid value for {flag}: {value} is not a positive integer"
                )
            }
            Self::UnknownFlag { flag } => write!(f, "unknown option: {flag}"),
            Self::UnknownScene(name) => write!(
                f,
                "unknown scene: {name} (valid: {})",
                crate::scene::SCENE_NAMES.join(", ")
            ),
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
            | Self::UnknownFlag { .. }
            | Self::UnknownScene(_) => None,
        }
    }
}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
