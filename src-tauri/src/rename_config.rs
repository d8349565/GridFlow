//! Password-encrypted portable configurations and account-protected local drafts.
use crate::{engine::Engine, model::{id, now, AppError, Result}, rename::RenameRules};
use duckdb::{params, Connection, OptionalExt};
use aes_gcm::{aead::{Aead, KeyInit, Payload}, Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, path::Path};
use zeroize::Zeroizing;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all="camelCase")]
pub struct RenamePresetSummary { pub id:String, pub name:String, pub updated_at:String }
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RenamePreset { pub summary:RenamePresetSummary, pub rules:RenameRules }

fn preset_connection(core:&Engine) -> Result<Connection> {
    core.guard()?;
    let conn=core.conn()?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS rename_presets (id VARCHAR PRIMARY KEY, name VARCHAR NOT NULL, name_key VARCHAR UNIQUE NOT NULL, updated_at VARCHAR NOT NULL, encrypted BLOB NOT NULL)")?;
    Ok(conn)
}
pub fn presets(core:&Engine) -> Result<Vec<RenamePresetSummary>> {
    let conn=preset_connection(core)?;
    let mut query=conn.prepare("SELECT id,name,updated_at FROM rename_presets ORDER BY updated_at DESC,name")?;
    let rows=query.query_map([],|r|Ok(RenamePresetSummary {id:r.get(0)?,name:r.get(1)?,updated_at:r.get(2)?}))?;
    Ok(rows.collect::<std::result::Result<Vec<_>,_>>()?)
}
pub fn save_preset(core:&Engine, preset_id:Option<String>, name:&str, rules:&RenameRules) -> Result<RenamePresetSummary> {
    let name=name.trim();
    if name.is_empty() || name.chars().count()>80 || name.chars().any(char::is_control) { return Err(AppError::new("CONFIG","请填写 1～80 个字符的配置名称")); }
    let encrypted=local_protection(&serialize(rules)?,false)?;
    let conn=preset_connection(core)?;
    if let Some(value)=&preset_id {
        let found:Option<String>=conn.query_row("SELECT id FROM rename_presets WHERE id=?",[value],|r|r.get(0)).optional()?;
        if found.is_none() { return Err(AppError::new("CONFIG","这份本机配置已不存在，请另存为新配置")); }
    }
    let key=name.to_lowercase();
    let duplicate:Option<String>=conn.query_row("SELECT id FROM rename_presets WHERE name_key=?",[&key],|r|r.get(0)).optional()?;
    if duplicate.as_ref().is_some_and(|value|Some(value)!=preset_id.as_ref()) { return Err(AppError::new("CONFIG","已有同名配置，请选用后更新，或填写另一个名称")); }
    let summary=RenamePresetSummary {id:preset_id.unwrap_or_else(id),name:name.into(),updated_at:now()};
    conn.execute("INSERT INTO rename_presets (id,name,name_key,updated_at,encrypted) VALUES (?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,name_key=excluded.name_key,updated_at=excluded.updated_at,encrypted=excluded.encrypted",params![&summary.id,&summary.name,&key,&summary.updated_at,encrypted])?;
    Ok(summary)
}
pub fn load_preset(core:&Engine,preset_id:&str) -> Result<RenamePreset> {
    let conn=preset_connection(core)?;
    let record:Option<(String,String,Vec<u8>)>=conn.query_row("SELECT name,updated_at,encrypted FROM rename_presets WHERE id=?",[preset_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let (name,updated_at,encrypted)=record.ok_or_else(||AppError::new("CONFIG","本机配置不存在，请刷新列表后重试"))?;
    let plain=Zeroizing::new(local_protection(&encrypted,true)?);
    Ok(RenamePreset {summary:RenamePresetSummary {id:preset_id.into(),name,updated_at},rules:parse(&plain)?})
}
pub fn import_preset(core:&Engine,path:&Path,password:&str) -> Result<RenamePreset> {
    let rules=load(path,password)?;
    let stem=path.file_stem().and_then(|s|s.to_str()).unwrap_or("导入的配置").trim();
    let base:String=stem.chars().filter(|c|!c.is_control()).take(64).collect();
    let base=if base.is_empty(){"导入的配置".to_string()}else{base};
    let names=presets(core)?.into_iter().map(|p|p.name.to_lowercase()).collect::<std::collections::HashSet<_>>();
    let mut name=base.clone();let mut number=2;
    while names.contains(&name.to_lowercase()) {name=format!("{base}（{number}）");number+=1;}
    let summary=save_preset(core,None,&name,&rules)?;
    Ok(RenamePreset {summary,rules})
}

const MAGIC: &[u8; 8] = b"GFRNCFG1";
const DRAFT_MAGIC: &[u8; 8] = b"GFRNDPA1";
const MAX_BYTES: usize = 16 * 1024 * 1024;
const HEADER: usize = 8 + 16 + 12;
const DRAFT_NAME: &str = "rename-draft.encrypted";

#[derive(Serialize, Deserialize)]
struct Configuration { version: u8, rules: RenameRules }

fn serialize(rules: &RenameRules) -> Result<Zeroizing<Vec<u8>>> {
    if rules.parts.len() > 64 || rules.checks.len() > 32 {
        return Err(AppError::new("CONFIG", "命名内容最多 64 项，校验最多 32 项"));
    }
    let bytes = Zeroizing::new(serde_json::to_vec(&Configuration { version:1, rules:rules.clone() })?);
    if bytes.len() > MAX_BYTES - 4096 { return Err(AppError::new("CONFIG", "配置过大，请减少对照内容后保存")); }
    Ok(bytes)
}
fn parse(bytes: &[u8]) -> Result<RenameRules> {
    let value: Configuration = serde_json::from_slice(bytes).map_err(|_| AppError::new("CONFIG", "配置格式无效，当前规则未改变"))?;
    if value.version != 1 { return Err(AppError::new("CONFIG", "此配置版本暂不支持，请使用相应版本的汇格打开")); }
    serialize(&value.rules)?;
    Ok(value.rules)
}
fn read_limited(path: &Path) -> Result<Vec<u8>> {
    let mut bytes=Vec::new(); File::open(path)?.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES { return Err(AppError::new("CONFIG", "配置文件超过 16MB，无法加载")); }
    Ok(bytes)
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent=path.parent().ok_or_else(|| AppError::new("CONFIG", "请选择有效的保存位置"))?;
    let temporary=parent.join(format!(".gridflow-config-{}.tmp",id()));
    let result=(|| {
        let mut file=OpenOptions::new().create_new(true).write(true).open(&temporary)?;
        file.write_all(bytes)?; file.sync_all()?; drop(file);
        fs::rename(&temporary,path)?;
        Ok(())
    })();
    if result.is_err() { let _=fs::remove_file(&temporary); }
    result
}
fn cipher(password: &str, salt: &[u8]) -> Result<Aes256Gcm> {
    if password.is_empty() || password.len() > 1024 { return Err(AppError::new("PASSWORD", "请输入配置密码，长度不超过 1024 字节")); }
    // Fixed, versioned KDF parameters cannot be inflated by an untrusted file.
    let params=Params::new(64 * 1024,3,1,Some(32)).map_err(|_| AppError::new("CONFIG", "无法初始化配置加密"))?;
    let mut key=Zeroizing::new([0u8;32]);
    Argon2::new(Algorithm::Argon2id,Version::V0x13,params).hash_password_into(password.as_bytes(),salt,key.as_mut())
        .map_err(|_| AppError::new("CONFIG", "配置密码处理失败"))?;
    Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| AppError::new("CONFIG", "无法初始化配置加密"))
}
pub fn save(path: &Path, password: &str, rules: &RenameRules) -> Result<()> {
    if path.extension().and_then(|v|v.to_str()).is_none_or(|v|!v.eq_ignore_ascii_case("gfrcfg")) { return Err(AppError::new("CONFIG", "请保存为 .gfrcfg 加密命名配置")); }
    if password.chars().count() < 8 { return Err(AppError::new("PASSWORD", "配置密码至少需要 8 个字符")); }
    let plain=serialize(rules)?; let mut salt=[0u8;16]; let mut nonce=[0u8;12];
    OsRng.try_fill_bytes(&mut salt).and_then(|_|OsRng.try_fill_bytes(&mut nonce)).map_err(|_|AppError::new("CONFIG","系统无法生成安全随机数，未保存配置"))?;
    let encrypted=cipher(password,&salt)?.encrypt(Nonce::from_slice(&nonce),Payload { msg:&plain, aad:MAGIC })
        .map_err(|_|AppError::new("CONFIG","配置加密失败，未保存文件"))?;
    let mut bytes=Vec::with_capacity(HEADER+encrypted.len());
    bytes.extend_from_slice(MAGIC); bytes.extend_from_slice(&salt); bytes.extend_from_slice(&nonce); bytes.extend_from_slice(&encrypted);
    atomic_write(path,&bytes)
}
pub fn load(path: &Path, password: &str) -> Result<RenameRules> {
    let bytes=read_limited(path)?;
    if bytes.len() < HEADER+16 || &bytes[..8] != MAGIC { return Err(AppError::new("CONFIG","这不是支持的加密命名配置，请选择 .gfrcfg 文件")); }
    let plain=Zeroizing::new(cipher(password,&bytes[8..24])?.decrypt(Nonce::from_slice(&bytes[24..36]),Payload { msg:&bytes[HEADER..], aad:MAGIC })
        .map_err(|_|AppError::new("PASSWORD","密码不正确或配置文件已损坏，当前规则未改变"))?);
    parse(&plain)
}
pub fn save_draft(root: &Path, rules: &RenameRules) -> Result<()> {
    let plain=serialize(rules)?; let encrypted=local_protection(&plain,false)?;
    let mut bytes=DRAFT_MAGIC.to_vec(); bytes.extend_from_slice(&encrypted);
    atomic_write(&root.join(DRAFT_NAME),&bytes)
}
pub fn load_draft(root: &Path) -> Result<Option<RenameRules>> {
    let path=root.join(DRAFT_NAME);
    if !path.try_exists()? { return Ok(None); }
    let bytes=read_limited(&path)?;
    if !bytes.starts_with(DRAFT_MAGIC) { return Err(AppError::new("CONFIG","本机加密草稿格式异常，请加载已保存的命名配置")); }
    let plain=Zeroizing::new(local_protection(&bytes[8..],true)?);
    parse(&plain).map(Some)
}

#[cfg(windows)]
fn local_protection(bytes: &[u8], decrypt: bool) -> Result<Vec<u8>> {
    use std::{ffi::c_void, ptr};
    #[repr(C)] struct Blob { len:u32, data:*mut u8 }
    #[link(name="crypt32")]
    extern "system" {
        fn CryptProtectData(input:*const Blob, description:*const u16, entropy:*const Blob, reserved:*mut c_void, prompt:*mut c_void, flags:u32, output:*mut Blob)->i32;
        fn CryptUnprotectData(input:*const Blob, description:*mut *mut u16, entropy:*const Blob, reserved:*mut c_void, prompt:*mut c_void, flags:u32, output:*mut Blob)->i32;
    }
    #[link(name="kernel32")]
    extern "system" { fn LocalFree(memory:*mut c_void)->*mut c_void; }
    let input=Blob { len:bytes.len().try_into().map_err(|_|AppError::new("CONFIG","配置过大"))?, data:bytes.as_ptr().cast_mut() };
    let mut output=Blob { len:0, data:ptr::null_mut() };
    // Account scope; no machine-wide flag or hard-coded encryption key.
    let success=unsafe { if decrypt { CryptUnprotectData(&input,ptr::null_mut(),ptr::null(),ptr::null_mut(),ptr::null_mut(),1,&mut output) }
        else { CryptProtectData(&input,ptr::null(),ptr::null(),ptr::null_mut(),ptr::null_mut(),1,&mut output) } };
    if success == 0 { return Err(AppError::new("CONFIG",if decrypt { "无法解密本机草稿，请用密码加载保存的配置文件" } else { "无法加密保存本机草稿，请检查 Windows 账号和数据目录" })); }
    if output.data.is_null() { return Err(AppError::new("CONFIG","系统返回了空的加密配置，请重新保存")); }
    let value=unsafe { let value=std::slice::from_raw_parts(output.data,output.len as usize).to_vec(); ptr::write_bytes(output.data,0,output.len as usize); LocalFree(output.data.cast()); value };
    Ok(value)
}
#[cfg(not(windows))]
fn local_protection(_: &[u8], _: bool) -> Result<Vec<u8>> { Err(AppError::new("CONFIG","当前系统不支持本机加密草稿，请使用密码配置文件")) }
