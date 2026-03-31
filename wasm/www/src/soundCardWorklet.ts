// Minimal AudioWorklet global declarations — these types live in AudioWorkletGlobalScope,
// which is not part of the standard DOM lib included by tsconfig.app.json.
declare const sampleRate: number;
declare abstract class AudioWorkletProcessor {
    public readonly port: MessagePort;
}
declare function registerProcessor(name: string, ctor: new () => AudioWorkletProcessor): void;

interface AudioMessage {
    type: string;
    samples: Float32Array;
}

class SoundCardProcessor extends AudioWorkletProcessor {
    private _buf: Float32Array;
    private _pos: number;
    private _underrunCount: number;
    private _nonzeroCount: number;
    private _totalCount: number;
    private readonly _logInterval: number;

    public constructor() {
        super();
        this._buf = new Float32Array(0);
        this._pos = 0;
        this._underrunCount = 0;
        this._nonzeroCount = 0;
        this._totalCount = 0;
        this._logInterval = sampleRate;
        this.port.onmessage = (e: MessageEvent<AudioMessage>) => {
            if (e.data?.type === 'samples') {
                const incoming = e.data.samples;
                const remaining = this._buf.length - this._pos;
                const merged = new Float32Array(remaining + incoming.length);
                merged.set(this._buf.subarray(this._pos));
                merged.set(incoming, remaining);
                this._buf = merged;
                this._pos = 0;
            }
        };
    }

    public process(_inputs: Float32Array[][], outputs: Float32Array[][]): boolean {
        const out = outputs[0][0];
        if (!out) {
            return true;
        }

        const avail = this._buf.length - this._pos;
        const n = Math.min(out.length, avail);
        out.set(this._buf.subarray(this._pos, this._pos + n));
        this._pos += n;
        for (let i = n; i < out.length; i++) {
            out[i] = 0;
        }

        this._underrunCount += out.length - n;
        this._totalCount += out.length;
        for (let i = 0; i < n; i++) {
            if (Math.abs(out[i]) > 1e-6) {
                this._nonzeroCount++;
            }
        }

        if (this._totalCount >= this._logInterval) {
            this.port.postMessage({
                type: 'stats',
                underrunCount: this._underrunCount,
                nonzeroCount: this._nonzeroCount,
                totalCount: this._totalCount,
                backlog: this._buf.length - this._pos,
            });
            this._underrunCount = 0;
            this._nonzeroCount = 0;
            this._totalCount = 0;
        }
        return true;
    }
}

registerProcessor('sound-card-processor', SoundCardProcessor);
