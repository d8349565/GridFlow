import { useId, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";

export default function WorkflowBar({ label, steps, current, hint, actions }: {
  label: string;
  steps: { label: string; onClick?: () => void; disabled?: boolean }[];
  current: number;
  hint: string;
  actions?: ReactNode;
}) {
  const hintId = useId();
  return <section className="workflow-bar" aria-label={label}>
    <div className="workflow-context">
      <nav aria-label={label}>
        <ol className="workflow-steps">{steps.map((step, index) => <li key={step.label}>
          {index > 0 && <ChevronRight size={14} aria-hidden="true" />}
          {step.onClick ? <button type="button" className={index === current ? "current" : ""}
            aria-current={index === current ? "step" : undefined} disabled={step.disabled} onClick={step.onClick}>
            <span className="workflow-number" aria-hidden="true">{index + 1}</span>{step.label}
          </button> : <span className={`workflow-step-label ${index === current ? "current" : ""}`} aria-current={index === current ? "step" : undefined}>
            <span className="workflow-number" aria-hidden="true">{index + 1}</span>{step.label}
          </span>}
        </li>)}</ol>
      </nav>
      <p id={hintId} className="workflow-hint" role="status">{hint}</p>
    </div>
    {actions && <div className="workflow-actions" role="group" aria-label="当前步骤操作" aria-describedby={hintId}>{actions}</div>}
  </section>;
}
