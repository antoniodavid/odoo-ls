use std::time::Instant;
use odoo_ls_server::core::odoo::SyncOdoo;
use odoo_ls_server::core::evaluation::Evaluation;
use ruff_python_ast::{Stmt, Expr};
use ruff_text_size::TextSize;

mod setup;

#[test]
fn bench_evaluation_performance() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let mut session = setup::setup::create_init_session(&mut odoo, config);

    // Create a dummy file with some complex inference
    let file_path = std::env::current_dir().unwrap().join("tests/data/bench_file.py");
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

a = test_func()
b = a + 1
c = [a, b]
d = c[0]
"#;
    
    // Write content to file
    let path = file_path.to_str().unwrap().to_string();
    std::fs::write(&file_path, content).expect("Failed to write bench file");
    
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
    
    let symbol = found_symbol.expect("Could not find symbol for bench file");

    // Get AST and find the expression "c[0]" assigned to "d"
    let file_mgr = session.sync_odoo.get_file_mgr();
    let file_mgr_ref = file_mgr.borrow();
    let file_info = file_mgr_ref.get_file_info(&path).expect("File info not found");
    let file_info = file_info.borrow();
    let ast_cell = file_info.file_info_ast.borrow();
    let stmts = ast_cell.get_stmts().expect("AST not loaded");
    
    let mut target_expr: Option<Expr> = None;
    
    for stmt in stmts {
        if let Stmt::Assign(assign) = stmt {
            // Check if target is "d"
            for target in &assign.targets {
                if let Expr::Name(name) = target {
                    if name.id == "d" {
                        target_expr = Some(*assign.value.clone());
                        break;
                    }
                }
            }
        }
    }
    
    let expr = target_expr.expect("Could not find assignment to 'd'");
    
    // Benchmark loop
    let start = Instant::now();
    let iterations = 1000; // Increased iterations since we are testing micro-op
    
    let mut dummy_req = vec![vec![], vec![], vec![]];
    let max_infer = TextSize::new(u32::MAX);

    for _ in 0..iterations {
        let _ = Evaluation::eval_from_ast(&mut session, &expr, symbol.clone(), &max_infer, false, &mut dummy_req);
    }
    
    let duration = start.elapsed();
    println!("Time per iteration: {:?}", duration / iterations);
    
    // Cleanup
    std::fs::remove_file(file_path).unwrap();
}
