use odoo_ls_server::core::odoo::SyncOdoo;
use odoo_ls_server::utils::PathSanitizer;
use odoo_ls_server::S;
use std::env;
use std::path::PathBuf;
use std::time::Instant;

mod setup;

#[test]
fn test_odoo18_cold_start() {
    let odoo_path = "/home/adruban/Workspace/Doodba_ENV/O18/odoo/custom/src/odoo";
    unsafe {
        env::set_var("COMMUNITY_PATH", odoo_path);
    }

    // Remove cache to force cold start
    let cache_dir = dirs::data_dir().unwrap().join("odoo-ls");
    if cache_dir.exists() {
        std::fs::remove_dir_all(&cache_dir).ok();
    }

    let start = Instant::now();
    let (mut odoo, mut config) = setup::setup::setup_server(true);
    config.addons_paths = vec![
        format!("{}/addons", odoo_path),
        format!("{}/odoo/addons", odoo_path),
    ]
    .into_iter()
    .collect();
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    SyncOdoo::process_rebuilds(&mut session, false);
    let cold_dur = start.elapsed();
    println!(">>> COLD start: {:?}", cold_dur);

    // Measure cache size on disk
    let cache_dir = dirs::data_dir().unwrap().join("odoo-ls");
    let mut cache_size: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for e in entries.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.is_dir() {
                if let Ok(sub) = std::fs::read_dir(&path) {
                    for f in sub.filter_map(|f| f.ok()) {
                        cache_size += f.metadata().map(|m| m.len()).unwrap_or(0);
                    }
                }
            } else {
                cache_size += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    println!(
        ">>> Cache size on disk: {:.2} MB",
        cache_size as f64 / 1_048_576.0
    );
}

#[test]
fn test_odoo18_warm_start() {
    let odoo_path = "/home/adruban/Workspace/Doodba_ENV/O18/odoo/custom/src/odoo";
    unsafe {
        env::set_var("COMMUNITY_PATH", odoo_path);
    }

    let start = Instant::now();
    let (mut odoo, mut config) = setup::setup::setup_server(true);
    config.addons_paths = vec![
        format!("{}/addons", odoo_path),
        format!("{}/odoo/addons", odoo_path),
    ]
    .into_iter()
    .collect();
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    SyncOdoo::process_rebuilds(&mut session, false);
    let warm_dur = start.elapsed();
    println!(">>> WARM start (from cache): {:?}", warm_dur);
}

#[test]
fn test_odoo18_completion() {
    let odoo_path = "/home/adruban/Workspace/Doodba_ENV/O18/odoo/custom/src/odoo";
    unsafe {
        env::set_var("COMMUNITY_PATH", odoo_path);
    }

    let (mut odoo, mut config) = setup::setup::setup_server(true);
    config.addons_paths = vec![
        format!("{}/addons", odoo_path),
        format!("{}/odoo/addons", odoo_path),
    ]
    .into_iter()
    .collect();
    let mut session = setup::setup::create_init_session(&mut odoo, config);
    SyncOdoo::process_rebuilds(&mut session, false);

    // Open res_partner.py
    let test_file = PathBuf::from(odoo_path)
        .join("odoo/addons/base/models/res_partner.py")
        .sanitize();

    setup::setup::prepare_custom_entry_point(&mut session, &test_file);

    let file_mgr = session.sync_odoo.get_file_mgr();
    let file_info = file_mgr
        .borrow()
        .get_file_info(&S!(test_file.as_str()))
        .expect("file not found");
    let file_symbol = SyncOdoo::get_symbol_of_opened_file(&mut session, &PathBuf::from(&test_file))
        .expect("no symbol");

    // Run completion at several positions and measure
    for (line, ch, label) in [
        (50u32, 10u32, "mid-class body"),
        (100, 8, "method line"),
        (200, 12, "deep body"),
    ] {
        let t = Instant::now();
        let completions = odoo_ls_server::features::completion::CompletionFeature::autocomplete(
            &mut session,
            &file_symbol,
            &file_info,
            line,
            ch,
        );
        let dur = t.elapsed();
        let count = match completions {
            Some(lsp_types::CompletionResponse::Array(arr)) => arr.len(),
            Some(lsp_types::CompletionResponse::List(list)) => list.items.len(),
            None => 0,
        };
        println!(
            ">>> Completion at {} (L{}:C{}): {} items in {:?}",
            label, line, ch, count, dur
        );
    }
}
