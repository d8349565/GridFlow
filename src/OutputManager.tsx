import { Plus, Trash2 } from "lucide-react";
import { Button, Modal } from "./components";
import { defaultRules } from "./api";
import { outputNameError } from "./summary";
import type { RuleSet, SourceFile } from "./types";

export default function OutputManager({ rules, files, activeId, onChange, onSelect, onClose }: {
  rules: RuleSet; files: SourceFile[]; activeId: string; onChange: (rules: RuleSet) => void;
  onSelect: (id: string) => void; onClose: () => void;
}) {
  const sheetNames = [...new Set(files.flatMap((f) => f.sheets.map((s) => s.name)))];
  function add() {
    const id = crypto.randomUUID();
    let number = rules.outputSheets.length + 1;
    while (rules.outputSheets.some((o) => o.name === `汇总表 ${number}`)) number++;
    onChange({ ...rules, outputSheets: [...rules.outputSheets, {
      id, name: `汇总表 ${number}`, sheets: [], rules: { ...defaultRules(), mergeMode: "single", sources: rules.sources.map((s) => ({ ...s, enabled: false })) },
    }] });
    onSelect(id);
  }
  return <Modal title="设置要导出的汇总表" wide onClose={onClose}>
    <div className="modal-body">
      <p className="output-manager-intro">每一项对应 Excel 中的一张结果表。填写名称，再勾选要汇总的来源工作表；只用于补充信息的工作表不用勾选。</p>
      <div className="output-manager-list">{rules.outputSheets.map((output, index) => {
        const problem = outputNameError(output.name, rules.outputSheets.filter((o) => o.id !== output.id).map((o) => o.name));
        const selectedCount = output.rules.sources.filter((s) => s.enabled).length;
        return <section className={`output-manager-item ${output.id === activeId ? "selected" : ""}`} key={output.id}>
          <div className="output-manager-heading"><strong>结果表 {index + 1}</strong><Button icon={Trash2} onClick={() => {
            const remaining = rules.outputSheets.filter((o) => o.id !== output.id);
            onChange(remaining.length ? { ...rules, outputSheets: remaining } : { ...defaultRules(), sources: rules.sources.map((s) => ({ ...s, enabled: false })) });
            if (!remaining.length) onClose();
            else if (activeId === output.id) onSelect(remaining[0].id);
          }}>移除</Button></div>
          <label className="stacked-label">Excel 中的表名<input value={output.name} aria-invalid={!!problem} aria-describedby={problem ? `output-name-${output.id}` : undefined} onChange={(e) => onChange({ ...rules, outputSheets: rules.outputSheets.map((o) => o.id === output.id ? { ...o, name: e.target.value } : o) })} /></label>
          {problem && <p className="field-error" id={`output-name-${output.id}`}>{problem}</p>}
          <fieldset className="output-sources"><legend>汇总哪些工作表？<span>已选 {selectedCount} 张</span></legend>
            {sheetNames.map((name) => {
              const ids = files.flatMap((f) => f.sheets.filter((s) => s.name === name).map((s) => s.id));
              const selected = output.rules.sources.filter((s) => s.enabled && ids.includes(s.sheetId)).length;
              return <label key={name}><input type="checkbox" checked={selected === ids.length} ref={(input) => { if (input) input.indeterminate = selected > 0 && selected < ids.length; }} onChange={(e) => onChange({ ...rules, outputSheets: rules.outputSheets.map((o) => o.id !== output.id ? o : {
                ...o, rules: { ...o.rules, sources: o.rules.sources.map((s) => ids.includes(s.sheetId) ? { ...s, enabled: e.target.checked } : s) },
              }) })} /><span>{name}</span><small>{ids.length} 张</small></label>;
            })}
          </fieldset>
          {!selectedCount && <p className="field-error">请勾选至少一张来源工作表。</p>}
          <button className="text-button" onClick={() => { onSelect(output.id); onClose(); }}>配置这张表的表头、输出列和补充信息 →</button>
        </section>;
      })}</div>
      <Button icon={Plus} disabled={rules.outputSheets.length >= 32} onClick={add}>再增加一张汇总表</Button>
    </div>
    <div className="modal-footer"><small className="muted">设置自动保存，可用顶部“撤销”恢复。</small><Button primary onClick={onClose}>完成设置</Button></div>
  </Modal>;
}
