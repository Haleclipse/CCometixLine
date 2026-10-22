import { expect, test } from 'claude-code/testing'

// The bridge only watches: with it loaded, a tool call goes through and
// comes back as the tool answered it.
test('tool calls pass through untouched', async ($, on) => {
  on('tool.call', () => ({ result: 'answered' }))
  const r = await $.tool.call({ tool: 'Glob', pattern: '*.json' })
  expect(r).toEqual(expect.objectContaining({ result: 'answered' }))
})
