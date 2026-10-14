import { useEffect, useLayoutEffect, useState, useRef } from "react";
import { X, ArrowRight, ArrowLeft, Check, Sparkles } from "lucide-react";
import { Button } from "./components";

export interface TourStep {
  selector: string;
  title: string;
  description: string;
  placement: "bottom" | "right" | "bottom-left" | "center";
}

export const TOUR_STEPS: TourStep[] = [
  {
    selector: '[data-tour="file-sidebar"]',
    title: "按文件配置工作表",
    description: "展开文件，勾选需要汇总的 Sheet，直接填写表头行和来源名称。只需设置首个参与汇总的文件，其余文件的同名表会自动跟随，无需复制。文件开关会保留 Sheet 选择。",
    placement: "right",
  },
  {
    selector: '[data-tour="set-header-btn"]',
    title: "确认列名所在行",
    description: "单行表头可直接在左侧填写行号，也可采用识别建议。多行表头在原表中框选后点“设为表头”，再应用到其他同名表。",
    placement: "bottom",
  },
  {
    selector: '[data-tour="add-column-btn"]',
    title: "固定信息补充为新列",
    description: "在“新增列”直接填写 Sheet、单元格地址和列名，也可在原表点选一个单元格。每份文件各取自己的值，默认补到所有汇总表中。",
    placement: "bottom",
  },
  {
    selector: '[data-tour="preview-btn"]',
    title: "生成并保存全部汇总表",
    description: "配置完成后，点击右上角「生成汇总结果」（或 Ctrl+Enter）。处理完成后可查看完整结果、筛选数据，并保存到电脑。",
    placement: "bottom-left",
  },
];

interface OnboardingTourProps {
  active: boolean;
  step: number;
  onStepChange: (step: number) => void;
  onClose: () => void;
  onComplete: () => void;
}

export default function OnboardingTour({
  active,
  step,
  onStepChange,
  onClose,
  onComplete,
}: OnboardingTourProps) {
  const [rect, setRect] = useState<DOMRect | null>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const currentStep = TOUR_STEPS[step] ?? TOUR_STEPS[0];
  const isLast = step === TOUR_STEPS.length - 1;

  const updatePosition = () => {
    if (!active) return;
    const el = document.querySelector(currentStep.selector);
    if (el) {
      setRect(el.getBoundingClientRect());
    } else {
      setRect(null);
    }
  };

  useLayoutEffect(() => {
    updatePosition();
    const timer = setTimeout(updatePosition, 100);
    return () => clearTimeout(timer);
  }, [active, step, currentStep.selector]);

  useEffect(() => {
    if (!active) return;
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [active, currentStep.selector]);

  useEffect(() => {
    if (!active) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowRight" || e.key === "Enter") {
        e.preventDefault();
        if (isLast) onComplete();
        else onStepChange(step + 1);
      } else if (e.key === "ArrowLeft" && step > 0) {
        e.preventDefault();
        onStepChange(step - 1);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [active, step, isLast, onStepChange, onClose, onComplete]);

  if (!active) return null;

  // 计算聚光灯样式
  const spotlightStyle: React.CSSProperties = rect
    ? {
        top: Math.max(0, rect.top - 4),
        left: Math.max(0, rect.left - 4),
        width: rect.width + 8,
        height: rect.height + 8,
      }
    : {
        top: "50%",
        left: "50%",
        width: 0,
        height: 0,
      };

  // 计算弹出气泡位置
  const popoverStyle: React.CSSProperties = {
    position: "fixed",
    zIndex: 9999,
  };

  const popoverWidth = 340;
  if (rect) {
    let top = 0;
    let left = 0;

    if (currentStep.placement === "right") {
      left = rect.right + 14;
      top = rect.top + rect.height / 2 - 80;
    } else if (currentStep.placement === "bottom-left") {
      left = rect.right - popoverWidth;
      top = rect.bottom + 14;
    } else {
      // 默认 bottom
      left = rect.left + rect.width / 2 - popoverWidth / 2;
      top = rect.bottom + 14;
    }

    // 边界防护，防止超出屏幕
    left = Math.max(16, Math.min(window.innerWidth - popoverWidth - 16, left));
    top = Math.max(16, Math.min(window.innerHeight - 260, top));

    popoverStyle.top = `${top}px`;
    popoverStyle.left = `${left}px`;
  } else {
    popoverStyle.top = "50%";
    popoverStyle.left = "50%";
    popoverStyle.transform = "translate(-50%, -50%)";
  }

  return (
    <div className="tour-overlay" onClick={onClose} role="dialog" aria-modal="true" aria-label="新手操作指引">
      {/* 聚光灯镂空框 */}
      <div
        className="tour-spotlight-box"
        style={spotlightStyle}
        onClick={(e) => e.stopPropagation()}
      />

      {/* 气泡卡片 */}
      <div
        ref={popoverRef}
        className="tour-popover"
        style={popoverStyle}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="tour-popover-header">
          <div className="tour-step-badge">
            <Sparkles size={13} className="tour-badge-icon" />
            <span>核心指引 {step + 1} / {TOUR_STEPS.length}</span>
          </div>
          <button
            type="button"
            className="tour-close-btn"
            aria-label="关闭指引"
            onClick={onClose}
          >
            <X size={15} />
          </button>
        </div>

        <div className="tour-popover-body">
          <h3 className="tour-title">{currentStep.title}</h3>
          <p className="tour-desc">{currentStep.description}</p>
        </div>

        <div className="tour-popover-footer">
          <button type="button" className="tour-skip-btn" onClick={onClose}>
            跳过
          </button>

          <div className="tour-dots">
            {TOUR_STEPS.map((_, i) => (
              <span
                key={i}
                className={`tour-dot ${i === step ? "active" : ""}`}
                onClick={() => onStepChange(i)}
                title={`跳转到第 ${i + 1} 步`}
              />
            ))}
          </div>

          <div className="tour-buttons">
            {step > 0 && (
              <Button
                className="quiet"
                icon={ArrowLeft}
                onClick={() => onStepChange(step - 1)}
              >
                上一步
              </Button>
            )}
            <Button
              primary
              icon={isLast ? Check : ArrowRight}
              onClick={() => (isLast ? onComplete() : onStepChange(step + 1))}
            >
              {isLast ? "完成指引" : "下一步"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
