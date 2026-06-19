import { LogOut, Power } from "lucide-react";
import type { Translate } from "../types/ui";

type CloseDialogProps = {
  onClose: () => void;
  onLogout: () => void;
  onQuit: () => void;
  t: Translate;
};

function CloseDialog({ onClose, onLogout, onQuit, t }: CloseDialogProps) {
  return (
    <div
      className="dialog-backdrop close-backdrop"
      role="presentation"
      onMouseDown={onClose}
    >
      <div
        aria-labelledby="close-dialog-title"
        aria-modal="true"
        className="close-dialog"
        role="dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <h2 id="close-dialog-title">{t("close-title")}</h2>
        <div className="close-dialog-actions">
          <button className="secondary-button" type="button" onClick={onLogout}>
            <LogOut size={18} />
            {t("close-logout")}
          </button>
          <button className="quit-button" type="button" onClick={onQuit}>
            <Power size={18} />
            {t("close-quit")}
          </button>
        </div>
      </div>
    </div>
  );
}

export default CloseDialog;
