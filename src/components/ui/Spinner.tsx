export function Spinner({ size = 20 }: { size?: number }) {
  return (
    <div
      className="spinner"
      style={{ width: size, height: size }}
    />
  );
}
