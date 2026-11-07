import { useEffect, useRef, useState, type Ref } from "react";
import { ArrowDown, ArrowUp, MousePointer2, Plus, X } from "lucide-react";
import { Button } from "./components";
import { coordinate } from "./api";
import { rangeLabel } from "./ExcelPreview";
import { parseLookupTable } from "./renameTable";
import type { RenameCell, RenameCheck, RenameCondition, RenamePart, RenameRow, RenameRules, RenameSum } from "./types";

export type PickTarget = { kind: "tag"; part?: number } | { kind: "expected"; check?: number } | { kind: "sum"; check: number; sum?: number } | { kind: "sumCondition"; check: number; sum: number };

function Location({ sheet, address }: { sheet: string; address: string }) {
  return <span className="rename-source-location"><span className="rename-source-sheet">{sheet}</span><span className="rename-source-address">{address}</span></span>;
}
function SourcePicker({ sheet, address, onPick, purpose }: { sheet: string; address: string; onPick: () => void; purpose: string }) {
  return <Button className="quiet rename-source-picker" icon={MousePointer2} title={`${purpose}：${sheet}!${address}`} aria-label={`${purpose}：${sheet}!${address}`} onClick={onPick}>
    <Location sheet={sheet} address={address} /><span className="rename-source-action">重选</span>
  </Button>;
}
function CellLocation({ source }: { source: RenameCell }) {
  return <div className="rename-source-readonly" title={`${source.sheet}!${coordinate(source.row, source.column)}`}><Location sheet={source.sheet} address={coordinate(source.row, source.column)} /></div>;
}
function ConditionFields({ value, onChange, onPick }: { value: RenameCondition; onChange: (value: RenameCondition) => void; onPick: () => void }) {
  return <div className="rename-condition">
    <div className="rename-field"><span>判断依据</span><SourcePicker sheet={value.source.sheet} address={coordinate(value.source.row,value.source.column)} onPick={onPick} purpose="重新选择条件单元格" /></div>
    <label className="rename-inline">满足条件<select aria-label="触发条件" value={value.operator} onChange={e => onChange({ ...value, operator:e.target.value })}><option value="positive">大于 0</option><option value="nonempty">有内容</option><option value="equals">等于</option></select></label>
    {value.operator === "equals" && <label className="rename-field">比较内容<input aria-label="条件值" value={value.value} onChange={e => onChange({ ...value, value:e.target.value })} /></label>}
  </div>;
}
function LookupRows({ part, onChange }: { part: Extract<RenamePart, { kind:"lookup" }>; onChange: (part: RenamePart) => void }) {
  const asText=() => part.entries.map(e => [e.key,e.value].map(v => /[\t\r\n"]/.test(v) ? `"${v.replaceAll('"','""')}"` : v).join("\t")).join("\n");
  const [draft,setDraft]=useState(asText);
  const lastEntries=useRef(part.entries);
  useEffect(() => { if(part.entries !== lastEntries.current) { lastEntries.current=part.entries; setDraft(asText()); } },[part.entries]);
  const parsed=parseLookupTable(draft);
  return <details className="rename-advanced"><summary>对照表 · {part.entries.length} 行</summary><div className="rename-advanced-body">
    <textarea aria-label="对照表，两列用制表符分隔" rows={5} value={draft} onChange={e => {
      setDraft(e.target.value); const table=parseLookupTable(e.target.value);
      const entries=table.error ? [] : table.rows.map(row => ({ key:row[0] ?? "", value:row.length === 2 ? row[1] : "" }));
      lastEntries.current=entries; onChange({ ...part,entries });
    }} />
    {parsed.error && <small className="rename-invalid">{parsed.error}</small>}
    <label className="rename-field">找不到对照时<input placeholder="留空则跳过该文件" value={part.fallback ?? ""} onChange={e => onChange({ ...part,fallback:e.target.value || null })} /></label>
  </div></details>;
}
const purposes: Record<RenamePart["kind"],string> = {
  cell:"取出每份文件相同位置的内容。", lookup:"用单元格里的编码，查找对应信息。",
  text:"这段文字会原样加入文件名。", condition:"满足条件时，才把标签加入文件名。",
};

export default function RenameRulesPanel({ rules, live, livePending, busy, picking, ready, editorRef, onChange, onPick, onReset }: {
  rules:RenameRules; live:RenameRow | null; livePending:boolean; busy:boolean; picking:boolean; ready:boolean;
  editorRef:Ref<HTMLElement>; onChange:(rules:RenameRules) => void; onPick:(target:PickTarget) => void; onReset:() => void;
}) {
  const disabled=busy || picking || !ready;
  const setPart=(index:number,part:RenamePart) => onChange({ ...rules,parts:rules.parts.map((p,i) => i === index ? part : p) });
  const setCheck=(index:number,check:RenameCheck) => onChange({ ...rules,checks:rules.checks.map((c,i) => i === index ? check : c) });
  const setSum=(checkIndex:number,sumIndex:number,sum:RenameSum) => {
    const check=rules.checks[checkIndex]; setCheck(checkIndex,{ ...check,sums:check.sums.map((s,i) => i === sumIndex ? sum : s) });
  };
  function movePart(index:number,direction:number) {
    const parts=[...rules.parts]; [parts[index],parts[index+direction]]=[parts[index+direction],parts[index]]; onChange({ ...rules,parts });
  }
  return <aside ref={editorRef} id="rename-rules-panel" className="rename-editor">
    <header className="rename-editor-heading"><div><h2>命名规则</h2><small>设置内容，立即查看新文件名</small></div><Button className="quiet" disabled={disabled || (!rules.parts.length && !rules.checks.length)} onClick={onReset}>清空规则</Button></header>
    <div className="rename-editor-scroll"><fieldset disabled={disabled}>
      <section className="rename-builder-section" aria-labelledby="rename-format-heading"><h3 id="rename-format-heading">文件名格式</h3>
        <div className="rename-affixes"><label>前缀<input value={rules.prefix} onChange={e => onChange({ ...rules,prefix:e.target.value })} /></label><label>分隔符<input value={rules.separator} onChange={e => onChange({ ...rules,separator:e.target.value })} /></label><label>后缀<input placeholder="如：万元" value={rules.suffix} onChange={e => onChange({ ...rules,suffix:e.target.value })} /></label></div>
        <small className="rename-field-help">原文件扩展名自动保留。</small>
      </section>
      <section className="rename-builder-section" aria-labelledby="rename-parts-heading"><div className="rename-section-heading"><h3 id="rename-parts-heading">文件名组成</h3><span>{rules.parts.length} 项</span></div><p className="rename-section-purpose">下面的内容会从上到下依次拼接。可在预览上选单元格，再加入文件名或添加对照。</p>
        <ol className="rename-parts">{rules.parts.map((part,index) => {
          const value=live?.values[index];
          const omitted=part.kind === "condition" && value === "";
          const preview=livePending ? "正在更新…" : value !== undefined ? value || (omitted ? "不加入（条件不满足）" : "空白") : live ? "暂无法取值" : "等待预览";
          const title=part.kind === "cell" ? "单元格内容" : part.kind === "lookup" ? part.name || "对照信息" : part.kind === "condition" ? "条件标签" : "固定文字";
          return <li key={index} className="rename-rule-card"><div className="rename-rule-header"><div className="rename-rule-heading"><span className="rename-rule-order">{index+1}</span><h4>{title}</h4></div><div className="rename-rule-actions">
            <Button className="quiet icon-button" aria-label={`上移第 ${index+1} 项`} title="上移" icon={ArrowUp} disabled={!index} onClick={() => movePart(index,-1)} /><Button className="quiet icon-button" aria-label={`下移第 ${index+1} 项`} title="下移" icon={ArrowDown} disabled={index === rules.parts.length-1} onClick={() => movePart(index,1)} /><Button className="quiet icon-button" icon={X} title="移除此命名项" aria-label={`移除第 ${index+1} 项`} onClick={() => onChange({ ...rules,parts:rules.parts.filter((_,i) => i !== index) })} />
          </div></div><p className="rename-rule-purpose">{purposes[part.kind]}</p><div className="rename-rule-fields">
            {(part.kind === "cell" || part.kind === "lookup") && <div className="rename-field"><span>{part.kind === "lookup" ? "编码取值位置" : "取值位置"}</span><CellLocation source={part.source} /></div>}
            {part.kind === "cell" && <label className="rename-inline">数字格式<select aria-label={`第 ${index+1} 项数字格式`} value={part.decimals === null ? "raw" : part.decimals} onChange={e => setPart(index,{ ...part,decimals:e.target.value === "raw" ? null : Number(e.target.value) })}><option value="raw">保留原值</option><option value="0">整数</option><option value="1">1 位小数</option><option value="2">2 位小数</option><option value="4">4 位小数</option></select></label>}
            {(part.kind === "text" || part.kind === "condition") && <label className="rename-field">{part.kind === "condition" ? "加入的标签" : "加入的文字"}<input aria-label={part.kind === "condition" ? "条件标签文字" : "固定文字"} value={part.text} onChange={e => setPart(index,{ ...part,text:e.target.value })} /></label>}
            {part.kind === "condition" && <ConditionFields value={part.condition} onPick={() => onPick({kind:"tag",part:index})} onChange={condition => setPart(index,{ ...part,condition })} />}
            {part.kind === "lookup" && <LookupRows key={`${index}:${JSON.stringify(part.source)}`} part={part} onChange={next => setPart(index,next)} />}
          </div><div className={`rename-rule-result ${omitted ? "is-omitted" : ""}`}><span>本文件将加入</span><strong>{preview}</strong></div></li>;
        })}</ol>
        {!rules.parts.length && <div className="rename-empty-guide">先在原表中选择内容，再点击“加入文件名”。也可以添加一段固定文字。</div>}
        <Button className="rename-add-action" icon={Plus} onClick={() => onChange({ ...rules,parts:[...rules.parts,{kind:"text",text:""}] })}>添加固定文字</Button>
      </section>
      <section className="rename-builder-section rename-checks" aria-labelledby="rename-checks-heading"><div className="rename-section-heading"><h3 id="rename-checks-heading">内容校验</h3><span>可选</span></div><p className="rename-section-purpose">核对填写值和明细合计；不一致时，在文件名前加入异常标记。</p>
        {rules.checks.map((check,i) => {
          const raw=live?.checks[i], prefix=`${check.label}：`;
          const result=raw?.startsWith(prefix) ? raw.slice(prefix.length) : raw;
          return <div className="rename-check rename-rule-card" key={i}><div className="rename-rule-header"><h4>校验 {i+1}</h4><Button className="quiet icon-button" icon={X} title="移除此校验" aria-label={`移除校验 ${i+1}`} onClick={() => onChange({ ...rules,checks:rules.checks.filter((_,n) => n !== i) })} /></div><p className="rename-rule-purpose">比较原表填写的总额或总量，与下方明细的合计。</p><div className="rename-rule-fields">
            <label className="rename-field">不一致时添加的标记<input value={check.label} onChange={e => setCheck(i,{ ...check,label:e.target.value })} /></label>
            <div className="rename-field rename-bound-source"><span>填写值</span><SourcePicker sheet={check.expected.sheet} address={coordinate(check.expected.row,check.expected.column)} onPick={() => onPick({kind:"expected",check:i})} purpose="重新选择填写值" /></div>
            <label className="rename-inline">允许误差（%）<input type="number" min="0" max="100" step="0.1" value={check.tolerancePercent} onChange={e => setCheck(i,{ ...check,tolerancePercent:Number(e.target.value) })} /></label>
            <div className="rename-calculation-heading"><h5>明细合计</h5><span>{check.sums.length} 个区域相加</span></div>
            {check.sums.map((sum,j) => <div className="rename-sum" key={j}><div className="rename-sum-heading"><strong>合计区域 {j+1}</strong><Button className="quiet icon-button" icon={X} title="移除此区域" aria-label={`移除校验 ${i+1} 的区域 ${j+1}`} onClick={() => setCheck(i,{ ...check,sums:check.sums.filter((_,n) => n !== j) })} /></div>
              <SourcePicker sheet={sum.area.sheet} address={sum.area.range.endRow === 1048576 ? `${coordinate(sum.area.range.startRow,sum.area.range.startColumn)} 起至表尾` : rangeLabel(sum.area.range)} onPick={() => onPick({kind:"sum",check:i,sum:j})} purpose="重新选择合计区域" />
              <label className="rename-inline">换算除数<input aria-label={`校验 ${i+1} 区域 ${j+1} 的换算除数`} type="number" min="0.000001" step="any" value={sum.divisor} onChange={e => setSum(i,j,{ ...sum,divisor:Number(e.target.value) })} /></label><small className="rename-field-help">不换算填 1；件转千件填 1000。</small>
              <details className="rename-advanced"><summary>行筛选与计入条件</summary><div className="rename-advanced-body"><label className="rename-inline">整行至少有几格内容<input type="number" min="0" max="2048" value={sum.minFilled} onChange={e => setSum(i,j,{ ...sum,minFilled:Number(e.target.value) })} /></label><small className="rename-field-help">用于忽略内容较少的说明行，0 表示不过滤。</small>
                {sum.condition ? <><ConditionFields value={sum.condition} onPick={() => onPick({kind:"sumCondition",check:i,sum:j})} onChange={condition => setSum(i,j,{ ...sum,condition })} /><Button className="quiet" onClick={() => setSum(i,j,{ ...sum,condition:null })}>移除计入条件</Button></> : <Button onClick={() => onPick({kind:"sumCondition",check:i,sum:j})}>点选条件单元格</Button>}
              </div></details>
            </div>)}
            <Button className="rename-add-action" icon={Plus} onClick={() => onPick({kind:"sum",check:i})}>点选合计区域</Button>{!check.sums.length && <small className="rename-invalid">还需要选择合计区域</small>}
          </div><div className={`rename-check-result ${result?.startsWith("不一致") ? "is-warning" : result ? "is-passed" : ""}`}><span>本文件校验结果</span><p title="显示保留两位小数，校验使用原始精度。">{livePending ? "正在核对…" : result || "选择填写值和合计区域后显示结果"}</p></div></div>;
        })}
        {!rules.checks.length && <div className="rename-empty-guide"><p>需要检查金额或数量时，先添加校验，再到原表选择填写值和明细。</p><Button icon={Plus} onClick={() => onPick({kind:"expected"})}>添加校验规则</Button></div>}
      </section>
    </fieldset></div>
  </aside>;
}
