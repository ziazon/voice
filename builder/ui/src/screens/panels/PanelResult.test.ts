import { expect, it } from 'vitest'
import { screen } from '../../test-support'
import type { PanelResultView } from '../../types/generated/PanelResultView'
import PanelResult from './PanelResult.vue'

const result: PanelResultView = {
  outcome: 'agreed', answerText: 'Choose a small trial.\nThen review it.', answerItems: [],
  groups: [], dissent: [], markdown: '**bold**\nThe whole result.',
}
function show(overrides: Partial<PanelResultView> = {}) {
  return screen(PanelResult, undefined, { result: { ...result, ...overrides } })
}
it('shows the text answer, proposal reminder and outcome', () => {
  const wrapper = show()
  expect(wrapper.text()).toContain('Their answer')
  expect(wrapper.text()).toContain('A proposal for you, not a decision.')
  expect(wrapper.text()).toContain('They agreed.')
  const answer = wrapper.findAll('p, div, pre').find(node => node.text() === result.answerText)
  expect(answer, 'the complete text answer occupies one block').toBeDefined()
})
it('shows the list answer as ordered items in the supplied order', () => {
  const wrapper = show({ answerText: null, answerItems: ['Start small', 'Review together', 'Decide next'] })
  expect(wrapper.find('ol').exists()).toBe(true)
  expect(wrapper.get('ol').findAll('li').map(item => item.text())).toEqual(['Start small', 'Review together', 'Decide next'])
})
it('explains when there is no answer', () => {
  expect(show({ answerText: null }).text()).toContain('They gave no answer.')
})
it('shows grouped members, rounded weights and each text or ordered answer', () => {
  const wrapper = show({ groups: [
    { members: ['Pebble', 'Quill'], weight: 0.6, answerText: 'Try the pilot.', answerItems: [] },
    { members: ['Moss'], weight: 1 / 3, answerText: null, answerItems: ['Wait', 'Ask again'] },
  ] })
  expect(wrapper.text()).toContain('How they grouped')
  expect(wrapper.text()).toContain('Pebble, Quill')
  expect(wrapper.text()).toContain('60% of the panel')
  expect(wrapper.text()).toContain('Moss')
  expect(wrapper.text()).toContain('33% of the panel')
  expect(wrapper.text()).toContain('Try the pilot.')
  expect(wrapper.findAll('ol li').map(item => item.text())).toEqual(['Wait', 'Ask again'])
})
it('quotes every dissent in full and marks only the holdout', () => {
  const longObjection = 'z'.repeat(1500) + '\nKeep the alternative open.'
  const wrapper = show({ dissent: [
    { seat: 'Pebble', holdout: true, objections: longObjection },
    { seat: 'Quill', holdout: false, objections: 'The timing needs work.' },
  ] })
  expect(wrapper.text()).toContain('Who disagreed')
  expect(wrapper.text()).toContain('Pebble')
  expect(wrapper.text()).toContain('Quill')
  expect(wrapper.findAll('blockquote').map(item => item.text())).toEqual([longObjection, 'The timing needs work.'])
  expect(wrapper.text().match(/Held out to the end\./g)).toHaveLength(1)
  expect(wrapper.text()).not.toContain('show more')
})
it('says nobody disagreed when the dissent list is empty', () => {
  expect(show().text()).toContain('Nobody disagreed.')
})
it('keeps the whole result in a details pre as raw markdown', () => {
  const wrapper = show()
  expect(wrapper.find('details').exists()).toBe(true)
  expect(wrapper.get('details summary').text()).toBe('The whole result as text')
  expect(wrapper.get('details pre').text()).toBe(result.markdown)
  expect(wrapper.find('strong').exists()).toBe(false)
})
it('renders hostile model text literally without creating elements or running code', () => {
  const answerText = '<img src=x onerror="window.hit=1">'
  const item = '<script>window.hit=1</script>'
  const groupAnswer = '[go](javascript:window.hit=1)'
  const objections = '<a href="https://example.com">x</a><b>y</b>'
  const markdown = '<iframe src="https://example.com">'
  const wrapper = show({ answerText, answerItems: [], groups: [
    { members: ['Pebble'], weight: 0.5, answerText: groupAnswer, answerItems: [] },
    { members: ['Quill'], weight: 0.5, answerText: null, answerItems: [item] },
  ], dissent: [{ seat: 'Moss', holdout: true, objections }], markdown })
  for (const text of [answerText, item, groupAnswer, objections, markdown]) expect(wrapper.text()).toContain(text)
  expect(wrapper.findAll('img, script, a, b, iframe')).toHaveLength(0)
  expect((window as Window & { hit?: number }).hit).toBeUndefined()
})
it('wraps all model text and preserves its line breaks', () => {
  const texts = ['Answer\nline', 'Item\nline', 'Group\nline', 'Group item\nline', 'Objection\nline', '**raw**\nline']
  const wrapper = show({ answerText: texts[0]!, answerItems: [], groups: [
    { members: ['Pebble'], weight: 0.5, answerText: texts[2]!, answerItems: [] },
    { members: ['Quill'], weight: 0.5, answerText: null, answerItems: [texts[3]!] },
  ], dissent: [{ seat: 'Moss', holdout: false, objections: texts[4]! }], markdown: texts[5]! })
  const listWrapper = show({ answerText: null, answerItems: [texts[1]!] })
  for (const [index, text] of texts.entries()) {
    const view = index === 1 ? listWrapper : wrapper
    const node = view.findAll('.text-break').find(element => element.text() === text)
    expect(node, `wrapping element for ${text}`).toBeDefined()
    expect((node!.element as HTMLElement).style.whiteSpace).toBe('pre-wrap')
  }
})
