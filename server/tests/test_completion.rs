
use std::env;
use std::rc::Rc;
use std::cell::RefCell;
use odoo_ls_server::features::completion::CompletionFeature;
use lsp_types::CompletionResponse;
use odoo_ls_server::oyarn;

mod setup;

#[test]
fn test_completion_inherit() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    
    let temp_dir = env::temp_dir();
    let file_path = temp_dir.join("test_completion.py");
    let file_path_str = file_path.to_str().unwrap().to_string();
    
    let content = r#"
from odoo import models
class Test(models.Model):
    _inherit = "res"
"#;
    std::fs::write(&file_path, content).unwrap();
    
    setup::setup::prepare_custom_entry_point(&mut session, &file_path_str);
    
    {
        use odoo_ls_server::core::model::Model;
        use odoo_ls_server::core::symbols::symbol::Symbol;
        let model_name = oyarn!("res.partner");
        let root = Symbol::new_root(); 
        let model = Model::new(model_name.clone(), root);
        session.sync_odoo.models.insert(model_name, Rc::new(RefCell::new(model)));
    }

    let line = 3;
    let character = 19;
    
    let symbols = session.sync_odoo.get_symbol(&file_path_str, &(vec![], vec![]), u32::MAX);
    assert!(!symbols.is_empty(), "File symbol not found");
    let file_symbol = &symbols[0];

    let file_mgr = session.sync_odoo.get_file_mgr();
    let file_info_rc = file_mgr.borrow().get_file_info(&file_path_str).expect("File info not found");
    
    let result = CompletionFeature::autocomplete(&mut session, file_symbol, &file_info_rc, line, character);
    
    assert!(result.is_some(), "No completion result");
    if let Some(CompletionResponse::List(list)) = result {
        let has_partner = list.items.iter().any(|item| item.label == "res.partner");
        assert!(has_partner, "Should complete res.partner for _inherit. Items: {:?}", list.items.iter().map(|i| &i.label).collect::<Vec<_>>());
    }
}

#[test]
fn test_completion_compute() {
    let (mut odoo, config) = setup::setup::setup_server(false);
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    let temp_dir = env::temp_dir();
    let file_path = temp_dir.join("test_completion_compute.py");
    let file_path_str = file_path.to_str().unwrap().to_string();
    
    let content = r#"
from odoo import models, fields

class Test(models.Model):
    name = fields.Char(compute="_comp")

    def _compute_name(self):
        pass
"#;
    std::fs::write(&file_path, content).unwrap();
    setup::setup::prepare_custom_entry_point(&mut session, &file_path_str);
    
    let line = 4;
    let character = 34; 
    
    let symbols = session.sync_odoo.get_symbol(&file_path_str, &(vec![], vec![]), u32::MAX);
    assert!(!symbols.is_empty(), "File symbol not found");
    let file_symbol = &symbols[0];

    let file_mgr = session.sync_odoo.get_file_mgr();
    let file_info_rc = file_mgr.borrow().get_file_info(&file_path_str).expect("File info not found");
    
    let result = CompletionFeature::autocomplete(&mut session, file_symbol, &file_info_rc, line, character);
    
    assert!(result.is_some(), "No completion result");
    if let Some(CompletionResponse::List(list)) = result {
        let has_method = list.items.iter().any(|item| item.label == "_compute_name");
        assert!(has_method, "Should complete _compute_name for compute arg. Items: {:?}", list.items.iter().map(|i| &i.label).collect::<Vec<_>>());
    }
}
