use std::sync::{Arc, Mutex};

use crate::activity::ActivityStore;
use crate::data::DataStore;
use crate::error::AppResult;
use crate::gateway::{GatewayExposureProcess, GatewayProcess};
use crate::runtime::RuntimeSupervisor;

pub struct AppState {
    pub activity: Arc<ActivityStore>,
    pub data: Mutex<DataStore>,
    pub runtime: Mutex<RuntimeSupervisor>,
    pub gateway: Mutex<Option<GatewayProcess>>,
    pub gateway_exposure: Mutex<Option<GatewayExposureProcess>>,
}

impl AppState {
    pub fn new() -> AppResult<Self> {
        let mut store = DataStore::load()?;
        store.init_shared_secrets()?;
        Ok(Self {
            activity: Arc::new(ActivityStore::new()),
            data: Mutex::new(store),
            runtime: Mutex::new(RuntimeSupervisor::default()),
            gateway: Mutex::new(None),
            gateway_exposure: Mutex::new(None),
        })
    }

    pub fn with_data<R>(&self, f: impl FnOnce(&mut DataStore) -> AppResult<R>) -> AppResult<R> {
        let mut guard = self
            .data
            .lock()
            .map_err(|_| crate::error::AppError::Message("data store poisoned".into()))?;
        f(&mut guard)
    }

    pub fn with_workspaces<R>(&self, f: impl FnOnce(&mut DataStore) -> AppResult<R>) -> AppResult<R> {
        self.with_data(f)
    }

    pub fn with_settings<R>(&self, f: impl FnOnce(&mut DataStore) -> AppResult<R>) -> AppResult<R> {
        self.with_data(f)
    }

    pub fn with_runtime<R>(&self, f: impl FnOnce(&mut RuntimeSupervisor) -> AppResult<R>) -> AppResult<R> {
        let mut guard = self
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }

    pub fn with_gateway<R>(
        &self,
        f: impl FnOnce(&mut Option<GatewayProcess>) -> AppResult<R>,
    ) -> AppResult<R> {
        let mut guard = self
            .gateway
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }

    pub fn with_gateway_exposure<R>(
        &self,
        f: impl FnOnce(&mut Option<GatewayExposureProcess>) -> AppResult<R>,
    ) -> AppResult<R> {
        let mut guard = self
            .gateway_exposure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new().expect("failed to initialize app state")
    }
}

pub fn bootstrap_workspace(store: &mut DataStore, profile_id: &str) -> AppResult<()> {
    store.init_workspace_secrets(profile_id)
}

pub fn teardown_workspace(store: &mut DataStore, profile_id: &str) -> AppResult<()> {
    store.remove_workspace_secrets(profile_id)
}
