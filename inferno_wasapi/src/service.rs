use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode,
        ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};
use std::time::Duration;

const SERVICE_NAME: &str = "InfernoAoIP";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

pub fn run_as_service() -> Result<(), windows_service::Error> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

fn service_main(_arguments: Vec<std::ffi::OsString>) {
    if let Err(e) = run_service() {
        tracing::error!("Service error: {e}");
    }
}

fn run_service() -> Result<(), Box<dyn std::error::Error>> {
    // Create a watch channel so the Windows service control handler can signal shutdown.
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let shutdown_tx_clone = shutdown_tx.clone();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                shutdown_tx_clone.send(true).ok();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    tracing::info!("InfernoAoIP service started");

    let config = crate::Config::load();
    let rt = tokio::runtime::Runtime::new()?;
    if let Err(e) = rt.block_on(crate::run_audio_service(config, shutdown_rx)) {
        tracing::error!(error=%e, backtrace=?std::backtrace::Backtrace::capture(), "Audio service error");
    }

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    tracing::info!("InfernoAoIP service stopped");
    Ok(())
}
