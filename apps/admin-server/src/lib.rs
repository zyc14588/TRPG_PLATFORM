mod production_operations;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use trpg_contracts::{HttpRequest, HttpResponse};
use trpg_platform::admin_control_plane::{AdminControlPlane, AdminHttpRequest, AdminHttpResponse};

pub use production_operations::ProductionAdminOperations;

#[derive(Clone)]
pub struct AdminApplication {
    control: Arc<Mutex<AdminControlPlane>>,
}

impl AdminApplication {
    pub fn new(control: AdminControlPlane) -> Self {
        Self {
            control: Arc::new(Mutex::new(control)),
        }
    }

    pub fn readiness(&self) -> Result<String, String> {
        self.control
            .lock()
            .map_err(|_| "ADMIN_CONTROL_LOCK_POISONED".to_owned())?
            .readiness()
            .map_err(|error| error.code().to_owned())
    }

    pub fn handle(&self, request: &HttpRequest) -> Option<HttpResponse> {
        let normalized_path = request
            .path
            .strip_prefix("/v1/")
            .map(|suffix| format!("/admin/v1/{suffix}"))
            .unwrap_or_else(|| request.path.clone());
        let mut control = match self.control.lock() {
            Ok(control) => control,
            Err(_) if normalized_path.starts_with("/admin/v1/") => {
                return Some(convert_response(AdminHttpResponse::error(
                    503,
                    "ADMIN_CONTROL_UNAVAILABLE",
                )))
            }
            Err(_) => return None,
        };
        control
            .handle(AdminHttpRequest {
                method: request.method.clone(),
                path: normalized_path,
                headers: request.headers.clone(),
                body: request.body.clone(),
            })
            .map(convert_response)
    }
}

fn convert_response(response: AdminHttpResponse) -> HttpResponse {
    HttpResponse::json(response.status, response.body)
}

pub(crate) fn required_environment(name: &str) -> Result<String, &'static str> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or("ADMIN_REQUIRED_ENVIRONMENT_MISSING")
}

pub(crate) fn required_regular_file(name: &str) -> Result<PathBuf, &'static str> {
    let path = PathBuf::from(required_environment(name)?);
    validate_regular_file(&path)?;
    Ok(path)
}

pub(crate) fn optional_regular_file(name: &str) -> Result<Option<PathBuf>, &'static str> {
    let Some(value) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    validate_regular_file(&path)?;
    Ok(Some(path))
}

fn validate_regular_file(path: &Path) -> Result<(), &'static str> {
    if !path.is_absolute() {
        return Err("ADMIN_ABSOLUTE_FILE_PATH_REQUIRED");
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| "ADMIN_REQUIRED_FILE_UNAVAILABLE")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("ADMIN_REGULAR_FILE_REQUIRED");
    }
    Ok(())
}

pub(crate) fn required_private_directory(name: &str) -> Result<PathBuf, &'static str> {
    let path = PathBuf::from(required_environment(name)?);
    if !path.is_absolute() {
        return Err("ADMIN_ABSOLUTE_DIRECTORY_REQUIRED");
    }
    fs::create_dir_all(&path).map_err(|_| "ADMIN_DIRECTORY_CREATE_FAILED")?;
    let metadata = fs::symlink_metadata(&path).map_err(|_| "ADMIN_DIRECTORY_UNAVAILABLE")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("ADMIN_REGULAR_DIRECTORY_REQUIRED");
    }
    set_private_directory_permissions(&path).map_err(|_| "ADMIN_DIRECTORY_PERMISSION_FAILED")?;
    Ok(path)
}

pub(crate) fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| "ADMIN_DIRECTORY_SYNC_FAILED".to_owned())
}

#[cfg(unix)]
pub(crate) fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| "ADMIN_FILE_PERMISSION_FAILED".to_owned())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| "ADMIN_DIRECTORY_PERMISSION_FAILED".to_owned())
}
