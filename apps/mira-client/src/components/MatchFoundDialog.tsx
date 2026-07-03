import type { CSSProperties } from "react";
import type { Translate } from "../types/ui";

type MatchFoundDialogProps = {
  acceptedCount: number;
  busy: boolean;
  countdownClassName: string;
  countdownStyle: CSSProperties;
  currentPlayerAccepted: boolean;
  overlayStroke: string;
  remainingSeconds: number;
  requiredAcceptCount: number;
  t: Translate;
  onAccept: () => void;
  onDecline: () => void;
};

function MatchFoundDialog({
  acceptedCount,
  busy,
  countdownClassName,
  countdownStyle,
  currentPlayerAccepted,
  overlayStroke,
  remainingSeconds,
  requiredAcceptCount,
  t,
  onAccept,
  onDecline,
}: MatchFoundDialogProps) {
  return (
    <div className="match-found-backdrop" role="presentation">
      <section
        aria-labelledby="match-found-title"
        aria-modal="true"
        className="match-found-modal"
        role="dialog"
      >
        <div className={countdownClassName} style={countdownStyle}>
          <svg
            aria-hidden="true"
            className="match-found-border"
            focusable="false"
            viewBox="0 0 100 100"
          >
            <g className="match-found-border-spinner match-found-border-spinner-base">
              <path
                className="match-found-border-ring match-found-border-ring-base"
                d="M 50 2 L 8.43 26 L 8.43 74 L 50 98 L 91.57 74 L 91.57 26 Z"
              />
            </g>
            <g className="match-found-border-spinner match-found-border-spinner-overlay">
              <path
                className="match-found-border-ring match-found-border-ring-overlay"
                d="M 50 5 L 11.03 27.5 L 11.03 72.5 L 50 95 L 88.97 72.5 L 88.97 27.5 Z"
                style={{ stroke: overlayStroke }}
              />
            </g>
          </svg>
          <div aria-hidden="true" className="match-found-timer-ring" />
          <div className="match-found-countdown-core">
            <h2 id="match-found-title">{t("match-found-title")}</h2>
            <p>{t("match-found-mode-ranked")}</p>
            <span>{remainingSeconds}</span>
            <small>
              {acceptedCount} / {requiredAcceptCount}
            </small>
          </div>
        </div>
        <div className="match-found-actions">
          <button
            className="match-found-accept"
            disabled={busy || currentPlayerAccepted}
            type="button"
            onClick={onAccept}
          >
            {currentPlayerAccepted
              ? t("match-found-waiting")
              : t("match-found-accept")}
          </button>
          <button
            className="match-found-decline"
            disabled={busy || currentPlayerAccepted}
            type="button"
            onClick={onDecline}
          >
            {t("match-found-decline")}
          </button>
        </div>
      </section>
    </div>
  );
}

export default MatchFoundDialog;
