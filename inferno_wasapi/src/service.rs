use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode,
        ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                running_clone.store(false, Ordering::SeqCst);
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

    // Run the main logic in a tokio runtime
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        // Import and call the existing main logic
        // For now, just wait until stop signal
        while running.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

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
