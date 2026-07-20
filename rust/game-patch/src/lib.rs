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

pub use payload::{
    PayloadPart, TargetName, TargetType,
    filter_strings, filter_voice, merge_parts,
    string_belongs_to, voice_belongs_to, string_record_group,
    encode_part, decode_part,
};

pub type Result<T> = std::result::Result<T, String>;
