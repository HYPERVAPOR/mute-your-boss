pub mod pb {
    tonic::include_proto!("myb.v1");
}

pub mod session;
pub mod traits;

pub use session::{FocusSession, SessionState};
pub use traits::{
    AudioCapture, AudioChunk, AudioProcessInfo, AudioStream, EventLog, KeywordEntry, KeywordVocab,
    KwsEngine, KwsHit, PolicyEngine, TriggerEvent, VolumeController, VolumeDecision,
};
