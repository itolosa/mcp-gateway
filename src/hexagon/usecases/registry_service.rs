use std::collections::BTreeMap;

use crate::hexagon::ports::ProviderConfigStore;
use crate::hexagon::usecases::registry_error::RegistryError;

use super::add_allowed_operations::AddAllowedOperations;
use super::add_denied_operations::AddDeniedOperations;
use super::add_provider::AddProvider;
use super::get_allowed_operations::GetAllowedOperations;
use super::get_denied_operations::GetDeniedOperations;
use super::list_providers::ListProviders;
use super::remove_allowed_operations::RemoveAllowedOperations;
use super::remove_denied_operations::RemoveDeniedOperations;
use super::remove_provider::RemoveProvider;

pub struct RegistryService<S: ProviderConfigStore> {
    store: S,
}

impl<S: ProviderConfigStore> RegistryService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn list_providers(&self) -> Result<BTreeMap<String, S::Entry>, RegistryError> {
        ListProviders::execute(&self.store)
    }

    pub fn add_provider(&self, name: String, entry: S::Entry) -> Result<(), RegistryError> {
        AddProvider::execute(&self.store, name, entry)
    }

    pub fn remove_provider(&self, name: &str) -> Result<(), RegistryError> {
        RemoveProvider::execute(&self.store, name)
    }

    pub fn get_allowed_operations(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        GetAllowedOperations::execute(&self.store, name)
    }

    pub fn add_allowed_operations(
        &self,
        name: &str,
        tools: &[String],
    ) -> Result<(), RegistryError> {
        AddAllowedOperations::execute(&self.store, name, tools)
    }

    pub fn remove_allowed_operations(
        &self,
        name: &str,
        tools: &[String],
    ) -> Result<(), RegistryError> {
        RemoveAllowedOperations::execute(&self.store, name, tools)
    }

    pub fn get_denied_operations(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        GetDeniedOperations::execute(&self.store, name)
    }

    pub fn add_denied_operations(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        AddDeniedOperations::execute(&self.store, name, tools)
    }

    pub fn remove_denied_operations(
        &self,
        name: &str,
        tools: &[String],
    ) -> Result<(), RegistryError> {
        RemoveDeniedOperations::execute(&self.store, name, tools)
    }
}
