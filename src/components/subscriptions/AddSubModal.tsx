import { useState } from "react";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { useApp } from "../../context/AppContext";
import { api } from "../../api/tauri";
import { t } from "../../i18n/translations";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function AddSubModal({ open, onClose }: Props) {
  const { state, dispatch, toast } = useApp();
  void state.langTick;
  const [url, setUrl] = useState("");
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);

  const handleAdd = async () => {
    if (!url.trim()) return;
    setLoading(true);
    try {
      const servers = await api.addSubscription(url.trim(), name.trim() || undefined);
      dispatch({ type: "ADD_SERVERS", servers });
      const subs = await api.getSubscriptions();
      dispatch({ type: "SET_SUBSCRIPTIONS", subs });
      toast(`${t("toast.subAdded")}: ${servers.length} ${t("subs.servers")}`, "success");
      setUrl("");
      setName("");
      onClose();
    } catch (e) {
      toast(`${e}`, "error");
    }
    setLoading(false);
  };

  return (
    <Modal open={open} onClose={onClose} title={t("subs.addTitle")}>
      <div className="form-group">
        <label className="form-label">{t("subs.urlLabel")}</label>
        <input
          className="form-input"
          placeholder={t("subs.urlPlaceholder")}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
        />
      </div>
      <div className="form-group">
        <label className="form-label">{t("subs.nameLabel")}</label>
        <input
          className="form-input"
          placeholder={t("subs.namePlaceholder")}
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      <div className="form-actions">
        <Button onClick={handleAdd} disabled={loading}>
          {loading ? <Spinner size={14} /> : t("common.add")}
        </Button>
        <Button variant="ghost" onClick={onClose}>
          {t("common.cancel")}
        </Button>
      </div>
    </Modal>
  );
}
