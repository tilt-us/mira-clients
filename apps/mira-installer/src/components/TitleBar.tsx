import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, X } from "lucide-react";
import { useEffect, type ReactNode } from "react";

type TitleBarProps = {
  children?: ReactNode;
  t: (id: string) => string;
};

function runWindowCommand(command: () => Promise<void>) {
  if (!isTauri()) {
    return;
  }

  void command();
}

function TitleBar({ children, t }: TitleBarProps) {
  useEffect(() => {
    runWindowCommand(() => getCurrentWindow().setDecorations(false));
  }, []);

  function handleDrag() {
    runWindowCommand(() => getCurrentWindow().startDragging());
  }

  function handleMinimize() {
    runWindowCommand(() => getCurrentWindow().minimize());
  }

  function handleClose() {
    runWindowCommand(() => getCurrentWindow().close());
  }

  return (
    <header className="titlebar">
      <div className="titlebar-drag-strip titlebar-drag-left" onMouseDown={handleDrag} />
      <div className="titlebar-drag-strip titlebar-drag-right" onMouseDown={handleDrag} />

      <div className="titlebar-left" onMouseDown={(event) => event.stopPropagation()}>
        {children}
      </div>

      <div className="titlebar-controls" onMouseDown={(event) => event.stopPropagation()}>
        <button
          aria-label={t("window-minimize")}
          title={t("window-minimize")}
          type="button"
          onClick={handleMinimize}
        >
          <Minus size={15} />
        </button>
        <button
          aria-label={t("window-close")}
          className="titlebar-close"
          title={t("window-close")}
          type="button"
          onClick={handleClose}
        >
          <X size={16} />
        </button>
      </div>
    </header>
  );
}

export default TitleBar;
