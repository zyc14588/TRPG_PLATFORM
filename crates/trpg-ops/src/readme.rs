// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

#[macro_export]
macro_rules! define_ops_runbook_module {
    (
        $command:ident,
        $service:ident,
        $repository:ident,
        $error:ident,
        $append_fn:ident,
        $module_name:literal,
        $event_type:literal,
        $operation:expr,
        [$($read_model:literal),* $(,)?],
        $runbook_path:literal
    ) => {
        pub const MODULE_NAME: &str = $module_name;
        pub const EVENT_TYPE: &str = $event_type;
        pub const READ_MODELS: &[&str] = &[$($read_model),*];
        pub const RUNBOOK_PATH: &str = $runbook_path;

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $command {
            pub operation: $crate::OpsRunbookOperation,
            pub reason: &'static str,
            pub evidence_path: &'static str,
        }

        impl $command {
            pub const fn record(reason: &'static str) -> Self {
                Self {
                    operation: $operation,
                    reason,
                    evidence_path: RUNBOOK_PATH,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $error {
            GovernanceViolation(&'static str),
        }

        pub fn $append_fn<T>(
            store: &mut $crate::OpsEventStore,
            authority: &$crate::AuthorityContract,
            command: &$crate::CommandEnvelope<T>,
        ) -> $crate::KernelResult<$crate::OpsEventEnvelope> {
            $crate::append_ops_event(store, authority, command, contract(), RUNBOOK_PATH)
        }

        pub fn contract() -> $crate::OpsRunbookContract {
            $crate::OpsRunbookContract::new(
                MODULE_NAME,
                EVENT_TYPE,
                $operation,
                READ_MODELS,
            )
        }
    };
}

include!("readme/01_module_prelude.rs");
include!("readme/02_is_current_safe_name.rs");
