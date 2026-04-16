let _resolve: ((v: boolean) => void) | null = null;
let _message = "";
let _listener: (() => void) | null = null;

/** Trigger a confirm dialog. Returns a promise that resolves when user clicks Yes/No. */
export function showConfirm(message: string): Promise<boolean> {
  return new Promise((resolve) => {
    _message = message;
    _resolve = resolve;
    _listener?.();
  });
}

/** Get current confirm state (used by ConfirmDialog component) */
export function getConfirmState() {
  return { message: _message, pending: _resolve !== null };
}

/** Resolve the pending confirm */
export function resolveConfirm(value: boolean) {
  _resolve?.(value);
  _resolve = null;
  _message = "";
  _listener?.();
}

/** Subscribe to confirm state changes */
export function onConfirmChange(fn: () => void) {
  _listener = fn;
}
