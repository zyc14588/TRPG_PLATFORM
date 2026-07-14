use crate::{shared_kernel::TrpgError, WireErrorCode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorDescriptor {
    pub code: WireErrorCode,
    pub retryable: bool,
    pub visibility_safe: bool,
}

pub fn describe_error(error: &TrpgError) -> ErrorDescriptor {
    ErrorDescriptor {
        code: error.wire_code(),
        retryable: error.retryable(),
        visibility_safe: true,
    }
}

pub fn compose_error(code: WireErrorCode, retryable: bool) -> ErrorDescriptor {
    ErrorDescriptor {
        code,
        retryable,
        visibility_safe: true,
    }
}
