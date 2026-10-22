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

type Category = { name: string; kind: string; tokens: number }
type Context = {
  threshold?: number
  auto: boolean
  window?: number
  used?: number
  categories: Category[]
}
type Turn = {
  model?: string
  ttft_ms?: number
  tps?: number
  duration_ms?: number
  output_tokens?: number
  ended_at?: number
}
type Tool = { name: string; since: number; agent?: string }
type Agent = { id: string; type: string; status: string; description?: string }
/** One model request's timing. */
type Step = { sent: number; first?: number; output?: number; last?: number }

/** What the bridge holds, kept between events. */
const live = {
  dir: undefined as string | undefined,
  session: undefined as string | undefined,
  context: undefined as Context | undefined,
  turn: undefined as Turn | undefined,
  agents: [] as Agent[],
  tools: new Map<string, Tool>(),
  steps: new Map<string, Step>(),
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

export const register: Register = (on) => {
  live.dir = undefined
  live.pending = false

  on('session.start', async ($, e, next) => {
    const started = await next(e)
    try {
      live.session = await $.session.id()
      await measure($)
      await write($)
    } catch {}
    return started
  })

  // After each main-thread turn, and when a rate limit moves.
  on('session.measure', async ($, e, next) => {
    const echoed = await next(e)
    if (e.changed.includes('context')) {
      await measure($)
      flush($)
    }
    return echoed
  })

  // Time each model request on the main loop: when it went out, when the
  // first piece came back, and what its stop counted.
  on('turn.step', async function* ($, e, next) {
    const main = (e as { agentId?: string }).agentId === undefined
    const step: Step = { sent: main ? await now($) : 0 }
    const stream = next(e)
    let ended = false
    try {
      for (;;) {
        const piece = await stream.next()
        if (piece.done) {
          ended = true
          if (main) {
            step.last = await now($)
            live.steps.set(e.turnId, step)
          }
          return piece.value
        }
        if (main) {
          const c = piece.value as { kind?: string; usage?: { output_tokens?: number } | null }
          if (step.first === undefined && c.kind !== undefined && c.kind !== 'stop') {
            step.first = await now($)
          }
          if (c.kind === 'stop' && c.usage) step.output = c.usage.output_tokens
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
      if (e.agentId === undefined) {
        const step = live.steps.get(e.turnId)
        live.steps.delete(e.turnId)
        const streamed =
          step?.first !== undefined && step.last !== undefined ? step.last - step.first : 0
        const output = e.usage?.output_tokens ?? step?.output
        live.turn = {
          model: e.usage?.model,
          ttft_ms: step?.first !== undefined ? step.first - step.sent : live.turn?.ttft_ms,
          tps: output && streamed > 0 ? (output * 1000) / streamed : live.turn?.tps,
          duration_ms: e.durationMs,
          output_tokens: output,
          ended_at: await now($),
        }
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
