import { useState } from "react";
import { Keyboard } from "lucide-react";
import { cn } from "../../lib/utils";
import { formatAccelerator } from "../../lib/hotkeys";

interface HotkeyInputProps {
  id: string;
  value: string;
  invalid?: boolean;
  onChange: (accelerator: string) => void;
}

/** 快捷键录制输入框：聚焦后按下组合键即可录入 */
export default function HotkeyInput({ id, value, invalid, onChange }: HotkeyInputProps) {
  const [recording, setRecording] = useState(false);

  return (
    <button
      id={id}
      type="button"
      onClick={() => setRecording(true)}
      onBlur={() => setRecording(false)}
      onKeyDown={(event) => {
        if (!recording) return;
        event.preventDefault();
        event.stopPropagation();

        if (event.key === "Escape") {
          setRecording(false);
          return;
        }
        if (event.key === "Backspace") {
          onChange("");
          setRecording(false);
          return;
        }

        const accelerator = formatAccelerator(event.nativeEvent);
        if (accelerator) {
          onChange(accelerator);
          setRecording(false);
        }
      }}
      className={cn(
        "flex min-w-[150px] cursor-pointer items-center justify-center gap-1.5 rounded-lg border px-3 py-2 font-mono text-[12px] transition-colors",
        invalid
          ? "border-danger/50 bg-danger/10 text-danger"
          : recording
            ? "border-primary/60 bg-primary/15 text-primary-3"
            : "border-white/10 bg-background-3/60 text-text hover:border-primary/40"
      )}
    >
      <Keyboard className="h-3.5 w-3.5 shrink-0 opacity-70" />
      {recording ? "请按下组合键…" : value || "未设置"}
    </button>
  );
}
