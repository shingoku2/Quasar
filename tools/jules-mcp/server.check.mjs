// Run with `npm run test:jules-mcp` (node:test). Deliberately not named *.test.mjs,
// which Vitest's default include pattern would pick up and run under jsdom.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  API_BASE,
  handleMessage,
  normalizeSessionId,
  normalizeSourceName,
  slimActivities,
} from './server.mjs';

function fakeFetch(status, payload) {
  const calls = [];
  const impl = async (url, init) => {
    calls.push({ url, init });
    return { ok: status < 400, status, text: async () => JSON.stringify(payload) };
  };
  return { calls, impl };
}

const call = (name, args, options) =>
  handleMessage({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name, arguments: args } }, options);

test('normalizes session ids and rejects path escapes', () => {
  assert.equal(normalizeSessionId('123'), '123');
  assert.equal(normalizeSessionId('sessions/123'), '123');
  assert.throws(() => normalizeSessionId('../sources'));
  assert.throws(() => normalizeSessionId('1?x=y'));
  assert.throws(() => normalizeSessionId(''));
});

test('normalizes source names', () => {
  assert.equal(normalizeSourceName('shingoku2/Quasar'), 'sources/github/shingoku2/Quasar');
  assert.equal(normalizeSourceName('github/a/b'), 'sources/github/a/b');
  assert.equal(normalizeSourceName('sources/github/a/b'), 'sources/github/a/b');
  assert.throws(() => normalizeSourceName('a/b/c'));
  assert.throws(() => normalizeSourceName('a/../b'));
});

test('strips media blobs and truncates huge patches', () => {
  const out = slimActivities({
    activities: [{ artifacts: [{ media: { data: 'A'.repeat(50), mimeType: 'image/png' } }, { changeSet: { gitPatch: { unidiffPatch: 'x'.repeat(25_000) } } }] }],
  });
  const [media, change] = out.activities[0].artifacts;
  assert.equal(media.media.data, '[50 chars of base64 omitted]');
  assert.equal(media.media.mimeType, 'image/png');
  assert.match(change.changeSet.gitPatch.unidiffPatch, /truncated 5000 chars/);
});

test('initialize and tools/list', async () => {
  const init = await handleMessage({ jsonrpc: '2.0', id: 0, method: 'initialize', params: { protocolVersion: '2025-06-18' } });
  assert.equal(init.result.serverInfo.name, 'jules');
  assert.deepEqual(init.result.capabilities, { tools: {} });
  const list = await handleMessage({ jsonrpc: '2.0', id: 1, method: 'tools/list' });
  const names = list.result.tools.map((t) => t.name);
  assert.ok(names.includes('jules_create_session'));
  assert.ok(list.result.tools.every((t) => !('run' in t)));
  assert.equal(await handleMessage({ jsonrpc: '2.0', method: 'notifications/initialized' }), null);
});

test('create_session builds the documented request body and sends the key header', async () => {
  const f = fakeFetch(200, { name: 'sessions/9', id: '9' });
  const res = await call('jules_create_session', { prompt: 'fix it', source: 'o/r', autoCreatePr: true, requirePlanApproval: true, title: 't' }, { apiKey: 'k', fetchImpl: f.impl });
  assert.equal(res.result.isError, undefined);
  assert.equal(f.calls[0].url, `${API_BASE}/sessions`);
  assert.equal(f.calls[0].init.method, 'POST');
  assert.equal(f.calls[0].init.headers['X-Goog-Api-Key'], 'k');
  assert.deepEqual(JSON.parse(f.calls[0].init.body), {
    prompt: 'fix it',
    sourceContext: { source: 'sources/github/o/r', githubRepoContext: { startingBranch: 'main' } },
    title: 't',
    requirePlanApproval: true,
    automationMode: 'AUTO_CREATE_PR',
  });
});

test('approve/send/list-activities hit the right endpoints', async () => {
  const f = fakeFetch(200, {});
  await call('jules_approve_plan', { sessionId: 'sessions/5' }, { apiKey: 'k', fetchImpl: f.impl });
  await call('jules_send_message', { sessionId: '5', prompt: 'hi' }, { apiKey: 'k', fetchImpl: f.impl });
  await call('jules_list_activities', { sessionId: '5' }, { apiKey: 'k', fetchImpl: f.impl });
  assert.deepEqual(f.calls.map((c) => c.url), [
    `${API_BASE}/sessions/5:approvePlan`,
    `${API_BASE}/sessions/5:sendMessage`,
    `${API_BASE}/sessions/5/activities?pageSize=30`,
  ]);
});

test('API errors and a missing key come back as tool errors without leaking the key', async () => {
  const f = fakeFetch(401, { error: { message: 'API key not valid.' } });
  const res = await call('jules_list_sessions', {}, { apiKey: 'secret-key', fetchImpl: f.impl });
  assert.equal(res.result.isError, true);
  assert.match(res.result.content[0].text, /HTTP 401 API key not valid/);
  assert.doesNotMatch(res.result.content[0].text, /secret-key/);

  const missing = await call('jules_list_sessions', {}, { apiKey: '', fetchImpl: f.impl });
  assert.match(missing.result.content[0].text, /JULES_API_KEY is not set/);
});

test('unknown tool and method are JSON-RPC errors', async () => {
  assert.equal((await call('nope', {})).error.code, -32602);
  assert.equal((await handleMessage({ jsonrpc: '2.0', id: 3, method: 'bogus' })).error.code, -32601);
});
