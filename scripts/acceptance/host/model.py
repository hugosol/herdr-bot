"""Configure the explicitly authorized model and record its actual traffic."""
import hashlib
import json
import os
import urllib.parse

from .processes import require
from .wire import WireProxy


def configure_model(self):
    # Explicit acceptance-only configuration, not the user's agent state.
    require(self.args.model_config.is_file(), ('Missing isolated actual-model configuration', str(self.args.model_config)))
    config = json.loads(self.args.model_config.read_text(encoding='utf-8'))
    for key in ['provider', 'model', 'baseUrl', 'apiKeyEnv']:
        require(isinstance(config.get(key), str) and bool(config[key]), f'model config requires {key}')
    require(config.get('api', 'openai-completions') == 'openai-completions',
            'This recorder currently supports actual OpenAI-compatible chat-completions models only')
    parsed = urllib.parse.urlparse(config['baseUrl'])
    require(parsed.scheme in ['http', 'https'] and parsed.hostname and not parsed.username and not parsed.password and not parsed.query,
            'baseUrl must be an HTTP(S) service URL without inline credentials or query')
    credential = os.environ.get(config['apiKeyEnv'])
    require(credential, f"Missing actual model credential environment variable {config['apiKeyEnv']}")
    self.model_wire = WireProxy(config['baseUrl'], self.root / 'model-wire')
    self.provider_model = config['provider'] + '/' + config['model']
    self.evidence['model'] = {key: config[key] for key in ['provider', 'model', 'baseUrl']}
    self.evidence['model']['credentialEnvironmentName'] = config['apiKeyEnv']
    self.evidence['model']['credentialFingerprint'] = hashlib.sha256(credential.encode()).hexdigest()[:12]
    # Remove even a nonstandard credential variable from the inherited child environment.
    self.env.pop(config['apiKeyEnv'], None)
    agent = self.root / 'agent'
    settings = {'sessionDir': str(agent / 'sessions')}
    if self.args.shell_path:
        shell = self.args.shell_path.resolve()
        require(shell.is_file(), ('Missing explicit native shell', str(shell)))
        settings['shellPath'] = str(shell)
        self.evidence['nativeShell'] = str(shell)
    (agent / 'settings.json').write_text(json.dumps(settings), encoding='utf-8')
    (agent / 'models.json').write_text(json.dumps({'providers': {config['provider']: {
        'baseUrl': self.model_wire.base, 'api': 'openai-completions', 'apiKey': credential,
        'models': [{'id': config['model'], 'name': 'Actual role acceptance model',
            'reasoning': config.get('reasoning', False), 'input': ['text'],
            'contextWindow': config.get('contextWindow', 128000), 'maxTokens': config.get('maxTokens', 8192),
            'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}), encoding='utf-8')



def message_text(message):
    content = message.get('content', '')
    if isinstance(content, str):
        return content
    return '\n'.join(part.get('text', '') for part in content if isinstance(part, dict)) if isinstance(content, list) else ''

