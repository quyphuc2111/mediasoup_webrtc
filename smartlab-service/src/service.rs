//! Windows Service registration and lifecycle

use std::ffi::OsString;
use std::time::Duration;
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl,
        ServiceExitCode, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

const SERVICE_NAME: &str = "SmartlabService";
const SERVICE_DISPLAY: &str = "Smartlab Student Service";
const SERVICE_DESCRIPTION: &str =
    "Smartlab Student background service - allows teacher to connect before user login";
const LISTEN_PORT: u16 = 3019;

// Generate the Windows service entry point
define_windows_service!(ffi_service_main, service_main);

/// Run as a Windows Service (called by SCM)
pub fn run_service() -> Result<(), Box<dyn std::error::Error>> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

/// Service main function — called by the Windows Service Control Manager
fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service_inner() {
        log::error!("[Service] Fatal error: {}", e);
    }
}

fn run_service_inner() -> Result<(), Box<dyn std::error::Error>> {
    // Create a channel to receive stop events
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown_tx = std::sync::Mutex::new(Some(shutdown_tx));

    // Register the service control handler
    let status_handle = service_control_handler::register(
        SERVICE_NAME,
        move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop => {
                    log::info!("[Service] Stop requested");
                    if let Ok(mut tx) = shutdown_tx.lock() {
                        if let Some(tx) = tx.take() {
                            let _ = tx.send(());
                        }
                    }
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        },
    )?;

    // Report: Starting
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    })?;

    // Report: Running
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    log::info!("[Service] Running, listening on port {}", LISTEN_PORT);

    // Run the command server + discovery responder
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        // Spawn UDP discovery responder in a background thread
        // so teacher can find this machine even before user login
        tokio::task::spawn_blocking(|| {
            crate::discovery::run_discovery_responder();
        });

        tokio::select! {
            _ = crate::commands::run_command_server(LISTEN_PORT) => {
                log::info!("[Service] Command server exited");
            }
            _ = shutdown_rx => {
                log::info!("[Service] Shutdown signal received");
            }
        }
    });

    // Report: Stopped
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    log::info!("[Service] Stopped");
    Ok(())
}

/// Install the service into Windows SCM
pub fn install_service() -> Result<(), Box<dyn std::error::Error>> {
    use windows_service::service::{
        ServiceAction, ServiceActionType, ServiceDependency, ServiceFailureActions,
        ServiceFailureResetPeriod,
    };

    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CREATE_SERVICE)?;

    let exe_path = std::env::current_exe()?;

    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe_path,
        launch_arguments: vec![],
        dependencies: vec![
            // Wait for network stack to be ready before starting
            ServiceDependency::Service(OsString::from("Tcpip")),
            ServiceDependency::Service(OsString::from("Dhcp")),
        ],
        account_name: None, // LocalSystem
        account_password: None,
    };

    let service_access =
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_CONFIG;
    let service = manager.create_service(&service_info, service_access)?;
    service.set_description(SERVICE_DESCRIPTION)?;

    // Configure recovery: auto-restart on failure (critical for reliability after reboot)
    let recovery_actions = vec![
        ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(5),
        },
        ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(10),
        },
        ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(30),
        },
    ];
    let failure_actions = ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86400)),
        reboot_msg: None,
        command: None,
        actions: Some(recovery_actions),
    };
    service.update_failure_actions(failure_actions)?;
    service.set_failure_actions_on_non_crash_failures(true)?;

    log::info!(
        "[Service] Installed with recovery policy: {}",
        SERVICE_NAME
    );
    Ok(())
}

/// Uninstall the service from Windows SCM
pub fn uninstall_service() -> Result<(), Box<dyn std::error::Error>> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
    )?;

    // Stop if running
    let status = service.query_status()?;
    if status.current_state != ServiceState::Stopped {
        let _ = service.stop();
        // Wait for stop
        std::thread::sleep(Duration::from_secs(2));
    }

    service.delete()?;
    log::info!("[Service] Uninstalled: {}", SERVICE_NAME);
    Ok(())
}
