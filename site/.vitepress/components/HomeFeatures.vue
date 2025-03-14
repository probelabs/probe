 <template>
  <div class="home-features">
    <FullWidthFeatureSection class="features-section">
      <template #content>
        <div class="features-grid">
          <div class="feature-card">
            <div class="feature-header">
              <span class="feature-icon">🔬</span>
              <h3>Deep Code Understanding</h3>
            </div>
            <p>Extract complete functions, classes and structures. Not just lines of code - full context every time.</p>
          </div>

          <div class="feature-card">
            <div class="feature-header">
              <span class="feature-icon">🛡️</span>
              <h3>Local Search Engine</h3>
            </div>
            <p>Probe runs locally without requiring embedding generation. No indexing needed, works like a full elastic search for your code.</p>
          </div>

          <div class="feature-card">
            <div class="feature-header">
              <span class="feature-icon">🖥️</span>
              <h3>For AI Code Editor Users</h3>
            </div>
            <p>Integrate with Cline, Roo, Cursor, Windsurf, or any MCP-compatible AI assistant. <br/><br/><a href="/use-cases/ai-code-editors">Setup Guide →</a></p>
          </div>

          <div class="feature-card">
            <div class="feature-header">
              <span class="feature-icon">⌨️</span>
              <h3>For CLI Power Users</h3>
            </div>
            <p>Advanced text-based queries, token limiting, and large repo handling.<br/><br/><a href="/use-cases/cli-ai-workflows">CLI Guide →</a></p>
          </div>

          <div class="feature-card">
            <div class="feature-header">
              <span class="feature-icon">🏢</span>
              <h3>Chat with Code</h3>
            </div>
            <p>A source of truth for how your product works. Enables quick issue resolution, documentation generation, and architecture understanding.<br/><br/><a href="/use-cases/deploying-probe-web-interface">Deployment Guide →</a></p>
          </div>

          <div class="feature-card">
            <div class="feature-header">
              <span class="feature-icon">🛠️</span>
              <h3>For AI Tooling Developers</h3>
            </div>
            <p>Embed Probe into your Node.js apps with our SDK.<br/><br/><a href="/use-cases/building-ai-tools">SDK Guide →</a></p>
          </div>
        </div>
      </template>
    </FullWidthFeatureSection>

    <FeatureSection>
      <template #content>
        <h2>Intelligent Code Search</h2>

        <p>Find complete code blocks with semantic understanding. Probe combines ripgrep's speed with tree-sitter's AST parsing to return only the relevant code. Supports full Elastic Search syntax for powerful queries.</p>
        
        <p>Advanced search examples:</p>
        
        <div class="language-bash"><pre style="font-family:monospace;color: rgb(68, 68, 68); background-color: rgb(243, 243, 243); font-weight: 400; "><code><span style="color: rgb(105, 112, 112); font-weight: 400;"># Basic search with natural language</span>
probe search <span style="color: rgb(136, 0, 0); font-weight: 400;">"user authentication"</span> ./src

<span style="color: rgb(105, 112, 112); font-weight: 400;"># Token-limited search for AI context windows</span>
probe search <span style="color: rgb(136, 0, 0); font-weight: 400;">"error handling"</span> --max-tokens 8000

<span style="color: rgb(105, 112, 112); font-weight: 400;"># Elastic search syntax with operators</span>
probe search <span style="color: rgb(136, 0, 0); font-weight: 400;">"(login OR auth) AND NOT test"</span> ./src

<span style="color: rgb(105, 112, 112); font-weight: 400;"># Extract full code just by specifying line number</span>
probe extract src/auth.js:15 

<span style="color: rgb(105, 112, 112); font-weight: 400;"># Extract by function/symbol name using # syntax</span>
probe extract <span style="color: rgb(136, 0, 0); font-weight: 400;">"src/main.rs#authenticateUser"</span>

<span style="color: rgb(105, 112, 112); font-weight: 400;"># Extract from unstructurred command output</span>
go <span style="color: rgb(57, 115, 0); font-weight: 400;">test</span> ./... | probe extract</code></pre></div>
      </template>
      <template #code>
        <div class="language-typescript"><pre style="font-family:monospace;color: rgb(68, 68, 68); background-color: rgb(243, 243, 243); font-weight: 400; "><code>$ probe search <span style="color: rgb(136, 0, 0); font-weight: 400;">"error handling"</span> --max-tokens 8000 --format json
{
  <span style="color: rgb(136, 0, 0); font-weight: 400;">"results"</span>: [
    {
      <span style="color: rgb(136, 0, 0); font-weight: 400;">"file"</span>: <span style="color: rgb(136, 0, 0); font-weight: 400;">"src/error.ts"</span>,
      <span style="color: rgb(136, 0, 0); font-weight: 400;">"lines"</span>: [10, 25],
      <span style="color: rgb(136, 0, 0); font-weight: 400;">"node_type"</span>: <span style="color: rgb(136, 0, 0); font-weight: 400;">"function"</span>,
      <span style="color: rgb(136, 0, 0); font-weight: 400;">"code"</span>: <span style="color: rgb(136, 0, 0); font-weight: 400;">"function handleApiError(error: ApiError) { ... }"</span>,
      <span style="color: rgb(136, 0, 0); font-weight: 400;">"matched_keywords"</span>: [<span style="color: rgb(136, 0, 0); font-weight: 400;">"error"</span>, <span style="color: rgb(136, 0, 0); font-weight: 400;">"handle"</span>],
      <span style="color: rgb(136, 0, 0); font-weight: 400;">"rank"</span>: 1,
      <span style="color: rgb(136, 0, 0); font-weight: 400;">"score"</span>: 0.95
    }
    ...
  ],
}

// Extract from <span style="color: rgb(57, 115, 0); font-weight: 400;">test</span> output, like a backtrace of failed <span style="color: rgb(57, 115, 0); font-weight: 400;">test</span> line
$ go <span style="color: rgb(57, 115, 0); font-weight: 400;">test</span> ./... | probe extract
<span style="color: rgb(105, 112, 112); font-weight: 400;">## File: user_auth_test.go</span>
Lines: 56-62
```
<span style="color: rgb(68, 68, 68); font-weight: 700;">function</span> TestUserAuth(t *testing.T) {
  user := createTestUser()
  token, err := authenticateUser(user.credentials)
  assert.NoError(t, err)
  assert.NotEmpty(t, token)
}
```</code></pre></div>
      </template>
    </FeatureSection>

    <FeatureSection>
      <template #content>
        <h2>Developer-Friendly Tools</h2>

        <p>Built by developers for developers, Probe is designed to be easy to install and extend:</p>
        
        <div class="language-bash"><pre style="font-family:monospace;color: rgb(68, 68, 68); background-color: rgb(243, 243, 243); font-weight: 400; "><code><span style="color: rgb(105, 112, 112); font-weight: 400;"># Use the core Probe tool without installation</span>
npx -y @buger/probe search <span style="color: rgb(136, 0, 0); font-weight: 400;">"error handling"</span> ./src

<span style="color: rgb(105, 112, 112); font-weight: 400;"># Start an interactive AI chat with your codebase</span>
npx -y @buger/probe-chat

<span style="color: rgb(105, 112, 112); font-weight: 400;"># Start the web interface on localhost:3000</span>
npx -y @buger/probe-web</code></pre></div>
      </template>
      <template #code>
        <div class="language-typescript"><pre style="font-family:monospace;color: rgb(68, 68, 68); background-color: rgb(243, 243, 243); font-weight: 400; "><code><span style="color: rgb(105, 112, 112); font-weight: 400;">// Example of using Probe's Node.js SDK</span>
<span style="color: rgb(0, 0, 136); font-weight: 400;">import</span> { search } <span style="color: rgb(0, 0, 136); font-weight: 400;">from</span> <span style="color: rgb(136, 0, 0); font-weight: 400;">'@buger/probe'</span>;

<span style="color: rgb(0, 0, 136); font-weight: 400;">const</span> results = <span style="color: rgb(0, 0, 136); font-weight: 400;">await</span> search({
  <span style="color: rgb(136, 0, 0); font-weight: 400;">query</span>: <span style="color: rgb(136, 0, 0); font-weight: 400;">"error handling"</span>,
  <span style="color: rgb(136, 0, 0); font-weight: 400;">path</span>: <span style="color: rgb(136, 0, 0); font-weight: 400;">"./src"</span>,
  <span style="color: rgb(136, 0, 0); font-weight: 400;">options</span>: {
    <span style="color: rgb(136, 0, 0); font-weight: 400;">maxResults</span>: 10,
    <span style="color: rgb(136, 0, 0); font-weight: 400;">contextLines</span>: 3,
    <span style="color: rgb(136, 0, 0); font-weight: 400;">maxTokens</span>: 8000  <span style="color: rgb(105, 112, 112); font-weight: 400;">// Token-aware for AI context windows</span>
  }
});

console.log(<span style="color: rgb(136, 0, 0); font-weight: 400;">`Found ${results.length} matches`</span>);</code></pre></div>
      </template>
    </FeatureSection>

    <FeatureSection>
      <template #content>
        <h2>Supercharge Your AI Assistant</h2>

        <p>Probe enhances AI coding assistants with accurate code context, enabling them to understand your entire codebase:</p>
        
        <ul>
          <li><strong>Complete Context</strong>: Get entire functions and classes, not fragments</li>
          <li><strong>Token-Aware</strong>: Limits results to fit AI context windows</li>
          <li><strong>Multiple Integration Options</strong>: MCP server, CLI chat, or SDK</li>
          <li><strong>Privacy Note</strong>: When using with external AI services, code snippets are sent to those services</li>
        </ul>
      </template>
      <template #code>
        <div class="language-typescript"><pre style="font-family:monospace;color: rgb(68, 68, 68); background-color: rgb(243, 243, 243); font-weight: 400; "><code><span style="color: rgb(0, 0, 136); font-weight: 400;">import</span> { search } <span style="color: rgb(0, 0, 136); font-weight: 400;">from</span> <span style="color: rgb(136, 0, 0); font-weight: 400;">'@buger/probe'</span>;
<span style="color: rgb(0, 0, 136); font-weight: 400;">import</span> { ChatOpenAI } <span style="color: rgb(0, 0, 136); font-weight: 400;">from</span> <span style="color: rgb(136, 0, 0); font-weight: 400;">'@langchain/openai'</span>;
<span style="color: rgb(0, 0, 136); font-weight: 400;">import</span> { tools } <span style="color: rgb(0, 0, 136); font-weight: 400;">from</span> <span style="color: rgb(136, 0, 0); font-weight: 400;">'@buger/probe'</span>;

<span style="color: rgb(105, 112, 112); font-weight: 400;">// Create AI assistant with code search capability</span>
<span style="color: rgb(0, 0, 136); font-weight: 400;">const</span> searchTool = tools.createSearchTool();
<span style="color: rgb(0, 0, 136); font-weight: 400;">const</span> model = <span style="color: rgb(0, 0, 136); font-weight: 400;">new</span> ChatOpenAI()
  .withTools([searchTool]);

<span style="color: rgb(105, 112, 112); font-weight: 400;">// Ask questions about your codebase</span>
<span style="color: rgb(0, 0, 136); font-weight: 400;">const</span> result = <span style="color: rgb(0, 0, 136); font-weight: 400;">await</span> model.invoke([
  { <span style="color: rgb(136, 0, 0); font-weight: 400;">role</span>: <span style="color: rgb(136, 0, 0); font-weight: 400;">"system"</span>,
    <span style="color: rgb(136, 0, 0); font-weight: 400;">content</span>: <span style="color: rgb(136, 0, 0); font-weight: 400;">"You are a code assistant."</span> },
  { <span style="color: rgb(136, 0, 0); font-weight: 400;">role</span>: <span style="color: rgb(136, 0, 0); font-weight: 400;">"user"</span>,
    <span style="color: rgb(136, 0, 0); font-weight: 400;">content</span>: <span style="color: rgb(136, 0, 0); font-weight: 400;">"How is error handling implemented?"</span> }
]);</code></pre></div>
      </template>
    </FeatureSection>


    <FeatureSection>
      <template #content>
        <h2>Advanced Pattern Matching</h2>

        <p>Find code based on its structure, not just text content, using AST-aware pattern matching:</p>
        
        <ul>
          <li><strong>AST-Grep Patterns</strong>: Search for specific code structures</li>
          <li><strong>Placeholder Variables</strong>: Match function names, parameters, and bodies</li>
          <li><strong>Structure-Aware Matching</strong>: Find patterns regardless of formatting</li>
        </ul>
      </template>
      <template #code>
        <div class="language-bash"><pre style="font-family:monospace;color: rgb(68, 68, 68); background-color: rgb(243, 243, 243); font-weight: 400; "><code><span style="color: rgb(105, 112, 112); font-weight: 400;"># Find all try/catch blocks</span>
probe query -p <span style="color: rgb(136, 0, 0); font-weight: 400;">"try $$$BODY catch ($ERROR) $$$HANDLER"</span> ./src

<span style="color: rgb(105, 112, 112); font-weight: 400;"># Find all React useEffect hooks with dependencies</span>
probe query -p <span style="color: rgb(136, 0, 0); font-weight: 400;">"useEffect(() => $$$BODY, [$$$DEPS])"</span> ./src

<span style="color: rgb(105, 112, 112); font-weight: 400;"># Find all functions that call a specific method</span>
probe query -p <span style="color: rgb(136, 0, 0); font-weight: 400;">"function $NAME($$$) { $$$; fetchData($$$); $$$; }"</span> ./src</code></pre></div>
      </template>
    </FeatureSection>

    <FullWidthFeatureSection>
      <template #title>
        <h2>Choose Your Path</h2>
      </template>
      <template #content>
        <p>Probe adapts to different workflows and use cases. Select the approach that works best for you:</p>
        
        <div class="audience-grid">
          <div class="audience-card">
            <h3>🖥️ AI Code Editor User</h3>
            <p>Enhance your AI-powered editor with deep code understanding</p>
            <a href="/use-cases/ai-code-editors" class="audience-link">AI Editor Integration →</a>
          </div>
          
          <div class="audience-card">
            <h3>⌨️ CLI Power User</h3>
            <p>Ask questions about your code directly from your terminal</p>
            <a href="/use-cases/cli-ai-workflows" class="audience-link">CLI AI Workflows →</a>
          </div>
          
          <div class="audience-card">
            <h3>🏢 Chat with Code</h3>
            <p>Use code as a source of truth for how your product works, resolve issues quickly, and generate documentation</p>
            <a href="/use-cases/deploying-probe-web-interface" class="audience-link">Deployment Guide →</a>
          </div>
          
          <div class="audience-card">
            <h3>🛠️ AI Tooling Developer</h3>
            <p>Build custom AI-powered code tools with the Node.js SDK</p>
            <a href="/use-cases/building-ai-tools" class="audience-link">SDK & LangChain →</a>
          </div>
        </div>
      </template>
    </FullWidthFeatureSection>
  </div>
</template>

<script>
import FeatureSection from './FeatureSection.vue'
import FullWidthFeatureSection from './FullWidthFeatureSection.vue'

export default {
  components: {
    FeatureSection,
    FullWidthFeatureSection
  }
}
</script>

<style>
.home-features {
  margin-top: 2rem;
}

/* Hide default VitePress features */
:deep(.vp-features),
:deep(.VPFeatures) {
  display: none !important;
}

.features-section {
  width: 100%;
}

:deep(.features-section *) {
  text-align: left !important;
  margin-left: 0;
  margin-right: 0;
}

.features-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 2rem;
  margin: 2rem auto;
  max-width: 1200px;
}

.feature-card {
  background-color: var(--vp-c-bg-soft);
  border-radius: 12px;
  padding: 1.5rem;
  transition: transform 0.2s ease, box-shadow 0.2s ease;
}

.feature-card:hover {
  transform: translateY(-5px);
  box-shadow: 0 5px 15px rgba(0, 0, 0, 0.1);
}

.feature-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
}

.feature-icon {
  display: inline;
  font-size: 1.2rem;
  line-height: 1;
}

.feature-card h3 {
  margin: 0;
  font-size: 1.2rem;
  color: var(--vp-c-text-1);
  line-height: 1;
}

.feature-card p {
  margin: 1rem 0 0 0;
  color: var(--vp-c-text-2);
  line-height: 1.6;
}

.feature-card a {
  color: var(--vp-c-brand);
  text-decoration: none;
}

.feature-card a:hover {
  text-decoration: underline;
}

@media (max-width: 768px) {
  .features-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 480px) {
  .features-grid {
    grid-template-columns: 1fr;
  }
}

.audience-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  grid-template-rows: repeat(2, auto);
  gap: 1.5rem;
  margin-top: 1.5rem;
  max-width: 900px;
  margin-left: auto;
  margin-right: auto;
}

@media (max-width: 640px) {
  .audience-grid {
    grid-template-columns: 1fr;
    gap: 1rem;
  }
}

.audience-card {
  background-color: var(--vp-c-bg-soft);
  border-radius: 8px;
  padding: 1.5rem;
  transition: transform 0.2s, box-shadow 0.2s;
}

@media (max-width: 640px) {
  .audience-card {
    padding: 1rem 0.75rem;
  }
}

.audience-card:hover {
  transform: translateY(-5px);
  box-shadow: 0 5px 15px rgba(0, 0, 0, 0.1);
}

.audience-card h3 {
  margin-top: 0;
  font-size: 1.2rem;
}

.audience-link {
  display: inline-block;
  margin-top: 1rem;
  font-weight: 500;
  color: var(--vp-c-brand);
  text-decoration: none;
}

.audience-link:hover {
  text-decoration: underline;
}
</style>