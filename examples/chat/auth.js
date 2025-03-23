import 'dotenv/config';

/**
 * Basic authentication middleware
 * Checks for valid username and password in the Authorization header
 * Can be enabled/disabled via environment variables
 */
export function authMiddleware(req, res, next) {
	// Check if authentication is enabled
	const AUTH_ENABLED = process.env.AUTH_ENABLED === 'true' || process.env.AUTH_ENABLED === '1';

	// If authentication is not enabled, skip authentication check
	if (!AUTH_ENABLED) {
		return next(req, res);
	}

	// Get configured username and password from environment variables
	const AUTH_USERNAME = process.env.AUTH_USERNAME || 'admin';
	const AUTH_PASSWORD = process.env.AUTH_PASSWORD || 'password';

	// Check if request has Authorization header
	const authHeader = req.headers.authorization;

	if (!authHeader) {
		// No Authorization header, return 401 Unauthorized
		res.writeHead(401, {
			'Content-Type': 'text/plain',
			'WWW-Authenticate': 'Basic realm="Probe Code Search"'
		});
		res.end('Authentication required');
		return;
	}

	// Parse Authorization header
	try {
		// Basic auth format: "Basic base64(username:password)"
		const authParts = authHeader.split(' ');
		if (authParts.length !== 2 || authParts[0] !== 'Basic') {
			throw new Error('Invalid Authorization header format');
		}

		// Decode base64 credentials
		const credentials = Buffer.from(authParts[1], 'base64').toString('utf-8');
		const [username, password] = credentials.split(':');

		// Check if credentials match
		if (username === AUTH_USERNAME && password === AUTH_PASSWORD) {
			// Authentication successful, proceed to next middleware
			return next(req, res);
		} else {
			// Invalid credentials, return 401 Unauthorized
			res.writeHead(401, {
				'Content-Type': 'text/plain',
				'WWW-Authenticate': 'Basic realm="Probe Code Search"'
			});
			res.end('Invalid credentials');
			return;
		}
	} catch (error) {
		// Error parsing Authorization header, return 400 Bad Request
		res.writeHead(400, { 'Content-Type': 'text/plain' });
		res.end('Invalid Authorization header');
		return;
	}
}

/**
 * Apply authentication middleware to a request handler
 * @param {Function} handler - The request handler function
 * @returns {Function} - A new handler function with authentication
 */
export function withAuth(handler) {
	return (req, res) => {
		authMiddleware(req, res, () => handler(req, res));
	};
}