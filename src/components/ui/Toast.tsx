import { useEffect } from "react";
import { useApp, Toast as ToastType } from "../../context/AppContext";

function ToastItem({ toast }: { toast: ToastType }) {
  const { dispatch } = useApp();

  useEffect(() => {
    const timer = setTimeout(() => {
      dispatch({ type: "REMOVE_TOAST", id: toast.id });
    }, 4000);
    return () => clearTimeout(timer);
  }, [toast.id, dispatch]);

  return (
    <div className={`toast toast-${toast.type}`}>
      <span>{toast.message}</span>
      <button
        className="toast-close"
        onClick={() => dispatch({ type: "REMOVE_TOAST", id: toast.id })}
      >
        &times;
      </button>
    </div>
  );
}

export function ToastStack() {
  const { state } = useApp();

  if (state.toasts.length === 0) return null;

  return (
    <div className="toast-stack">
      {state.toasts.map((t) => (
        <ToastItem key={t.id} toast={t} />
      ))}
    </div>
  );
}
