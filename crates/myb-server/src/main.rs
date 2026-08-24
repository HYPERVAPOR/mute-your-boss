use std::net::SocketAddr;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response};

use myb_core::pb::{
    mute_your_boss_server::{MuteYourBoss, MuteYourBossServer},
    *,
};

type GetEventStreamStream = ReceiverStream<Result<Event, tonic::Status>>;

#[derive(Debug, Default)]
pub struct MybServerService;

#[tonic::async_trait]
impl MuteYourBoss for MybServerService {
    type GetEventStreamStream = GetEventStreamStream;

    async fn list_audio_processes(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ProcessList>, tonic::Status> {
        Ok(Response::new(ProcessList {
            processes: vec![AudioProcess {
                pid: 0,
                name: "stub-process".to_string(),
                window_title: "Stub".to_string(),
                current_volume: 0.5,
                is_meeting_app: false,
            }],
        }))
    }

    async fn start_session(
        &self,
        request: Request<StartSessionReq>,
    ) -> Result<Response<Session>, tonic::Status> {
        let req = request.into_inner();
        Ok(Response::new(Session {
            session_id: "session-001".to_string(),
            pid: req.pid,
            state: "listening".to_string(),
        }))
    }

    async fn stop_session(
        &self,
        _request: Request<SessionRef>,
    ) -> Result<Response<Empty>, tonic::Status> {
        Ok(Response::new(Empty {}))
    }

    async fn set_volume(
        &self,
        _request: Request<SetVolumeReq>,
    ) -> Result<Response<Empty>, tonic::Status> {
        Ok(Response::new(Empty {}))
    }

    async fn get_event_stream(
        &self,
        _request: Request<SessionRef>,
    ) -> Result<Response<GetEventStreamStream>, tonic::Status> {
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn get_status(
        &self,
        _request: Request<SessionRef>,
    ) -> Result<Response<SessionStatus>, tonic::Status> {
        Ok(Response::new(SessionStatus {
            session_id: "session-001".to_string(),
            state: "idle".to_string(),
            current_volume: 0,
            target_pid: 0,
            latency_ms: 0,
        }))
    }

    async fn validate_policy(
        &self,
        _request: Request<PolicyYaml>,
    ) -> Result<Response<ValidationResult>, tonic::Status> {
        Ok(Response::new(ValidationResult {
            ok: true,
            error: "".to_string(),
        }))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let addr: SocketAddr = "127.0.0.1:50051".parse()?;
    let service = MybServerService;

    tracing::info!("myb-server listening on {}", addr);

    Server::builder()
        .add_service(MuteYourBossServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
