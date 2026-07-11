mod builtins;
mod types;

pub use builtins::{builtin_provider_catalog, builtin_providers};
pub use types::{
    ProviderAction, ProviderConfig, ProviderContext, ProviderDiagnostics, ProviderExecutionHint,
    ProviderHealth, ProviderId, ProviderMetadata, ProviderOutcome, ProviderPermissions,
    ProviderResult,
};

pub trait Provider: Sync {
    fn metadata(&self) -> &'static ProviderMetadata;

    fn config(&self, context: &ProviderContext<'_>) -> ProviderConfig;

    fn execution_hint(&self, _context: &ProviderContext<'_>) -> ProviderExecutionHint {
        ProviderExecutionHint::Local
    }

    fn diagnostics(&self, context: &ProviderContext<'_>) -> ProviderDiagnostics {
        let enabled = self.config(context).enabled;
        ProviderDiagnostics {
            provider_id: self.metadata().id.clone(),
            health: if enabled {
                ProviderHealth::Ready
            } else {
                ProviderHealth::Disabled
            },
            message: if enabled {
                "Ready".to_owned()
            } else {
                "Disabled by configuration".to_owned()
            },
        }
    }

    fn query(&self, context: &ProviderContext<'_>) -> ProviderOutcome;

    fn run(&self, context: &ProviderContext<'_>) -> ProviderOutcome {
        let mut outcome = self.query(context);
        outcome.execution_hint = self.execution_hint(context);
        outcome
    }
}

pub fn query_execution_hint(context: &ProviderContext<'_>) -> ProviderExecutionHint {
    builtin_providers()
        .iter()
        .filter(|provider| provider.config(context).enabled)
        .filter_map(|provider| match provider.execution_hint(context) {
            ProviderExecutionHint::Local => None,
            ProviderExecutionHint::DebouncedNetwork { debounce_ms } => Some(debounce_ms),
        })
        .max()
        .map_or(ProviderExecutionHint::Local, |debounce_ms| {
            ProviderExecutionHint::DebouncedNetwork { debounce_ms }
        })
}
