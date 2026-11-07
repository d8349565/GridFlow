import { useEffect, useState, type FormEvent } from "react";
import { Download, FolderOpen, Save, Upload } from "lucide-react";
import { call } from "./api";
import { Button, Modal } from "./components";
import type { RenamePreset, RenamePresetSummary, RenameRules } from "./types";

export default function RenamePresetDialog({ mode, current, rules, onClose, onSaved, onLoad, onTransfer, onSaveAs, onWorking }: {
  mode: "save" | "list"; current: RenamePresetSummary | null; rules: RenameRules;
  onClose: () => void; onSaved: (preset: RenamePresetSummary) => void; onLoad: (preset: RenamePreset) => void;
  onTransfer: (mode: "save" | "load") => void; onSaveAs: () => void; onWorking: (working: boolean) => void;
}) {
  const [name, setName] = useState(current?.name ?? "");
  const [asNew, setAsNew] = useState(false);
  const [presets, setPresets] = useState<RenamePresetSummary[]>([]);
  const [loading, setLoading] = useState(mode === "list");
  const [working, setWorking] = useState("");
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (mode !== "list") return;
    let disposed = false; setLoading(true); setError("");
    void call<RenamePresetSummary[]>("rename_presets").then(next => { if (!disposed) setPresets(next); }).catch(e => { if (!disposed) setError(String(e?.message ?? e)); }).finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [mode, attempt]);
  async function run(id: string, action: () => Promise<void>) {
    if (working) return; setWorking(id); onWorking(true); setError("");
    try { await action(); onClose(); } catch (e) { setError(String((e as { message?: string })?.message ?? e)); }
    finally { setWorking(""); onWorking(false); }
  }
  function save(event: FormEvent) {
    event.preventDefault();
    void run("save", async () => onSaved(await call<RenamePresetSummary>("rename_save_preset", { presetId: asNew ? null : current?.id ?? null, name: name.trim(), rules })));
  }
  return <Modal title={mode === "save" ? "保存到本机配置" : "本机配置"} onClose={() => { if (!working) onClose(); }}>
    {mode === "save" ? <form onSubmit={save}>
      <div className="modal-body rename-dialog-body">
        <p>命名规则、校验和完整对照表一起保存在本机，下次从“本机配置”直接选择。</p>
        <label className="rename-field">配置名称<input autoFocus required maxLength={80} value={name} disabled={!!working} placeholder="例如：销售预算命名" onChange={e => setName(e.target.value)} /></label>
        {current && <label className="check-label"><input type="checkbox" checked={asNew} disabled={!!working} onChange={e => setAsNew(e.target.checked)} />另存为一份新配置</label>}
        <small>{current && !asNew ? `将更新“${current.name}”。` : "文件清单和输出位置按本次任务选择。"} 对照内容自动加密保存。</small>
        {error && <p className="notice error" role="alert">{error}</p>}
      </div>
      <div className="modal-footer"><Button disabled={!!working} onClick={onClose}>取消</Button><Button type="submit" primary icon={Save} loading={!!working} disabled={!name.trim() || !!working}>{current && !asNew ? "更新本机配置" : "保存到本机"}</Button></div>
    </form> : <>
      <div className="modal-body rename-dialog-body">
        <p>选择后应用命名规则、校验和对照表。当前规则会替换，文件清单保持不变。</p>
        {loading ? <p role="status">正在读取配置…</p> : error ? <p className="notice error" role="alert">{error} <Button onClick={() => setAttempt(v => v + 1)}>重新读取</Button></p> : !presets.length ? <div className="rename-preset-empty"><FolderOpen size={28} /><strong>还没有保存的配置</strong><span>先配好命名规则，再点“保存配置”命名保存。</span></div> : <ul className="rename-preset-list">{presets.map(p => <li key={p.id}><div><strong>{p.name}</strong><small>{p.id === current?.id ? "当前配置 · " : ""}更新于 {new Date(p.updatedAt).toLocaleString("zh-CN", { hour12: false })}</small></div><Button primary={p.id === current?.id} loading={working === p.id} disabled={!!working} onClick={() => void run(p.id, async () => onLoad(await call<RenamePreset>("rename_load_preset", { presetId: p.id })))}>使用配置</Button></li>)}</ul>}
        <div className="rename-preset-portable"><strong>在其他电脑使用</strong><p>导出密码加密的配置文件，换电脑后导入；对照表一起带走。</p><div><Button icon={Upload} disabled={!!working} onClick={() => onTransfer("load")}>导入配置文件</Button><Button icon={Download} disabled={!!working} onClick={() => onTransfer("save")}>导出当前配置</Button></div></div>
      </div>
      <div className="modal-footer"><Button disabled={!!working} icon={Save} onClick={onSaveAs}>另存当前配置</Button><Button disabled={!!working} onClick={onClose}>关闭</Button></div>
    </>}
  </Modal>;
}
