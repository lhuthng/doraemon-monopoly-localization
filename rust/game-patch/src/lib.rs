pub mod cue;
pub mod delta;
pub mod hash;
pub mod install;
pub mod payload;
pub mod pe;
pub mod strings;
pub mod sysfont;

pub type Result<T> = std::result::Result<T, String>;
