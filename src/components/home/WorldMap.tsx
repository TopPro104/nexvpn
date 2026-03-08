import { useState, useRef, useCallback } from "react";
import { useApp } from "../../context/AppContext";
import { countryCoords, countryPaths } from "../../utils/geoData";
import { extractCountryCode } from "../../utils/countryUtils";

const MIN_ZOOM = 1;
const MAX_ZOOM = 5;

export function WorldMap() {
  const { state, dispatch } = useApp();
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const dragStart = useRef({ x: 0, y: 0, panX: 0, panY: 0 });

  const serversByCountry: Record<string, { code: string; servers: typeof state.servers }> = {};
  for (const server of state.servers) {
    const code = extractCountryCode(server.name) || "??";
    if (!serversByCountry[code]) serversByCountry[code] = { code, servers: [] };
    serversByCountry[code].servers.push(server);
  }

  const activeServer = state.servers.find(s => s.id === state.selectedServerId);
  const activeCountry = activeServer ? (extractCountryCode(activeServer.name) || "??") : null;
  const activeCode = activeCountry?.toUpperCase();
  const userPos = state.userLocation || { x: 400, y: 150 };

  const getPingColor = (servers: typeof state.servers) => {
    const pings = servers.map(s => s.latency_ms).filter((p): p is number => p !== null);
    if (pings.length === 0) return "var(--text-muted)";
    const avg = pings.reduce((a, b) => a + b, 0) / pings.length;
    if (avg < 150) return "var(--success)";
    if (avg < 300) return "var(--warning)";
    return "var(--danger)";
  };

  const handleDotClick = (code: string) => {
    const group = serversByCountry[code];
    if (!group) return;
    const idx = group.servers.findIndex(s => s.id === state.selectedServerId);
    dispatch({ type: "SELECT_SERVER", id: group.servers[(idx + 1) % group.servers.length].id });
  };

  const clampPan = (x: number, y: number, z: number) => {
    const maxPanX = (800 * (z - 1)) / (2 * z) * (800 / 800);
    const maxPanY = (400 * (z - 1)) / (2 * z) * (400 / 400);
    return {
      x: Math.max(-maxPanX, Math.min(maxPanX, x)),
      y: Math.max(-maxPanY, Math.min(maxPanY, y)),
    };
  };

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    setZoom(z => {
      const next = Math.max(MIN_ZOOM, Math.min(z + (e.deltaY < 0 ? 0.3 : -0.3), MAX_ZOOM));
      if (next <= 1) setPan({ x: 0, y: 0 });
      return next;
    });
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (zoom <= 1) return;
    setDragging(true);
    dragStart.current = { x: e.clientX, y: e.clientY, panX: pan.x, panY: pan.y };
  }, [zoom, pan]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!dragging) return;
    const dx = e.clientX - dragStart.current.x;
    const dy = e.clientY - dragStart.current.y;
    setPan(clampPan(dragStart.current.panX + dx, dragStart.current.panY + dy, zoom));
  }, [dragging, zoom]);

  const handleMouseUp = useCallback(() => setDragging(false), []);

  return (
    <div className="world-map-container">
      <div
        className={`world-map-viewport ${dragging ? "dragging" : ""}`}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
      >
        <svg
          viewBox="0 0 800 400"
          className="world-map-svg"
          style={{
            transform: `scale(${zoom}) translate(${pan.x / zoom}px, ${pan.y / zoom}px)`,
          }}
        >
          {/* Country shapes */}
          {countryPaths.map((d, i) => (
            <path
              key={i}
              d={d}
              fill="var(--bg-tertiary)"
              stroke="var(--border)"
              strokeWidth="0.4"
            />
          ))}

          {/* Connection line */}
          {state.connected && activeCode && countryCoords[activeCode] && (
            <line
              x1={userPos.x} y1={userPos.y}
              x2={countryCoords[activeCode].x}
              y2={countryCoords[activeCode].y}
              stroke="var(--accent)"
              strokeWidth="1"
              strokeDasharray="5 3"
              opacity="0.5"
              className="connection-line"
            />
          )}

          {/* Server markers */}
          {Object.entries(serversByCountry).map(([code, group]) => {
            const coords = countryCoords[code.toUpperCase()];
            if (!coords) return null;
            const isActive = code === activeCountry;
            const color = getPingColor(group.servers);
            return (
              <g key={code} onClick={() => handleDotClick(code)} style={{ cursor: "pointer" }}>
                {isActive && state.connected && (
                  <circle cx={coords.x} cy={coords.y} r="10" fill={color} opacity="0.15" className="server-dot-glow" />
                )}
                <circle
                  cx={coords.x} cy={coords.y}
                  r={isActive ? 4 : 2.5}
                  fill={color}
                  stroke={isActive ? "white" : "none"}
                  strokeWidth={isActive ? 1.2 : 0}
                />
                {group.servers.length > 1 && (
                  <text x={coords.x + 5} y={coords.y - 4} fill="var(--text-muted)" fontSize="6.5">{group.servers.length}</text>
                )}
                <title>{`${coords.name} (${group.servers.length})`}</title>
              </g>
            );
          })}

          {/* User dot — always visible */}
          {state.userLocation && (
            <g>
              <circle cx={userPos.x} cy={userPos.y} r="3" fill="var(--accent)" />
              <circle cx={userPos.x} cy={userPos.y} r="7" fill="var(--accent)" opacity="0.2" className="user-dot-pulse" />
              <text x={userPos.x} y={userPos.y - 9} textAnchor="middle" fill="var(--accent)" fontSize="7" opacity="0.8">You</text>
            </g>
          )}
        </svg>
      </div>

      <div className="map-controls">
        <button className="map-ctrl-btn" onClick={() => setZoom(z => Math.min(z + 0.5, MAX_ZOOM))}>+</button>
        <button className="map-ctrl-btn" onClick={() => { setZoom(z => { const n = Math.max(z - 0.5, 1); if (n <= 1) setPan({x:0,y:0}); return n; }); }}>&minus;</button>
        <button className="map-ctrl-btn" onClick={() => { setZoom(1); setPan({x:0,y:0}); }}>&#8634;</button>
      </div>
      {zoom > 1 && <div className="map-zoom-badge">{zoom.toFixed(1)}x</div>}
    </div>
  );
}
