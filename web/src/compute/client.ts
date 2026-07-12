// client.ts — the main-thread API over the dedicated compute Web Worker (D-10).
// Spins the worker, forwards a job spec to it, and fans the worker's JobStatus /
// TierComplete / capability messages out to subscribers (the CalcPanel binds these
// into the calc store). The heavy solve runs in the worker + its rayon pool; this
// module only marshals messages — it issues NO server request and touches NO SSE
// endpoint (the client-side solve is not the Phase-6 server job machine, D-10).
//
// # Module I/O
// - Input  `submit(spec)` / `cancel()` from the UI, and `WorkerOutbound` messages
//   from the worker.
// - Output the posted worker messages, delivered to every `subscribe(cb)` listener
//   (unsubscribe via the returned disposer). The worker is created lazily on the
//   first `submit` and reused (a single client-side job at a time, T-10-04-05).
// - Valid input range: a `CalcJobSpec` (the worker validates capability + shape).

import type { CalcJobSpec, WorkerInbound, WorkerOutbound } from "./worker";

// A subscriber to the worker's outbound message stream.
export type CalcSubscriber = (msg: WorkerOutbound) => void;

// The main-thread compute client: lazily owns ONE dedicated worker and forwards
// its messages to subscribers. Reused across submits (a new submit supersedes the
// prior in-flight run via the worker's cooperative cancel, not a new worker).
export class CalcClient {
  #worker: Worker | null = null;
  readonly #subscribers = new Set<CalcSubscriber>();

  // Lazily create (once) the dedicated module worker. `new URL(..., import.meta.url)`
  // lets Vite bundle the worker + its threaded-wasm glue as a separate chunk.
  #ensureWorker(): Worker {
    if (this.#worker === null) {
      this.#worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
      this.#worker.onmessage = (ev: MessageEvent<WorkerOutbound>) => {
        for (const cb of this.#subscribers) {
          cb(ev.data);
        }
      };
    }
    return this.#worker;
  }

  // Subscribe to the worker's outbound stream; returns an unsubscribe disposer.
  subscribe(cb: CalcSubscriber): () => void {
    this.#subscribers.add(cb);
    return () => {
      this.#subscribers.delete(cb);
    };
  }

  // Submit a calc job to the worker (creates the worker on first use).
  submit(spec: CalcJobSpec): void {
    const msg: WorkerInbound = { type: "submit", spec };
    this.#ensureWorker().postMessage(msg);
  }

  // Request cooperative cancellation of the in-flight run (D-11 — the worker flips
  // the pool's atomic flag; NO worker.terminate()).
  cancel(): void {
    if (this.#worker === null) {
      return;
    }
    const msg: WorkerInbound = { type: "cancel" };
    this.#worker.postMessage(msg);
  }

  // Tear down the client: terminate the dedicated worker (releasing its rayon pool /
  // SharedArrayBuffer memory) and drop all subscribers (WR-05). Called on panel
  // unmount and when the store replaces this client, so repeated mounts (HMR, route
  // change, conditional render) do not leak workers. This is TEARDOWN of the whole
  // client — distinct from the D-11 rule that a RUNNING solve is aborted
  // cooperatively via `cancel()`, never by terminating a live job. Idempotent.
  dispose(): void {
    if (this.#worker !== null) {
      this.#worker.terminate();
      this.#worker = null;
    }
    this.#subscribers.clear();
  }
}
