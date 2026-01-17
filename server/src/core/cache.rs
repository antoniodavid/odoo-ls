use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{info, warn};

const CACHE_VERSION: u32 = 1;
const CACHE_FILENAME: &str = "odoo_ls_cache.bin";

#[derive(Serialize, Deserialize, Debug)]
pub struct FileMetadata {
    pub mtime: u64,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CacheData {
    pub version: u32,
    pub server_version: String,
    pub odoo_path: String,
    pub files: HashMap<String, FileMetadata>,
}

/// Cached representation of a model field
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedField {
    pub name: String,
    pub field_type: String,
    pub string: Option<String>,
    pub required: bool,
    pub readonly: bool,
    pub compute: Option<String>,
    pub inverse: Option<String>,
    pub related: Option<String>,
    pub default: Option<String>,
    pub store: bool,
    pub help: Option<String>,
    pub translate: bool,
}

/// Cached representation of an Odoo model
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedModel {
    pub name: String,
    pub description: String,
    pub inherit: Vec<String>,
    pub inherits: Vec<(String, String)>,
    pub fields: Vec<CachedField>,
    pub is_abstract: bool,
    pub transient: bool,
    pub table: String,
    pub rec_name: Option<String>,
    pub order: String,
    pub auto: bool,
    pub log_access: bool,
    pub parent_name: String,
    pub active_name: Option<String>,
}

/// Cached representation of an Odoo module
#[derive(Serialize, Deserialize, Debug)]
pub struct CachedModule {
    pub name: String,
    pub path: String,
    pub dir_name: String,
    pub module_name: String,
    pub depends: Vec<String>,
    pub all_depends: Vec<String>,
    pub data: Vec<String>,
    pub file_hashes: HashMap<String, u64>,
    pub models: Vec<CachedModel>,
    pub xml_ids: HashMap<String, Vec<String>>,
    pub is_external: bool,
    pub processed_text_hash: u64,
}

impl CacheData {
    pub fn new(odoo_path: &str) -> Self {
        Self {
            version: CACHE_VERSION,
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            odoo_path: odoo_path.to_string(),
            files: HashMap::new(),
        }
    }
}

pub struct CacheManager {
    cache_dir: PathBuf,
    cache_path: PathBuf,
}

impl CacheManager {
    pub fn new() -> Option<Self> {
        let cache_dir = dirs::data_local_dir()?.join("odoo-ls");
        if !cache_dir.exists() {
            if let Err(e) = fs::create_dir_all(&cache_dir) {
                warn!("Failed to create cache directory: {}", e);
                return None;
            }
        }
        let cache_path = cache_dir.join(CACHE_FILENAME);
        Some(Self { cache_dir, cache_path })
    }

    pub fn load(&self, odoo_path: &str) -> Option<CacheData> {
        if !self.cache_path.exists() {
            info!("No cache file found at {:?}", self.cache_path);
            return None;
        }

        let file = match fs::File::open(&self.cache_path) {
            Ok(f) => f,
            Err(e) => {
                warn!("Failed to open cache file: {}", e);
                return None;
            }
        };

        let reader = BufReader::new(file);
        let cache: CacheData = match bincode::deserialize_from(reader) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to deserialize cache: {}", e);
                return None;
            }
        };

        if cache.version != CACHE_VERSION {
            info!("Cache version mismatch (got {}, expected {})", cache.version, CACHE_VERSION);
            return None;
        }

        if cache.server_version != env!("CARGO_PKG_VERSION") {
            info!("Server version mismatch (got {}, expected {})", cache.server_version, env!("CARGO_PKG_VERSION"));
            return None;
        }

        if cache.odoo_path != odoo_path {
            info!("Odoo path mismatch (got {}, expected {})", cache.odoo_path, odoo_path);
            return None;
        }

        info!("Loaded cache with {} file entries", cache.files.len());
        Some(cache)
    }

    pub fn save(&self, cache: &CacheData) -> bool {
        let file = match fs::File::create(&self.cache_path) {
            Ok(f) => f,
            Err(e) => {
                warn!("Failed to create cache file: {}", e);
                return false;
            }
        };

        let writer = BufWriter::new(file);
        if let Err(e) = bincode::serialize_into(writer, cache) {
            warn!("Failed to serialize cache: {}", e);
            return false;
        }

        info!("Saved cache with {} file entries to {:?}", cache.files.len(), self.cache_path);
        true
    }

    pub fn invalidate(&self) {
        if self.cache_path.exists() {
            if let Err(e) = fs::remove_file(&self.cache_path) {
                warn!("Failed to remove cache file: {}", e);
            } else {
                info!("Cache invalidated");
            }
        }
    }
}

#[derive(Debug)]
pub struct ModuleCacheManager {
    cache_dir: PathBuf,
}

impl ModuleCacheManager {
    pub fn new() -> Option<Self> {
        let cache_dir = dirs::data_local_dir()?.join("odoo-ls").join("modules");
        if !cache_dir.exists() {
            if let Err(e) = fs::create_dir_all(&cache_dir) {
                warn!("Failed to create module cache directory: {}", e);
                return None;
            }
        }
        Some(Self { cache_dir })
    }

    pub fn get_module_cache_path(&self, module_name: &str, odoo_path: &str) -> PathBuf {
        let hash = format!("{:x}", md5::compute(format!("{}:{}", odoo_path, module_name).as_bytes()));
        self.cache_dir.join(format!("{}.bin", hash))
    }

    pub fn save_module(&self, module: &CachedModule, odoo_path: &str) -> bool {
        let cache_path = self.get_module_cache_path(&module.name, odoo_path);
        let file = match fs::File::create(&cache_path) {
            Ok(f) => f,
            Err(e) => {
                warn!("Failed to create module cache file {:?}: {}", cache_path, e);
                return false;
            }
        };

        let writer = BufWriter::new(file);
        if let Err(e) = bincode::serialize_into(writer, module) {
            warn!("Failed to serialize module cache: {}", e);
            return false;
        }

        info!("Saved module cache for {} at {:?}", module.name, cache_path);
        true
    }

    pub fn load_module(&self, module_name: &str, odoo_path: &str) -> Option<CachedModule> {
        let cache_path = self.get_module_cache_path(module_name, odoo_path);

        if !cache_path.exists() {
            return None;
        }

        let file = match fs::File::open(&cache_path) {
            Ok(f) => f,
            Err(e) => {
                warn!("Failed to open module cache file {:?}: {}", cache_path, e);
                return None;
            }
        };

        let reader = BufReader::new(file);
        let module: CachedModule = match bincode::deserialize_from(reader) {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to deserialize module cache: {}", e);
                return None;
            }
        };

        Some(module)
    }

    pub fn invalidate_module(&self, module_name: &str, odoo_path: &str) {
        let cache_path = self.get_module_cache_path(module_name, odoo_path);
        if cache_path.exists() {
            if let Err(e) = fs::remove_file(&cache_path) {
                warn!("Failed to remove module cache file {:?}: {}", cache_path, e);
            }
        }
    }

    pub fn clear_all(&self) {
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "bin") {
                    let _ = fs::remove_file(&path);
                }
            }
            info!("Cleared all module caches");
        }
    }
}

pub fn get_file_metadata(path: &Path) -> Option<FileMetadata> {
    let metadata = fs::metadata(path).ok()?;
    let mtime = metadata
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let size = metadata.len();
    Some(FileMetadata { mtime, size })
}

pub fn is_file_unchanged(path: &str, cached: &FileMetadata) -> bool {
    if let Some(current) = get_file_metadata(Path::new(path)) {
        current.mtime == cached.mtime && current.size == cached.size
    } else {
        false
    }
}
