pub mod pb {
    tonic::include_proto!("myb.v1");
}

pub mod traits;

pub use traits::{AudioCapture, AudioChunk, AudioProcessInfo, AudioStream, VolumeController};
