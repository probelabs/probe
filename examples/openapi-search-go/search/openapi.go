package search

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"gopkg.in/yaml.v3"
)

// OpenAPISpec represents a parsed OpenAPI specification
type OpenAPISpec struct {
	FilePath string
	Version  string
	Info     Info
	Paths    map[string]PathItem
	Servers  []Server
}

type Info struct {
	Title       string `json:"title" yaml:"title"`
	Description string `json:"description" yaml:"description"`
	Version     string `json:"version" yaml:"version"`
}

type Server struct {
	URL         string `json:"url" yaml:"url"`
	Description string `json:"description" yaml:"description"`
}

type PathItem struct {
	Summary     string     `json:"summary" yaml:"summary"`
	Description string     `json:"description" yaml:"description"`
	Get         *Operation `json:"get" yaml:"get"`
	Post        *Operation `json:"post" yaml:"post"`
	Put         *Operation `json:"put" yaml:"put"`
	Delete      *Operation `json:"delete" yaml:"delete"`
	Patch       *Operation `json:"patch" yaml:"patch"`
}

type Operation struct {
	Summary     string      `json:"summary" yaml:"summary"`
	Description string      `json:"description" yaml:"description"`
	OperationID string      `json:"operationId" yaml:"operationId"`
	Tags        []string    `json:"tags" yaml:"tags"`
	Parameters  []Parameter `json:"parameters" yaml:"parameters"`
}

type Parameter struct {
	Name        string `json:"name" yaml:"name"`
	In          string `json:"in" yaml:"in"`
	Description string `json:"description" yaml:"description"`
	Required    bool   `json:"required" yaml:"required"`
}

// Endpoint represents a searchable API endpoint
type Endpoint struct {
	SpecFile    string
	Path        string
	Method      string
	Summary     string
	Description string
	OperationID string
	Tags        []string
	Parameters  []Parameter
	Tokens      []string // Pre-tokenized content for efficient search
}

// LoadSpec loads an OpenAPI spec from a file (JSON or YAML)
func LoadSpec(path string) (*OpenAPISpec, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("failed to read file: %w", err)
	}

	spec := &OpenAPISpec{FilePath: path}

	// Try to parse as JSON first, then YAML
	ext := strings.ToLower(filepath.Ext(path))
	if ext == ".json" {
		if err := json.Unmarshal(data, spec); err != nil {
			return nil, fmt.Errorf("failed to parse JSON: %w", err)
		}
	} else {
		if err := yaml.Unmarshal(data, spec); err != nil {
			return nil, fmt.Errorf("failed to parse YAML: %w", err)
		}
	}

	return spec, nil
}

// ExtractEndpoints extracts all API endpoints from a spec
func (s *OpenAPISpec) ExtractEndpoints() []Endpoint {
	var endpoints []Endpoint

	for path, pathItem := range s.Paths {
		operations := map[string]*Operation{
			"GET":    pathItem.Get,
			"POST":   pathItem.Post,
			"PUT":    pathItem.Put,
			"DELETE": pathItem.Delete,
			"PATCH":  pathItem.Patch,
		}

		for method, op := range operations {
			if op == nil {
				continue
			}

			// Safely extract operation fields
			summary := op.Summary
			description := op.Description
			operationID := op.OperationID
			tags := op.Tags
			parameters := op.Parameters

			endpoint := Endpoint{
				SpecFile:    s.FilePath,
				Path:        path,
				Method:      method,
				Summary:     summary,
				Description: description,
				OperationID: operationID,
				Tags:        tags,
				Parameters:  parameters,
			}

			// Include path-level description if operation doesn't have one
			if endpoint.Description == "" && pathItem.Description != "" {
				endpoint.Description = pathItem.Description
			}

			endpoints = append(endpoints, endpoint)
		}
	}

	return endpoints
}

// GetSearchableText returns all searchable text for an endpoint
func (e *Endpoint) GetSearchableText() string {
	parts := []string{
		e.Path,
		e.Method,
		e.Summary,
		e.Description,
		e.OperationID,
		strings.Join(e.Tags, " "),
	}

	// Add parameter names and descriptions
	for _, param := range e.Parameters {
		parts = append(parts, param.Name, param.Description)
	}

	return strings.Join(parts, " ")
}

// String returns a human-readable representation of the endpoint
func (e *Endpoint) String() string {
	tags := ""
	if len(e.Tags) > 0 {
		tags = fmt.Sprintf(" [%s]", strings.Join(e.Tags, ", "))
	}

	return fmt.Sprintf("%s %s%s\n  %s", e.Method, e.Path, tags, e.Summary)
}
