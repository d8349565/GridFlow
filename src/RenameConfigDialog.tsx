import { useState, type FormEvent } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { LockKeyhole } from "lucide-react";
import { call } from "./api";
import { Button, Modal } from "./components";
import type { RenamePreset, RenameRules } from "./types";

export default function RenameConfigDialog({ mode, path, rules, onLoad, onSaved, onClose, onWorking }: {
  mode: "save" | "load"; path: string; rules: RenameRules;
  onLoad: (preset: RenamePreset, path: string) => void; onSaved: (path: string) => void;
  onClose: () => void; onWorking: (working: boolean) => void;
}) {
  const [password, setPassword] = useState("");
  const [repeat, setRepeat] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (working) return;
    if (mode === "save" && (Array.from(password).length < 8 || password !== repeat)) { setError("密码至少需要 8 个字符，且两次输入需一致。"); return; }
    setWorking(true); onWorking(true); setError("");
    try {
      if (mode === "save") {
        const target=await save({ defaultPath:"汇格命名配置.gfrcfg", filters:[{name:"汇格加密命名配置",extensions:["gfrcfg"]}] });
        if (!target) return;
        await call("rename_save_config", { path:target, password, rules }); onSaved(target);
      } else {
        const next=await call<RenamePreset>("rename_import_preset", { path, password }); onLoad(next,path);
      }
      setPassword(""); setRepeat(""); onClose();
    } catch (e) { setError(e && typeof e === "object" && "message" in e ? String(e.message) : String(e)); }
    finally { setWorking(false); onWorking(false); }
  }
  return <Modal title={mode === "save" ? "导出加密配置" : "导入加密配置"} onClose={() => { if (!working) onClose(); }}>
    <form className="rename-config-form" onSubmit={e => void submit(e)}>
      <div className="modal-body rename-dialog-body">
        <p>{mode === "save" ? "命名规则、校验和完整对照表会一起加密导出。复制配置文件到其他电脑，输入此密码即可导入。" : `输入配置密码，导入 ${path.split(/[\\/]/).at(-1)} 中的命名规则和对照表，并加入本机配置列表。`}</p>
        <label className="rename-field">配置密码<input autoFocus type="password" autoComplete={mode === "save" ? "new-password" : "current-password"} required maxLength={1024} value={password} disabled={working} onChange={e => setPassword(e.target.value)} placeholder={mode === "save" ? "至少 8 个字符" : "输入保存时设置的密码"} /></label>
        {mode === "save" && <><label className="rename-field">再次输入密码<input type="password" autoComplete="new-password" required value={repeat} disabled={working} onChange={e => setRepeat(e.target.value)} /></label><small>请保管好密码，忘记密码将无法打开此配置。</small></>}
        {error && <p className="notice error" role="alert">{error}</p>}
      </div>
      <div className="modal-footer"><Button disabled={working} onClick={onClose}>取消</Button><Button type="submit" primary icon={LockKeyhole} loading={working} disabled={!password || (mode === "save" && (Array.from(password).length < 8 || password !== repeat))}>{mode === "save" ? "选择位置并导出" : "解密并导入"}</Button></div>
    </form>
  </Modal>;
}
