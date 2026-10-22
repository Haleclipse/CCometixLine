import { expect, mock, test } from 'claude-code/testing'

// The bridge only watches: with it loaded, a tool call goes through and
// comes back as the tool answered it.
test('tool calls pass through untouched', async ($, on) => {
  on('tool.call', () => ({ result: 'answered' }))
  const r = await $.tool.call({ tool: 'Glob', pattern: '*.json' })
  expect(r).toEqual(expect.objectContaining({ result: 'answered' }))
})

const USAGE = {
  input_tokens: 10,
  output_tokens: 150,
  cache_read_input_tokens: 0,
  cache_creation_input_tokens: 0,
  model: 'claude-opus-5-5',
}

// A model request answered 0.8 s after it went out, then streaming for 2 s.
async function* answer(clock: { sleep: (ms: number) => Promise<void> }, e: { turnId: string; index: number }) {
  await clock.sleep(800)
  yield { kind: 'thinking' as const, index: 0, text: 'hm' }
  await clock.sleep(2_000)
  yield { kind: 'text' as const, index: 1, text: 'done' }
  yield { kind: 'stop' as const, stopReason: 'end_turn' as const, usage: USAGE }
  return {
    turnId: e.turnId,
    index: e.index,
    answer: 'done',
    toolUses: [],
    stopReason: 'end_turn' as const,
    usage: USAGE,
  }
}

test('times each main-loop request as it finishes, before the turn ends', async ($, on) => {
  const clock = mock.clock(on, { now: 1_000_000 })
  const written: { path: string; text: string }[] = []
  on('fs.read', () => ({ value: JSON.stringify({ dir: '/home/u/.claude/ccline/bridge' }) }))
  on('fs.write', (_, e) => {
    written.push({ path: e.path, text: e.text })
    return { value: undefined }
  })
  let session = 'session-1'
  on('session.id', () => ({ value: session }))
  on('agent.list', () => ({ value: [] }))
  on('turn.step', async function* (_, e) {
    return yield* answer(clock, e)
  })
  on('turn.complete', () => ({ text: 'done' }))
  const bridge = () => JSON.parse(written[written.length - 1]?.text ?? '{}')

  const step = async (agentId?: string) => {
    const chunks: string[] = []
    const stream = $.turn.step({ turnId: 't1', index: 0, model: 'claude-opus-5-5', messageCount: 1, agentId })
    const done = (async () => {
      for await (const c of stream) chunks.push(c.kind)
    })()
    await clock.advance(800)
    await clock.advance(2_000)
    await done
    // The write follows a burst of events by a moment.
    await clock.advance(200)
    return chunks
  }

  // The answer passes through whole, and its timing lands while the turn
  // is still going.
  expect(await step()).toEqual(['thinking', 'text', 'stop'])
  expect(written[written.length - 1].path).toBe('/home/u/.claude/ccline/bridge/session-1.json')
  expect(bridge().turn).toEqual(
    expect.objectContaining({ ttft_ms: 800, tps: 75, output_tokens: 150, model: 'claude-opus-5-5' }),
  )

  // A subagent's request leaves the main loop's timing alone.
  const before = written.length
  expect(await step('agent-1')).toEqual(['thinking', 'text', 'stop'])
  expect(written.length).toBe(before)

  // The turn's end adds how long the whole turn took.
  await $.turn.complete({ answer: 'done', durationMs: 5_000, isAborted: false, turnId: 't1', reason: 'answer' })
  await clock.advance(200)
  expect(bridge().turn).toEqual(expect.objectContaining({ ttft_ms: 800, tps: 75, duration_ms: 5_000 }))

  // /clear starts another session in the same process: its file starts
  // empty of the last one's figures.
  session = 'session-2'
  await $.turn.complete({ answer: '', durationMs: 1_000, isAborted: false, turnId: 't2', reason: 'answer' })
  await clock.advance(200)
  expect(written[written.length - 1].path).toBe('/home/u/.claude/ccline/bridge/session-2.json')
  expect(bridge().session).toBe('session-2')
  expect(bridge().turn).toEqual({ duration_ms: 1_000, ended_at: expect.any(Number) })
})
