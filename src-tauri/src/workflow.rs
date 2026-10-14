use crate::model::*;
use std::collections::{HashMap, HashSet};

pub fn mode(rules: &RuleSet) -> &str {
    rules.merge_mode.as_deref().unwrap_or(if rules.output_sheets.is_empty() { "single" } else { "legacy" })
}

pub fn file_enabled(rules: &RuleSet, file_id: &str) -> bool {
    rules.file_selections.iter().find(|f| f.file_id == file_id).is_none_or(|f| f.enabled)
}
fn sync_first_file(rules: &mut RuleSet, files: &[SourceFile]) {
    if mode(rules) == "legacy" { return; }
    let Some(first) = files.iter().find(|f| file_enabled(rules, &f.id)) else { return; };
    let settings = first.sheets.iter().filter_map(|sheet| rules.sources.iter().find(|s| s.sheet_id == sheet.id).map(|s| (sheet.name.clone(), s.clone()))).collect::<HashMap<_, _>>();
    let first_stem = std::path::Path::new(&first.name).file_stem().unwrap_or_default().to_string_lossy();
    for file in files.iter().filter(|f| f.id != first.id) {
        for sheet in &file.sheets {
            if let Some(source) = rules.sources.iter_mut().find(|s| s.sheet_id == sheet.id) {
                if let Some(template) = settings.get(&sheet.name) {
                    source.enabled = template.enabled; source.header_row = template.header_row; source.header_range = template.header_range.clone();
                    let stem = std::path::Path::new(&file.name).file_stem().unwrap_or_default().to_string_lossy();
                    source.source_name = if template.source_name.contains(&first.name) { template.source_name.replace(&first.name, &file.name) }
                        else if !first_stem.is_empty() { template.source_name.replace(first_stem.as_ref(), stem.as_ref()) } else { template.source_name.clone() };
                } else { source.enabled = false; }
            }
        }
    }
}
pub fn validate_template(rules: &RuleSet, files: &[SourceFile]) -> Result<()> {
    if mode(rules) == "legacy" { return Ok(()); }
    let Some(first) = files.iter().find(|f| file_enabled(rules, &f.id)) else { return Ok(()); };
    for sheet in &first.sheets {
        let Some(source) = rules.sources.iter().find(|s| s.sheet_id == sheet.id && s.enabled) else { continue; };
        for file in files.iter().filter(|f| file_enabled(rules, &f.id)) {
            let target = file.sheets.iter().find(|s| s.name == sheet.name).ok_or_else(|| AppError::new("SOURCE_SCHEMA", format!("{} 缺少已选工作表「{}」，请补齐或暂时排除此文件", file.name, sheet.name)))?;
            if source.header_row < 1 || source.header_row > target.rows || source.header_range.as_ref().is_some_and(|r| r.end_column > target.width) {
                return Err(AppError::new("SOURCE_SCHEMA", format!("{} / {} 无法使用首文件的表头范围，请检查原文件", file.name, sheet.name)));
            }
        }
    }
    Ok(())
}

pub fn normalize(mut rules: RuleSet, files: &[SourceFile]) -> RuleSet {
    let names = files.iter().flat_map(|f| &f.sheets).map(|s| (s.id.as_str(), s.name.as_str())).collect::<HashMap<_, _>>();
    if rules.merge_mode.is_none() && !rules.output_sheets.is_empty() {
        let mut enabled = HashMap::<String, SourceSelection>::new();
        let mut group_names = HashSet::new();
        let mut groups = Vec::new();
        let mut fixed = HashMap::<String, FixedCellRule>::new();
        let mut fixed_order = Vec::new();
        let mut compatible = true;
        for output in &rules.output_sheets {
            let selected = output.rules.sources.iter().filter(|s| s.enabled).collect::<Vec<_>>();
            let group = selected.iter().filter_map(|s| names.get(s.sheet_id.as_str()).copied()).collect::<HashSet<_>>();
            if group.len() != 1 || selected.is_empty() || selected.iter().any(|s| !names.contains_key(s.sheet_id.as_str())) {
                compatible = false; break;
            }
            let name = (*group.iter().next().unwrap()).to_string();
            if !group_names.insert(name.clone()) { compatible = false; break; }
            for source in selected {
                if enabled.get(&source.sheet_id).is_some_and(|old| old != source) { compatible = false; break; }
                enabled.insert(source.sheet_id.clone(), source.clone());
            }
            for rule in &output.rules.fixed_cells {
                if let Some(old) = fixed.get_mut(&rule.id) {
                    if old.name != rule.name || old.sheet_name != rule.sheet_name || old.address != rule.address { compatible = false; break; }
                    old.target_sheets.push(name.clone());
                } else {
                    fixed_order.push(rule.id.clone());
                    let mut rule = rule.clone(); rule.target_sheets = vec![name.clone()]; fixed.insert(rule.id.clone(), rule);
                }
            }
            groups.push(name);
        }
        if compatible {
            for source in &mut rules.sources {
                if let Some(selected) = enabled.remove(&source.sheet_id) { *source = selected; } else { source.enabled = false; }
            }
            rules.sources.extend(enabled.into_values());
            rules.fixed_cells = fixed_order.iter().filter_map(|id| fixed.remove(id)).collect();
            for (output, name) in rules.output_sheets.iter_mut().zip(groups) {
                output.source_sheet_name = Some(name);
                output.rules.sources.clear(); output.rules.fixed_cells.clear();
                output.rules.file_selections.clear(); output.rules.merge_mode = Some("single".into());
            }
            rules.merge_mode = Some("bySheet".into());
        } else { rules.merge_mode = Some("legacy".into()); }
    }
    if rules.merge_mode.is_none() { rules.merge_mode = Some("single".into()); }
    let valid_files = files.iter().map(|f| f.id.as_str()).collect::<HashSet<_>>();
    rules.file_selections.retain(|f| valid_files.contains(f.file_id.as_str()));
    for file in files {
        if !rules.file_selections.iter().any(|f| f.file_id == file.id) {
            let enabled = if mode(&rules) == "legacy" { rules.output_sheets.iter().any(|o| o.rules.sources.iter().any(|r| r.enabled && file.sheets.iter().any(|s| s.id == r.sheet_id))) }
                else { file.sheets.iter().any(|s| rules.sources.iter().any(|r| r.sheet_id == s.id && r.enabled)) };
            rules.file_selections.push(FileSelection { file_id: file.id.clone(), enabled });
        }
    }
    sync_first_file(&mut rules, files);
    if mode(&rules) != "bySheet" { return rules; }
    let mut used = rules.output_sheets.iter().map(|o| o.name.trim().to_lowercase()).collect::<HashSet<_>>();
    for source in rules.sources.iter().filter(|s| s.enabled) {
        let Some(name) = names.get(source.sheet_id.as_str()).copied() else { continue; };
        if rules.output_sheets.iter().any(|o| o.source_sheet_name.as_deref() == Some(name)) { continue; }
        let mut leaf = rules.clone();
        leaf.sources.clear(); leaf.output_sheets.clear(); leaf.fixed_cells.clear(); leaf.file_selections.clear(); leaf.merge_mode = Some("single".into());
        let base = name.chars().map(|c| if "[]:*?/\\".contains(c) || c.is_control() { '_' } else { c }).collect::<String>();
        let base = base.trim().trim_matches('\'');
        let base = if base.is_empty() || base.eq_ignore_ascii_case("history") { "汇总数据" } else { base };
        let mut number = 1;
        let output_name = loop {
            let suffix = if number == 1 { String::new() } else { format!(" ({number})") };
            let candidate = format!("{}{}", base.chars().take(31 - suffix.chars().count()).collect::<String>(), suffix);
            if used.insert(candidate.to_lowercase()) { break candidate; }
            number += 1;
        };
        rules.output_sheets.push(OutputSheet { id: id(), name: output_name, source_sheet_name: Some(name.into()), rules: leaf, sheets: vec![] });
    }
    rules
}

pub fn execution(rules: &RuleSet, files: &[SourceFile], output: Option<&OutputSheet>) -> RuleSet {
    if mode(rules) == "legacy" { return output.map(|o| o.rules.clone()).unwrap_or_else(|| rules.clone()); }
    let mut leaf = output.map(|o| o.rules.clone()).unwrap_or_else(|| rules.clone());
    let group = output.and_then(|o| o.source_sheet_name.as_deref());
    let sheets = files.iter().flat_map(|f| &f.sheets).map(|s| (s.id.as_str(), s)).collect::<HashMap<_, _>>();
    leaf.sources = rules.sources.iter().map(|s| {
        let mut s = s.clone();
        s.enabled = s.enabled && sheets.get(s.sheet_id.as_str()).is_some_and(|sheet| file_enabled(rules, &sheet.file_id) && group.is_none_or(|name| name == sheet.name));
        s
    }).collect();
    leaf.fixed_cells = rules.fixed_cells.iter().filter(|r| r.target_sheets.is_empty() || group.map(|name| r.target_sheets.iter().any(|n| n == name))
        .unwrap_or_else(|| leaf.sources.iter().filter(|s| s.enabled).any(|s| sheets.get(s.sheet_id.as_str()).is_some_and(|sheet| r.target_sheets.contains(&sheet.name))))).cloned().collect();
    leaf.output_sheets.clear(); leaf.file_selections.clear(); leaf.merge_mode = Some("single".into());
    leaf
}

pub fn active_outputs<'a>(rules: &'a RuleSet, files: &[SourceFile]) -> Vec<&'a OutputSheet> {
    if mode(rules) == "legacy" { return rules.output_sheets.iter().collect(); }
    rules.output_sheets.iter().filter(|o| execution(rules, files, Some(o)).sources.iter().any(|s| s.enabled)).collect()
}
