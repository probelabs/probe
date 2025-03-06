package middleware

// IPWhiteListMiddleware is a middleware that checks if the client's IP is in the whitelist
type IPWhiteListMiddleware struct {
	Whitelist []string
}

// Name returns the name of the middleware
func (i *IPWhiteListMiddleware) Name() string {
	return "IPWhiteListMiddleware"
}