import type {
	IAuthenticateGeneric,
	ICredentialTestRequest,
	ICredentialType,
	Icon,
	INodeProperties,
} from 'n8n-workflow';

export class PlineApi implements ICredentialType {
	name = 'plineApi';

	displayName = 'Pline API';

	icon: Icon = { light: 'file:../nodes/Pline/pline.svg', dark: 'file:../nodes/Pline/pline.svg' };

	documentationUrl = 'https://github.com/grepsr/pline-api/tree/main/n8n-pline#credentials';

	properties: INodeProperties[] = [
		{
			displayName: 'API Key',
			name: 'apiKey',
			type: 'string',
			typeOptions: { password: true },
			required: true,
			default: '',
			description:
				'Your own Pline API key. Pline is a paid service: <a href="https://app.pline.ai/signup" target="_blank">sign up</a>, then <a href="https://app.pline.ai/login" target="_blank">sign in</a> and copy the key from the API keys page. The key is sent as the X-API-Key header.',
		},
	];

	authenticate: IAuthenticateGeneric = {
		type: 'generic',
		properties: {
			headers: {
				'x-api-key': '={{$credentials.apiKey}}',
			},
		},
	};

	/**
	 * Spends no credits: a status lookup for a job that cannot exist. A valid key gets 404 (unknown
	 * job), which counts as success; a missing or invalid key gets a 401/403 body whose `code` the
	 * rules below turn into a clear message.
	 */
	test: ICredentialTestRequest = {
		request: {
			baseURL: 'https://apix.pline.ai/v1',
			url: '/crawl/00000000-0000-0000-0000-000000000000',
			method: 'GET',
			ignoreHttpStatusErrors: true,
		},
		rules: [
			{
				type: 'responseSuccessBody',
				properties: {
					key: 'code',
					value: 'API_KEY_NOT_AUTHORIZED',
					message:
						'The API key is not authorized. Check the key and your workspace status at https://app.pline.ai/login.',
				},
			},
			{
				type: 'responseSuccessBody',
				properties: {
					key: 'code',
					value: 'UNAUTHORIZED',
					message: 'No API key was sent, or the key was revoked. Enter a valid Pline API key.',
				},
			},
		],
	};
}
