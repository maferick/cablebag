// In-app replacements for window.prompt and window.confirm, drawn in Cablebag's own style.

export interface AskRequest {
  title: string;
  message?: string;
  /** Present for a question that needs typed input. */
  input?: { label: string; value: string };
  /** Present for a question with several named lines to fill in. */
  inputs?: { name: string; label: string; value: string }[];
  okLabel: string;
  danger: boolean;
  resolve: (value: string | boolean | null) => void;
}

class Ask {
  current = $state<AskRequest | null>(null);

  /** Asks for a line of text; resolves to null when cancelled. */
  text(title: string, label: string, value = "", okLabel = "OK"): Promise<string | null> {
    return new Promise((resolve) => {
      this.current = { title, input: { label, value }, okLabel, danger: false, resolve: (v) => resolve(v as string | null) };
    });
  }

  /** Asks for several lines of text at once; resolves to the answers by name, or null when cancelled. */
  fields(title: string, message: string, inputs: { name: string; label: string; value: string }[], okLabel = "OK"): Promise<Record<string, string> | null> {
    return new Promise((resolve) => {
      this.current = {
        title,
        message: message || undefined,
        inputs,
        okLabel,
        danger: false,
        resolve: (v) => resolve(v ? JSON.parse(v as string) : null),
      };
    });
  }

  /** Asks a yes/no question; resolves to true when confirmed. */
  confirm(title: string, message: string, opts: { okLabel?: string; danger?: boolean } = {}): Promise<boolean> {
    return new Promise((resolve) => {
      this.current = {
        title,
        message,
        okLabel: opts.okLabel ?? "OK",
        danger: !!opts.danger,
        resolve: (v) => resolve(v === true),
      };
    });
  }

  answer(value: string | boolean | null) {
    const c = this.current;
    this.current = null;
    c?.resolve(value);
  }
}

export const ask = new Ask();
