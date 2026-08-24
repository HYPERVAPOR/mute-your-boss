pub mod pb {
    tonic::include_proto!("myb.v1");
}

pub mod session;
pub mod traits;

pub use session::FocusSession;
pub use traits::{
    AudioCapture, AudioChunk, AudioProcessInfo, AudioStream, EventLog, KwsEngine, KwsHit,
    PolicyEngine, TriggerEvent, VolumeController, VolumeDecision,
};
