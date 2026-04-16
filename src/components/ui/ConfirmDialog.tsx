import { useState, useEffect } from "react";
import { Modal } from "./Modal";
import { Button } from "./Button";
import { getConfirmState, resolveConfirm, onConfirmChange } from "../../utils/confirm";
import { t } from "../../i18n/translations";

export function ConfirmDialog() {
  const [state, setState] = useState(getConfirmState());

  useEffect(() => {
    onConfirmChange(() => setState({ ...getConfirmState() }));
  }, []);

  if (!state.pending) return null;

  return (
    <Modal open={true} onClose={() => resolveConfirm(false)} title={t("common.confirm")}>
      <p style={{ whiteSpace: "pre-line", marginBottom: 16 }}>{state.message}</p>
      <div className="form-actions">
        <Button variant="danger" onClick={() => resolveConfirm(true)}>
          {t("common.yes")}
        </Button>
        <Button variant="ghost" onClick={() => resolveConfirm(false)}>
          {t("common.cancel")}
        </Button>
      </div>
    </Modal>
  );
}
