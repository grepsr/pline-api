import type { IDataObject, IExecuteFunctions, IHttpRequestMethods, JsonObject } from 'n8n-workflow';
import { NodeApiError, NodeOperationError, sleep } from 'n8n-workflow';

export const BASE_URL = 'https://apix.pline.ai/v1';

/** Per-request budget: the API's own scrape budget is ~300s. */
const REQUEST_TIMEOUT_MS = 360_000;
const POLL_INTERVAL_MS = 5_000;
export const MAX_WAIT_SECONDS = 600;
const TERMINAL_JOB_STATUSES = ['completed', 'failed', 'cancelled'];

/** What each status means for the user, in front of the API's own message. */
const STATUS_HINTS: Record<number, string> = {
	400: 'Invalid request, or a session ID reused on a different domain.',
	401: 'No API key was sent, or the key was revoked. Check the Pline credential.',
	403: 'The API key is not authorized. Check the key and your workspace status at https://app.pline.ai/login.',
	404: 'No job with this ID.',
	409: 'This job has already finished and cannot be cancelled.',
	422: 'The request failed validation. The message names the field or combination the API rejected.',
	429: 'Rate limit reached, session busy, or the workspace is out of credits. Read the message, then retry after a short delay or top up at https://app.pline.ai/login.',
	500: 'Unexpected error at Pline. Quote the request ID when reporting it.',
	502: 'Every attempt to fetch the target failed, timed out, or was blocked.',
	503: 'A Pline backend is temporarily unavailable. Retry shortly.',
	504: 'The request exceeded its time budget.',
};

interface ApiErrorBody {
	code?: string;
	message?: string;
	detail?: string;
	request_id?: string;
}

function apiMessage(body: unknown): string {
	if (typeof body === 'string') return body;
	const b = (body ?? {}) as ApiErrorBody;
	const text = b.message ?? b.detail;
	if (!text) return JSON.stringify(body ?? '');
	return b.code ? `${b.code}: ${text}` : text;
}

/**
 * Calls the Pline API with the node's credential. Non-2xx responses become a NodeApiError that
 * shows the API's own message plus a hint for the status code.
 */
export async function plineRequest(
	this: IExecuteFunctions,
	method: IHttpRequestMethods,
	path: string,
	body?: IDataObject,
	qs?: IDataObject,
	itemIndex = 0,
): Promise<IDataObject> {
	const response = await this.helpers.httpRequestWithAuthentication.call(this, 'plineApi', {
		method,
		url: `${BASE_URL}${path}`,
		body,
		qs,
		json: true,
		timeout: REQUEST_TIMEOUT_MS,
		ignoreHttpStatusErrors: true,
		returnFullResponse: true,
	});
	const status = response.statusCode as number;
	if (status >= 200 && status < 300) {
		return (response.body ?? {}) as IDataObject;
	}
	const message = apiMessage(response.body);
	const requestId = (response.body as ApiErrorBody | undefined)?.request_id;
	throw new NodeApiError(this.getNode(), (response.body ?? {}) as JsonObject, {
		httpCode: String(status),
		message: `Pline API error ${status}: ${message}`,
		description: [STATUS_HINTS[status], requestId ? `Request ID: ${requestId}` : undefined]
			.filter(Boolean)
			.join(' '),
		itemIndex,
	});
}

/** GET a job, polling every 5s until it is terminal or `waitSeconds` pass (0 = one GET). */
export async function getJob(
	this: IExecuteFunctions,
	path: string,
	waitSeconds: number,
	itemIndex: number,
): Promise<IDataObject> {
	const deadline = Date.now() + Math.min(Math.max(waitSeconds, 0), MAX_WAIT_SECONDS) * 1000;
	for (;;) {
		const job = await plineRequest.call(this, 'GET', path, undefined, undefined, itemIndex);
		const remaining = deadline - Date.now();
		if (TERMINAL_JOB_STATUSES.includes(job.status as string) || remaining <= 0) return job;
		await sleep(Math.min(POLL_INTERVAL_MS, remaining));
	}
}

/** Downloads a presigned artifact URL. Unauthenticated on purpose: the API key must not go to S3. */
export async function downloadText(this: IExecuteFunctions, url: string): Promise<string> {
	const text = await this.helpers.httpRequest({
		method: 'GET',
		url,
		json: false,
		timeout: 600_000,
	});
	return typeof text === 'string' ? text : String(text);
}

export function assertUuid(this: IExecuteFunctions, id: string, itemIndex: number): string {
	const trimmed = id.trim();
	if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(trimmed)) {
		throw new NodeOperationError(
			this.getNode(),
			'Job ID must be a UUID returned by a Start operation',
			{
				itemIndex,
			},
		);
	}
	return trimmed;
}
