use serde::{Serialize, Deserialize};
use ruff_text_size::TextRange;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::cell::RefCell;
use crate::constants::OYarn;
use crate::core::symbols::symbol::Symbol;
use crate::core::symbols::file_symbol::FileSymbol;
use crate::core::symbols::class_symbol::ClassSymbol;
use crate::core::symbols::function_symbol::{FunctionSymbol, Argument, ArgumentType};
use crate::core::symbols::variable_symbol::VariableSymbol;
use crate::core::model::{Model, ModelData};
use crate::oyarn;
use crate::threads::SessionInfo;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CachedSymbol {
    File(CachedFile),
    Class(CachedClass),
    Function(CachedFunction),
    Variable(CachedVariable),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedFile {
    pub path: String,
    pub is_external: bool,
    pub symbols: HashMap<String, Vec<CachedSymbol>>,
    pub processed_text_hash: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedClass {
    pub name: String,
    pub range: TextRange,
    pub body_range: TextRange,
    pub doc_string: Option<String>,
    pub symbols: HashMap<String, Vec<CachedSymbol>>,
    pub is_external: bool,
    pub model_data: Option<CachedModelData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedModelData {
    pub name: String,
    pub inherit: Vec<String>,
    pub inherits: Vec<(String, String)>,
    pub description: String,
    pub auto: bool,
    pub log_access: bool,
    pub table: String,
    pub sequence: String,
    pub sql_constraints: Vec<String>,
    pub is_abstract: bool,
    pub transient: bool,
    pub rec_name: Option<String>,
    pub order: String,
    pub check_company_auto: bool,
    pub parent_name: String,
    pub active_name: Option<String>,
    pub parent_store: bool,
    pub data_name: String,
    pub fold_name: String,
    pub computes: HashMap<String, HashSet<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedFunction {
    pub name: String,
    pub range: TextRange,
    pub body_range: TextRange,
    pub doc_string: Option<String>,
    pub args: Vec<CachedArgument>,
    pub is_static: bool,
    pub is_property: bool,
    pub is_class_method: bool,
    pub is_overloaded: bool,
    pub symbols: HashMap<String, Vec<CachedSymbol>>,
    pub is_external: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedVariable {
    pub name: String,
    pub range: TextRange,
    pub is_external: bool,
    pub is_import_variable: bool,
    pub is_parameter: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedArgument {
    pub name: String,
    pub arg_type: String,
    pub has_default: bool,
}

impl CachedSymbol {
    pub fn from_symbol(symbol: &Rc<RefCell<Symbol>>) -> Option<Self> {
        match &*symbol.borrow() {
            Symbol::File(f) => Some(CachedSymbol::File(CachedFile::from_file(f))),
            Symbol::Class(c) => Some(CachedSymbol::Class(CachedClass::from_class(c))),
            Symbol::Function(f) => Some(CachedSymbol::Function(CachedFunction::from_function(f))),
            Symbol::Variable(v) => Some(CachedSymbol::Variable(CachedVariable::from_variable(v))),
            _ => None,
        }
    }

    pub fn restore_to_symbol(self, session: &mut SessionInfo, parent: &Rc<RefCell<Symbol>>) {
        match self {
            CachedSymbol::File(_) => panic!("File cannot be restored as child"),
            CachedSymbol::Class(c) => c.restore(session, parent),
            CachedSymbol::Function(f) => f.restore(session, parent),
            CachedSymbol::Variable(v) => v.restore(session, parent),
        }
    }

    pub fn save_to_disk(&self, cache_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(cache_path)?;
        let writer = std::io::BufWriter::new(file);
        bincode::serialize_into(writer, self)?;
        Ok(())
    }

    pub fn load_from_disk(cache_path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(cache_path)?;
        let reader = std::io::BufReader::new(file);
        let cached = bincode::deserialize_from(reader)?;
        Ok(cached)
    }
}

impl CachedFile {
    pub fn from_file(f: &FileSymbol) -> Self {
        CachedFile {
            path: f.path.clone(),
            is_external: f.is_external,
            symbols: convert_symbols_map(&f.symbols),
            processed_text_hash: f.processed_text_hash,
        }
    }

    pub fn restore(&self, session: &mut SessionInfo, file_symbol: &Rc<RefCell<Symbol>>) {
        let mut file = file_symbol.borrow_mut();
        file.set_processed_text_hash(self.processed_text_hash);
        file.set_is_external(self.is_external);
        drop(file);
        
        for (_name, cached_list) in &self.symbols {
            for cached in cached_list {
                match cached {
                    CachedSymbol::Class(c) => c.clone().restore(session, file_symbol),
                    CachedSymbol::Function(f) => f.clone().restore(session, file_symbol),
                    CachedSymbol::Variable(v) => v.clone().restore(session, file_symbol),
                    _ => {}
                }
            }
        }
    }
}

impl CachedClass {
    pub fn from_class(c: &ClassSymbol) -> Self {
        CachedClass {
            name: c.name.to_string(),
            range: c.range,
            body_range: c.body_range,
            doc_string: c.doc_string.clone(),
            symbols: convert_symbols_map(&c.symbols),
            is_external: c.is_external,
            model_data: c._model.as_ref().map(CachedModelData::from_model_data),
        }
    }

    pub fn restore(self, session: &mut SessionInfo, parent: &Rc<RefCell<Symbol>>) {
        let sym = parent.borrow_mut().add_new_class(
            session, &self.name, &self.range, &self.body_range.start()
        );

        if let Some(md) = self.model_data {
            let model_data = md.to_model_data();
            let model_name = model_data.name.clone();

            sym.borrow_mut().as_class_sym_mut()._model = Some(model_data);

            match session.sync_odoo.models.get(&model_name).cloned() {
                Some(model) => model.borrow_mut().add_symbol(session, sym.clone()),
                None => {
                    let model = Model::new(model_name.clone(), sym.clone());
                    session.sync_odoo.models.insert(model_name.clone(), Rc::new(RefCell::new(model)));
                }
            }
        }

        let mut sym_bw = sym.borrow_mut();
        let class_sym = sym_bw.as_class_sym_mut();
        class_sym.doc_string = self.doc_string;
        class_sym.body_range = self.body_range;
        drop(sym_bw);

        for (_name, cached_list) in &self.symbols {
            for cached in cached_list {
                cached.clone().restore_to_symbol(session, &sym);
            }
        }
    }
}

impl CachedModelData {
    pub fn from_model_data(md: &ModelData) -> Self {
        CachedModelData {
            name: md.name.to_string(),
            inherit: md.inherit.iter().map(|s| s.to_string()).collect(),
            inherits: md.inherits.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            description: md.description.clone(),
            auto: md.auto,
            log_access: md.log_access,
            table: md.table.clone(),
            sequence: md.sequence.clone(),
            sql_constraints: md.sql_constraints.clone(),
            is_abstract: md.is_abstract,
            transient: md.transient,
            rec_name: md.rec_name.clone(),
            order: md.order.clone(),
            check_company_auto: md.check_company_auto,
            parent_name: md.parent_name.clone(),
            active_name: md.active_name.clone(),
            parent_store: md.parent_store,
            data_name: md.data_name.clone(),
            fold_name: md.fold_name.clone(),
            computes: md.computes.iter().map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect())).collect(),
        }
    }

    pub fn to_model_data(&self) -> ModelData {
        ModelData {
            name: oyarn!("{}", self.name),
            inherit: self.inherit.iter().map(|s| oyarn!("{}", s)).collect(),
            inherits: self.inherits.iter().map(|(k, v)| (oyarn!("{}", k), oyarn!("{}", v))).collect(),
            description: self.description.clone(),
            auto: self.auto,
            log_access: self.log_access,
            table: self.table.clone(),
            sequence: self.sequence.clone(),
            sql_constraints: self.sql_constraints.clone(),
            is_abstract: self.is_abstract,
            transient: self.transient,
            rec_name: self.rec_name.clone(),
            order: self.order.clone(),
            check_company_auto: self.check_company_auto,
            parent_name: self.parent_name.clone(),
            active_name: self.active_name.clone(),
            parent_store: self.parent_store,
            data_name: self.data_name.clone(),
            fold_name: self.fold_name.clone(),
            computes: self.computes.iter().map(|(k, v)| (oyarn!("{}", k), v.iter().map(|s| oyarn!("{}", s)).collect())).collect(),
        }
    }
}


impl CachedFunction {
    pub fn from_function(f: &FunctionSymbol) -> Self {
        CachedFunction {
            name: f.name.to_string(),
            range: f.range,
            body_range: f.body_range,
            doc_string: f.doc_string.clone(),
            args: f.args.iter().map(CachedArgument::from_arg).collect(),
            is_static: f.is_static,
            is_property: f.is_property,
            is_class_method: f.is_class_method,
            is_overloaded: f.is_overloaded,
            symbols: convert_symbols_map(&f.symbols),
            is_external: f.is_external,
        }
    }

    pub fn restore(self, session: &mut SessionInfo, parent: &Rc<RefCell<Symbol>>) {
        let sym = parent.borrow_mut().add_new_function(
            session, &self.name, &self.range, &self.body_range.start()
        );
        
        let mut restored_args = Vec::new();
        for arg_dto in &self.args {
            let range = self.range;
            let var = sym.borrow_mut().add_new_variable(session, oyarn!("{}", arg_dto.name), &range);
            var.borrow_mut().as_variable_mut().is_parameter = true;
            restored_args.push(Argument {
                symbol: Rc::downgrade(&var),
                default_value: if arg_dto.has_default { Some(crate::core::evaluation::Evaluation::new_none()) } else { None },
                arg_type: parse_arg_type(&arg_dto.arg_type),
                annotation: None,
            });
        }

        let mut sym_bw = sym.borrow_mut();
        let func_sym = sym_bw.as_func_mut();
        func_sym.doc_string = self.doc_string;
        func_sym.body_range = self.body_range;
        func_sym.is_static = self.is_static;
        func_sym.is_property = self.is_property;
        func_sym.is_class_method = self.is_class_method;
        func_sym.is_overloaded = self.is_overloaded;
        func_sym.args = restored_args;
        drop(sym_bw);

        for (_name, cached_list) in &self.symbols {
            for cached in cached_list {
                cached.clone().restore_to_symbol(session, &sym);
            }
        }
    }
}

impl CachedVariable {
    pub fn from_variable(v: &VariableSymbol) -> Self {
        CachedVariable {
            name: v.name.to_string(),
            range: v.range,
            is_external: v.is_external,
            is_import_variable: v.is_import_variable,
            is_parameter: v.is_parameter,
        }
    }

    pub fn restore(self, session: &mut SessionInfo, parent: &Rc<RefCell<Symbol>>) {
        let sym = parent.borrow_mut().add_new_variable(session, oyarn!("{}", self.name), &self.range);
        let mut sym_bw = sym.borrow_mut();
        let var_sym = sym_bw.as_variable_mut();
        var_sym.is_import_variable = self.is_import_variable;
        var_sym.is_parameter = self.is_parameter;
    }
}

impl CachedArgument {
    pub fn from_arg(arg: &Argument) -> Self {
        let symbol = arg.symbol.upgrade();
        let name = if let Some(s) = symbol {
            s.borrow().name().to_string()
        } else {
            "".to_string()
        };
        
        CachedArgument {
            name,
            arg_type: format!("{:?}", arg.arg_type),
            has_default: arg.default_value.is_some(),
        }
    }
}

fn parse_arg_type(s: &str) -> ArgumentType {
    match s {
        "POS_ONLY" => ArgumentType::POS_ONLY,
        "ARG" => ArgumentType::ARG,
        "VARARG" => ArgumentType::VARARG,
        "KWORD_ONLY" => ArgumentType::KWORD_ONLY,
        "KWARG" => ArgumentType::KWARG,
        _ => ArgumentType::ARG,
    }
}

fn convert_symbols_map(map: &HashMap<OYarn, HashMap<u32, Vec<Rc<RefCell<Symbol>>>>>) -> HashMap<String, Vec<CachedSymbol>> {
    let mut res = HashMap::new();
    for (name, sections) in map {
        let mut list = Vec::new();
        for (_, syms) in sections {
            for sym in syms {
                if let Some(cached) = CachedSymbol::from_symbol(sym) {
                    list.push(cached);
                }
            }
        }
        if !list.is_empty() {
            res.insert(name.to_string(), list);
        }
    }
    res
}
