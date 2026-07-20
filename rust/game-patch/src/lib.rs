pub mod cue;
pub mod delta;
pub mod hash;
pub mod install;
pub mod music;
pub mod payload;
pub mod pe;
pub mod strings;
pub mod sysfont;
pub mod voice;

pub type Result<T> = std::result::Result<T, String>;
