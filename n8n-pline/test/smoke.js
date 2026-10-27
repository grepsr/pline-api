// Smoke test: drives the built node with a stubbed n8n context and asserts the exact requests it
// sends, so wire casing and validation stay in step with the Pline API. Run: npm test
const assert = require('node:assert/strict');
const { Pline } = require('../dist/nodes/Pline/Pline.node.js');
const { PlineApi } = require('../dist/credentials/PlineApi.credentials.js');

const BASE = 'https://apix.pline.ai/v1';

/** Runs one node configuration; `replies` is a queue of {statusCode, body} the API answers with. */
async function run(params, replies, options = {}) {
	const calls = [];
	const ctx = {
		getInputData: () => [{ json: {} }],
		getNodeParameter: (name, _i, fallback) => (name in params ? params[name] : fallback),
		continueOnFail: () => options.continueOnFail ?? false,
		getNode: () => ({ name: 'Pline', type: 'n8n-nodes-pline.pline' }),
		helpers: {
			httpRequestWithAuthentication: async (_cred, req) => {
				calls.push(req);
				return replies.shift() ?? { statusCode: 200, body: {} };
			},
			httpRequest: async (req) => {
				calls.push(req);
				return replies.shift()?.body ?? '';
			},
		},
	};
	const out = await new Pline().execute.call(ctx);
	return { calls, items: out[0].map((i) => i.json) };
}

(async () => {
	// Scrape: snake_case body, tri-state js_render, tags split, default-only fields omitted.
	let r = await run(
		{
			resource: 'scrape',
			operation: 'scrape',
			url: ' https://example.com/pricing ',
			output: ['markdown', 'json'],
			options: {
				jsRender: 'true',
				proxyStrategy: 'basic',
				tag: 'a, b',
				prompt: 'plans',
				geolocation: 'us',
				onlyMainContent: false,
			},
		},
		[{ statusCode: 200, body: { data: { markdown: '# Hi' } } }],
	);
	assert.equal(r.calls[0].method, 'POST');
	assert.equal(r.calls[0].url, `${BASE}/scrape`);
	assert.deepEqual(r.calls[0].body, {
		url: 'https://example.com/pricing',
		output: ['markdown', 'json'],
		js_render: true,
		proxy_strategy: 'basic',
		only_main_content: false,
		geolocation: 'US',
		tag: ['a', 'b'],
		prompt: 'plans',
	});
	assert.deepEqual(r.items, [{ data: { markdown: '# Hi' } }]);

	// Local validation fires before any request is sent.
	for (const [options, output, needle] of [
		[{}, ['json'], 'Prompt and/or a Schema'],
		[{ jsRender: 'false' }, ['screenshot'], 'Forbid a Browser'],
		[{ body: '{"a":1}' }, ['html'], 'POST, PUT or DELETE'],
		[{ actions: 'nope' }, ['html'], 'valid JSON'],
	]) {
		await assert.rejects(
			run({ resource: 'scrape', operation: 'scrape', url: 'https://x.io', output, options }, []),
			(e) => e.message.includes(needle),
			needle,
		);
	}

	// Errors carry the API's code and message plus a hint; continueOnFail turns them into items.
	await assert.rejects(
		run(
			{
				resource: 'scrape',
				operation: 'scrape',
				url: 'https://x.io',
				output: ['html'],
				options: {},
			},
			[
				{
					statusCode: 403,
					body: { code: 'API_KEY_NOT_AUTHORIZED', message: 'The API key is not authorized.' },
				},
			],
		),
		(e) =>
			e.message.includes('403') &&
			e.message.includes('API_KEY_NOT_AUTHORIZED') &&
			e.description.includes('workspace status'),
	);
	r = await run(
		{ resource: 'scrape', operation: 'scrape', url: 'https://x.io', output: ['html'], options: {} },
		[{ statusCode: 429, body: { detail: 'Session busy' } }],
		{ continueOnFail: true },
	);
	assert.match(r.items[0].error, /429.*Session busy/);

	// Batch start: urls split on newlines, delivery fields camelCase, shared options snake_case.
	r = await run(
		{
			resource: 'batchScrape',
			operation: 'start',
			urls: 'https://a.io/1\nhttps://a.io/2, https://a.io/3',
			output: ['markdown'],
			options: {
				maxUrls: 50,
				email: 'me@x.io',
				webhookUrl: 'https://hook.io/p',
				webhookEvents: ['completed'],
				jsRender: 'false',
				sessionId: 's1',
			},
		},
		[{ statusCode: 202, body: { id: 'job' } }],
	);
	assert.equal(r.calls[0].url, `${BASE}/batch/scrape`);
	assert.deepEqual(r.calls[0].body, {
		output: ['markdown'],
		js_render: false,
		session_id: 's1',
		urls: ['https://a.io/1', 'https://a.io/2', 'https://a.io/3'],
		limit: 50,
		email: 'me@x.io',
		webhook: { url: 'https://hook.io/p', events: ['completed'] },
	});

	// Crawl start: camelCase top level, snake_case nested scrape, output is a list.
	r = await run(
		{
			resource: 'crawl',
			operation: 'start',
			url: 'https://docs.io',
			maxPages: 20,
			output: ['markdown', 'links'],
			crawlOptions: {
				includePaths: '^/docs/',
				crawlEntireDomain: true,
				ignoreRobotsTxt: false,
				sitemap: 'only',
			},
			pageOptions: { proxyStrategy: 'premium', waitSelector: 'main', timeout: 45000 },
		},
		[{ statusCode: 202, body: { id: 'job' } }],
	);
	assert.deepEqual(r.calls[0].body, {
		url: 'https://docs.io',
		limit: 20,
		output: ['markdown', 'links'],
		includePaths: ['^/docs/'],
		sitemap: 'only',
		ignoreRobotsTxt: false,
		crawlEntireDomain: true,
		scrape: { proxy_strategy: 'premium', timeout: 45000, wait_selector: 'main' },
	});

	// Job status: polls until terminal, rejects non-UUID ids, cancel uses DELETE.
	r = await run(
		{
			resource: 'crawl',
			operation: 'getStatus',
			jobId: '5f0d6c1e-3a52-4b6e-9c40-1c7f2b8a9d11',
			wait: true,
			maxWait: 1,
		},
		[{ statusCode: 200, body: { status: 'completed', s3PresignedUrls: {} } }],
	);
	assert.equal(r.calls[0].url, `${BASE}/crawl/5f0d6c1e-3a52-4b6e-9c40-1c7f2b8a9d11`);
	assert.equal(r.items[0].status, 'completed');
	await assert.rejects(run({ resource: 'crawl', operation: 'cancel', jobId: 'nope' }, []), /UUID/);
	r = await run(
		{ resource: 'batchScrape', operation: 'cancel', jobId: '5f0d6c1e-3a52-4b6e-9c40-1c7f2b8a9d11' },
		[{ statusCode: 200, body: { status: 'cancelled' } }],
	);
	assert.equal(r.calls[0].method, 'DELETE');

	// Batch results: JSONL per format merged by index into one item per URL; S3 fetch is unauthenticated.
	r = await run(
		{
			resource: 'batchScrape',
			operation: 'getResults',
			jobId: '5f0d6c1e-3a52-4b6e-9c40-1c7f2b8a9d11',
			wait: false,
		},
		[
			{
				statusCode: 200,
				body: {
					id: 'j',
					status: 'completed',
					s3PresignedUrls: { markdown: 'https://s3/m', links: 'https://s3/l' },
				},
			},
			{
				body: '{"index":1,"url":"https://a/2","success":true,"status":200,"finalUrl":"https://a/2","output":"# Two","error":null}\n{"index":0,"url":"https://a/1","success":true,"status":200,"finalUrl":"https://a/1","output":"# One","error":null}\n',
			},
			{
				body: '{"index":0,"url":"https://a/1","success":true,"status":200,"finalUrl":"https://a/1","output":["https://a/x"],"error":null}\n',
			},
		],
	);
	assert.equal(r.calls[1].url, 'https://s3/m');
	assert.equal(r.calls[1].headers, undefined);
	assert.deepEqual(
		r.items.map((i) => [i.index, i.url, i.markdown, i.links]),
		[
			[0, 'https://a/1', '# One', ['https://a/x']],
			[1, 'https://a/2', '# Two', undefined],
		],
	);
	await assert.rejects(
		run(
			{
				resource: 'batchScrape',
				operation: 'getResults',
				jobId: '5f0d6c1e-3a52-4b6e-9c40-1c7f2b8a9d11',
			},
			[{ statusCode: 200, body: { status: 'scraping' } }],
		),
		/not completed/,
	);

	// Map and search: split into items by default; serp uses snake_case query params.
	r = await run(
		{
			resource: 'map',
			operation: 'map',
			url: 'https://x.io',
			split: true,
			options: { maxLinks: 5, includeSubdomains: false },
		},
		[
			{
				statusCode: 200,
				body: { success: true, links: [{ url: 'https://x.io/a' }, { url: 'https://x.io/b' }] },
			},
		],
	);
	assert.deepEqual(r.calls[0].body, { url: 'https://x.io', includeSubdomains: false, limit: 5 });
	assert.equal(r.items.length, 2);
	r = await run(
		{
			resource: 'search',
			operation: 'search',
			query: 'pizza',
			split: false,
			options: { mode: 'fast', numResults: 3, output: ['links', 'html'], onlyMainContent: false },
		},
		[{ statusCode: 200, body: { organic: [{ position: 1 }], creditsUsed: 1 } }],
	);
	assert.equal(r.calls[0].method, 'GET');
	assert.equal(r.calls[0].url, `${BASE}/serp`);
	assert.deepEqual(r.calls[0].qs, {
		query: 'pizza',
		num_results: 3,
		mode: 'fast',
		output: 'links,html',
		only_main_content: false,
	});
	assert.equal(r.items[0].creditsUsed, 1);

	// Description and credential sanity.
	const desc = new Pline().description;
	assert.equal(desc.credentials[0].name, new PlineApi().name);
	for (const p of desc.properties) {
		if (p.type === 'collection') {
			const names = p.options.map((o) => o.displayName);
			assert.deepEqual(
				names,
				[...names].sort((a, b) => a.localeCompare(b)),
				`${p.name} options sorted`,
			);
		}
	}
	const cred = new PlineApi();
	assert.equal(cred.properties[0].typeOptions.password, true);
	assert.equal(cred.authenticate.properties.headers['x-api-key'], '={{$credentials.apiKey}}');
	assert.equal(cred.test.request.ignoreHttpStatusErrors, true);

	console.log('smoke test passed');
})().catch((e) => {
	console.error(e);
	process.exit(1);
});
