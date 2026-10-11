import { useState } from "react";
import {
  FileSpreadsheet,
  Table2,
  Plus,
  Play,
  Sparkles,
  Command,
  Lightbulb,
  CheckCircle2,
  FolderOpen,
} from "lucide-react";
import { Modal, Button } from "./components";

interface HelpModalProps {
  onClose: () => void;
  onStartTour: () => void;
  onLoadSample: () => void;
  hasFiles: boolean;
  busy: boolean;
}

export default function HelpModal({
  onClose,
  onStartTour,
  onLoadSample,
  hasFiles,
  busy,
}: HelpModalProps) {
  const [tab, setTab] = useState<"workflow" | "tips" | "shortcuts">("workflow");
  const steps = [
    {
      icon: FileSpreadsheet,
      title: "批量添加文件",
      body: "点击「添加文件」或直接将 XLSX / XLS / CSV 拖入窗口。支持一次性添加几十份文件，多工作表自动识别。",
    },
    {
      icon: Table2,
      title: "框选设定表头",
      body: "在表格预览中按住鼠标划选列名行（支持单行与多行合并表头），点击顶部「设为表头」，同名列自动智能归集。",
    },
    {
      icon: Plus,
      title: "孤立信息提取为新列",
      body: "单选表格顶部的“填报门店”或“日期”单据单元格，点击「添加为新列」，即可自动平铺为母表整列维度。",
    },
    {
      icon: Play,
      title: "预览结果与保存导出",
      body: "点击右上角「预览结果」（Ctrl+Enter），毫秒级合并全量数据。可在结果区直接筛选、分类汇总与导出。",
    },
  ];

  return (
    <Modal title="帮助与新手指引" onClose={onClose} wide>
      <div className="modal-body">
        <div className="help-modal-container">
        <div className="help-tabs">
          <button
            type="button"
            className={tab === "workflow" ? "active" : ""}
            onClick={() => setTab("workflow")}
          >
            <CheckCircle2 size={15} />
            4 步极速上手
          </button>
          <button
            type="button"
            className={tab === "tips" ? "active" : ""}
            onClick={() => setTab("tips")}
          >
            <Lightbulb size={15} />
            进阶与实用技巧
          </button>
          <button
            type="button"
            className={tab === "shortcuts" ? "active" : ""}
            onClick={() => setTab("shortcuts")}
          >
            <Command size={15} />
            快捷键速查
          </button>
        </div>

        <div className="help-content">
          {tab === "workflow" && (
            <div className="help-workflow">
              <div className="workflow-intro">
                <p>
                  汇格（GridFlow）专为多表格合并汇总设计：无需公式或脚本，通过 4 步可视化操作快速对齐异构报表。全部计算与存储均在本地离线完成，源文件绝不被篡改。
                </p>
              </div>

              <div className="workflow-cards">
                {steps.map((step, index) => (
                  <div className="workflow-card" key={step.title}>
                    <div className="workflow-head">
                      <span className="workflow-step">{index + 1}</span>
                      <step.icon size={16} />
                      <h4>{step.title}</h4>
                    </div>
                    <p>{step.body}</p>
                  </div>
                ))}
              </div>
            </div>
          )}

          {tab === "tips" && (
            <div className="help-tips">
              <div className="tip-item">
                <strong>💡 多层复合表头怎么并？</strong>
                <p>
                  划选包含多行的表头选区，点击「设为表头」。系统会自动分析合并单元格层级，将“大类 - 子类”拼接为清晰的标准字段名。
                </p>
              </div>

              <div className="tip-item">
                <strong>💡 工号、订单号前导零保留（如 0089）</strong>
                <p>
                  在右侧「配置 ➔ 输出列」面板中，将对应字段类型设为「文字 / 编号」，合并时将绝对保留字符原貌，不被 Excel 自动转为数字吞零。
                </p>
              </div>

              <div className="tip-item">
                <strong>💡 异常格式自动容错与坐标追溯</strong>
                <p>
                  若数据列混入非标准格式（如金额列填了“暂无”），合并引擎会保留原文本并标记。在「问题记录」中能精确定位至对应文件的行列坐标。
                </p>
              </div>

              <div className="tip-item">
                <strong>💡 常用规则保存与重复使用</strong>
                <p>
                  面对每月定期的常规报表，完成一次配置后点击顶部「重复使用 ➔ 保存为方案」。下个月只需导入新表一键套用，无需重复圈选！
                </p>
              </div>
            </div>
          )}

          {tab === "shortcuts" && (
            <div className="help-shortcuts">
              <table className="shortcuts-table">
                <thead>
                  <tr>
                    <th>快捷键</th>
                    <th>操作功能</th>
                  </tr>
                </thead>
                <tbody>
                  <tr>
                    <td><kbd>Ctrl</kbd> + <kbd>Enter</kbd></td>
                    <td>执行合并规则并切换至「预览结果」</td>
                  </tr>
                  <tr>
                    <td><kbd>Ctrl</kbd> + <kbd>O</kbd></td>
                    <td>打开添加文件对话框</td>
                  </tr>
                  <tr>
                    <td><kbd>Ctrl</kbd> + <kbd>Z</kbd> / <kbd>Ctrl</kbd> + <kbd>Y</kbd></td>
                    <td>撤销 / 重做配置操作</td>
                  </tr>
                  <tr>
                    <td><kbd>Esc</kbd></td>
                    <td>取消当前选区或关闭弹窗</td>
                  </tr>
                  <tr>
                    <td><kbd>方向键</kbd> / <kbd>Shift</kbd> + <kbd>方向键</kbd></td>
                    <td>单元格移动与矩形连续选区扩选</td>
                  </tr>
                  <tr>
                    <td><kbd>Shift</kbd> + <kbd>空格</kbd></td>
                    <td>快速选中当前整行</td>
                  </tr>
                  <tr>
                    <td><kbd>Ctrl</kbd> + <kbd>Home</kbd> / <kbd>End</kbd></td>
                    <td>快速跳转至表格左上角首格 / 右下角尾格</td>
                  </tr>
                </tbody>
              </table>
            </div>
          )}
        </div>

        <div className="help-modal-footer">
          <Button className="quiet" onClick={onClose}>
            关闭
          </Button>
          <div className="help-footer-actions">
            <Button
              icon={Sparkles}
              disabled={busy}
              onClick={() => {
                onClose();
                onLoadSample();
              }}
            >
              体验示例数据（2份连锁门店月报）
            </Button>
            {hasFiles && (
              <Button
                primary
                icon={Play}
                disabled={busy}
                onClick={() => {
                  onClose();
                  onStartTour();
                }}
              >
                播放 4 步实操引导
              </Button>
            )}
          </div>
        </div>
      </div>
      </div>
    </Modal>
  );
}
