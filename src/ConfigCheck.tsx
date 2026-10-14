import { useState } from "react";
import { Button, Grid, Modal } from "./components";
import { call, queryFor } from "./api";
import type { DataPage, Dataset } from "./types";

export default function ConfigCheck({ dataset, name, onClose }: { dataset: Dataset; name: string; onClose: () => void }) {
  const [error, setError] = useState("");
  const columns = dataset.columns.filter((c) => !dataset.rules.columns.some((r) => r.key === c.key && r.hidden));
  return <Modal title={`配置检查 · ${name}`} wide onClose={onClose}>
    <div className="modal-body">
      <p>检查每张所选来源表的前 200 行，看看表头、输出列和补充信息是否正确。此检查不生成正式汇总结果。</p>
      <div className="config-check-summary"><span>{dataset.summary.outputRows.toLocaleString()} 条检查数据</span><span>{columns.length} 列</span>
        {dataset.summary.issueCount > 0 && <span className="field-error">{dataset.summary.issueCount} 处信息缺失或格式异常，正式汇总后可查看明细。</span>}
      </div>
      {error && <p className="field-error" role="alert">{error}</p>}
      <div className="config-check-grid"><Grid columns={columns} version={dataset.id} totalRows={dataset.summary.outputRows} load={(offset, limit, sorts) =>
        call<DataPage>("query_dataset", { query: { ...queryFor(dataset.id), offset, limit, sorts } }).catch((error) => { setError(error.message ?? String(error)); throw error; })} /></div>
    </div>
    <div className="modal-footer"><Button primary onClick={onClose}>返回配置</Button></div>
  </Modal>;
}
