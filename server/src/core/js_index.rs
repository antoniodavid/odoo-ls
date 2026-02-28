use crate::core::js_parser::{JsComponent, JsModuleInfo, JsParser};
use crate::utils::PathSanitizer;
use glob::glob;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct JsModuleIndex {
    /// Mapping of @addon_name/path -> absolute file path
    pub module_paths: HashMap<String, PathBuf>,
    /// absolute file path -> parsed module info
    pub modules: HashMap<PathBuf, JsModuleInfo>,
    /// template name -> (js file path, byte range)
    pub templates: HashMap<String, (PathBuf, (usize, usize))>,
    /// service name -> (js file path, byte range)
    pub services: HashMap<String, (PathBuf, (usize, usize))>,
    /// component class name -> (js file path, JsComponent)
    pub components: HashMap<String, (PathBuf, JsComponent)>,
    /// registry category -> [(key, js file path, byte range)]
    pub registry_entries: HashMap<String, Vec<(String, PathBuf, (usize, usize))>>,
}

impl JsModuleIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Scan an addon directory for JS files and index them
    pub fn scan_addon(&mut self, addon_name: &str, addon_path: &Path) {
        let static_src = addon_path.join("static").join("src");
        if !static_src.exists() {
            return;
        }

        let pattern = format!("{}/**/*.js", static_src.to_string_lossy());
        if let Ok(entries) = glob(&pattern) {
            for entry in entries.flatten() {
                if let Ok(content) = std::fs::read_to_string(&entry) {
                    if let Some(info) = JsParser::parse(&content) {
                        self.add_module(addon_name, entry, info);
                    }
                }
            }
        }
    }

    /// Add a new parsed JS module to the index
    pub fn add_module(&mut self, addon_name: &str, file_path: PathBuf, info: JsModuleInfo) {
        let path = file_path.sanitize();
        let path_buf = PathBuf::from(&path);

        // Build the import path: @addon/path/to/file (without .js extension)
        // file_path is like /some/path/addons/web/static/src/core/utils.js
        // We want to extract what comes after 'static/src' or just use the relative path if that fails
        let path_str = path_buf.to_string_lossy().to_string();
        if let Some(idx) = path_str.find("/static/src/") {
            let rel_path = &path_str[idx + 12..]; // skip "/static/src/"
            if let Some(no_ext) = rel_path.strip_suffix(".js") {
                let import_path = format!("@{}/{}", addon_name, no_ext);
                self.module_paths.insert(import_path, path_buf.clone());
            }
        }

        // Index components
        for comp in &info.components {
            self.components
                .insert(comp.name.clone(), (path_buf.clone(), comp.clone()));
            if let Some(template) = &comp.template {
                self.templates
                    .insert(template.clone(), (path_buf.clone(), comp.range));
            }
        }

        // Index registry calls and services
        for reg in &info.registry_calls {
            let entries = self
                .registry_entries
                .entry(reg.category.clone())
                .or_default();
            entries.push((reg.key.clone(), path_buf.clone(), reg.range));

            // If it's a service registration, add to services index too
            if reg.category == "services" {
                self.services
                    .insert(reg.key.clone(), (path_buf.clone(), reg.range));
            }
        }

        self.modules.insert(path_buf, info);
    }

    /// Update a module when the file changes
    pub fn update_module(&mut self, file_path: PathBuf, info: JsModuleInfo) {
        // First remove old entries for this file
        let path = file_path.sanitize();
        let path_buf = PathBuf::from(&path);

        self.components.retain(|_, (p, _)| p != &path_buf);
        self.templates.retain(|_, (p, _)| p != &path_buf);
        self.services.retain(|_, (p, _)| p != &path_buf);
        for entries in self.registry_entries.values_mut() {
            entries.retain(|(_, p, _)| p != &path_buf);
        }

        // We don't remove from module_paths since the path hasn't changed
        // To properly update, we just add the new info
        // We don't have the addon_name here, but we don't need it because module_paths is already populated

        // Re-index components
        for comp in &info.components {
            self.components
                .insert(comp.name.clone(), (path_buf.clone(), comp.clone()));
            if let Some(template) = &comp.template {
                self.templates
                    .insert(template.clone(), (path_buf.clone(), comp.range));
            }
        }

        // Re-index registry calls and services
        for reg in &info.registry_calls {
            let entries = self
                .registry_entries
                .entry(reg.category.clone())
                .or_default();
            entries.push((reg.key.clone(), path_buf.clone(), reg.range));

            if reg.category == "services" {
                self.services
                    .insert(reg.key.clone(), (path_buf.clone(), reg.range));
            }
        }

        self.modules.insert(path_buf, info);
    }

    /// Remove a module from the index
    pub fn remove_module(&mut self, file_path: PathBuf) {
        let path = file_path.sanitize();
        let path_buf = PathBuf::from(&path);

        self.modules.remove(&path_buf);
        self.components.retain(|_, (p, _)| p != &path_buf);
        self.templates.retain(|_, (p, _)| p != &path_buf);
        self.services.retain(|_, (p, _)| p != &path_buf);
        for entries in self.registry_entries.values_mut() {
            entries.retain(|(_, p, _)| p != &path_buf);
        }
        self.module_paths.retain(|_, p| p != &path_buf);
    }

    /// Resolve an import path to a physical file path
    pub fn resolve_import(&self, source: &str, current_file: &Path) -> Option<PathBuf> {
        // Handle @odoo/owl (framework, not a file)
        if source == "@odoo/owl" {
            return None;
        }

        // Handle @addon/path
        if let Some(path) = self.module_paths.get(source) {
            return Some(path.clone());
        }

        // Handle relative imports
        if source.starts_with("./") || source.starts_with("../") {
            if let Some(parent) = current_file.parent() {
                let mut resolved = parent.to_path_buf();
                for part in source.split('/') {
                    if part == "." {
                        continue;
                    } else if part == ".." {
                        resolved.pop();
                    } else {
                        resolved.push(part);
                    }
                }
                resolved.set_extension("js");
                let sanitized = resolved.sanitize();
                return Some(PathBuf::from(sanitized));
            }
        }

        None
    }
}
