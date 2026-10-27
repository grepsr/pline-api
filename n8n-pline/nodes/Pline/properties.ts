import type { INodeProperties, INodePropertyOptions } from 'n8n-workflow';

const SCRAPE_OUTPUTS: INodePropertyOptions[] = [
	{
		name: 'Clean HTML',
		value: 'clean_html',
		description: 'HTML with scripts, styles and clutter removed',
	},
	{
		name: 'Clean Markdown',
		value: 'clean_markdown',
		description: 'Markdown with links, URLs and images removed',
	},
	{
		name: 'HTML',
		value: 'html',
		description: 'Raw HTML of the final page (returned as html_body)',
	},
	{
		name: 'JSON',
		value: 'json',
		description: 'AI extraction driven by the Prompt and/or Schema options',
	},
	{
		name: 'Links',
		value: 'links',
		description: 'Absolute, deduplicated links from the cleaned HTML',
	},
	{ name: 'Markdown', value: 'markdown', description: 'Markdown conversion, good LLM input' },
	{
		name: 'Screenshot',
		value: 'screenshot',
		description: 'Viewport screenshot as a temporary signed image URL',
	},
	{
		name: 'Screenshot (Full Page)',
		value: 'screenshot_full_page',
		description: 'Full-page screenshot as a temporary signed image URL',
	},
];

const CRAWL_OUTPUTS = SCRAPE_OUTPUTS.filter(
	(o) => !['clean_markdown', 'json'].includes(o.value as string),
);

const jsRender: INodeProperties = {
	displayName: 'JavaScript Rendering',
	name: 'jsRender',
	type: 'options',
	options: [
		{
			name: 'Let Pline Decide',
			value: 'auto',
			description:
				'A plain fetch first; a browser only when the page comes back empty, as a JavaScript shell, or blocked',
		},
		{
			name: 'Require a Browser',
			value: 'true',
			description:
				'Use a browser from the first attempt (costs more). For content known to be built client-side.',
		},
		{
			name: 'Forbid a Browser',
			value: 'false',
			description:
				'Cheapest and fastest; fails instead of escalating. Cannot be combined with actions, wait options or screenshots.',
		},
	],
	default: 'auto',
	description: 'Whether a browser may, must, or must not be used to fetch the page',
};

const proxyStrategy: INodeProperties = {
	displayName: 'Proxy Strategy',
	name: 'proxyStrategy',
	type: 'options',
	options: [
		{
			name: 'Auto',
			value: 'auto',
			description: 'Start cheap and escalate only as far as the site requires',
		},
		{
			name: 'Basic',
			value: 'basic',
			description: 'Cheapest tier only; fails rather than escalating',
		},
		{ name: 'Enhanced', value: 'enhanced', description: 'A stronger tier from the first attempt' },
		{
			name: 'Premium',
			value: 'premium',
			description: 'The strongest and most expensive tier from the first attempt',
		},
	],
	default: 'auto',
	description: 'Access-strength tier, in ascending cost',
};

/** Per-page fetch options shared by Scrape, Batch Scrape and (as a subset) Crawl. */
const fetchOptions: INodeProperties[] = [
	{
		displayName: 'Actions',
		name: 'actions',
		type: 'json',
		default: '',
		placeholder: '[{"action":"click","selector":"#more"},{"action":"wait","value":"1500"}]',
		description:
			'Browser steps run in order after the page loads, as a JSON array. Any action implies a browser and the GET method.',
	},
	{
		displayName: 'Geolocation',
		name: 'geolocation',
		type: 'string',
		default: '',
		placeholder: 'US',
		description: 'Fetch as a visitor from this 2-letter ISO country code',
	},
	jsRender,
	{
		displayName: 'Only Main Content',
		name: 'onlyMainContent',
		type: 'boolean',
		default: true,
		description:
			'Whether to strip navigation, footers and ads from the cleaned outputs (clean HTML, Markdown, links, JSON)',
	},
	proxyStrategy,
	{
		displayName: 'Tags',
		name: 'tag',
		type: 'string',
		default: '',
		placeholder: 'pricing, weekly',
		description:
			'Comma-separated labels for usage logs and request history. Up to 25 tags of 1-25 characters (letters, digits, period, hyphen, underscore).',
	},
	{
		displayName: 'Timeout (Ms)',
		name: 'timeout',
		type: 'number',
		default: 30000,
		typeOptions: { minValue: 1000, maxValue: 60000 },
		description: 'Per-request deadline in milliseconds (1000-60000)',
	},
	{
		displayName: 'Wait (Ms)',
		name: 'waitMs',
		type: 'number',
		default: 0,
		typeOptions: { minValue: 0, maxValue: 60000 },
		description:
			'Extra browser settle wait in milliseconds before returning (0-60000). Requires a browser.',
	},
	{
		displayName: 'Wait for Selector',
		name: 'waitSelector',
		type: 'string',
		default: '',
		placeholder: 'main',
		description:
			'CSS selector to wait for before returning a browser-rendered page. Requires a browser.',
	},
];

/** Options that only /scrape and /batch/scrape accept on top of fetchOptions. */
const scrapeOnlyOptions: INodeProperties[] = [
	{
		displayName: 'Body',
		name: 'body',
		type: 'json',
		default: '',
		description:
			'Request body for POST, PUT or DELETE targets, as a JSON object or a raw string. Not allowed with GET or with a browser.',
	},
	{
		displayName: 'Method',
		name: 'method',
		type: 'options',
		options: [
			{ name: 'DELETE', value: 'DELETE' },
			{ name: 'GET', value: 'GET' },
			{ name: 'POST', value: 'POST' },
			{ name: 'PUT', value: 'PUT' },
		],
		default: 'GET',
		description: 'HTTP method used to fetch the target. Browser-rendered requests are GET only.',
	},
	{
		displayName: 'Prompt',
		name: 'prompt',
		type: 'string',
		typeOptions: { rows: 3 },
		default: '',
		description:
			'Natural-language extraction instruction for the JSON output (max 16384 characters). JSON output needs a prompt and/or a schema.',
	},
	{
		displayName: 'Schema',
		name: 'schema',
		type: 'json',
		default: '',
		placeholder: '{"name":"string","price":"number"}',
		description:
			'Desired JSON shape for the JSON output; lenient, not strict JSON Schema. A list-shaped schema extracts a listing page and returns an array.',
	},
	{
		displayName: 'Session ID',
		name: 'sessionId',
		type: 'string',
		default: '',
		description:
			'Session ID returned by an earlier scrape of the same site. Reusing it keeps cookies and settled access choices, which is cheaper and faster.',
	},
];

const jobDeliveryOptions: INodeProperties[] = [
	{
		displayName: 'Email',
		name: 'email',
		type: 'string',
		placeholder: 'name@email.com',
		default: '',
		description: 'Address to notify once the job completes',
	},
	{
		displayName: 'Heartbeat Timeout (Seconds)',
		name: 'heartbeatTimeoutSecs',
		type: 'number',
		default: 30,
		description: 'Workflow heartbeat timeout in seconds',
	},
	{
		displayName: 'Schedule-to-Close Timeout (Seconds)',
		name: 'scheduleToCloseTimeoutSecs',
		type: 'number',
		default: 86400,
		description: 'Deadline for the whole job in seconds',
	},
	{
		displayName: 'Webhook Headers',
		name: 'webhookHeaders',
		type: 'json',
		default: '',
		placeholder: '{"authorization":"Bearer ..."}',
		description: 'Extra HTTP headers sent with every webhook call, as a JSON object',
	},
	{
		displayName: 'Webhook Metadata',
		name: 'webhookMetadata',
		type: 'json',
		default: '',
		description:
			'Arbitrary JSON echoed back unchanged in every webhook payload, for correlating callbacks',
	},
	{
		displayName: 'Webhook URL',
		name: 'webhookUrl',
		type: 'string',
		default: '',
		placeholder: 'https://example.com/hooks/pline',
		description:
			'Public HTTPS endpoint that receives job lifecycle events. Must not point at localhost or a private address.',
	},
];

function sorted(options: INodeProperties[]): INodeProperties[] {
	return [...options].sort((a, b) => a.displayName.localeCompare(b.displayName));
}

const show = (resource: string, operation?: string) => ({
	show: operation ? { resource: [resource], operation: [operation] } : { resource: [resource] },
});

const urlField = (resource: string, operation: string, description: string): INodeProperties => ({
	displayName: 'URL',
	name: 'url',
	type: 'string',
	required: true,
	default: '',
	placeholder: 'https://example.com/pricing',
	description,
	displayOptions: show(resource, operation),
});

const jobIdField = (resource: string, operations: string[]): INodeProperties => ({
	displayName: 'Job ID',
	name: 'jobId',
	type: 'string',
	required: true,
	default: '',
	placeholder: '5f0d6c1e-3a52-4b6e-9c40-1c7f2b8a9d11',
	description: 'The ID returned by the Start operation',
	displayOptions: { show: { resource: [resource], operation: operations } },
});

const waitFields = (resource: string, operations: string[]): INodeProperties[] => [
	{
		displayName: 'Wait for Completion',
		name: 'wait',
		type: 'boolean',
		default: false,
		description:
			'Whether to poll every 5 seconds until the job is completed, failed or cancelled, or the maximum wait passes. The job keeps running on Pline either way.',
		displayOptions: { show: { resource: [resource], operation: operations } },
	},
	{
		displayName: 'Maximum Wait (Seconds)',
		name: 'maxWait',
		type: 'number',
		default: 120,
		typeOptions: { minValue: 1, maxValue: 600 },
		description: 'Stop polling after this many seconds (max 600) and return the latest status',
		displayOptions: { show: { resource: [resource], operation: operations, wait: [true] } },
	},
];

export const properties: INodeProperties[] = [
	{
		displayName: 'Resource',
		name: 'resource',
		type: 'options',
		noDataExpression: true,
		options: [
			{
				name: 'Batch Scrape',
				value: 'batchScrape',
				description: 'Scrape a known list of URLs as one background job',
			},
			{
				name: 'Crawl',
				value: 'crawl',
				description: 'Discover and scrape a whole site by following links, as a background job',
			},
			{ name: 'Map', value: 'map', description: 'List the URLs of a site without scraping them' },
			{ name: 'Scrape', value: 'scrape', description: 'Fetch one page and return its content' },
			{ name: 'Search', value: 'search', description: 'Get parsed web search results' },
		],
		default: 'scrape',
	},

	// Scrape
	{
		displayName: 'Operation',
		name: 'operation',
		type: 'options',
		noDataExpression: true,
		displayOptions: show('scrape'),
		options: [
			{
				name: 'Scrape a Page',
				value: 'scrape',
				description: 'Fetch one URL and return its content in the chosen formats',
				action: 'Scrape a page',
			},
		],
		default: 'scrape',
	},
	urlField('scrape', 'scrape', 'Absolute http or https URL of the page to fetch'),
	{
		displayName: 'Output Formats',
		name: 'output',
		type: 'multiOptions',
		options: SCRAPE_OUTPUTS,
		default: ['markdown'],
		description:
			'Formats to return under <code>data</code>. Screenshot and full-page screenshot are mutually exclusive.',
		displayOptions: show('scrape', 'scrape'),
	},
	{
		displayName: 'Options',
		name: 'options',
		type: 'collection',
		placeholder: 'Add option',
		default: {},
		displayOptions: show('scrape', 'scrape'),
		options: sorted([...fetchOptions, ...scrapeOnlyOptions]),
	},

	// Batch Scrape
	{
		displayName: 'Operation',
		name: 'operation',
		type: 'options',
		noDataExpression: true,
		displayOptions: show('batchScrape'),
		options: [
			{
				name: 'Cancel',
				value: 'cancel',
				description: 'Cancel a running batch scrape job',
				action: 'Cancel a batch scrape',
			},
			{
				name: 'Get Results',
				value: 'getResults',
				description: "Download the finished job's results and return one item per URL",
				action: 'Get batch scrape results',
			},
			{
				name: 'Get Status',
				value: 'getStatus',
				description: "Get a batch scrape job's progress and, once completed, its download links",
				action: 'Get batch scrape status',
			},
			{
				name: 'Start',
				value: 'start',
				description: 'Scrape many URLs with the same options as one background job',
				action: 'Start a batch scrape',
			},
		],
		default: 'start',
	},
	{
		displayName: 'URLs',
		name: 'urls',
		type: 'string',
		required: true,
		default: '',
		typeOptions: { rows: 4 },
		placeholder: 'https://example.com/a\nhttps://example.com/b',
		description:
			'URLs to scrape, one per line or comma-separated. An expression may also return an array.',
		displayOptions: show('batchScrape', 'start'),
	},
	{
		displayName: 'Output Formats',
		name: 'output',
		type: 'multiOptions',
		options: SCRAPE_OUTPUTS,
		default: ['markdown'],
		description:
			'Formats to store for every URL. Screenshot and full-page screenshot are mutually exclusive.',
		displayOptions: show('batchScrape', 'start'),
	},
	{
		displayName: 'Options',
		name: 'options',
		type: 'collection',
		placeholder: 'Add option',
		default: {},
		displayOptions: show('batchScrape', 'start'),
		options: sorted([
			...fetchOptions,
			...scrapeOnlyOptions,
			...jobDeliveryOptions,
			{
				displayName: 'Maximum URLs',
				name: 'maxUrls',
				type: 'number',
				default: 10000,
				typeOptions: { minValue: 1, maxValue: 10000 },
				description: 'Maximum URLs this job may process (1-10000); the URL list must not be longer',
			},
			{
				displayName: 'Webhook Events',
				name: 'webhookEvents',
				type: 'multiOptions',
				options: [
					{ name: 'Completed', value: 'completed' },
					{ name: 'Failed', value: 'failed' },
					{ name: 'Started', value: 'started' },
				],
				default: [],
				description: 'Events sent to the webhook URL. Empty means all events.',
			},
		]),
	},
	jobIdField('batchScrape', ['getStatus', 'getResults', 'cancel']),
	...waitFields('batchScrape', ['getStatus', 'getResults']),

	// Crawl
	{
		displayName: 'Operation',
		name: 'operation',
		type: 'options',
		noDataExpression: true,
		displayOptions: show('crawl'),
		options: [
			{
				name: 'Cancel',
				value: 'cancel',
				description: 'Cancel a running crawl',
				action: 'Cancel a crawl',
			},
			{
				name: 'Get Status',
				value: 'getStatus',
				description: "Get a crawl's progress and, once completed, its download links",
				action: 'Get crawl status',
			},
			{
				name: 'Start',
				value: 'start',
				description: 'Discover and scrape pages by following links from a URL, as a background job',
				action: 'Start a crawl',
			},
		],
		default: 'start',
	},
	urlField('crawl', 'start', 'Seed URL; the crawl follows links from here'),
	{
		displayName: 'Maximum Pages',
		name: 'maxPages',
		type: 'number',
		default: 100,
		typeOptions: { minValue: 1, maxValue: 10000 },
		description:
			'Maximum pages to crawl (1-10000). An unscoped crawl of a large site burns credits fast; use Map first to size it.',
		displayOptions: show('crawl', 'start'),
	},
	{
		displayName: 'Output Formats',
		name: 'output',
		type: 'multiOptions',
		options: CRAWL_OUTPUTS,
		default: ['markdown'],
		description: 'Formats to store for every crawled page',
		displayOptions: show('crawl', 'start'),
	},
	{
		displayName: 'Crawl Options',
		name: 'crawlOptions',
		type: 'collection',
		placeholder: 'Add option',
		default: {},
		displayOptions: show('crawl', 'start'),
		options: sorted([
			...jobDeliveryOptions,
			{
				displayName: 'Allow Subdomains',
				name: 'allowSubdomains',
				type: 'boolean',
				default: false,
				description: "Whether to follow links on subdomains of the seed's domain",
			},
			{
				displayName: 'Crawl Entire Domain',
				name: 'crawlEntireDomain',
				type: 'boolean',
				default: false,
				description:
					"Whether to follow links anywhere on the domain instead of staying below the seed URL's path",
			},
			{
				displayName: 'Exclude Paths',
				name: 'excludePaths',
				type: 'string',
				default: '',
				placeholder: '^/blog/, \\.pdf$',
				description:
					'Comma-separated regular expressions; URLs whose path matches any of them are skipped',
			},
			{
				displayName: 'Ignore Query Parameters',
				name: 'ignoreQueryParameters',
				type: 'boolean',
				default: false,
				description: 'Whether to treat URLs that differ only by query string as duplicates',
			},
			{
				displayName: 'Ignore Robots.txt',
				name: 'ignoreRobotsTxt',
				type: 'boolean',
				default: true,
				description: 'Whether to skip robots.txt rules during URL discovery',
			},
			{
				displayName: 'Include Paths',
				name: 'includePaths',
				type: 'string',
				default: '',
				placeholder: '^/docs/',
				description:
					'Comma-separated regular expressions; only URLs whose path matches one of them are crawled',
			},
			{
				displayName: 'Maximum Discovery Depth',
				name: 'maxDiscoveryDepth',
				type: 'number',
				default: 3,
				typeOptions: { minValue: 0 },
				description: 'Maximum link-following depth from the seed URL',
			},
			{
				displayName: 'Sitemap',
				name: 'sitemap',
				type: 'options',
				options: [
					{ name: 'Include', value: 'include', description: 'Use sitemaps plus followed links' },
					{ name: 'Only', value: 'only', description: "Use the site's sitemaps alone" },
					{ name: 'Skip', value: 'skip', description: 'Ignore sitemaps' },
				],
				default: 'include',
				description: 'How sitemaps are used for URL discovery',
			},
			{
				displayName: 'Webhook Events',
				name: 'webhookEvents',
				type: 'multiOptions',
				options: [
					{ name: 'Completed', value: 'completed' },
					{ name: 'Failed', value: 'failed' },
					{ name: 'Page', value: 'page' },
					{ name: 'Started', value: 'started' },
				],
				default: [],
				description:
					'Events sent to the webhook URL. Empty means all events. Page fires once per completed page.',
			},
		]),
	},
	{
		displayName: 'Page Options',
		name: 'pageOptions',
		type: 'collection',
		placeholder: 'Add option',
		default: {},
		description: 'Fetch options applied to every page the crawl scrapes',
		displayOptions: show('crawl', 'start'),
		options: sorted(fetchOptions),
	},
	jobIdField('crawl', ['getStatus', 'cancel']),
	...waitFields('crawl', ['getStatus']),

	// Map
	{
		displayName: 'Operation',
		name: 'operation',
		type: 'options',
		noDataExpression: true,
		displayOptions: show('map'),
		options: [
			{
				name: 'Map a Site',
				value: 'map',
				description: "Discover a site's URLs from its sitemaps, with search as a fallback",
				action: 'Map a site',
			},
		],
		default: 'map',
	},
	urlField('map', 'map', 'Base URL of the site to discover URLs for'),
	{
		displayName: 'Split Into Items',
		name: 'split',
		type: 'boolean',
		default: true,
		description:
			'Whether to return one item per discovered link instead of the whole response as one item',
		displayOptions: show('map', 'map'),
	},
	{
		displayName: 'Options',
		name: 'options',
		type: 'collection',
		placeholder: 'Add option',
		default: {},
		displayOptions: show('map', 'map'),
		options: [
			{
				displayName: 'Ignore Cache',
				name: 'ignoreCache',
				type: 'boolean',
				default: false,
				description: 'Whether to bypass discovery caches and refresh',
			},
			{
				displayName: 'Ignore Query Parameters',
				name: 'ignoreQueryParameters',
				type: 'boolean',
				default: true,
				description: 'Whether to drop URLs that carry query strings',
			},
			{
				displayName: 'Include Subdomains',
				name: 'includeSubdomains',
				type: 'boolean',
				default: true,
				description: 'Whether to include subdomains of the registrable domain',
			},
			{
				displayName: 'Maximum Links',
				name: 'maxLinks',
				type: 'number',
				default: 1000,
				typeOptions: { minValue: 1, maxValue: 1000000 },
				description: 'Maximum links to return (1-1000000)',
			},
			{
				displayName: 'Sitemap',
				name: 'sitemap',
				type: 'options',
				options: [
					{
						name: 'Include',
						value: 'include',
						description: 'Sitemaps, with search as a fallback only when none exist',
					},
					{ name: 'Only', value: 'only', description: 'Sitemaps only' },
					{ name: 'Skip', value: 'skip', description: 'Search-based discovery only' },
				],
				default: 'include',
				description: 'How sitemaps are used for discovery',
			},
			{
				displayName: 'Timeout (Ms)',
				name: 'timeout',
				type: 'number',
				default: 60000,
				typeOptions: { minValue: 1000 },
				description: 'Whole-request budget in milliseconds; the API answers 504 when exceeded',
			},
		],
	},

	// Search
	{
		displayName: 'Operation',
		name: 'operation',
		type: 'options',
		noDataExpression: true,
		displayOptions: show('search'),
		options: [
			{
				name: 'Search',
				value: 'search',
				description:
					'Search the web and get parsed results. Costs credits; full mode costs more than fast mode.',
				action: 'Search the web',
			},
		],
		default: 'search',
	},
	{
		displayName: 'Query',
		name: 'query',
		type: 'string',
		required: true,
		default: '',
		placeholder: 'best pizza in SF',
		description: 'Search keywords (max 512 characters)',
		displayOptions: show('search', 'search'),
	},
	{
		displayName: 'Split Into Items',
		name: 'split',
		type: 'boolean',
		default: true,
		description:
			'Whether to return one item per organic result instead of the whole response as one item. Turn off to keep full-mode extras such as the knowledge graph and related searches.',
		displayOptions: show('search', 'search'),
	},
	{
		displayName: 'Options',
		name: 'options',
		type: 'collection',
		placeholder: 'Add option',
		default: {},
		displayOptions: show('search', 'search'),
		options: [
			{
				displayName: 'Country',
				name: 'country',
				type: 'string',
				default: 'all',
				placeholder: 'US',
				description: 'Localization: a 2-letter country code, or "all"',
			},
			{
				displayName: 'Device',
				name: 'device',
				type: 'options',
				options: [
					{ name: 'Desktop', value: 'desktop' },
					{ name: 'Mobile', value: 'mobile' },
					{ name: 'Tablet', value: 'tablet' },
				],
				default: 'desktop',
				description: 'Device profile for the search',
			},
			{
				displayName: 'Language',
				name: 'language',
				type: 'string',
				default: 'en',
				description: '2-letter result language code',
			},
			{
				displayName: 'Mode',
				name: 'mode',
				type: 'options',
				options: [
					{
						name: 'Fast',
						value: 'fast',
						description: 'Organic results and metadata; the cheaper mode',
					},
					{
						name: 'Full',
						value: 'full',
						description:
							'Adds knowledge graph, people-also-ask, related searches and sitelinks; costs more',
					},
				],
				default: 'full',
				description: 'How much of the results page to parse',
			},
			{
				displayName: 'Number of Results',
				name: 'numResults',
				type: 'number',
				default: 10,
				typeOptions: { minValue: 1, maxValue: 100 },
				description: 'Organic results to return (1-100)',
			},
			{
				displayName: 'Only Main Content',
				name: 'onlyMainContent',
				type: 'boolean',
				default: true,
				description:
					'Whether to strip navigation and layout from fetched result pages (with Page Content set)',
			},
			{
				displayName: 'Page Content',
				name: 'output',
				type: 'multiOptions',
				options: [
					{ name: 'Clean HTML', value: 'clean_html' },
					{ name: 'HTML', value: 'html' },
					{ name: 'Links', value: 'links' },
				],
				default: [],
				description:
					'Also fetch each result page in these formats. Best-effort, and one extra credit per page attempted.',
			},
			{
				displayName: 'Time Range',
				name: 'tbs',
				type: 'options',
				options: [
					{ name: 'Past Day', value: 'qdr:d' },
					{ name: 'Past Month', value: 'qdr:m' },
					{ name: 'Past Week', value: 'qdr:w' },
				],
				default: 'qdr:w',
				description: 'Restrict results to this recency',
			},
		],
	},
];
