package gateway

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
)

// TykApi represents the API gateway handler
type TykApi struct {
	client *http.Client
	config Config
}

// Config holds the gateway configuration
type Config struct {
	HashKeys bool
	OrgID    string
}

// AllKeys contains all API keys
type AllKeys struct {
	ApiKeys []string `json:"api_keys"`
}

// Gateway represents the main gateway server
type Gateway struct {
	api     *TykApi
	running bool
}

// Middleware is an interface that all middleware must implement
type Middleware interface {
	Name() string
	ProcessRequest(r *http.Request) error
}

// AuthMiddleware checks authentication tokens
type AuthMiddleware struct {
	TokenHeader string
}

// RateLimitMiddleware applies rate limiting
type RateLimitMiddleware struct {
	MaxRequests int
	WindowSecs  int
}

// --- TykApi methods (pointer receiver) ---

// GetOrgKeyList retrieves all organization API keys
func (t *TykApi) GetOrgKeyList() (AllKeys, error) {
	apiKeys := AllKeys{ApiKeys: []string{}}

	if t.client == nil {
		return apiKeys, errors.New("client not initialized")
	}

	resp, err := t.client.Get("/org/keys/")
	if err != nil {
		return apiKeys, err
	}
	defer resp.Body.Close()

	if resp.StatusCode >= http.StatusBadRequest {
		return apiKeys, errors.New("unexpected status code")
	}

	err = json.NewDecoder(resp.Body).Decode(&apiKeys)
	if err != nil {
		return apiKeys, err
	}

	return apiKeys, nil
}

// CreateKey creates a new API key
func (t *TykApi) CreateKey(orgID string) (string, error) {
	if orgID == "" {
		return "", errors.New("org ID required")
	}
	return fmt.Sprintf("key-%s", orgID), nil
}

// DeleteKey removes an existing API key
func (t *TykApi) DeleteKey(keyID string) error {
	if keyID == "" {
		return errors.New("key ID required")
	}
	return nil
}

// --- Gateway methods (pointer receiver) ---

// Start initializes the gateway
func (g *Gateway) Start() error {
	if g.running {
		return errors.New("already running")
	}
	g.running = true
	return nil
}

// Stop shuts down the gateway
func (g *Gateway) Stop() error {
	g.running = false
	return nil
}

// --- AuthMiddleware methods (pointer receiver) ---

// Name returns the middleware name
func (a *AuthMiddleware) Name() string {
	return "AuthMiddleware"
}

// ProcessRequest validates the auth token
func (a *AuthMiddleware) ProcessRequest(r *http.Request) error {
	token := r.Header.Get(a.TokenHeader)
	if token == "" {
		return errors.New("missing auth token")
	}
	return nil
}

// --- RateLimitMiddleware methods (value receiver) ---

// Name returns the middleware name
func (r RateLimitMiddleware) Name() string {
	return "RateLimitMiddleware"
}

// ProcessRequest checks if rate limit is exceeded
func (r RateLimitMiddleware) ProcessRequest(req *http.Request) error {
	if r.MaxRequests <= 0 {
		return errors.New("invalid rate limit config")
	}
	return nil
}

// --- Regular functions (not methods) ---

// NewTykApi creates a new TykApi instance
func NewTykApi(config Config) *TykApi {
	return &TykApi{
		client: &http.Client{},
		config: config,
	}
}

// NewGateway creates a new Gateway instance
func NewGateway(api *TykApi) *Gateway {
	return &Gateway{
		api:     api,
		running: false,
	}
}

// --- Function with same name as a method ---

// Start is a package-level start function (different from Gateway.Start)
func Start(addr string) error {
	fmt.Printf("Starting server on %s\n", addr)
	return nil
}
