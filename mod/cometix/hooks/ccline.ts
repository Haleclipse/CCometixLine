// The ccline companion mod: a data bridge, nothing else. It watches the
// session from inside Claude Code and writes what the status line's JSON
// lacks to `<ccline home>/bridge/<session id>.json`, which ccline reads when
// it renders. It draws no UI and changes nothing about the session: every
// hook passes its event on untouched.
//
// Written by ccline from the copy built into it. Format: src/state/bridge.rs
// in CCometixLine.

import type { EngineInterface, Register } from 'claude-code'

type $ = EngineInterface

/** Bridge format version; ccline ignores files with another. */
const VERSION = 1
/** Events within this many milliseconds share one write. */
const FLUSH_MS = 200
/** A response that streamed for less than this gives no speed. */
const MIN_STREAM_MS = 100

type Category = { name: string; kind: string; tokens: number }
type Context = {
  threshold?: number
  auto: boolean
  window?: number
  used?: number
  categories: Category[]
}
/** The main loop's last model request, and how long its last turn took. */
type Turn = {
  model?: string
  ttft_ms?: number
  tps?: number
  /** The last finished turn's wall-clock length. */
  duration_ms?: number
  /** The last request's. */
  output_tokens?: number
  ended_at?: number
}
type Tool = { name: string; since: number; agent?: string }
type Agent = { id: string; type: string; status: string; description?: string }
type Chunk = { kind?: string; usage?: { output_tokens?: number; model?: string } | null }

/** What the bridge holds, kept between events. */
const live = {
  dir: undefined as string | undefined,
  session: undefined as string | undefined,
  context: undefined as Context | undefined,
  turn: undefined as Turn | undefined,
  agents: [] as Agent[],
  tools: new Map<string, Tool>(),
  pending: false,
  seq: 0,
}

// Where to write: the path the installer left beside the mod, else the
// home ccline uses by default.
async function where($: $): Promise<string | undefined> {
  if (live.dir !== undefined) return live.dir
  try {
    const saved = JSON.parse(String(await $.fs.read(`${$.plugin.root}/bridge.json`)))
    if (typeof saved?.dir === 'string' && saved.dir !== '') live.dir = saved.dir
  } catch {}
  if (live.dir === undefined) {
    const own = await $.env.get('CCLINE_HOME')
    const home = (await $.env.get('HOME')) ?? (await $.env.get('USERPROFILE'))
    if (own) live.dir = `${own}/bridge`
    else if (home) live.dir = `${home}/.claude/ccline/bridge`
  }
  return live.dir
}

// Forget what the bridge held for another session.
function reset(session: string) {
  live.session = session
  live.context = undefined
  live.turn = undefined
  live.agents = []
  live.tools.clear()
}

// The session can change under a running process (/clear, /resume), and
// each one's figures are its own: start over when the id moves.
async function follow($: $) {
  try {
    const id = await $.session.id()
    if (id && id !== live.session) reset(id)
  } catch {}
}

async function write($: $) {
  try {
    const to = await where($)
    live.session ??= await $.session.id()
    if (to === undefined || !live.session) return
    const doc = {
      v: VERSION,
      session: live.session,
      updated_at: await $.clock.now(),
      context: live.context,
      turn: live.turn,
      tools: [...live.tools.values()],
      agents: live.agents,
    }
    await $.fs.write(`${to}/${live.session}.json`, JSON.stringify(doc))
  } catch {}
}

// Coalesce a burst of events into one write shortly after.
function flush($: $) {
  if (live.pending) return
  live.pending = true
  try {
    $.clock.after(FLUSH_MS, () => {
      live.pending = false
      void write($)
    })
  } catch {
    live.pending = false
  }
}

async function measure($: $) {
  try {
    const usage = await $.session.usage({ breakdown: 'summary' })
    const b = usage.context.breakdown
    if (b === undefined) return
    live.context = {
      threshold: b.autoCompactThreshold,
      auto: b.isAutoCompactEnabled,
      window: b.rawMaxTokens,
      used: b.totalTokens,
      categories: b.categories.map((c) => ({ name: c.name, kind: c.kind, tokens: c.tokens })),
    }
  } catch {}
}

async function listAgents($: $) {
  try {
    live.agents = (await $.agent.list()).map((a) => ({
      id: a.id,
      type: a.type,
      status: a.status,
      description: a.description,
    }))
  } catch {}
}

async function now($: $): Promise<number> {
  return $.clock.now()
}

// One main-loop request is done: its time to first token and its speed,
// shown while the turn goes on rather than once it ends.
function timed(sent: number, first: number, last: number, output?: number, model?: string) {
  const streamed = last - first
  live.turn = {
    ...live.turn,
    model: model ?? live.turn?.model,
    ttft_ms: first - sent,
    tps: output && streamed >= MIN_STREAM_MS ? (output * 1000) / streamed : live.turn?.tps,
    output_tokens: output,
    ended_at: last,
  }
}

export const register: Register = (on) => {
  live.dir = undefined
  live.pending = false

  on('session.start', async ($, e, next) => {
    const started = await next(e)
    try {
      await follow($)
      await measure($)
      await write($)
    } catch {}
    return started
  })

  // After each main-thread turn, and when a rate limit moves.
  on('session.measure', async ($, e, next) => {
    const echoed = await next(e)
    await follow($)
    if (e.changed.includes('context')) {
      await measure($)
      flush($)
    }
    return echoed
  })

  // Time each model request on the main loop as Claude Code does: from
  // sending it to the first thing back (the response's envelope), then the
  // output its stop counted over the time it streamed, thinking included.
  // Subagents' requests pass straight through.
  on('turn.step', async function* ($, e, next) {
    if (e.agentId !== undefined) return yield* next(e)
    await follow($)
    const sent = await now($)
    let first: number | undefined
    let output: number | undefined
    let model: string | undefined
    const stream = next(e)
    let ended = false
    try {
      for (;;) {
        const piece = await stream.next()
        if (piece.done) {
          ended = true
          try {
            if (first !== undefined) {
              const usage = piece.value?.usage
              timed(sent, first, await now($), output ?? usage?.output_tokens, model ?? usage?.model)
              flush($)
            }
          } catch {}
          return piece.value
        }
        const c = piece.value as Chunk
        if (first === undefined) first = await now($)
        if (c.kind === 'stop' && c.usage) {
          output = c.usage.output_tokens
          model = c.usage.model
        }
        yield piece.value
      }
    } finally {
      // Left early (the answer was interrupted): close the stream beneath,
      // as a for-await loop would.
      if (!ended) await stream.return?.(undefined as never)
    }
  })

  on('turn.complete', async ($, e, next) => {
    const answered = await next(e)
    try {
      await follow($)
      if (e.agentId === undefined) {
        live.turn = { ...live.turn, duration_ms: e.durationMs, ended_at: await now($) }
      }
      await listAgents($)
      flush($)
    } catch {}
    return answered
  })

  // Tools running now, the main loop's and the agents'.
  on('tool.call', async ($, e, next) => {
    const key = e.tool_use_id ?? `ccline-${live.seq++}`
    try {
      await follow($)
      live.tools.set(key, { name: e.tool, since: await now($), agent: e.agentId })
      flush($)
    } catch {}
    try {
      return await next(e)
    } finally {
      live.tools.delete(key)
      flush($)
    }
  })

  on('agent.spawn', async ($, e, next) => {
    const spawned = await next(e)
    await follow($)
    await listAgents($)
    flush($)
    return spawned
  })

  on('session.end', async ($, e, next) => {
    live.tools.clear()
    live.agents = []
    await write($)
    return next(e)
  })
}
