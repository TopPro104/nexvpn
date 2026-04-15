import { useApp } from "../../context/AppContext";
import { MegaphoneIcon } from "../ui/Icons";

interface Props {
  activeTab: string; // "all" | "manual" | subscription_id
}

export function AnnounceBanner({ activeTab }: Props) {
  const { state } = useApp();

  const announces = state.subscriptions
    .filter((s) => {
      if (!s.announce) return false;
      if (activeTab === "all") return true;
      return s.id === activeTab;
    })
    .map((s) => ({ name: s.name, text: s.announce! }));

  if (announces.length === 0) return null;

  return (
    <>
      {announces.map((a, i) => (
        <div key={i} className="announce-banner">
          <MegaphoneIcon size={16} className="announce-banner-icon" />
          <div className="announce-banner-content">
            {a.text}
            <div className="announce-banner-source">{a.name}</div>
          </div>
        </div>
      ))}
    </>
  );
}
