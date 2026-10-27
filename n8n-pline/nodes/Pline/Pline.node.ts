import type {
	IDataObject,
	IExecuteFunctions,
	INodeExecutionData,
	INodeType,
	INodeTypeDescription,
	JsonObject,
} from 'n8n-workflow';
import { NodeApiError, NodeConnectionTypes, NodeOperationError } from 'n8n-workflow';

import { properties } from './properties';
import { assertUuid, downloadText, getJob, plineRequest } from './transport';

type Options = Record<string, unknown>;

/** Parses a JSON-typed parameter; n8n hands it over as a string or an already-parsed value. */
function parseJson(
	this: IExecuteFunctions,
	value: unknown,
	name: string,
	itemIndex: number,
): unknown {
	if (value === '' || value === undefined || value === null) return undefined;
	if (typeof value !== 'string') return value;
	try {
		return JSON.parse(value);
	} catch {
		throw new NodeOperationError(this.getNode(), `${name} must be valid JSON`, { itemIndex });
	}
}

function list(value: unknown): string[] {
	const parts = Array.isArray(value) ? value.map(String) : String(value ?? '').split(/[\n,]/);
	return parts.map((p) => p.trim()).filter(Boolean);
}

function triState(value: unknown): boolean | undefined {
	return value === 'true' ? true : value === 'false' ? false : undefined;
}

/** Shared /scrape fields, snake_case, as accepted by /scrape, /batch/scrape and crawl's `scrape`. */
function fetchBody(
	this: IExecuteFunctions,
	o: Options,
	output: string[],
	itemIndex: number,
): IDataObject {
	const body: IDataObject = {};
	if (output.length) body.output = output;
	const jsRender = triState(o.jsRender);
	if (jsRender !== undefined) body.js_render = jsRender;
	if (o.proxyStrategy && o.proxyStrategy !== 'auto') body.proxy_strategy = o.proxyStrategy;
	if (o.onlyMainContent !== undefined) body.only_main_content = o.onlyMainContent;
	if (o.geolocation) body.geolocation = String(o.geolocation).trim().toUpperCase();
	if (o.tag) body.tag = list(o.tag);
	if (o.timeout !== undefined) body.timeout = o.timeout;
	if (o.waitSelector) body.wait_selector = o.waitSelector;
	if (o.waitMs !== undefined) body.wait_ms = o.waitMs;
	const actions = parseJson.call(this, o.actions, 'Actions', itemIndex);
	if (actions !== undefined) body.actions = actions as IDataObject[];
	if (o.method && o.method !== 'GET') body.method = o.method;
	const reqBody = parseJson.call(this, o.body, 'Body', itemIndex);
	if (reqBody !== undefined) body.body = reqBody as IDataObject;
	if (o.prompt) body.prompt = o.prompt;
	const schema = parseJson.call(this, o.schema, 'Schema', itemIndex);
	if (schema !== undefined) body.schema = schema as IDataObject;
	if (o.sessionId) body.session_id = o.sessionId;

	// Mirror the API's validation so bad combinations fail before a paid request.
	const browser =
		body.actions !== undefined ||
		!!body.wait_selector ||
		(typeof body.wait_ms === 'number' && body.wait_ms > 0) ||
		output.some((f) => f.startsWith('screenshot'));
	const fail = (message: string) => new NodeOperationError(this.getNode(), message, { itemIndex });
	if (output.includes('json') && !body.prompt && body.schema === undefined) {
		throw fail('JSON output needs a Prompt and/or a Schema');
	}
	if (output.includes('screenshot') && output.includes('screenshot_full_page')) {
		throw fail('Choose either Screenshot or Screenshot (Full Page), not both');
	}
	if (jsRender === false && browser) {
		throw fail(
			'Forbid a Browser cannot be combined with actions, wait options or screenshot output',
		);
	}
	if (browser && body.method)
		throw fail('Actions, wait options and screenshots support the GET method only');
	if (body.body !== undefined && (!body.method || browser)) {
		throw fail('Body is only allowed with POST, PUT or DELETE and without a browser');
	}
	return body;
}

function webhook(this: IExecuteFunctions, o: Options, itemIndex: number): IDataObject | undefined {
	if (!o.webhookUrl) return undefined;
	const hook: IDataObject = {
		url: String(o.webhookUrl).trim(),
		events: (o.webhookEvents as string[]) ?? [],
	};
	const headers = parseJson.call(this, o.webhookHeaders, 'Webhook Headers', itemIndex);
	if (headers !== undefined) hook.headers = headers as IDataObject;
	const metadata = parseJson.call(this, o.webhookMetadata, 'Webhook Metadata', itemIndex);
	if (metadata !== undefined) hook.metadata = metadata as IDataObject;
	return hook;
}

function jobFields(this: IExecuteFunctions, o: Options, itemIndex: number): IDataObject {
	const body: IDataObject = {};
	if (o.email) body.email = String(o.email).trim();
	if (o.scheduleToCloseTimeoutSecs !== undefined)
		body.scheduleToCloseTimeoutSecs = o.scheduleToCloseTimeoutSecs;
	if (o.heartbeatTimeoutSecs !== undefined) body.heartbeatTimeoutSecs = o.heartbeatTimeoutSecs;
	const hook = webhook.call(this, o, itemIndex);
	if (hook) body.webhook = hook;
	return body;
}

/** Turns a completed batch job's JSONL artifacts into one object per URL, outputs keyed by format. */
async function batchResults(this: IExecuteFunctions, job: IDataObject): Promise<IDataObject[]> {
	const urls = (job.s3PresignedUrls ?? {}) as Record<string, string>;
	const rows = new Map<number, IDataObject>();
	for (const [format, url] of Object.entries(urls)) {
		const text = await downloadText.call(this, url);
		for (const line of text.split('\n')) {
			if (!line.trim()) continue;
			const { index, output, ...rest } = JSON.parse(line) as IDataObject & { index: number };
			const row = rows.get(index) ?? { index, ...rest, jobId: job.id };
			row[format] = output;
			rows.set(index, row);
		}
	}
	return [...rows.values()].sort((a, b) => (a.index as number) - (b.index as number));
}

export class Pline implements INodeType {
	description: INodeTypeDescription = {
		displayName: 'Pline',
		name: 'pline',
		icon: { light: 'file:pline.svg', dark: 'file:pline.svg' },
		group: ['input'],
		version: 1,
		subtitle: '={{$parameter["operation"] + ": " + $parameter["resource"]}}',
		description: 'Scrape, batch-scrape, crawl, map and search the web with the Pline API',
		defaults: { name: 'Pline' },
		inputs: [NodeConnectionTypes.Main],
		outputs: [NodeConnectionTypes.Main],
		usableAsTool: true,
		credentials: [{ name: 'plineApi', required: true }],
		properties,
	};

	async execute(this: IExecuteFunctions): Promise<INodeExecutionData[][]> {
		const items = this.getInputData();
		const returnData: INodeExecutionData[] = [];
		const resource = this.getNodeParameter('resource', 0) as string;
		const operation = this.getNodeParameter('operation', 0) as string;

		for (let i = 0; i < items.length; i++) {
			try {
				const results = await runOperation.call(this, resource, operation, i);
				for (const json of results) returnData.push({ json, pairedItem: { item: i } });
			} catch (error) {
				if (this.continueOnFail()) {
					returnData.push({ json: { error: error.message }, pairedItem: { item: i } });
					continue;
				}
				// Both constructors hand back an error that is already wrapped, so context is kept.
				if (error instanceof NodeOperationError) {
					throw new NodeOperationError(this.getNode(), error, { itemIndex: i });
				}
				throw new NodeApiError(this.getNode(), error as JsonObject, { itemIndex: i });
			}
		}
		return [returnData];
	}
}

async function runOperation(
	this: IExecuteFunctions,
	resource: string,
	operation: string,
	i: number,
): Promise<IDataObject[]> {
	const param = (name: string, fallback?: unknown) => this.getNodeParameter(name, i, fallback);
	const jobPath = resource === 'crawl' ? '/crawl' : '/batch/scrape';

	if (resource === 'scrape') {
		const body = fetchBody.call(
			this,
			param('options', {}) as Options,
			param('output', []) as string[],
			i,
		);
		body.url = String(param('url')).trim();
		return [await plineRequest.call(this, 'POST', '/scrape', body, undefined, i)];
	}

	if (resource === 'map') {
		const o = param('options', {}) as Options;
		const body: IDataObject = { url: String(param('url')).trim() };
		if (o.sitemap) body.sitemap = o.sitemap;
		if (o.includeSubdomains !== undefined) body.includeSubdomains = o.includeSubdomains;
		if (o.ignoreQueryParameters !== undefined) body.ignoreQueryParameters = o.ignoreQueryParameters;
		if (o.ignoreCache !== undefined) body.ignoreCache = o.ignoreCache;
		if (o.maxLinks !== undefined) body.limit = o.maxLinks;
		if (o.timeout !== undefined) body.timeout = o.timeout;
		const response = await plineRequest.call(this, 'POST', '/map', body, undefined, i);
		return param('split', true) ? ((response.links as IDataObject[]) ?? []) : [response];
	}

	if (resource === 'search') {
		const o = param('options', {}) as Options;
		const qs: IDataObject = { query: String(param('query')).trim() };
		if (o.country) qs.country = o.country;
		if (o.language) qs.language = o.language;
		if (o.numResults !== undefined) qs.num_results = o.numResults;
		if (o.tbs) qs.tbs = o.tbs;
		if (o.device) qs.device = o.device;
		if (o.mode) qs.mode = o.mode;
		if (Array.isArray(o.output) && o.output.length) qs.output = o.output.join(',');
		if (o.onlyMainContent !== undefined) qs.only_main_content = o.onlyMainContent;
		const response = await plineRequest.call(this, 'GET', '/serp', undefined, qs, i);
		return param('split', true) ? ((response.organic as IDataObject[]) ?? []) : [response];
	}

	// Batch Scrape and Crawl share the job operations.
	if (operation === 'getStatus' || operation === 'getResults') {
		const id = assertUuid.call(this, String(param('jobId')), i);
		const wait = param('wait', false) ? (param('maxWait', 120) as number) : 0;
		const job = await getJob.call(this, `${jobPath}/${id}`, wait, i);
		if (operation === 'getStatus') return [job];
		if (job.status !== 'completed') {
			throw new NodeOperationError(
				this.getNode(),
				`Batch scrape job is ${job.status as string}, not completed; results are only available for completed jobs`,
				{ itemIndex: i },
			);
		}
		return await batchResults.call(this, job);
	}
	if (operation === 'cancel') {
		const id = assertUuid.call(this, String(param('jobId')), i);
		return [await plineRequest.call(this, 'DELETE', `${jobPath}/${id}`, undefined, undefined, i)];
	}

	if (resource === 'batchScrape') {
		const o = param('options', {}) as Options;
		const body = fetchBody.call(this, o, param('output', []) as string[], i);
		const urls = list(param('urls'));
		if (!urls.length)
			throw new NodeOperationError(this.getNode(), 'URLs must not be empty', { itemIndex: i });
		body.urls = urls;
		if (o.maxUrls !== undefined) body.limit = o.maxUrls;
		Object.assign(body, jobFields.call(this, o, i));
		return [await plineRequest.call(this, 'POST', '/batch/scrape', body, undefined, i)];
	}

	// crawl start
	const o = param('crawlOptions', {}) as Options;
	const body: IDataObject = {
		url: String(param('url')).trim(),
		limit: param('maxPages', 100),
		output: param('output', ['markdown']),
	};
	const includePaths = list(o.includePaths);
	if (includePaths.length) body.includePaths = includePaths;
	const excludePaths = list(o.excludePaths);
	if (excludePaths.length) body.excludePaths = excludePaths;
	if (o.maxDiscoveryDepth !== undefined) body.maxDiscoveryDepth = o.maxDiscoveryDepth;
	if (o.sitemap) body.sitemap = o.sitemap;
	for (const key of [
		'ignoreQueryParameters',
		'ignoreRobotsTxt',
		'crawlEntireDomain',
		'allowSubdomains',
	]) {
		if (o[key] !== undefined) body[key] = o[key] as boolean;
	}
	Object.assign(body, jobFields.call(this, o, i));
	const page = fetchBody.call(
		this,
		param('pageOptions', {}) as Options,
		body.output as string[],
		i,
	);
	delete page.output;
	if (Object.keys(page).length) body.scrape = page;
	return [await plineRequest.call(this, 'POST', '/crawl', body, undefined, i)];
}
