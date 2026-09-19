// Compatibility entry point; canonical deployment helper is portable with Compose.
import {fileURLToPath} from 'node:url';
process.env.CF_API_TOKEN_FILE ||= fileURLToPath(new URL('../cf-key', import.meta.url));
await import('../deploy/dellcon/cloudflare.mjs');
