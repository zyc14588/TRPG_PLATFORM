use crate::shared_kernel::TrpgError;
use trpg_contracts::WireErrorCode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorDescriptor {
    pub code: &'static str,
    pub retryable: bool,
    pub visibility_safe: bool,
}

pub fn describe_error(error: &TrpgError) -> ErrorDescriptor {
    ErrorDescriptor {
        code: error.code(),
        retryable: error.retryable(),
        visibility_safe: true,
    }
}

pub fn compose_error(code: WireErrorCode, retryable: bool) -> ErrorDescriptor {
    ErrorDescriptor {
        code: code.as_str(),
        retryable,
        visibility_safe: true,
    }
}
