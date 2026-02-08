import { themes } from "../../themes/themes";

interface Props {
  current: string;
  onChange: (name: string) => void;
}

export function ThemePicker({ current, onChange }: Props) {
  return (
    <div className="theme-grid">
      {themes.map((theme) => (
        <button
          key={theme.name}
          className={`theme-card ${current === theme.name ? "active" : ""}`}
          onClick={() => onChange(theme.name)}
          style={{
            background: theme.vars["--bg-primary"],
            borderColor:
              current === theme.name
                ? theme.vars["--accent"]
                : theme.vars["--border"],
          }}
        >
          <div
            className="theme-preview-accent"
            style={{ background: theme.vars["--accent"] }}
          />
          <div
            className="theme-preview-text"
            style={{ color: theme.vars["--text-primary"] }}
          >
            {theme.label}
          </div>
        </button>
      ))}
    </div>
  );
}
