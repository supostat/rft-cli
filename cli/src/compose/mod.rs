mod detect;
mod parse;

pub use detect::detect_compose_file;
pub use parse::{BuildConfig, ComposeFile, ComposeService, parse_compose_file};
