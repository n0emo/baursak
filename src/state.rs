use std::sync::Arc;

use indexmap::IndexMap;
use tokio::sync::RwLock;

use crate::task::Task;

#[derive(Default)]
pub struct App {
    pub tasks: Arc<RwLock<IndexMap<String, Task>>>,
}
