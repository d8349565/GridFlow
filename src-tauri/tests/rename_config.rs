use excel_workbench::{rename::{RenameCell, RenameLookupTable, RenamePart, RenameRules}, rename_config};
use std::{fs, path::Path};
use tempfile::TempDir;

fn isolated() -> TempDir { TempDir::new_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap() }
fn rules() -> RenameRules {
    RenameRules {
        parts: vec![RenamePart::TableLookup { source: RenameCell { sheet: "说明".into(), row: 3, column: 8 }, table_id: "customers".into(), match_column: "编号".into(), value_column: "名称".into(), fallback: None }],
        separator: "_".into(),
        prefix: "预算_".into(),
        suffix: String::new(),
        checks: vec![],
        lookup_tables: vec![RenameLookupTable {
            id: "customers".into(),
            name: "客户资料".into(),
            columns: vec!["编号".into(), "名称".into()],
            rows: vec![vec!["001".into(), "仅供加密验证的客户名称".into()]],
        }],
    }
}

#[test]
fn portable_configuration_authenticates_and_keeps_lookup_private() {
    let first=isolated(); let second=isolated(); let path=first.path().join("规则.gfrcfg"); let other=second.path().join("其他电脑.gfrcfg");
    let password="temporary-test-password"; let value=rules();
    rename_config::save(&path,password,&value).unwrap();
    let encrypted=fs::read(&path).unwrap();
    assert!(!encrypted.windows("仅供加密验证的客户名称".len()).any(|v|v=="仅供加密验证的客户名称".as_bytes()));
    assert!(!encrypted.windows(password.len()).any(|v|v==password.as_bytes()));
    fs::copy(&path,&other).unwrap();
    assert_eq!(serde_json::to_value(rename_config::load(&other,password).unwrap()).unwrap(),serde_json::to_value(&value).unwrap());
    assert!(rename_config::load(&other,"wrong-password").is_err());
    let mut damaged=encrypted.clone(); let last=damaged.len()-1; damaged[last]^=1; fs::write(&other,&damaged).unwrap();
    assert!(rename_config::load(&other,password).is_err());
    rename_config::save(&path,password,&value).unwrap();
    assert_ne!(fs::read(&path).unwrap(),encrypted,"each save needs a fresh salt and nonce");
    assert!(rename_config::save(&other,"short",&value).is_err());
    fs::write(&other,serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(rename_config::load(&other,password).is_err(),"plaintext must not be mistaken for an encrypted file");
}

#[cfg(windows)]
#[test]
fn local_draft_is_account_encrypted_and_recovers_all_lookup_entries() {
    let temp=isolated(); let value=rules();
    rename_config::save_draft(temp.path(),&value).unwrap();
    let encrypted=fs::read(temp.path().join("rename-draft.encrypted")).unwrap();
    assert!(!encrypted.windows("仅供加密验证的客户名称".len()).any(|v|v=="仅供加密验证的客户名称".as_bytes()));
    assert_eq!(serde_json::to_value(rename_config::load_draft(temp.path()).unwrap().unwrap()).unwrap(),serde_json::to_value(&value).unwrap());
    let empty=RenameRules {parts:vec![],separator:"_".into(),prefix:String::new(),suffix:String::new(),checks:vec![],lookup_tables:vec![]};
    rename_config::save_draft(temp.path(),&empty).unwrap();
    assert!(rename_config::load_draft(temp.path()).unwrap().unwrap().parts.is_empty());
}

#[cfg(windows)]
#[test]
fn named_local_presets_persist_encrypt_update_and_import_without_overwriting() {
    use excel_workbench::engine::Engine;
    let temp=isolated();let value=rules();
    let core=Engine::open(temp.path()).unwrap();
    let first=rename_config::save_preset(&core,None,"销售预算",&value).unwrap();
    let second=rename_config::save_preset(&core,None,"其他规则",&value).unwrap();
    assert_eq!(rename_config::presets(&core).unwrap().len(),2);
    assert!(rename_config::save_preset(&core,None,"销售预算",&value).is_err());
    let mut updated=value.clone();updated.prefix="本机已更新".into();
    rename_config::save_preset(&core,Some(first.id.clone()),"销售预算更新",&updated).unwrap();
    let portable=temp.path().join("其他规则.gfrcfg");rename_config::save(&portable,"temporary-password",&value).unwrap();
    let imported=rename_config::import_preset(&core,&portable,"temporary-password").unwrap();
    assert_eq!(imported.summary.name,"其他规则（2）");assert_ne!(second.id,imported.summary.id);
    drop(core);
    let core=Engine::open(temp.path()).unwrap();
    assert_eq!(rename_config::presets(&core).unwrap().len(),3);
    assert_eq!(rename_config::load_preset(&core,&first.id).unwrap().rules.prefix,"本机已更新");
    assert_eq!(serde_json::to_value(rename_config::load_preset(&core,&second.id).unwrap().rules).unwrap(),serde_json::to_value(&value).unwrap());
    drop(core);
    for entry in fs::read_dir(temp.path()).unwrap() {
        let path=entry.unwrap().path();if !path.is_file() {continue;}
        let bytes=fs::read(path).unwrap();let marker="仅供加密验证的客户名称".as_bytes();
        assert!(!bytes.windows(marker.len()).any(|v|v==marker),"local database must not contain lookup plaintext");
    }
}
