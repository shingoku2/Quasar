#!/usr/bin/env node
/**
 * Google Jules MCP connector for Claude Code.
 *
 * A dependency-free Model Context Protocol server (stdio transport,
 * newline-delimited JSON-RPC 2.0) wrapping the Jules REST API
 * (https://jules.googleapis.com/v1alpha). Auth is the `X-Goog-Api-Key`
 * header, read from the JULES_API_KEY environment variable. The key is
 * never logged or echoed back in tool output.
 */

import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';

export const API_BASE = 'https://jules.googleapis.com/v1alpha';
const PROTOCOL_VERSION = '2025-06-18';
const SERVER_INFO = { name: 'jules', version: '1.0.0' };
const REQUEST_TIMEOUT_MS = 30_000;
/** Cap per unidiff patch so one activity page can't flood the context window. */
const MAX_PATCH_CHARS = 20_000;
const ID_PATTERN = /^[A-Za-z0-9_.-]+$/;

/** Accepts `123` or `sessions/123`; rejects anything that could escape the path. */
export function normalizeSessionId(raw) {
  const id = String(raw ?? '').trim().replace(/^sessions\//, '');
  if (!ID_PATTERN.test(id)) {
    throw new Error(`Invalid session id: ${JSON.stringify(raw)}`);
  }
  return id;
}

/** Accepts `owner/repo`, `github/owner/repo`, or `sources/github/owner/repo`. */
export function normalizeSourceName(raw) {
  let s = String(raw ?? '').trim().replace(/^sources\//, '');
  if (!s.startsWith('github/')) s = `github/${s}`;
  const parts = s.split('/');
  if (parts.length !== 3 || !parts.every((p) => ID_PATTERN.test(p))) {
    throw new Error(`Invalid source: ${JSON.stringify(raw)} (expected owner/repo)`);
  }
  return `sources/${s}`;
}

/** Drops base64 media blobs and truncates huge patches in API responses. */
export function slimActivities(value) {
  if (Array.isArray(value)) return value.map(slimActivities);
  if (value === null || typeof value !== 'object') return value;
  const out = {};
  for (const [key, v] of Object.entries(value)) {
    if (key === 'media' && v && typeof v.data === 'string') {
      out.media = { ...v, data: `[${v.data.length} chars of base64 omitted]` };
    } else if (key === 'unidiffPatch' && typeof v === 'string' && v.length > MAX_PATCH_CHARS) {
      out.unidiffPatch = `${v.slice(0, MAX_PATCH_CHARS)}\n[... truncated ${v.length - MAX_PATCH_CHARS} chars]`;
    } else {
      out[key] = slimActivities(v);
    }
  }
  return out;
}

function query(params) {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null && v !== '') qs.set(k, String(v));
  }
  const s = qs.toString();
  return s ? `?${s}` : '';
}

export async function julesRequest(method, path, body, { apiKey = process.env.JULES_API_KEY, fetchImpl = fetch } = {}) {
  if (!apiKey) {
    throw new Error('JULES_API_KEY is not set. Create a key at https://jules.google.com/settings#api and export it before starting Claude Code.');
  }
  const res = await fetchImpl(`${API_BASE}${path}`, {
    method,
    headers: { 'X-Goog-Api-Key': apiKey, 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  const text = await res.text();
  let data;
  try {
    data = text ? JSON.parse(text) : {};
  } catch {
    data = { raw: text };
  }
  if (!res.ok) {
    const detail = data?.error?.message ?? text.slice(0, 500);
    throw new Error(`Jules API ${method} ${path.split('?')[0]} failed: HTTP ${res.status} ${detail}`);
  }
  return data;
}

const pagingProps = {
  pageSize: { type: 'integer', minimum: 1, maximum: 100, description: 'Max items to return.' },
  pageToken: { type: 'string', description: 'nextPageToken from a previous call.' },
};
const sessionIdProp = { sessionId: { type: 'string', description: 'Session id (`123` or `sessions/123`).' } };

export const TOOLS = [
  {
    name: 'jules_list_sources',
    description: 'List GitHub repositories connected to Jules (the Jules GitHub app must be installed on a repo first).',
    inputSchema: { type: 'object', properties: { ...pagingProps, filter: { type: 'string', description: 'AIP-160 filter expression.' } } },
    run: (a, o) => julesRequest('GET', `/sources${query({ pageSize: a.pageSize, pageToken: a.pageToken, filter: a.filter })}`, undefined, o),
  },
  {
    name: 'jules_get_source',
    description: 'Get one connected source, including its branches.',
    inputSchema: { type: 'object', properties: { source: { type: 'string', description: '`owner/repo` or `sources/github/owner/repo`.' } }, required: ['source'] },
    run: (a, o) => julesRequest('GET', `/${normalizeSourceName(a.source)}`, undefined, o),
  },
  {
    name: 'jules_create_session',
    description: 'Start a Jules coding task on a repo. Plans auto-approve unless requirePlanApproval is true. Returns the session; poll jules_get_session / jules_list_activities for progress.',
    inputSchema: {
      type: 'object',
      properties: {
        prompt: { type: 'string', description: 'The task for Jules.' },
        source: { type: 'string', description: '`owner/repo` or `sources/github/owner/repo`.' },
        startingBranch: { type: 'string', description: 'Branch to start from (default: main).' },
        title: { type: 'string' },
        requirePlanApproval: { type: 'boolean', description: 'Pause for jules_approve_plan before executing.' },
        autoCreatePr: { type: 'boolean', description: 'Open a pull request automatically when done.' },
      },
      required: ['prompt', 'source'],
    },
    run: (a, o) => {
      if (!String(a.prompt ?? '').trim()) throw new Error('prompt must not be empty');
      const body = {
        prompt: a.prompt,
        sourceContext: {
          source: normalizeSourceName(a.source),
          githubRepoContext: { startingBranch: a.startingBranch || 'main' },
        },
      };
      if (a.title) body.title = a.title;
      if (a.requirePlanApproval) body.requirePlanApproval = true;
      if (a.autoCreatePr) body.automationMode = 'AUTO_CREATE_PR';
      return julesRequest('POST', '/sessions', body, o);
    },
  },
  {
    name: 'jules_list_sessions',
    description: 'List your Jules sessions, newest first.',
    inputSchema: { type: 'object', properties: { ...pagingProps } },
    run: (a, o) => julesRequest('GET', `/sessions${query({ pageSize: a.pageSize, pageToken: a.pageToken })}`, undefined, o),
  },
  {
    name: 'jules_get_session',
    description: 'Get a session: state, prompt, and outputs (e.g. the pull request Jules opened).',
    inputSchema: { type: 'object', properties: { ...sessionIdProp }, required: ['sessionId'] },
    run: (a, o) => julesRequest('GET', `/sessions/${normalizeSessionId(a.sessionId)}`, undefined, o),
  },
  {
    name: 'jules_approve_plan',
    description: 'Approve the latest plan of a session created with requirePlanApproval.',
    inputSchema: { type: 'object', properties: { ...sessionIdProp }, required: ['sessionId'] },
    run: (a, o) => julesRequest('POST', `/sessions/${normalizeSessionId(a.sessionId)}:approvePlan`, {}, o),
  },
  {
    name: 'jules_send_message',
    description: 'Send a follow-up message to Jules in a session. The reply arrives later as a new activity.',
    inputSchema: { type: 'object', properties: { ...sessionIdProp, prompt: { type: 'string' } }, required: ['sessionId', 'prompt'] },
    run: (a, o) => {
      if (!String(a.prompt ?? '').trim()) throw new Error('prompt must not be empty');
      return julesRequest('POST', `/sessions/${normalizeSessionId(a.sessionId)}:sendMessage`, { prompt: a.prompt }, o);
    },
  },
  {
    name: 'jules_list_activities',
    description: 'List a session\'s activities (plans, progress, bash output, diffs, agent messages, completion). Screenshots are omitted and very large patches truncated.',
    inputSchema: { type: 'object', properties: { ...sessionIdProp, ...pagingProps }, required: ['sessionId'] },
    run: async (a, o) => slimActivities(
      await julesRequest('GET', `/sessions/${normalizeSessionId(a.sessionId)}/activities${query({ pageSize: a.pageSize ?? 30, pageToken: a.pageToken })}`, undefined, o),
    ),
  },
];

/** Handles one JSON-RPC message; returns the response object, or null for notifications. */
export async function handleMessage(msg, options = {}) {
  const { id, method, params } = msg ?? {};
  const isNotification = id === undefined || id === null;
  const reply = (result) => (isNotification ? null : { jsonrpc: '2.0', id, result });
  const fail = (code, message) => (isNotification ? null : { jsonrpc: '2.0', id, error: { code, message } });

  switch (method) {
    case 'initialize':
      return reply({
        protocolVersion: params?.protocolVersion ?? PROTOCOL_VERSION,
        capabilities: { tools: {} },
        serverInfo: SERVER_INFO,
      });
    case 'ping':
      return reply({});
    case 'tools/list':
      return reply({ tools: TOOLS.map(({ name, description, inputSchema }) => ({ name, description, inputSchema })) });
    case 'tools/call': {
      const tool = TOOLS.find((t) => t.name === params?.name);
      if (!tool) return fail(-32602, `Unknown tool: ${params?.name}`);
      try {
        const data = await tool.run(params?.arguments ?? {}, options);
        return reply({ content: [{ type: 'text', text: JSON.stringify(data, null, 2) }] });
      } catch (err) {
        return reply({ isError: true, content: [{ type: 'text', text: err instanceof Error ? err.message : String(err) }] });
      }
    }
    default:
      if (typeof method === 'string' && method.startsWith('notifications/')) return null;
      return fail(-32601, `Method not found: ${method}`);
  }
}

function main() {
  const rl = createInterface({ input: process.stdin });
  const send = (obj) => process.stdout.write(`${JSON.stringify(obj)}\n`);
  rl.on('line', async (line) => {
    if (!line.trim()) return;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      send({ jsonrpc: '2.0', id: null, error: { code: -32700, message: 'Parse error' } });
      return;
    }
    const res = await handleMessage(msg);
    if (res) send(res);
  });
  if (!process.env.JULES_API_KEY) {
    process.stderr.write('[jules-mcp] warning: JULES_API_KEY is not set; tool calls will fail until it is.\n');
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) main();
