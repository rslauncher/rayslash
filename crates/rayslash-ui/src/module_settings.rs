use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

use rayslash_core::{
    app_state, apps, config, modules, projects, providers::ProviderExecutionHint, ranking, search,
};
use semver::Version;
use slint::{ComponentHandle, Timer, TimerMode, VecModel};

use crate::{
    AppWindow, ModuleItem, ResultItem,
    result_items::IconImageCache,
    runtime_state::{
        ResultRefreshContext, ResultSelection, SearchResultSet, effective_search_query,
        query_execution_hint, refresh_result_view, refresh_settings_dependent_ui,
        search_result_set,
    },
};

pub(crate) struct RuntimeModules {
    pub config: modules::ModulesConfig,
    pub writes_blocked: bool,
    pub migration_pending: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ModuleOperationState {
    pending: bool,
    label: String,
    summary: String,
    details: String,
    failed: bool,
    details_expanded: bool,
}

type ModuleOperationResult = Result<Option<modules::InstalledModule>, String>;

fn concise_feedback(details: &str) -> String {
    details
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("The module operation failed.")
        .chars()
        .take(120)
        .collect()
}

pub(crate) fn load_runtime_modules(
    legacy_providers: &config::ProviderConfig,
    main_config_loaded_successfully: bool,
    migrate_legacy: bool,
) -> RuntimeModules {
    if !main_config_loaded_successfully {
        return RuntimeModules {
            config: modules::ModulesConfig::from_legacy_provider_config(legacy_providers),
            writes_blocked: true,
            migration_pending: false,
        };
    }

    match modules::load_or_create_modules_config_with_migration(legacy_providers, migrate_legacy) {
        Ok(outcome) => {
            let migration_pending =
                outcome.was_migrated() || (migrate_legacy && outcome.was_created());
            RuntimeModules {
                config: outcome.into_config(),
                writes_blocked: false,
                migration_pending,
            }
        }
        Err(error) => {
            eprintln!("{error}; using legacy provider settings and disabling module writes");
            RuntimeModules {
                config: modules::ModulesConfig::from_legacy_provider_config(legacy_providers),
                writes_blocked: true,
                migration_pending: false,
            }
        }
    }
}

pub(crate) fn module_items(
    config: &modules::ModulesConfig,
    catalog: &[modules::RegistryModule],
) -> Vec<ModuleItem> {
    module_items_with_operations(config, catalog, &BTreeMap::new())
}

fn module_items_with_operations(
    config: &modules::ModulesConfig,
    catalog: &[modules::RegistryModule],
    operations: &BTreeMap<String, ModuleOperationState>,
) -> Vec<ModuleItem> {
    let installed = modules::load_installed_modules().unwrap_or_default();
    let mut items = catalog
        .iter()
        .map(|module| {
            let installed_module = installed.modules.get(&module.id);
            let installed_manifest = installed_module.and_then(|installed| {
                std::fs::read_to_string(installed.install_path.join("module.toml"))
                    .ok()
                    .and_then(|text| toml::from_str::<modules::ModulePackageManifest>(&text).ok())
                    .filter(|manifest| {
                        manifest.id == module.id
                            && installed.install_path.join("module.wasm").is_file()
                    })
            });
            let installed_healthy = installed_module.is_none() || installed_manifest.is_some();
            let latest = latest_compatible_version(module);
            let update_available = installed_module
                .zip(latest)
                .is_some_and(|(installed, latest)| latest.version > installed.version);
            let target_permissions = latest.map(|version| &version.permissions);
            let permission_expansion =
                installed_module
                    .zip(target_permissions)
                    .is_some_and(|(installed, target)| {
                        permissions_expand(&installed.permissions, target)
                    });
            let version = installed_module
                .map(|installed| installed.version.to_string())
                .or_else(|| latest.map(|latest| latest.version.to_string()))
                .unwrap_or_default();
            let (icon_kind, icon_text) = module_icon(&module.id);
            let operation = operations.get(&module.id).cloned().unwrap_or_default();
            ModuleItem {
                id: module.id.clone().into(),
                name: module.name.clone().into(),
                description: module.description.clone().into(),
                author: module.author.clone().into(),
                version: version.into(),
                enabled: installed_module.is_some()
                    && config
                        .is_enabled(&module.id)
                        .unwrap_or(installed_module.is_some_and(|m| m.enabled)),
                installed: installed_module.is_some(),
                official: module.official,
                stars: module.github_stars.to_string().into(),
                action: if !installed_healthy {
                    "Repair".into()
                } else if update_available && permission_expansion {
                    "Review update".into()
                } else if update_available {
                    "Update".into()
                } else if installed_module.is_some() {
                    "Remove".into()
                } else if config.is_enabled(&module.id).is_some() {
                    "Restore".into()
                } else {
                    "Install".into()
                },
                update_available,
                permissions: target_permissions
                    .map(permission_summary)
                    .unwrap_or_else(|| "No compatible version".into())
                    .into(),
                repository: module.repository.clone().into(),
                license: module.license.clone().into(),
                review_status: match module.review_status {
                    modules::ReviewStatus::Reviewed => "Reviewed",
                    modules::ReviewStatus::LimitedReview => "Limited review",
                    modules::ReviewStatus::Blocked => "Blocked",
                }
                .into(),
                status: if !installed_healthy {
                    "Installed · Broken".into()
                } else if installed_module.is_some() {
                    if config.is_enabled(&module.id).unwrap_or(true) {
                        "Installed · Enabled".into()
                    } else {
                        "Installed · Disabled".into()
                    }
                } else {
                    "Not installed".into()
                },
                operation_pending: operation.pending,
                operation_label: operation.label.into(),
                operation_summary: operation.summary.into(),
                operation_details: operation.details.into(),
                operation_failed: operation.failed,
                operation_details_expanded: operation.details_expanded,
                icon_kind: icon_kind.into(),
                icon_text: icon_text.into(),
            }
        })
        .collect::<Vec<_>>();
    for descriptor in modules::official_module_descriptors() {
        if items.iter().any(|item| item.id.as_str() == descriptor.id) {
            continue;
        }
        let installed_module = installed.modules.get(descriptor.id);
        let (icon_kind, icon_text) = module_icon(descriptor.id);
        let operation = operations.get(descriptor.id).cloned().unwrap_or_default();
        items.push(ModuleItem {
            id: descriptor.id.into(),
            name: descriptor.name.into(),
            description: descriptor.description.into(),
            author: descriptor.author.into(),
            version: installed_module
                .map(|module| module.version.to_string())
                .unwrap_or_default()
                .into(),
            enabled: installed_module.is_some()
                && config.is_enabled(descriptor.id).unwrap_or(false),
            installed: installed_module.is_some(),
            official: true,
            stars: "".into(),
            action: if installed_module.is_some() {
                "Remove"
            } else if config.is_enabled(descriptor.id).is_some() {
                "Restore"
            } else {
                "Install"
            }
            .into(),
            update_available: false,
            permissions: "Permissions shown when the verified catalog is available".into(),
            repository: "".into(),
            license: "".into(),
            review_status: "Catalog unavailable".into(),
            status: if installed_module.is_some() {
                "Installed".into()
            } else {
                "Not installed".into()
            },
            operation_pending: operation.pending,
            operation_label: operation.label.into(),
            operation_summary: operation.summary.into(),
            operation_details: operation.details.into(),
            operation_failed: operation.failed,
            operation_details_expanded: operation.details_expanded,
            icon_kind: icon_kind.into(),
            icon_text: icon_text.into(),
        });
    }
    items.sort_by(|left, right| {
        right
            .official
            .cmp(&left.official)
            .then_with(|| left.name.cmp(&right.name))
    });
    items
}

fn permission_summary(permissions: &modules::PackagePermissions) -> String {
    let mut values = Vec::new();
    if !permissions.network.is_empty() {
        values.push(format!("network ({})", permissions.network.join(", ")));
    }
    if permissions.cache {
        values.push("cache".into());
    }
    if permissions.clipboard {
        values.push("clipboard".into());
    }
    if permissions.notifications {
        values.push("notifications".into());
    }
    if permissions.commands {
        values.push("commands".into());
    }
    if values.is_empty() {
        "Capabilities: none".into()
    } else {
        format!("Capabilities: {}", values.join(", "))
    }
}

fn permissions_expand(
    current: &modules::PackagePermissions,
    next: &modules::PackagePermissions,
) -> bool {
    (!current.cache && next.cache)
        || (!current.clipboard && next.clipboard)
        || (!current.notifications && next.notifications)
        || (!current.commands && next.commands)
        || next
            .network
            .iter()
            .any(|origin| !current.network.contains(origin))
}

fn permission_expansion_summary(
    current: &modules::PackagePermissions,
    next: &modules::PackagePermissions,
) -> String {
    let mut added = next
        .network
        .iter()
        .filter(|origin| !current.network.contains(origin))
        .map(|origin| format!("network {origin}"))
        .collect::<Vec<_>>();
    for (was_enabled, is_enabled, label) in [
        (current.cache, next.cache, "cache"),
        (current.clipboard, next.clipboard, "clipboard"),
        (current.notifications, next.notifications, "notifications"),
        (current.commands, next.commands, "commands"),
    ] {
        if !was_enabled && is_enabled {
            added.push(label.into());
        }
    }
    added.join(", ")
}

fn latest_compatible_version(
    module: &modules::RegistryModule,
) -> Option<&modules::RegistryVersion> {
    if module.review_status == modules::ReviewStatus::Blocked {
        return None;
    }
    let api = Version::new(1, 0, 0);
    module
        .versions
        .iter()
        .filter(|version| !version.yanked && version.api_version.matches(&api))
        .max_by(|left, right| left.version.cmp(&right.version))
}

fn module_icon(module_id: &str) -> (&'static str, &'static str) {
    match module_id {
        modules::CALCULATOR_MODULE_ID => ("calculator", ""),
        modules::UNITS_MODULE_ID => ("text", "U"),
        modules::CURRENCY_MODULE_ID => ("text", "$"),
        modules::TIME_MODULE_ID => ("time", ""),
        modules::WEB_SEARCH_MODULE_ID => ("search", ""),
        modules::TIMERS_MODULE_ID => ("stopwatch", ""),
        modules::ALIASES_MODULE_ID => ("link", ""),
        _ => ("placeholder", ""),
    }
}

pub(crate) fn refresh_module_items(
    model: &Rc<VecModel<ModuleItem>>,
    config: &modules::ModulesConfig,
    catalog: &[modules::RegistryModule],
) {
    model.set_vec(module_items(config, catalog));
}

fn refresh_module_items_with_operations(
    model: &Rc<VecModel<ModuleItem>>,
    config: &modules::ModulesConfig,
    catalog: &[modules::RegistryModule],
    operations: &BTreeMap<String, ModuleOperationState>,
) {
    model.set_vec(module_items_with_operations(config, catalog, operations));
}

pub(crate) struct ModuleSettingsCallbackContext {
    pub module_state: Rc<RefCell<modules::ModulesConfig>>,
    pub module_catalog: Rc<RefCell<Vec<modules::RegistryModule>>>,
    pub module_model: Rc<VecModel<ModuleItem>>,
    pub module_writes_blocked: bool,
    pub config_state: Rc<RefCell<config::Config>>,
    pub app_install_state: Rc<RefCell<app_state::AppInstallState>>,
    pub ranking_state: Rc<RefCell<ranking::RankingState>>,
    pub projects: Rc<RefCell<Vec<projects::Project>>>,
    pub apps: Rc<RefCell<Vec<apps::DesktopApp>>>,
    pub current_results: Rc<RefCell<Vec<search::SearchResult>>>,
    pub results_model: Rc<VecModel<ResultItem>>,
    pub icon_cache: Rc<RefCell<IconImageCache>>,
    pub socket_path: PathBuf,
    pub remote_search_generation: Arc<AtomicU64>,
    pub remote_result_tx: mpsc::Sender<(u64, String, SearchResultSet)>,
    pub profile: bool,
}

pub(crate) fn register_module_settings_callback(
    ui: &AppWindow,
    context: ModuleSettingsCallbackContext,
) -> Timer {
    let ModuleSettingsCallbackContext {
        module_state,
        module_catalog,
        module_model,
        module_writes_blocked,
        config_state,
        app_install_state,
        ranking_state,
        projects,
        apps,
        current_results,
        results_model,
        icon_cache,
        socket_path,
        remote_search_generation,
        remote_result_tx,
        profile,
    } = context;

    let (install_tx, install_rx) = mpsc::channel::<(String, String, ModuleOperationResult)>();
    let operations = Rc::new(RefCell::new(BTreeMap::<String, ModuleOperationState>::new()));
    let pending_permission_approvals = Rc::new(RefCell::new(BTreeSet::<String>::new()));
    let install_timer = Timer::default();
    install_timer.start(TimerMode::Repeated, std::time::Duration::from_millis(50), {
        let weak = ui.as_weak();
        let module_state = module_state.clone();
        let module_catalog = module_catalog.clone();
        let module_model = module_model.clone();
        let operations = operations.clone();
        move || {
            let completions = install_rx.try_iter().collect::<Vec<_>>();
            if completions.is_empty() {
                return;
            }
            let Some(ui) = weak.upgrade() else {
                return;
            };
            for (module_id, action, result) in completions {
                match result {
                    Ok(installed) => {
                        let mut config = module_state.borrow().clone();
                        if let Some(installed) = installed {
                            config.set_installed(&module_id, &installed.version.to_string(), true);
                        } else {
                            config.remove(&module_id);
                        }
                        if let Err(error) = modules::save_modules_config(&config) {
                            let details = format!(
                                "Module changed, but its setting could not be saved: {error}"
                            );
                            operations.borrow_mut().insert(
                                module_id.clone(),
                                ModuleOperationState {
                                    summary: concise_feedback(&details),
                                    details: details.clone(),
                                    failed: true,
                                    ..Default::default()
                                },
                            );
                            ui.set_status_text(details.into());
                        } else {
                            *module_state.borrow_mut() = config;
                            let message = match action.as_str() {
                                "Remove" => {
                                    "Module code removed; settings and data were kept".to_owned()
                                }
                                "Remove all" => {
                                    "Module code, settings, state, and cache removed".to_owned()
                                }
                                _ => format!("Module {} completed", action.to_ascii_lowercase()),
                            };
                            operations.borrow_mut().insert(
                                module_id.clone(),
                                ModuleOperationState {
                                    summary: message.clone(),
                                    details: message.clone(),
                                    ..Default::default()
                                },
                            );
                            ui.set_status_text(message.into());
                        }
                    }
                    Err(error) => {
                        let details =
                            format!("Could not {} module: {error}", action.to_ascii_lowercase());
                        operations.borrow_mut().insert(
                            module_id.clone(),
                            ModuleOperationState {
                                summary: concise_feedback(&details),
                                details: details.clone(),
                                failed: true,
                                ..Default::default()
                            },
                        );
                        ui.set_status_text(details.into());
                    }
                }
                refresh_module_items_with_operations(
                    &module_model,
                    &module_state.borrow(),
                    &module_catalog.borrow(),
                    &operations.borrow(),
                );
            }
        }
    });

    ui.on_settings_module_action_requested({
        let weak = ui.as_weak();
        let module_state = module_state.clone();
        let module_catalog = module_catalog.clone();
        let module_model = module_model.clone();
        let install_tx = install_tx.clone();
        let pending_permission_approvals = pending_permission_approvals.clone();
        let operations = operations.clone();
        move |module_id, action| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if module_writes_blocked {
                ui.set_status_text(
                    "Module state is read-only until the configuration error is fixed.".into(),
                );
                return;
            }
            let module_id = module_id.as_str();
            if action.as_str() == "Details" {
                if let Some(operation) = operations.borrow_mut().get_mut(module_id) {
                    operation.details_expanded = !operation.details_expanded;
                }
                refresh_module_items_with_operations(
                    &module_model,
                    &module_state.borrow(),
                    &module_catalog.borrow(),
                    &operations.borrow(),
                );
                return;
            }
            if action.as_str() == "Source" {
                let repository = module_catalog
                    .borrow()
                    .iter()
                    .find(|module| module.id == module_id)
                    .map(|module| module.repository.clone());
                match repository {
                    Some(repository) => {
                        if let Err(error) = rayslash_core::actions::run_module_action(
                            &search::ModuleAction::OpenUrl(repository),
                        ) {
                            ui.set_status_text(
                                format!("Could not open module source: {error}").into(),
                            );
                        }
                    }
                    None => ui.set_status_text("Module source is unavailable offline.".into()),
                }
                return;
            }
            if operations
                .borrow()
                .get(module_id)
                .is_some_and(|state| state.pending)
            {
                return;
            }
            match action.as_str() {
                "Install" | "Restore" | "Repair" | "Update" | "Review update" => {
                    let catalog = module_catalog.borrow();
                    let Some(module) = catalog.iter().find(|module| module.id == module_id) else {
                        let details =
                            "The verified registry has no installable record for this module.";
                        operations.borrow_mut().insert(
                            module_id.to_owned(),
                            ModuleOperationState {
                                summary: details.into(),
                                details: details.into(),
                                failed: true,
                                ..Default::default()
                            },
                        );
                        refresh_module_items_with_operations(
                            &module_model,
                            &module_state.borrow(),
                            &module_catalog.borrow(),
                            &operations.borrow(),
                        );
                        ui.set_status_text(details.into());
                        return;
                    };
                    let Some(version) = latest_compatible_version(module) else {
                        let details = "No compatible module version is available.";
                        operations.borrow_mut().insert(
                            module_id.to_owned(),
                            ModuleOperationState {
                                summary: details.into(),
                                details: details.into(),
                                failed: true,
                                ..Default::default()
                            },
                        );
                        refresh_module_items_with_operations(
                            &module_model,
                            &module_state.borrow(),
                            &module_catalog.borrow(),
                            &operations.borrow(),
                        );
                        ui.set_status_text(details.into());
                        return;
                    };
                    if action.as_str() == "Review update" {
                        let current_permissions = modules::load_installed_modules()
                            .ok()
                            .and_then(|installed| installed.modules.get(module_id).cloned())
                            .map(|installed| installed.permissions)
                            .unwrap_or_default();
                        let changes = permission_expansion_summary(
                            &current_permissions,
                            &version.permissions,
                        );
                        let mut approvals = pending_permission_approvals.borrow_mut();
                        if !approvals.remove(module_id) {
                            approvals.insert(module_id.to_owned());
                            let details = format!(
                                "New capabilities: {changes}. Click Review update again to approve."
                            );
                            operations.borrow_mut().insert(
                                module_id.to_owned(),
                                ModuleOperationState {
                                    summary: concise_feedback(&details),
                                    details: details.clone(),
                                    ..Default::default()
                                },
                            );
                            refresh_module_items_with_operations(
                                &module_model,
                                &module_state.borrow(),
                                &module_catalog.borrow(),
                                &operations.borrow(),
                            );
                            ui.set_status_text(details.into());
                            return;
                        }
                    } else {
                        pending_permission_approvals.borrow_mut().remove(module_id);
                    }
                    let label = format!("{}…", action.trim_end_matches(" update"));
                    operations.borrow_mut().insert(
                        module_id.to_owned(),
                        ModuleOperationState {
                            pending: true,
                            label: label.clone(),
                            summary: format!("{} {}…", action, module.name),
                            ..Default::default()
                        },
                    );
                    refresh_module_items_with_operations(
                        &module_model,
                        &module_state.borrow(),
                        &module_catalog.borrow(),
                        &operations.borrow(),
                    );
                    ui.set_status_text(format!("{} {}…", action, module.name).into());
                    let module = module.clone();
                    let version = version.clone();
                    let module_id = module_id.to_owned();
                    let action = action.to_string();
                    let install_tx = install_tx.clone();
                    thread::spawn(move || {
                        let result = modules::install_registry_version(&module, &version)
                            .map(Some)
                            .map_err(|error| error.to_string());
                        let _ = install_tx.send((module_id, action, result));
                    });
                }
                "Remove" | "Remove all" => {
                    let module_id = module_id.to_owned();
                    let action = action.to_string();
                    operations.borrow_mut().insert(
                        module_id.clone(),
                        ModuleOperationState {
                            pending: true,
                            label: "Removing…".into(),
                            summary: "Removing module…".into(),
                            ..Default::default()
                        },
                    );
                    refresh_module_items_with_operations(
                        &module_model,
                        &module_state.borrow(),
                        &module_catalog.borrow(),
                        &operations.borrow(),
                    );
                    let install_tx = install_tx.clone();
                    thread::spawn(move || {
                        let result =
                            modules::remove_installed_module(&module_id, action == "Remove all")
                                .map(|_| None)
                                .map_err(|error| error.to_string());
                        let _ = install_tx.send((module_id, action, result));
                    });
                }
                _ => {
                    ui.set_status_text(format!("Unknown module action: {action}").into());
                }
            }
        }
    });

    ui.on_settings_module_toggle_requested({
        let weak = ui.as_weak();
        move |module_id, enabled| {
            let Some(ui) = weak.upgrade() else {
                return;
            };

            if module_writes_blocked {
                refresh_module_items(&module_model, &module_state.borrow(), &module_catalog.borrow());
                ui.set_status_text(
                    "Could not save module settings: fix config.toml or modules.toml and restart rayslash."
                        .into(),
                );
                return;
            }

            let module_id = module_id.as_str();
            let module_name = module_catalog
                .borrow()
                .iter()
                .find(|module| module.id == module_id)
                .map(|module| module.name.clone())
                .or_else(|| {
                    modules::official_module_descriptor(module_id)
                        .map(|module| module.name.to_owned())
                })
                .unwrap_or_else(|| module_id.to_owned());
            let mut next_modules = module_state.borrow().clone();
            let changed = match next_modules.set_installed_enabled(module_id, enabled) {
                Ok(changed) => changed,
                Err(error) => {
                    refresh_module_items(&module_model, &module_state.borrow(), &module_catalog.borrow());
                    ui.set_status_text(format!("Could not update module: {error}").into());
                    return;
                }
            };
            if !changed {
                return;
            }

            if let Err(error) = modules::save_modules_config(&next_modules) {
                eprintln!("{error}");
                refresh_module_items(&module_model, &module_state.borrow(), &module_catalog.borrow());
                ui.set_status_text(format!("Could not save module setting: {error}").into());
                return;
            }

            *module_state.borrow_mut() = next_modules.clone();
            refresh_module_items(&module_model, &next_modules, &module_catalog.borrow());

            let compatibility_config = {
                let mut next_config = config_state.borrow().clone();
                next_modules.apply_to_provider_config(&mut next_config.providers);
                *config_state.borrow_mut() = next_config.clone();
                next_config
            };

            let compatibility_error =
                config::save_config_with_backup(&compatibility_config).err();
            if let Some(error) = compatibility_error.as_ref() {
                eprintln!(
                    "module state was saved, but config.toml compatibility mirror failed: {error}"
                );
            }

            let query = ui.get_query_text();
            let effective_query = effective_search_query(
                query.as_str(),
                ui.get_active_search_keyword().as_str(),
            );
            let needs_remote_lookup = matches!(
                query_execution_hint(&config_state.borrow()),
                ProviderExecutionHint::DebouncedNetwork { .. }
            );

            if needs_remote_lookup {
                let generation = remote_search_generation.fetch_add(1, Ordering::Relaxed) + 1;
                let expected_generation = remote_search_generation.clone();
                let config = config_state.borrow().clone();
                let ranking = ranking_state.borrow().clone();
                let app_install = app_install_state.borrow().clone();
                let projects_snapshot = projects.borrow().clone();
                let apps_snapshot = apps.borrow().clone();
                let query = effective_query.clone();
                let remote_result_tx = remote_result_tx.clone();
                thread::spawn(move || {
                    let result_set = search_result_set(
                        &config,
                        &ranking,
                        &app_install,
                        &projects_snapshot,
                        &apps_snapshot,
                        &query,
                    );
                    if expected_generation.load(Ordering::Relaxed) == generation {
                        let _ = remote_result_tx.send((generation, query, result_set));
                    }
                });
            } else {
                remote_search_generation.fetch_add(1, Ordering::Relaxed);
                refresh_result_view(
                    &ui,
                    ResultRefreshContext {
                        config: &config_state.borrow(),
                        ranking_state: &ranking_state.borrow(),
                        app_state: &app_install_state.borrow(),
                        projects: &projects.borrow(),
                        apps: &apps.borrow(),
                        current_results: &current_results,
                        results_model: &results_model,
                        icon_cache: &icon_cache,
                        profile,
                    },
                    effective_query.as_str(),
                    ResultSelection::QueryDefault,
                );
            }
            refresh_settings_dependent_ui(
                &ui,
                &config_state.borrow(),
                &projects.borrow(),
                &apps.borrow(),
                &ranking_state.borrow(),
                &icon_cache,
                &socket_path,
            );

            let state_label = if enabled { "enabled" } else { "disabled" };
            if compatibility_error.is_some() {
                ui.set_status_text(
                    format!(
                        "{} {state_label}; config.toml compatibility mirror failed.",
                        module_name
                    )
                    .into(),
                );
            } else {
                ui.set_status_text(format!("{module_name} {state_label}.").into());
            }
        }
    });

    install_timer
}

#[cfg(test)]
mod tests {
    use rayslash_core::config::ProviderConfig;
    use rayslash_core::modules::{CALCULATOR_MODULE_ID, OFFICIAL_AUTHOR};

    use super::*;

    #[test]
    fn module_items_reflect_official_descriptors_and_enabled_state() {
        let mut config = modules::ModulesConfig::default();
        config
            .disable(CALCULATOR_MODULE_ID)
            .expect("disable calculator");

        let items = module_items(&config, &[]);
        assert_eq!(items.len(), modules::official_module_descriptors().len());
        assert!(items.iter().all(|item| item.official));
        assert!(
            items
                .iter()
                .all(|item| item.author.as_str() == OFFICIAL_AUTHOR)
        );

        let calculator = items
            .iter()
            .find(|item| item.id.as_str() == CALCULATOR_MODULE_ID)
            .expect("calculator item");
        assert_eq!(calculator.name.as_str(), "Calculator");
        assert!(!calculator.enabled);
        assert!(!calculator.description.is_empty());
    }

    #[test]
    fn failed_main_config_uses_legacy_module_state_and_blocks_writes() {
        let legacy = ProviderConfig {
            calculator: false,
            utility_actions: false,
            ..ProviderConfig::default()
        };

        let runtime = load_runtime_modules(&legacy, false, true);

        assert!(runtime.writes_blocked);
        assert_eq!(runtime.config.is_enabled(CALCULATOR_MODULE_ID), Some(false));
        assert_eq!(
            runtime
                .config
                .is_enabled(rayslash_core::modules::TIMERS_MODULE_ID),
            Some(false)
        );
    }

    #[test]
    fn permission_expansion_lists_only_new_capabilities() {
        let current = modules::PackagePermissions {
            network: vec!["https://old.example".into()],
            cache: true,
            ..Default::default()
        };
        let next = modules::PackagePermissions {
            network: vec!["https://old.example".into(), "https://new.example".into()],
            cache: true,
            notifications: true,
            ..Default::default()
        };
        assert_eq!(
            permission_expansion_summary(&current, &next),
            "network https://new.example, notifications"
        );
    }

    #[test]
    fn module_operation_state_is_scoped_to_its_card() {
        let mut operations = BTreeMap::new();
        operations.insert(
            CALCULATOR_MODULE_ID.to_owned(),
            ModuleOperationState {
                pending: true,
                label: "Installing…".into(),
                summary: "Installing Calculator…".into(),
                ..Default::default()
            },
        );

        let items =
            module_items_with_operations(&modules::ModulesConfig::empty(), &[], &operations);
        let calculator = items
            .iter()
            .find(|item| item.id == CALCULATOR_MODULE_ID)
            .unwrap();
        assert!(calculator.operation_pending);
        assert_eq!(calculator.operation_label.as_str(), "Installing…");
        assert!(items.iter().filter(|item| item.operation_pending).count() == 1);
    }

    #[test]
    fn long_operation_errors_keep_full_details_and_a_concise_summary() {
        let details = format!(
            "Could not install module: invalid module package:\n{}",
            "module failed its startup probe: host missing; ".repeat(20)
        );
        let summary = concise_feedback(&details);

        assert!(summary.len() <= 120);
        assert_eq!(summary, "Could not install module: invalid module package:");
        assert!(details.len() > summary.len());
    }
}
