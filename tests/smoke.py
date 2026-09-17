"""Exercise an installed skill and MCP stdio against a local API; no live credentials needed."""

import http.client
import http.server
import json
import os
from pathlib import Path
import queue
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
JOB_ID = '00000000-0000-0000-0000-000000000001'
BAD_IDS = ('../health', '%2e%2e/health', '..\\health', '/health',
           JOB_ID + '?x=1', JOB_ID + '#fragment', JOB_ID + '\n/health', 'invalid')


class MockAPI(http.server.BaseHTTPRequestHandler):
    requests = []

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        self.requests.append((self.command, self.path, self.headers.get('x-api-key'), body))
        if self.path == '/scrape':
            result = {'status': 200, 'session_id': 'smoke-session',
                      'data': {'markdown': '# Test page', 'json': {'name': 'Example'}}}
        elif self.path in ('/crawl', '/batch/scrape'):
            result = {'id': '00000000-0000-0000-0000-000000000001'}
        elif self.path == '/map':
            result = {'links': [{'url': 'https://example.com', 'type': 'page'}]}
        else:
            self.send_error(404)
            return
        self.respond(result)

    def do_GET(self):
        self.requests.append((self.command, self.path, self.headers.get('x-api-key'), None))
        if self.path.startswith('/serp?'):
            self.respond({'organic': [{'title': 'Example', 'link': 'https://example.com'}]})
        elif self.path in ('/crawl/' + JOB_ID, '/batch/scrape/' + JOB_ID):
            self.respond({'id': JOB_ID, 'status': 'completed', 's3PresignedUrls': {
                'markdown': f'http://127.0.0.1:{self.server.server_port}/artifacts/markdown.jsonl'}})
        else:
            self.respond({'status': 'completed'})

    def do_DELETE(self):
        self.requests.append((self.command, self.path, self.headers.get('x-api-key'), None))
        self.respond({'status': 'cancelled'})

    def respond(self, payload):
        body = json.dumps(payload).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


def check_skill(workdir, env):
    # Relocate the complete skill, without the repo, and launch from a different directory.
    installed = workdir / 'installed skills' / 'pline-api'
    shutil.copytree(ROOT / 'skills/pline-api', installed)
    script = installed / 'scripts/pline.py'
    caller = workdir / 'caller'
    caller.mkdir()

    def cli(*args, success=True):
        result = subprocess.run([sys.executable, str(script), *args], cwd=caller, env=env,
                                text=True, capture_output=True, timeout=10)
        assert (result.returncode == 0) == success, (result.stdout, result.stderr)
        return result.stdout

    for command in [[], ['scrape'], ['map'], ['serp'], ['crawl', 'start'],
                    ['crawl', 'status'], ['crawl', 'cancel'], ['batch', 'start'],
                    ['batch', 'status'], ['batch', 'cancel']]:
        cli(*command, '--help')
    (caller / 'schema.json').write_text('{"name":"string"}')
    cli('scrape', '--url', 'https://example.com', '--output', 'json',
        '--schema', '@schema.json', '--session-file', '.pline-session-example', '--save', 'output')
    assert MockAPI.requests[-1][3]['schema'] == {'name': 'string'}
    assert json.loads((caller / 'output/json.json').read_text()) == {'name': 'Example'}
    cli('scrape', '--url', 'https://example.com/next', '--session-file', '.pline-session-example')
    assert MockAPI.requests[-1][3]['session_id'] == 'smoke-session'
    (caller / 'urls.txt').write_text('https://example.com/a\nhttps://example.com/b\n')
    cli('batch', 'start', '--urls-file', 'urls.txt', '--js-render', '--raw')
    assert MockAPI.requests[-1][1] == '/batch/scrape'
    assert MockAPI.requests[-1][3]['urls'] == ['https://example.com/a', 'https://example.com/b']
    assert MockAPI.requests[-1][3]['jsRender'] is True
    actions = [{'action': 'click', 'selector': '#more'}]
    cli('crawl', 'start', '--url', 'https://example.com', '--limit', '2',
        '--scrape-options', json.dumps({'actions': actions}),
        '--include-path', '/item/[0-9]{2,4}$', '--include-path', '/a,b$',
        '--exclude-path', '/archive/[0-9]{1,3}$', '--raw')
    assert MockAPI.requests[-1][3]['scrape']['actions'] == actions
    assert MockAPI.requests[-1][3]['includePaths'] == ['/item/[0-9]{2,4}$', '/a,b$']
    assert MockAPI.requests[-1][3]['excludePaths'] == ['/archive/[0-9]{1,3}$']
    assert json.loads(cli('map', '--url', 'https://example.com', '--raw'))['links']
    assert json.loads(cli('serp', '--query', 'example', '--raw'))['organic']
    assert json.loads(cli('crawl', 'status', '00000000-0000-0000-0000-000000000001', '--raw'))['status'] == 'completed'
    for surface in ('crawl', 'batch'):
        for action in ('status', 'cancel'):
            for job_id in BAD_IDS:
                before = len(MockAPI.requests)
                cli(surface, action, job_id, '--raw', success=False)
                assert len(MockAPI.requests) == before, (surface, action, job_id)
            cli(surface, action, JOB_ID, '--raw')
            prefix = '/crawl/' if surface == 'crawl' else '/batch/scrape/'
            assert MockAPI.requests[-1][1] == prefix + JOB_ID
    assert (installed / 'references/endpoints.md').is_file()
    print('PASS: relocated skill, all CLI help, relative inputs/outputs, sessions, five API surfaces')


def mcp_binary():
    binary = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT / 'mcp-server/target/debug/pline-mcp'
    if os.name == 'nt' and not binary.suffix:
        binary = binary.with_suffix('.exe')
    assert binary.is_file(), 'Build the MCP first with cargo build --manifest-path mcp-server/Cargo.toml --locked'
    return binary


def check_mcp(workdir, env):
    proc = subprocess.Popen([str(mcp_binary()), '--stdio'], cwd=workdir, env=env,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    # Windows select() cannot watch subprocess pipes; a reader thread works on all runners.
    replies = queue.Queue()

    def read_stdout():
        for line in proc.stdout:
            replies.put(line)

    reader = threading.Thread(target=read_stdout, daemon=True)
    reader.start()

    def rpc(method, params, request_id=None):
        message = {'jsonrpc': '2.0', 'method': method, 'params': params}
        if request_id is not None:
            message['id'] = request_id
        proc.stdin.write(json.dumps(message) + '\n')
        proc.stdin.flush()
        if request_id is None:
            return None
        try:
            reply = json.loads(replies.get(timeout=10))
        except queue.Empty:
            raise AssertionError(f'MCP timeout: {method}') from None
        assert reply['id'] == request_id and 'error' not in reply, reply
        return reply['result']

    try:
        info = rpc('initialize', {'protocolVersion': '2025-03-26', 'capabilities': {},
                                 'clientInfo': {'name': 'smoke', 'version': '1'}}, 1)
        assert info['serverInfo']['name'] == 'pline.ai'
        rpc('notifications/initialized', {})
        tools = rpc('tools/list', {}, 2)['tools']
        assert len(tools) == 10
        for tool in tools:
            if tool['name'] in ('scrape', 'crawl_status', 'batch_scrape_status'):
                assert not tool.get('annotations', {}).get('readOnlyHint', False), tool['name']
        assert {r['uri'] for r in rpc('resources/list', {}, 3)['resources']} == {'pline://guide', 'pline://reference'}
        for index, uri in enumerate(('pline://guide', 'pline://reference'), 4):
            assert 'pline.ai' in rpc('resources/read', {'uri': uri}, index)['contents'][0]['text']
        result = rpc('tools/call', {'name': 'scrape', 'arguments': {
            'url': 'https://example.com', 'output': ['markdown']}}, 8)
        assert not result.get('isError'), result
        assert result['structuredContent']['data']['markdown'] == '# Test page'
        assert MockAPI.requests[-1][1] == '/scrape'
        result = rpc('tools/call', {'name': 'scrape', 'arguments': {
            'url': 'https://example.com', 'method': 'DELETE', 'output': ['markdown'],
            'save_dir': str(workdir / 'stdio-output')}}, 9)
        assert not result.get('isError'), result
        assert (workdir / 'stdio-output/markdown.md').read_text() == '# Test page'
        assert MockAPI.requests[-1][3]['method'] == 'DELETE'
        for tool in ('crawl_status', 'crawl_cancel', 'batch_scrape_status', 'batch_scrape_cancel'):
            for job_id in BAD_IDS:
                before = len(MockAPI.requests)
                result = rpc('tools/call', {'name': tool, 'arguments': {'id': job_id}}, 10)
                assert result.get('isError'), (tool, job_id, result)
                assert len(MockAPI.requests) == before, (tool, job_id)
            args = {'id': JOB_ID}
            if tool.endswith('_status'):
                args['download_dir'] = str(workdir / tool)
            result = rpc('tools/call', {'name': tool, 'arguments': args}, 11)
            assert not result.get('isError'), result
            if tool.endswith('_status'):
                assert (workdir / tool / 'markdown.jsonl').is_file()
        print('PASS: MCP stdio, annotations, UUID validation, local saves and downloads')
    finally:
        proc.stdin.close()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
        reader.join(timeout=5)
        proc.stdout.close()
        proc.stderr.close()



def check_http(workdir, env):
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    proc = subprocess.Popen([str(mcp_binary()), '--http', f'127.0.0.1:{port}'],
                            cwd=workdir, env={**env, 'PLINE_MCP_ALLOWED_HOSTS': ''},
                            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    session_id = None

    def request(method, payload=None, key='smoke-dummy-key', path='/mcp', headers=None):
        request_headers = {'Accept': 'application/json, text/event-stream',
                           'Content-Type': 'application/json',
                           'MCP-Protocol-Version': '2025-03-26'}
        if key:
            request_headers['x-api-key'] = key
        if session_id:
            request_headers['Mcp-Session-Id'] = session_id
        request_headers.update(headers or {})
        conn = http.client.HTTPConnection('127.0.0.1', port, timeout=10)
        try:
            conn.request(method, path, json.dumps(payload) if payload is not None else None,
                         request_headers)
            response = conn.getresponse()
            if response.getheader('Content-Type', '').startswith('text/event-stream'):
                while line := response.readline():
                    if line.startswith(b'data:') and line[5:].strip():
                        body = json.loads(line[5:])
                        break
                else:
                    body = None
            else:
                raw = response.read()
                try:
                    body = json.loads(raw)
                except ValueError:
                    body = raw.decode()
            return response.status, dict(response.getheaders()), body
        finally:
            conn.close()

    def rpc(method, params, **kwargs):
        status, _, body = request('POST', {'jsonrpc': '2.0', 'id': 1,
                                           'method': method, 'params': params}, **kwargs)
        assert status == 200 and 'error' not in body, (status, body)
        return body['result']

    try:
        for _ in range(100):
            try:
                if request('GET', path='/health', key=None)[0] == 200:
                    break
            except OSError:
                time.sleep(.05)
        else:
            raise AssertionError('HTTP server did not start')
        status, headers, body = request('POST', {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize',
            'params': {'protocolVersion': '2025-03-26', 'capabilities': {},
                       'clientInfo': {'name': 'smoke', 'version': '1'}}})
        assert status == 200, (status, body)
        session_id = headers.get('mcp-session-id')
        assert request('POST', {'jsonrpc': '2.0', 'method': 'notifications/initialized'})[0] == 202
        output = workdir / 'http-output'
        output.mkdir()
        for name in ('markdown.md', 'markdown.jsonl'):
            (output / name).write_text('keep existing content')
        for path in ('/mcp', '/smoke-dummy-key/mcp'):
            for tool, args in (
                ('scrape', {'url': 'https://example.com', 'output': ['markdown'], 'save_dir': str(output)}),
                ('crawl_status', {'id': JOB_ID, 'download_dir': str(output)}),
                ('batch_scrape_status', {'id': JOB_ID, 'download_dir': str(output)}),
            ):
                before = len(MockAPI.requests)
                result = rpc('tools/call', {'name': tool, 'arguments': args}, path=path,
                             key='smoke-dummy-key' if path == '/mcp' else None)
                assert result.get('isError'), (tool, result)
                assert len(MockAPI.requests) == before, tool
                assert all(p.read_text() == 'keep existing content' for p in output.iterdir())
        # No persistent HTTP sessions means no session takeover, replay, or deletion across keys.
        assert session_id is None, 'HTTP initialize must not allocate a shared transport session'
        for method in ('GET', 'DELETE'):
            assert request(method, key='other-dummy-key', headers={'Mcp-Session-Id': 'foreign-session'})[0] == 405
        assert request('POST', key=None)[0] == 401
        assert len(rpc('tools/list', {}, key=None,
                       headers={'Authorization': 'Bearer smoke-dummy-key'})['tools']) == 10
        assert len(rpc('resources/list', {})['resources']) == 2
        args = {'url': 'https://example.com', 'output': ['markdown']}
        result = rpc('tools/call', {'name': 'scrape', 'arguments': args})
        assert not result.get('isError'), result
        assert MockAPI.requests[-1][2] == 'smoke-dummy-key'
        assert not MockAPI.requests[-1][3].get('session_id')
        sessions = rpc('tools/call', {'name': 'sessions_list', 'arguments': {}}, key='other-dummy-key',
                       headers={'Mcp-Session-Id': 'foreign-session'})
        assert sessions['structuredContent'] == {}, sessions
        result = rpc('tools/call', {'name': 'scrape', 'arguments': args}, key='other-dummy-key')
        assert not result.get('isError'), result
        assert MockAPI.requests[-1][2] == 'other-dummy-key'
        assert not MockAPI.requests[-1][3].get('session_id')
        result = rpc('tools/call', {'name': 'scrape', 'arguments': args})
        assert not result.get('isError'), result
        assert MockAPI.requests[-1][3]['session_id'] == 'smoke-session'
        result = rpc('tools/call', {'name': 'crawl_status', 'arguments': {'id': JOB_ID}})
        assert not result.get('isError'), result
        print('PASS: HTTP file writes blocked, stateless transport, per-key session reuse, auth routes')
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
        proc.stderr.close()


def main():
    with http.server.ThreadingHTTPServer(('127.0.0.1', 0), MockAPI) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        env = {**os.environ, 'PLINE_BASE_URL': f'http://127.0.0.1:{server.server_port}',
               'PLINE_API_KEY': 'smoke-dummy-key', 'PLINE_MCP_HTTP_ADDR': '127.0.0.1:1',
               'NO_PROXY': '127.0.0.1', 'no_proxy': '127.0.0.1'}
        try:
            with tempfile.TemporaryDirectory(prefix='pline-smoke-') as temp:
                check_skill(Path(temp), env)
                check_mcp(Path(temp), env)
                check_http(Path(temp), env)
            assert MockAPI.requests and all(r[2] in ('smoke-dummy-key', 'other-dummy-key')
                                          for r in MockAPI.requests if not r[1].startswith('/artifacts/'))
            print('PASS: both clients forward the user API key to the configured API')
        finally:
            server.shutdown()
            thread.join(timeout=5)


if __name__ == '__main__':
    main()
