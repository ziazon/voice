import { readFileSync } from 'node:fs'
import { relative } from 'node:path'
import { expect, it } from 'vitest'
import { parse } from 'vue/compiler-sfc'
import { files, source } from './test-support'

it('keeps every screen and TypeScript source free of HTML insertion', () => {
  const forbidden = files(source).filter(path => /\.(ts|vue)$/.test(path)
    && !path.endsWith('.test.ts')
    && !relative(source, path).startsWith('types/generated/')
    && /v-html|innerHTML|outerHTML|insertAdjacentHTML|document\.write/.test(readFileSync(path, 'utf8')))
  expect(forbidden.map(path => relative(source, path))).toEqual([])
})

function disallowedBindings(template: string): string[] {
  const ast = parse(`<template>${template}</template>`).descriptor.template?.ast
  if (!ast) throw new Error('Missing template')
  type Nodes = typeof ast.children
  const bindings: string[] = []
  function visit(nodes: Nodes) {
    for (const node of nodes) {
      if (node.type !== 1) continue
      for (const prop of node.props) {
        if (prop.type !== 7 || prop.name !== 'bind') continue
        const binding = prop.loc.source.split(/[\s=]/)[0]!
        if (!/^(?::|v-bind:)(?:class|disabled|id|key|result|start|value)$/.test(binding)) bindings.push(binding)
      }
      visit(node.children)
    }
  }
  visit(ast.children)
  return bindings
}

it('allows only the approved attribute bindings on panel screens', () => {
  const panelScreens = files(source).filter(path => /screens\/(Panels\.vue|panels\/.*\.vue)$/.test(relative(source, path)))
  expect(panelScreens.length).toBeGreaterThan(0)
  const bound = panelScreens.flatMap(path => {
    const template = parse(readFileSync(path, 'utf8')).descriptor.template
    expect(template, relative(source, path)).toBeDefined()
    return disallowedBindings(template!.content).map(binding => ({ file: relative(source, path), binding }))
  })
  expect(bound).toEqual([])
})

it.each([
  'v-bind="{ title: value }"', 'v-bind.prop="value"',
  ':[attribute]="value"', 'v-bind:[attribute]="value"',
  '.srcdoc="value"', '.href="value"', ':xlink:href="value"', ':srcset="value"', ':HREF="value"',
  ':href="value"', ':src="value"', ':srcdoc="value"', ':style="value"', ':action="value"', ':formaction="value"',
  ':title="value"', 'v-bind:title="value"',
  ...['class', 'disabled', 'id', 'key', 'result', 'start', 'value'].flatMap(name => [`:${name}.prop="value"`, `v-bind:${name}.camel="value"`]),
])('rejects a panel binding that can hide its attribute: %s', binding => {
  expect(disallowedBindings(`<div ${binding}>Text</div>`)).toEqual([binding.split(/\s*=/)[0]])
})

it.each(['class', 'disabled', 'id', 'key', 'result', 'start', 'value'].flatMap(name => [`:${name}="value"`, `v-bind:${name}="value"`]))('allows a panel binding: %s', binding => {
  expect(disallowedBindings(`<div ${binding}>Text</div>`)).toEqual([])
})

it.each(['@click="act"', 'v-on:click="act"', 'v-if="ok"', 'v-else-if="ok"', 'v-for="item in items"', 'v-model="value"', 'v-show="ok"', 'href="https://example.com"', 'title=":href"'])('ignores a non-binding: %s', attribute => {
  expect(disallowedBindings(`<div ${attribute}>Text</div>`)).toEqual([])
})
