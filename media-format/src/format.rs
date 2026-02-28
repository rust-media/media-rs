//! Format

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, RwLock},
};

use media_core::{invalid_error, not_found_error, Result};
use media_format_types::FormatBuilder;

pub struct FormatList<T: ?Sized + FormatBuilder> {
    format_builders: Vec<Arc<T>>,
    extension_map: HashMap<String, Arc<T>>,
}

impl<T: ?Sized + FormatBuilder> FormatList<T> {
    pub fn new() -> Self {
        Self {
            format_builders: Vec::new(),
            extension_map: HashMap::new(),
        }
    }

    /// Returns an iterator over all registered format builders
    pub fn iter(&self) -> impl Iterator<Item = &Arc<T>> {
        self.format_builders.iter()
    }

    /// Returns the number of registered format builders
    pub fn len(&self) -> usize {
        self.format_builders.len()
    }

    /// Returns true if no format builders are registered
    pub fn is_empty(&self) -> bool {
        self.format_builders.is_empty()
    }
}

impl<B: ?Sized + FormatBuilder> Default for FormatList<B> {
    fn default() -> Self {
        Self::new()
    }
}

/// Lazy-initialized format list with RwLock for thread safety
pub type LazyFormatList<T> = LazyLock<RwLock<FormatList<T>>>;

/// Registers a format builder in the format list
pub fn register_format<T>(format_list: &LazyFormatList<T>, builder: Arc<T>) -> Result<()>
where
    T: ?Sized + FormatBuilder,
{
    let mut list = format_list.write().map_err(|err| invalid_error!(err.to_string()))?;

    list.format_builders.push(Arc::clone(&builder));

    // Register by extensions
    for &ext in builder.extensions() {
        let ext = ext.to_lowercase();
        list.extension_map.insert(ext, Arc::clone(&builder));
    }

    Ok(())
}

/// Find a format builder by file extension
pub fn find_format_by_extension<T: ?Sized + FormatBuilder>(format_list: &LazyFormatList<T>, ext: &str) -> Result<Arc<T>> {
    let list = format_list.read().map_err(|err| invalid_error!(err.to_string()))?;

    let ext = ext.to_lowercase();
    list.extension_map.get(&ext).cloned().ok_or_else(|| not_found_error!("format for extension", ext))
}
