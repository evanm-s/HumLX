// The link from a remote browser to the Studio PC that served it. Commands go
// over one WebSocket; the Studio PC answers each and also says when another
// screen changed part of the show.

const KEY = "humlx-remote-key";
const NAME = "humlx-remote-name";
/** Close code the Studio PC sends when this browser isn't paired (any more). */
const UNPAIRED = 4001;

type State = "connecting" | "open" | "unpaired" | "lost";

interface Pending {
  resolve: (value: unknown) => void;
  reject: (error: unknown) => void;
}

function stored(name: string): string {
  try {
    return localStorage.getItem(name) ?? "";
  } catch {
    return "";
  }
}

function store(name: string, value: string | null) {
  try {
    if (value == null) localStorage.removeItem(name);
    else localStorage.setItem(name, value);
  } catch {
    // Pairing then lasts until the page reloads.
  }
}

class RemoteLink {
  state = $state<State>("connecting");
  /** Times the link has opened; above 1 means it came back after a drop. */
  opened = $state(0);
  /** Called with the parts of the show another screen changed. */
  onchange: (parts: string[]) => void = () => {};

  private key = stored(KEY);
  private socket: WebSocket | null = null;
  private pending = new Map<number, Pending>();
  private nextId = 1;
  private retryMs = 500;
  private started = false;

  get savedName() {
    return stored(NAME);
  }

  start() {
    if (this.started) return;
    this.started = true;
    if (!this.key) this.state = "unpaired";
    else this.connect();
  }

  private connect() {
    this.state = "connecting";
    const scheme = location.protocol === "https:" ? "wss" : "ws";
    const socket = new WebSocket(`${scheme}://${location.host}/ws?key=${encodeURIComponent(this.key)}`);
    this.socket = socket;
    socket.onopen = () => {
      this.retryMs = 500;
      this.state = "open";
      this.opened++;
    };
    socket.onmessage = (event) => {
      let message: { id?: number; ok?: unknown; err?: string; changed?: string[] };
      try {
        message = JSON.parse(event.data);
      } catch {
        return;
      }
      if (message.changed) {
        this.onchange(message.changed);
        return;
      }
      const waiting = message.id == null ? undefined : this.pending.get(message.id);
      if (!waiting) return;
      this.pending.delete(message.id!);
      if (message.err != null) waiting.reject(message.err);
      else waiting.resolve(message.ok ?? null);
    };
    socket.onclose = (event) => {
      if (this.socket !== socket) return;
      this.socket = null;
      for (const waiting of this.pending.values()) waiting.reject("Lost the connection to the Studio PC");
      this.pending.clear();
      if (event.code === UNPAIRED) {
        this.key = "";
        store(KEY, null);
        this.state = "unpaired";
        return;
      }
      this.state = "lost";
      setTimeout(() => this.connect(), this.retryMs);
      this.retryMs = Math.min(this.retryMs * 2, 5000);
    };
  }

  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    const socket = this.socket;
    if (!socket || this.state !== "open") return Promise.reject("Not connected to the Studio PC");
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
      socket.send(JSON.stringify({ id, cmd: command, args: args ?? {} }));
    });
  }

  /** Pairs this browser with the code shown on the Studio PC. Returns an error message, if any. */
  async pair(code: string, name: string): Promise<string | null> {
    let response: Response;
    try {
      response = await fetch("/pair", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ code: code.trim(), name: name.trim() }),
      });
    } catch {
      return "Couldn't reach the Studio PC. Check it's on and remote control is turned on.";
    }
    const body = await response.json().catch(() => ({}));
    if (!response.ok || typeof body.key !== "string") return body.error ?? "Pairing didn't work. Try again.";
    this.key = body.key;
    store(KEY, body.key);
    store(NAME, name.trim());
    this.connect();
    return null;
  }
}

export const link = new RemoteLink();
