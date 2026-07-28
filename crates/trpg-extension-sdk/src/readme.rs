// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

#[macro_export]
macro_rules! define_extension_sdk_module {
    (
        $command:ident,
        $service:ident,
        $append_fn:ident,
        $module_name:literal,
        $event_type:literal,
        $operation:expr,
        [$($read_model:literal),* $(,)?],
        [$($capability:expr),* $(,)?],
        $contract_reference:literal
    ) => {
        pub const MODULE_NAME: &str = $module_name;
        pub const EVENT_TYPE: &str = $event_type;
        pub const READ_MODELS: &[&str] = &[$($read_model),*];
        pub const ALLOWED_CAPABILITIES: &[$crate::ExtensionCapability] = &[$($capability),*];
        pub const CONTRACT_REFERENCE: &str = $contract_reference;

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $command {
            pub inner: $crate::ExtensionCommand,
        }

        impl $command {
            pub fn record(reason: &'static str) -> Self {
                Self {
                    inner: $crate::ExtensionCommand::record(
                        $operation,
                        reason,
                        CONTRACT_REFERENCE,
                        ALLOWED_CAPABILITIES.to_vec(),
                    ),
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $service {
            pub policy_gate: $crate::ExtensionPolicyGate,
        }

        impl $service {
            pub fn new(policy_gate: $crate::ExtensionPolicyGate) -> Self {
                Self { policy_gate }
            }

            pub fn execute(
                &self,
                store: &mut $crate::ExtensionEventStore,
                authority: &$crate::AuthorityContract,
                command: &$crate::CommandEnvelope<$command>,
            ) -> $crate::ExtensionSdkResult<$crate::ExtensionExecution> {
                self.policy_gate.authorize()?;
                let event = $append_fn(store, authority, command)?;
                Ok($crate::ExtensionExecution::from_command(
                    contract(),
                    event,
                    command,
                ))
            }
        }

        impl Default for $service {
            fn default() -> Self {
                Self::new($crate::ExtensionPolicyGate::default_deny(ALLOWED_CAPABILITIES))
            }
        }

        pub fn $append_fn<T>(
            store: &mut $crate::ExtensionEventStore,
            authority: &$crate::AuthorityContract,
            command: &$crate::CommandEnvelope<T>,
        ) -> $crate::KernelResult<$crate::ExtensionEventEnvelope> {
            $crate::append_extension_event(
                store,
                authority,
                command,
                contract(),
                CONTRACT_REFERENCE,
            )
        }

        pub fn contract() -> $crate::ExtensionContract {
            $crate::ExtensionContract::new(
                MODULE_NAME,
                EVENT_TYPE,
                $operation,
                READ_MODELS,
                ALLOWED_CAPABILITIES,
            )
        }
    };
}

include!("readme/01_module_prelude.rs");
include!("readme/02_extension_contract_new.rs");
