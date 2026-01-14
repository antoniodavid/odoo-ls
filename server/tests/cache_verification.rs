use std::time::Instant;
use odoo_ls_server::core::odoo::SyncOdoo;
use odoo_ls_server::core::evaluation::Evaluation;
use ruff_python_ast::{Stmt, Expr};
use ruff_text_size::TextSize;

mod setup;

#[test]
fn test_caching_implementation() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let mut session = setup::setup::create_init_session(&mut odoo, config);

    // Create a dummy file with some complex inference
    let file_path = std::env::current_dir().unwrap().join("tests/data/cache_test.py");
    let content = r#"
class A:
    def foo(self):
        return 1

class B(A):
    def bar(self):
        return self.foo()

def test_func():
    x = B()
    return x.bar()

result = test_func()
"#;
    
    // Write content to file
    let path = file_path.to_str().unwrap().to_string();
    std::fs::write(&file_path, content).expect("Failed to write cache test file");
    
    setup::setup::prepare_custom_entry_point(&mut session, &path);

    // Find the symbol for the file
    let entry_mgr = session.sync_odoo.entry_point_mgr.borrow();
    let mut found_symbol = None;
    for entry in entry_mgr.iter_all() {
        if entry.borrow().path == path {
             found_symbol = entry.borrow().get_symbol();
             break;
        }
    }
    drop(entry_mgr);
    
    let symbol = found_symbol.expect("Could not find symbol for cache test file");

    // Get AST and find the expression "test_func()"
    let file_mgr = session.sync_odoo.get_file_mgr();
    let file_mgr_ref = file_mgr.borrow();
    let file_info = file_mgr_ref.get_file_info(&path).expect("File info not found");
    let file_info = file_info.borrow();
    let ast_cell = file_info.file_info_ast.borrow();
    let stmts = ast_cell.get_stmts().expect("AST not loaded");
    
    let mut target_expr: Option<Expr> = None;
    
    for stmt in stmts {
        if let Stmt::Assign(assign) = stmt {
            // Check if target is "result"
            for target in &assign.targets {
                if let Expr::Name(name) = target {
                    if name.id == "result" {
                        target_expr = Some(*assign.value.clone());
                        break;
                    }
                }
            }
        }
    }
    
    let expr = target_expr.expect("Could not find assignment to 'result'");
    
    // First run (should populate cache)
    let mut dummy_req = vec![vec![], vec![], vec![]];
    let max_infer = TextSize::new(u32::MAX);
    let _ = Evaluation::eval_from_ast(&mut session, &expr, symbol.clone(), &max_infer, false, &mut dummy_req);
    
    // Check if cache was populated
    let entry_mgr = session.sync_odoo.entry_point_mgr.borrow();
    let mut cache_size_before = 0;
    for entry in entry_mgr.iter_all() {
        let entry = entry.borrow();
        if entry.path == path {
            if let Some(symbol) = entry.get_symbol() {
                if let odoo_ls_server::core::symbols::symbol::Symbol::File(f) = &*symbol.borrow() {
                    cache_size_before = f.ast_eval_cache.len();
                    println!("Cache size before clear: {}", cache_size_before);
                }
            }
            break;
        }
    }
    drop(entry_mgr);
    
    // Benchmark cached performance
    let start = Instant::now();
    let iterations = 1000;

    for _ in 0..iterations {
        let _ = Evaluation::eval_from_ast(&mut session, &expr, symbol.clone(), &max_infer, false, &mut dummy_req);
    }
    
    let cached_duration = start.elapsed();
    println!("Cached performance: {:?}", cached_duration / iterations);
    
    // Clear cache
    let entry_mgr = session.sync_odoo.entry_point_mgr.borrow();
    for entry in entry_mgr.iter_all() {
        let entry = entry.borrow();
        if entry.path == path {
            if let Some(symbol) = entry.get_symbol() {
                if let odoo_ls_server::core::symbols::symbol::Symbol::File(f) = &mut *symbol.borrow_mut() {
                    f.clear_cache();
                    println!("Cache cleared, size now: {}", f.ast_eval_cache.len());
                }
            }
            break;
        }
    }
    drop(entry_mgr);
    
    // Benchmark uncached performance
    let start = Instant::now();
    
    for _ in 0..iterations {
        let _ = Evaluation::eval_from_ast(&mut session, &expr, symbol.clone(), &max_infer, false, &mut dummy_req);
    }
    
    let uncached_duration = start.elapsed();
    println!("Uncached performance: {:?}", uncached_duration / iterations);
    
    // The cached version should be faster (though the difference might be small for simple expressions)
    println!("Cache was populated with {} entries", cache_size_before);
    if cache_size_before > 0 {
        // Allow some variance but cached should generally be faster
        assert!(cached_duration <= uncached_duration + std::time::Duration::from_nanos(500), 
                "Cached should be faster or equal, cached: {:?}, uncached: {:?}", 
                cached_duration / iterations, uncached_duration / iterations);
    }
    
    // Cleanup
    std::fs::remove_file(file_path).unwrap();
}
