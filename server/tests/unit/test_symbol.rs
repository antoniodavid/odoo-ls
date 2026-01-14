use std::env;
use odoo_ls_server::constants::OYarn;
use odoo_ls_server::utils::PathSanitizer;
use odoo_ls_server::Sy;

#[path = "../setup/mod.rs"]
mod setup;

#[test]
fn test_symbol_creation() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    let path = env::current_dir().unwrap().join("tests/data/python/expressions/assign.py").sanitize();
    setup::setup::prepare_custom_entry_point(&mut session, path.as_str());
    
    let a = session.sync_odoo.get_symbol(path.as_str(), &(vec![], vec![Sy!("a")]), u32::MAX);
    assert!(a.len() == 1);
    assert!(a[0].borrow().name() == "a");
}

#[test]
fn test_symbol_get_symbol() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    let path = env::current_dir().unwrap().join("tests/data/python/expressions/assign.py").sanitize();
    setup::setup::prepare_custom_entry_point(&mut session, path.as_str());
    
    let a = session.sync_odoo.get_symbol(path.as_str(), &(vec![], vec![Sy!("a")]), u32::MAX);
    assert!(a.len() == 1, "Should find symbol 'a'");
    
    let b = session.sync_odoo.get_symbol(path.as_str(), &(vec![], vec![Sy!("b")]), u32::MAX);
    assert!(b.len() == 1, "Should find symbol 'b'");
}

#[test]
fn test_symbol_not_found() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    let path = env::current_dir().unwrap().join("tests/data/python/expressions/assign.py").sanitize();
    setup::setup::prepare_custom_entry_point(&mut session, path.as_str());
    
    let nonexistent = session.sync_odoo.get_symbol(path.as_str(), &(vec![], vec![Sy!("nonexistent")]), u32::MAX);
    assert!(nonexistent.is_empty(), "Should not find nonexistent symbol");
}

#[test]
fn test_symbol_scope_resolution() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    let path = env::current_dir().unwrap().join("tests/data/python/expressions/assign.py").sanitize();
    setup::setup::prepare_custom_entry_point(&mut session, path.as_str());
    
    let root_symbols = session.sync_odoo.get_symbol(path.as_str(), &(vec![], vec![]), u32::MAX);
    assert!(!root_symbols.is_empty(), "Should find root symbols");
}

#[test]
fn test_symbol_evaluation() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    let path = env::current_dir().unwrap().join("tests/data/python/expressions/assign.py").sanitize();
    setup::setup::prepare_custom_entry_point(&mut session, path.as_str());
    
    let a = session.sync_odoo.get_symbol(path.as_str(), &(vec![], vec![Sy!("a")]), u32::MAX);
    assert!(a.len() == 1);
    let binding = a[0].borrow();
    let evaluations = binding.evaluations();
    assert!(evaluations.is_some(), "Should have evaluations");
    assert!(!evaluations.as_ref().unwrap().is_empty(), "Should have at least one evaluation");
}
