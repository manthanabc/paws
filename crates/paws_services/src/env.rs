use std::sync::Arc;

use paws_app::domain::Environment;
use paws_app::{EnvironmentInfra, EnvironmentService};

pub struct PawsEnvironmentService<F>(Arc<F>);

impl<F> PawsEnvironmentService<F> {
    pub fn new(infra: Arc<F>) -> Self {
        Self(infra)
    }
}

impl<F: EnvironmentInfra> EnvironmentService for PawsEnvironmentService<F> {
    fn get_environment(&self) -> Environment {
        self.0.get_environment()
    }
}
