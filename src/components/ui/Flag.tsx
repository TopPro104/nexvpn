import { useState } from "react";

// Circle flags CDN — circular SVG flags, cached by browser
const FLAG_CDN = "https://hatscripts.github.io/circle-flags/flags";

export function Flag({ code, size = 20 }: { code: string | null; size?: number }) {
  const [error, setError] = useState(false);

  if (!code || error) {
    return (
      <span
        className="flag-fallback"
        style={{ width: size, height: size, fontSize: size * 0.45 }}
      >
        {code ? code.toUpperCase() : "?"}
      </span>
    );
  }

  return (
    <img
      src={`${FLAG_CDN}/${code.toLowerCase()}.svg`}
      alt={code.toUpperCase()}
      width={size}
      height={size}
      className="flag-icon"
      onError={() => setError(true)}
      loading="lazy"
      draggable={false}
    />
  );
}
