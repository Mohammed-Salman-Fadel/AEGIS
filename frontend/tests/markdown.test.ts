import test from 'node:test';
import assert from 'node:assert/strict';

import { inferStandaloneCode, parseMarkdownBlocks, splitInlineMath, unwrapProseOnlyFence } from '../src/lib/markdown.js';

test('standalone compact Python functions render as language-tagged code', () => {
  const blocks = parseMarkdownBlocks(
    'def fibonacci(n): if n <= 1: return n else: return fibonacci(n-1) + fibonacci(n-2)',
  );
  assert.deepEqual(blocks, [{
    type: 'code',
    language: 'python',
    text: 'def fibonacci(n):\n    if n <= 1:\n        return n\n    else:\n        return fibonacci(n-1) + fibonacci(n-2)',
  }]);
});

test('programming prose remains prose', () => {
  assert.equal(inferStandaloneCode('A Python function begins with the def keyword.'), null);
});

test('existing fenced code remains unchanged', () => {
  assert.deepEqual(parseMarkdownBlocks('```python\ndef answer():\n    return 42\n```'), [{
    type: 'code',
    language: 'python',
    text: 'def answer():\n    return 42',
  }]);
});

test('generic fences around headings and prose are removed', () => {
  const input = '```\n# Hello\nYou are my user.\n```';
  assert.equal(unwrapProseOnlyFence(input), '# Hello\nYou are my user.');
  assert.deepEqual(parseMarkdownBlocks(input), [
    { type: 'heading', level: 1, text: 'Hello' },
    { type: 'paragraph', text: 'You are my user.' },
  ]);
});

test('generic fences containing executable commands are preserved', () => {
  const input = '```\nnpm install\n```';
  assert.equal(unwrapProseOnlyFence(input), input);
  assert.deepEqual(parseMarkdownBlocks(input), [
    { type: 'code', language: 'text', text: 'npm install' },
  ]);
});

test('inline LaTeX expressions are separated for math rendering', () => {
  assert.deepEqual(splitInlineMath('The derivative is $12x^{11}$.'), [
    { type: 'text', value: 'The derivative is ' },
    { type: 'math', value: '12x^{11}', display: false },
    { type: 'text', value: '.' },
  ]);
});

test('recognizes subtraction and function notation inside math delimiters', () => {
  assert.deepEqual(splitInlineMath('For $f(x)=x-y$, evaluate $f(2)$.'), [
    { type: 'text', value: 'For ' },
    { type: 'math', value: 'f(x)=x-y', display: false },
    { type: 'text', value: ', evaluate ' },
    { type: 'math', value: 'f(2)', display: false },
    { type: 'text', value: '.' },
  ]);
});

test('display math supports fractions and roots', () => {
  assert.deepEqual(splitInlineMath('Result:\n$$\\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}$$'), [
    { type: 'text', value: 'Result:\n' },
    { type: 'math', value: '\\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}', display: true },
  ]);
});

test('currency and escaped dollar signs remain ordinary text', () => {
  assert.deepEqual(splitInlineMath('It costs $20 and the limit is \\$30.'), [
    { type: 'text', value: 'It costs $20 and the limit is \\$30.' },
  ]);
});

test('escaped currency does not prevent later math rendering', () => {
  assert.deepEqual(splitInlineMath('Budget: \\$20; variable: $x$.'), [
    { type: 'text', value: 'Budget: \\$20; variable: ' },
    { type: 'math', value: 'x', display: false },
    { type: 'text', value: '.' },
  ]);
});
