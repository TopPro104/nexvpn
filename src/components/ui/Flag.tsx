import { useState } from "react";

// Locally bundled circle-flags — works offline, identical on every OS.
// Vite resolves these at build time; unused flags get tree-shaken from runtime
// imports but each SVG referenced becomes a static asset.
const FLAG_MODULES = import.meta.glob<string>(
  "../../../node_modules/circle-flags/flags/*.svg",
  { eager: true, query: "?url", import: "default" }
);

const FLAG_URLS: Record<string, string> = {};
for (const [path, url] of Object.entries(FLAG_MODULES)) {
  const m = path.match(/\/([^/]+)\.svg$/);
  if (m) FLAG_URLS[m[1].toLowerCase()] = url;
}

export function Flag({ code, size = 20 }: { code: string | null; size?: number }) {
  const [error, setError] = useState(false);
  const url = code ? FLAG_URLS[code.toLowerCase()] : undefined;

  if (!code || !url || error) {
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
      src={url}
      alt={code.toUpperCase()}
      width={size}
      height={size}
      className="flag-icon"
      onError={() => setError(true)}
      draggable={false}
    />
  );
}
