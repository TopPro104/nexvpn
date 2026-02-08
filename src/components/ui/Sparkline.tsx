export function Sparkline({
  data,
  width = 120,
  height = 32,
  color = "var(--accent)",
}: {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
}) {
  if (data.length < 2) {
    return <svg width={width} height={height} className="sparkline" />;
  }

  const max = Math.max(...data, 1);
  const pad = 1;
  const h = height - pad * 2;
  const w = width - pad * 2;

  const points = data.map((v, i) => {
    const x = pad + (i / (data.length - 1)) * w;
    const y = pad + h - (v / max) * h;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });

  // Fill area under the line
  const fillPoints = [
    `${pad},${pad + h}`,
    ...points,
    `${pad + w},${pad + h}`,
  ].join(" ");

  return (
    <svg width={width} height={height} className="sparkline">
      <polygon points={fillPoints} fill={color} opacity={0.12} />
      <polyline
        points={points.join(" ")}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
