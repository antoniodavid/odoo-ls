use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{info, warn};

const CACHE_VERSION: u32 = 3;
const CACHE_FILENAME: &str = "odoo_ls_cache.bin";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedTextRange {
    pub start: u32,
    pub end: u32,
}

impl Default for CachedTextRange {
    fn default() -> Self {
        Self { start: 0, end: 0 }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CachedSymbolType {
    File,
    Class,
    Function,
    Variable,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedVariable {
    pub name: String,
    pub range: CachedTextRange,
    pub is_import_variable: bool,
    pub is_parameter: bool,
    pub doc_string: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedArgument {
    pub name: String,
    pub arg_type: String,
    pub has_default: bool,
}

impl CachedArgument {
    pub fn to_argument_type(&self) -> crate::core::symbols::function_symbol::ArgumentType {
        use crate::core::symbols::function_symbol::ArgumentType;
        match self.arg_type.as_str() {
            "POS_ONLY" => ArgumentType::POS_ONLY,
            "ARG" => ArgumentType::ARG,
            "KWARG" => ArgumentType::KWARG,
            "VARARG" => ArgumentType::VARARG,
            "KWORD_ONLY" => ArgumentType::KWORD_ONLY,
            _ => ArgumentType::ARG,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedFunction {
    pub name: String,
    pub range: CachedTextRange,
    pub body_start: u32,
    pub is_static: bool,
    pub is_property: bool,
    pub is_class_method: bool,
    pub doc_string: Option<String>,
    pub args: Vec<CachedArgument>,
    pub symbols: Vec<CachedSymbol>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedClass {
    pub name: String,
    pub range: CachedTextRange,
    pub body_start: u32,
    pub doc_string: Option<String>,
    pub base_names: Vec<String>,
    pub model: Option<CachedModel>,
    pub symbols: Vec<CachedSymbol>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedFile {
    pub name: String,
    pub path: String,
    pub processed_text_hash: u64,
    pub symbols: Vec<CachedSymbol>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CachedSymbol {
    Variable(CachedVariable),
    Function(CachedFunction),
    Class(CachedClass),
}

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
    pub files: Vec<CachedFile>,
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

impl CachedTextRange {
    pub fn from_text_range(range: ruff_text_size::TextRange) -> Self {
        Self {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        }
    }

    pub fn to_text_range(&self) -> ruff_text_size::TextRange {
        ruff_text_size::TextRange::new(
            ruff_text_size::TextSize::new(self.start),
            ruff_text_size::TextSize::new(self.end),
        )
    }
}

impl CachedVariable {
    pub fn from_variable_symbol(var: &crate::core::symbols::variable_symbol::VariableSymbol) -> Self {
        Self {
            name: var.name.to_string(),
            range: CachedTextRange::from_text_range(var.range),
            is_import_variable: var.is_import_variable,
            is_parameter: var.is_parameter,
            doc_string: var.doc_string.clone(),
        }
    }
}

impl CachedFunction {
    pub fn from_function_symbol(func: &crate::core::symbols::function_symbol::FunctionSymbol) -> Self {
        let args: Vec<CachedArgument> = func.args.iter().map(|arg| {
            let name = arg.symbol.upgrade()
                .map(|s| s.borrow().name().to_string())
                .unwrap_or_default();
            CachedArgument {
                name,
                arg_type: format!("{:?}", arg.arg_type),
                has_default: arg.default_value.is_some(),
            }
        }).collect();

        let symbols = collect_cached_symbols_from_hashmap(&func.symbols);

        Self {
            name: func.name.to_string(),
            range: CachedTextRange::from_text_range(func.range),
            body_start: func.body_range.start().to_u32(),
            is_static: func.is_static,
            is_property: func.is_property,
            is_class_method: func.is_class_method,
            doc_string: func.doc_string.clone(),
            args,
            symbols,
        }
    }
}

impl CachedClass {
    pub fn from_class_symbol(class: &crate::core::symbols::class_symbol::ClassSymbol) -> Self {
        let base_names: Vec<String> = class.bases.iter()
            .filter_map(|w| w.upgrade())
            .map(|s| s.borrow().name().to_string())
            .collect();

        let model = class._model.as_ref().map(|m| CachedModel {
            name: m.name.to_string(),
            description: m.description.clone(),
            inherit: m.inherit.iter().map(|s| s.to_string()).collect(),
            inherits: m.inherits.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            fields: Vec::new(),
            is_abstract: m.is_abstract,
            transient: m.transient,
            table: m.table.clone(),
            rec_name: m.rec_name.clone(),
            order: m.order.clone(),
            auto: m.auto,
            log_access: m.log_access,
            parent_name: m.parent_name.clone(),
            active_name: m.active_name.clone(),
        });

        let symbols = collect_cached_symbols_from_hashmap(&class.symbols);

        Self {
            name: class.name.to_string(),
            range: CachedTextRange::from_text_range(class.range),
            body_start: class.body_range.start().to_u32(),
            doc_string: class.doc_string.clone(),
            base_names,
            model,
            symbols,
        }
    }
}

fn collect_cached_symbols_from_hashmap(
    symbols: &std::collections::HashMap<crate::constants::OYarn, std::collections::HashMap<u32, Vec<std::rc::Rc<std::cell::RefCell<crate::core::symbols::symbol::Symbol>>>>>
) -> Vec<CachedSymbol> {
    use crate::constants::SymType;
    
    let mut result = Vec::new();
    for sections in symbols.values() {
        for syms in sections.values() {
            for sym_rc in syms {
                let sym = sym_rc.borrow();
                match sym.typ() {
                    SymType::VARIABLE => {
                        result.push(CachedSymbol::Variable(
                            CachedVariable::from_variable_symbol(sym.as_variable())
                        ));
                    }
                    SymType::FUNCTION => {
                        result.push(CachedSymbol::Function(
                            CachedFunction::from_function_symbol(sym.as_func())
                        ));
                    }
                    SymType::CLASS => {
                        result.push(CachedSymbol::Class(
                            CachedClass::from_class_symbol(sym.as_class_sym())
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
    result
}

impl CachedFile {
    pub fn from_file_symbol(file: &crate::core::symbols::file_symbol::FileSymbol) -> Self {
        let symbols = collect_cached_symbols_from_hashmap(&file.symbols);
        Self {
            name: file.name.to_string(),
            path: file.path.clone(),
            processed_text_hash: file.processed_text_hash,
            symbols,
        }
    }
}

fn add_symbol_to_parent(
    parent: &std::rc::Rc<std::cell::RefCell<crate::core::symbols::symbol::Symbol>>,
    child: &std::rc::Rc<std::cell::RefCell<crate::core::symbols::symbol::Symbol>>,
    section: u32,
) {
    use crate::core::symbols::symbol::Symbol;
    use crate::core::symbols::package_symbol::PackageSymbol;
    
    let mut parent_ref = parent.borrow_mut();
    match &mut *parent_ref {
        Symbol::File(f) => f.add_symbol(child, section),
        Symbol::Package(PackageSymbol::Module(m)) => m.add_symbol(child, section),
        Symbol::Package(PackageSymbol::PythonPackage(p)) => p.add_symbol(child, section),
        Symbol::Class(c) => c.add_symbol(child, section),
        Symbol::Function(f) => f.add_symbol(child, section),
        _ => {}
    }
}

pub fn restore_symbols_to_parent(
    cached_symbols: &[CachedSymbol],
    parent: std::rc::Rc<std::cell::RefCell<crate::core::symbols::symbol::Symbol>>,
    is_external: bool,
) {
    use crate::core::symbols::symbol::Symbol;
    use crate::core::symbols::variable_symbol::VariableSymbol;
    use crate::core::symbols::function_symbol::FunctionSymbol;
    use crate::core::symbols::class_symbol::ClassSymbol;
    use crate::oyarn;
    use std::rc::Rc;
    use std::cell::RefCell;

    for cached_sym in cached_symbols {
        match cached_sym {
            CachedSymbol::Variable(cv) => {
                let mut var = VariableSymbol::new(
                    oyarn!("{}", cv.name),
                    cv.range.to_text_range(),
                    is_external,
                );
                var.is_import_variable = cv.is_import_variable;
                var.is_parameter = cv.is_parameter;
                var.doc_string = cv.doc_string.clone();
                let var_rc = Rc::new(RefCell::new(Symbol::Variable(var)));
                var_rc.borrow_mut().set_weak_self(Rc::downgrade(&var_rc));
                var_rc.borrow_mut().set_parent(Some(Rc::downgrade(&parent)));
                add_symbol_to_parent(&parent, &var_rc, 0);
            }
            CachedSymbol::Function(cf) => {
                let range = cf.range.to_text_range();
                let body_start = ruff_text_size::TextSize::new(cf.body_start);
                let mut func = FunctionSymbol::new(
                    cf.name.clone(),
                    range,
                    body_start,
                    is_external,
                );
                func.is_static = cf.is_static;
                func.is_property = cf.is_property;
                func.is_class_method = cf.is_class_method;
                func.doc_string = cf.doc_string.clone();

                let func_rc = Rc::new(RefCell::new(Symbol::Function(func)));
                func_rc.borrow_mut().set_weak_self(Rc::downgrade(&func_rc));
                func_rc.borrow_mut().set_parent(Some(Rc::downgrade(&parent)));

                restore_symbols_to_parent(&cf.symbols, func_rc.clone(), is_external);
                restore_function_args(&cf.args, &func_rc);
                
                add_symbol_to_parent(&parent, &func_rc, 0);
            }
            CachedSymbol::Class(cc) => {
                let range = cc.range.to_text_range();
                let body_start = ruff_text_size::TextSize::new(cc.body_start);
                let mut class = ClassSymbol::new(
                    cc.name.clone(),
                    range,
                    body_start,
                    is_external,
                );
                class.doc_string = cc.doc_string.clone();

                if let Some(cached_model) = &cc.model {
                    let mut model_data = crate::core::model::ModelData::new();
                    model_data.name = oyarn!("{}", cached_model.name);
                    model_data.description = cached_model.description.clone();
                    model_data.inherit = cached_model.inherit.iter().map(|s| oyarn!("{}", s)).collect();
                    model_data.inherits = cached_model.inherits.iter()
                        .map(|(k, v)| (oyarn!("{}", k), oyarn!("{}", v))).collect();
                    model_data.is_abstract = cached_model.is_abstract;
                    model_data.transient = cached_model.transient;
                    model_data.table = cached_model.table.clone();
                    model_data.rec_name = cached_model.rec_name.clone();
                    model_data.order = cached_model.order.clone();
                    model_data.auto = cached_model.auto;
                    model_data.log_access = cached_model.log_access;
                    model_data.parent_name = cached_model.parent_name.clone();
                    model_data.active_name = cached_model.active_name.clone();
                    class._model = Some(model_data);
                }

                let class_rc = Rc::new(RefCell::new(Symbol::Class(class)));
                class_rc.borrow_mut().set_weak_self(Rc::downgrade(&class_rc));
                class_rc.borrow_mut().set_parent(Some(Rc::downgrade(&parent)));

                restore_symbols_to_parent(&cc.symbols, class_rc.clone(), is_external);
                
                add_symbol_to_parent(&parent, &class_rc, 0);
            }
        }
    }
}

fn restore_function_args(
    cached_args: &[CachedArgument],
    func_rc: &std::rc::Rc<std::cell::RefCell<crate::core::symbols::symbol::Symbol>>,
) {
    use crate::core::symbols::function_symbol::Argument;
    use crate::core::symbols::symbol_mgr::SymbolMgr;
    use crate::core::evaluation::Evaluation;
    use crate::oyarn;
    use std::rc::Rc;

    for cached_arg in cached_args {
        let arg_name = oyarn!("{}", cached_arg.name);
        let func = func_rc.borrow();
        let func_sym = func.as_func();
        let content = func_sym.get_content_symbol(arg_name, u32::MAX);
        
        if let Some(param_sym) = content.symbols.first() {
            let param_sym_clone = param_sym.clone();
            let default_value = if cached_arg.has_default {
                Some(Evaluation::new_none())
            } else {
                None
            };
            let arg_type = cached_arg.to_argument_type();
            
            drop(func);
            func_rc.borrow_mut().as_func_mut().args.push(Argument {
                symbol: Rc::downgrade(&param_sym_clone),
                default_value,
                arg_type,
                annotation: None,
            });
        }
    }
}

pub fn restore_file_from_cache(
    cached_file: &CachedFile,
    parent: std::rc::Rc<std::cell::RefCell<crate::core::symbols::symbol::Symbol>>,
    is_external: bool,
) -> std::rc::Rc<std::cell::RefCell<crate::core::symbols::symbol::Symbol>> {
    use crate::core::symbols::symbol::Symbol;
    use crate::core::symbols::file_symbol::FileSymbol;
    use std::rc::Rc;
    use std::cell::RefCell;

    let mut file_sym = FileSymbol::new(
        cached_file.name.clone(),
        cached_file.path.clone(),
        is_external,
    );
    file_sym.processed_text_hash = cached_file.processed_text_hash;
    file_sym.arch_status = crate::constants::BuildStatus::DONE;
    file_sym.arch_eval_status = crate::constants::BuildStatus::PENDING;
    file_sym.validation_status = crate::constants::BuildStatus::PENDING;

    let file_rc = Rc::new(RefCell::new(Symbol::File(file_sym)));
    file_rc.borrow_mut().set_weak_self(Rc::downgrade(&file_rc));
    file_rc.borrow_mut().set_parent(Some(Rc::downgrade(&parent)));

    restore_symbols_to_parent(&cached_file.symbols, file_rc.clone(), is_external);

    file_rc
}

pub fn collect_files_recursively(
    module_symbols: &std::collections::HashMap<crate::constants::OYarn, std::rc::Rc<std::cell::RefCell<crate::core::symbols::symbol::Symbol>>>,
) -> Vec<CachedFile> {
    use crate::core::symbols::symbol::Symbol;
    use crate::core::symbols::package_symbol::PackageSymbol;
    use crate::constants::SymType;
    
    let mut cached_files = Vec::new();
    
    for (_name, sym_rc) in module_symbols.iter() {
        let sym = sym_rc.borrow();
        match sym.typ() {
            SymType::FILE => {
                cached_files.push(CachedFile::from_file_symbol(sym.as_file()));
            }
            SymType::PACKAGE(_) => {
                match &*sym {
                    Symbol::Package(PackageSymbol::PythonPackage(p)) => {
                        cached_files.extend(collect_files_recursively(&p.module_symbols));
                    }
                    Symbol::Package(PackageSymbol::Module(m)) => {
                        cached_files.extend(collect_files_recursively(&m.module_symbols));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    
    cached_files
}
