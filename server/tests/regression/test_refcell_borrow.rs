use std::env;
use std::path::PathBuf;
use odoo_ls_server::utils::PathSanitizer;
use odoo_ls_server::Sy;

#[path = "../setup/mod.rs"]
mod setup;

#[test]
fn test_refcell_borrow_no_panic_on_nested_symbol_access() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let test_addons_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("addons");
    
    let test_file = test_addons_path
        .join("module_1")
        .join("models")
        .join("base_test_models.py")
        .sanitize();
    
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    setup::setup::prepare_custom_entry_point(&mut session, &test_file.to_string());
    
    let sym = session.sync_odoo.get_symbol(&test_file.to_string(), &(vec![], vec![]), u32::MAX);
    assert!(!sym.is_empty(), "Should find file symbol");
    
    let class_sym = sym[0].borrow().get_symbol(
        &(vec![], vec![Sy!("BaseTestModel")]),
        u32::MAX
    );
    assert!(!class_sym.is_empty(), "Should find BaseTestModel");
    
    // Nested access that previously caused RefCell panic
    let fields = class_sym[0].borrow().get_symbol(
        &(vec![], vec![Sy!("partner_id")]),
        u32::MAX
    );
    let methods = class_sym[0].borrow().get_symbol(
        &(vec![], vec![Sy!("get_test_int")]),
        u32::MAX
    );
    
    assert!(fields.len() == 1, "Should find partner_id field");
    assert!(methods.len() == 1, "Should find get_test_int method");
}

#[test]
fn test_symbol_eviction_no_panic() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let test_addons_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("addons");
    
    let test_file = test_addons_path
        .join("module_1")
        .join("models")
        .join("base_test_models.py")
        .sanitize();
    
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    setup::setup::prepare_custom_entry_point(&mut session, &test_file.to_string());
    
    let sym = session.sync_odoo.get_symbol(&test_file.to_string(), &(vec![], vec![]), u32::MAX);
    assert!(!sym.is_empty(), "Should find file symbol");
    
    sym[0].borrow_mut().evict_data();
    
    let sym2 = session.sync_odoo.get_symbol(&test_file.to_string(), &(vec![], vec![]), u32::MAX);
    assert!(!sym2.is_empty(), "Should find file symbol after eviction");
}
