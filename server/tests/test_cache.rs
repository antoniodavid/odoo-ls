use odoo_ls_server::core::cache::{
    CachedArgument, CachedClass, CachedFile, CachedFunction, CachedModel, CachedSymbol,
    CachedTextRange, CachedVariable, restore_file_from_cache, restore_symbols_to_parent,
};
use odoo_ls_server::core::symbols::symbol::Symbol;
use odoo_ls_server::core::symbols::file_symbol::FileSymbol;
use odoo_ls_server::core::symbols::symbol_mgr::SymbolMgr;
use odoo_ls_server::oyarn;
use std::cell::RefCell;
use std::rc::Rc;

fn create_test_parent() -> Rc<RefCell<Symbol>> {
    let file = FileSymbol::new("test.py".to_string(), "/test/test.py".to_string(), false);
    let file_rc = Rc::new(RefCell::new(Symbol::File(file)));
    file_rc.borrow_mut().set_weak_self(Rc::downgrade(&file_rc));
    file_rc
}

#[test]
fn test_cached_variable_restoration() {
    let parent = create_test_parent();
    
    let cached_var = CachedVariable {
        name: "my_var".to_string(),
        range: CachedTextRange { start: 10, end: 20 },
        is_import_variable: true,
        is_parameter: false,
        doc_string: Some("Test docstring".to_string()),
    };
    
    let cached_symbols = vec![CachedSymbol::Variable(cached_var)];
    restore_symbols_to_parent(&cached_symbols, parent.clone(), false);
    
    let parent_ref = parent.borrow();
    let file = parent_ref.as_file();
    let content = file.get_content_symbol(oyarn!("my_var"), u32::MAX);
    
    assert_eq!(content.symbols.len(), 1);
    let var = content.symbols[0].borrow();
    assert_eq!(var.name().as_str(), "my_var");
    assert_eq!(var.as_variable().is_import_variable, true);
    assert_eq!(var.as_variable().is_parameter, false);
    assert_eq!(var.as_variable().doc_string, Some("Test docstring".to_string()));
}

#[test]
fn test_cached_function_restoration() {
    let parent = create_test_parent();
    
    let cached_func = CachedFunction {
        name: "my_func".to_string(),
        range: CachedTextRange { start: 0, end: 100 },
        body_start: 20,
        is_static: true,
        is_property: false,
        is_class_method: true,
        doc_string: Some("Function doc".to_string()),
        args: vec![
            CachedArgument {
                name: "self".to_string(),
                arg_type: "ARG".to_string(),
                has_default: false,
            },
            CachedArgument {
                name: "param1".to_string(),
                arg_type: "ARG".to_string(),
                has_default: true,
            },
        ],
        symbols: vec![
            CachedSymbol::Variable(CachedVariable {
                name: "self".to_string(),
                range: CachedTextRange { start: 10, end: 14 },
                is_import_variable: false,
                is_parameter: true,
                doc_string: None,
            }),
            CachedSymbol::Variable(CachedVariable {
                name: "param1".to_string(),
                range: CachedTextRange { start: 16, end: 22 },
                is_import_variable: false,
                is_parameter: true,
                doc_string: None,
            }),
        ],
    };
    
    let cached_symbols = vec![CachedSymbol::Function(cached_func)];
    restore_symbols_to_parent(&cached_symbols, parent.clone(), false);
    
    let parent_ref = parent.borrow();
    let file = parent_ref.as_file();
    let content = file.get_content_symbol(oyarn!("my_func"), u32::MAX);
    
    assert_eq!(content.symbols.len(), 1);
    let func = content.symbols[0].borrow();
    assert_eq!(func.name().as_str(), "my_func");
    assert_eq!(func.as_func().is_static, true);
    assert_eq!(func.as_func().is_property, false);
    assert_eq!(func.as_func().is_class_method, true);
    assert_eq!(func.as_func().doc_string, Some("Function doc".to_string()));
    assert_eq!(func.as_func().args.len(), 2);
}

#[test]
fn test_cached_class_restoration() {
    let parent = create_test_parent();
    
    let cached_class = CachedClass {
        name: "MyClass".to_string(),
        range: CachedTextRange { start: 0, end: 200 },
        body_start: 15,
        doc_string: Some("Class documentation".to_string()),
        base_names: vec!["BaseClass".to_string()],
        model: Some(CachedModel {
            name: "my.model".to_string(),
            description: "Test model".to_string(),
            inherit: vec!["base.model".to_string()],
            inherits: vec![],
            fields: vec![],
            is_abstract: false,
            transient: false,
            table: "my_model".to_string(),
            rec_name: Some("name".to_string()),
            order: "id".to_string(),
            auto: true,
            log_access: true,
            parent_name: "parent_id".to_string(),
            active_name: Some("active".to_string()),
        }),
        symbols: vec![
            CachedSymbol::Variable(CachedVariable {
                name: "name".to_string(),
                range: CachedTextRange { start: 20, end: 24 },
                is_import_variable: false,
                is_parameter: false,
                doc_string: None,
            }),
        ],
    };
    
    let cached_symbols = vec![CachedSymbol::Class(cached_class)];
    restore_symbols_to_parent(&cached_symbols, parent.clone(), false);
    
    let parent_ref = parent.borrow();
    let file = parent_ref.as_file();
    let content = file.get_content_symbol(oyarn!("MyClass"), u32::MAX);
    
    assert_eq!(content.symbols.len(), 1);
    let class = content.symbols[0].borrow();
    assert_eq!(class.name().as_str(), "MyClass");
    assert_eq!(class.as_class_sym().doc_string, Some("Class documentation".to_string()));
    
    let model = class.as_class_sym()._model.as_ref().unwrap();
    assert_eq!(model.name.as_str(), "my.model");
    assert_eq!(model.description, "Test model");
    assert_eq!(model.inherit.len(), 1);
    assert_eq!(model.inherit[0].as_str(), "base.model");
}

#[test]
fn test_cached_file_restoration() {
    let parent = create_test_parent();
    
    let cached_file = CachedFile {
        name: "restored.py".to_string(),
        path: "/test/restored.py".to_string(),
        processed_text_hash: 12345,
        symbols: vec![
            CachedSymbol::Variable(CachedVariable {
                name: "module_var".to_string(),
                range: CachedTextRange { start: 0, end: 10 },
                is_import_variable: false,
                is_parameter: false,
                doc_string: None,
            }),
        ],
    };
    
    let file_rc = restore_file_from_cache(&cached_file, parent.clone(), false);
    
    let file = file_rc.borrow();
    assert_eq!(file.name().as_str(), "restored.py");
    assert_eq!(file.as_file().path, "/test/restored.py");
    assert_eq!(file.as_file().processed_text_hash, 12345);
    
    let content = file.as_file().get_content_symbol(oyarn!("module_var"), u32::MAX);
    assert_eq!(content.symbols.len(), 1);
}

#[test]
fn test_argument_type_conversion() {
    let arg_pos = CachedArgument {
        name: "a".to_string(),
        arg_type: "POS_ONLY".to_string(),
        has_default: false,
    };
    let arg_arg = CachedArgument {
        name: "b".to_string(),
        arg_type: "ARG".to_string(),
        has_default: true,
    };
    let arg_vararg = CachedArgument {
        name: "c".to_string(),
        arg_type: "VARARG".to_string(),
        has_default: false,
    };
    let arg_kwonly = CachedArgument {
        name: "d".to_string(),
        arg_type: "KWORD_ONLY".to_string(),
        has_default: false,
    };
    let arg_kwarg = CachedArgument {
        name: "e".to_string(),
        arg_type: "KWARG".to_string(),
        has_default: false,
    };
    
    use odoo_ls_server::core::symbols::function_symbol::ArgumentType;
    
    assert!(matches!(arg_pos.to_argument_type(), ArgumentType::POS_ONLY));
    assert!(matches!(arg_arg.to_argument_type(), ArgumentType::ARG));
    assert!(matches!(arg_vararg.to_argument_type(), ArgumentType::VARARG));
    assert!(matches!(arg_kwonly.to_argument_type(), ArgumentType::KWORD_ONLY));
    assert!(matches!(arg_kwarg.to_argument_type(), ArgumentType::KWARG));
}
