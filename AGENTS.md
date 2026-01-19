# MacAmp Agent Instructions

## Tooling for Shell Interactions

**Quick Decision Tree:**

- Finding **FILES**? → Use `fd` (or Glob tool)
- Finding **TEXT/STRINGS**? → Use `rg` (or Grep tool)
- Finding **CODE STRUCTURE**? → Use `ast-grep` (sg)
- Parsing **JSON**? → Use `jq`
- Parsing **YAML/XML**? → Use `yq` (or mcp__XcodeBuildMCP__* for Xcode)

### Tool Priority for MacAmp Project

1. ✅ **fd** - Installed, fast file discovery
2. ✅ **rg** (ripgrep) - Installed, fast text search
3. ✅ **ast-grep** (sg) - Installed, syntax-aware code search
4. ✅ **jq** - Installed, JSON processing
5. ✅ **yq** - Installed, YAML/XML processing
6. ❌ **fzf** - MISSING, LOW PRIORITY (interactive selection, rarely needed for AI)

---

### 1. File Discovery with `fd`

**When to use:** Finding files by name, extension, or path pattern

**Advantages over `find`:**

- 10-100x faster with parallel traversal
- Respects `.gitignore` by default (skips node_modules, build/, etc.)
- Simpler syntax with smart defaults
- Colorized output

**Examples:**

```bash
# Find all Swift files
fd -e swift

# Find files with "Window" in name
fd Window

# Find in specific directory
fd -e swift . MacAmpApp/

# Find large files (>1MB)
fd --type f --size +1m

# Include hidden files
fd --hidden -e swift

# Exclude patterns
fd -e swift -E "*Tests*"
```

**Swift-specific patterns:**

```bash
# All view files
fd -e swift -g "*View.swift"

# All model files
fd -e swift -g "*Model.swift" -g "*Settings.swift"

# Find Objective-C headers
fd -e h -e m
```

**When to use Glob tool instead:**

- When you need results returned directly to Claude (no bash execution)
- When pattern matching syntax is sufficient
- When you want Claude to process the file list programmatically

---

### 2. Text Search with `rg` (ripgrep)

**When to use:** Finding text strings, function names, or regex patterns in files

**Advantages over `grep`:**

- 2-10x faster with Rust implementation
- Respects `.gitignore` automatically
- Better defaults (recursive, line numbers, colors)
- Excellent Swift syntax support

**Examples:**

```bash
# Basic text search
rg "AVAudioEngine"

# Case-insensitive
rg -i "appdelegate"

# Only in Swift files
rg "ObservableObject" --type swift

# Multiple patterns
rg "TODO|FIXME" -g "*.swift"

# With context lines
rg "func setupPlayer" -A 3 -B 2

# Count matches per file
rg "weak var" --count --type swift
```

**Swift-specific patterns:**

```bash
# Find all @MainActor usages
rg "@MainActor" --type swift

# Find all @Published properties
rg "@Published.*var" --type swift

# Find all weak references
rg "weak (var|let)" --type swift

# Find retain cycle risks
rg "self\." --type swift -g "*Delegate*"

# Find deprecated API usage
rg "NSColor" --type swift  # Should use Color in SwiftUI
```

**When to use Grep tool instead:**

- When you need structured results in Claude's context
- When you want Claude to analyze matches programmatically
- When you need output_mode options (content/files_with_matches/count)

---

### 3. Code Structure Search with `ast-grep` (sg)

**When to use:** Finding code patterns, refactoring, or analyzing AST structure

**Advantages over text search:**

- Syntax-aware (understands Swift grammar)
- No false positives from comments/strings
- Can match patterns with wildcards (`$NAME`, `$$$`)
- Semantic understanding of code structure

**Critical rule:** Whenever a search requires syntax-aware or structural matching, **ALWAYS use `sg`** and avoid falling back to text-only tools like `rg` or `grep` unless explicitly requested.

**Examples:**

```bash
# Find all classes conforming to ObservableObject
sg --lang swift -p 'class $NAME: ObservableObject { $$$ }'

# Find all @MainActor functions
sg --lang swift -p '@MainActor func $NAME($$$) { $$$ }'

# Find all weak delegate properties
sg --lang swift -p 'weak var $NAME: $TYPE?'

# Find all published properties
sg --lang swift -p '@Published var $NAME: $TYPE'

# Find all async throws functions
sg --lang swift -p 'func $NAME($$$) async throws -> $RET { $$$ }'
```

**MacAmp-specific patterns:**

```bash
# Find all AppSettings property accesses
sg --lang swift -p 'appSettings.$PROP'

# Find all NSMenu initializations
sg --lang swift -p 'NSMenu(title: $TITLE)'

# Find all AVAudioEngine usages
sg --lang swift -p '$VAR.engine.$METHOD($$$)'

# Find all @State properties in views
sg --lang swift -p '@State private var $NAME: $TYPE'

# Find all UserDefaults reads
sg --lang swift -p 'UserDefaults.standard.$METHOD($$$)'
```

**TypeScript/JavaScript patterns (for reference repositories):**

```bash
# Find all React functional components
sg --lang typescript -p 'function $NAME($$$): React.FC { $$$ }'
sg --lang typescript -p 'const $NAME: React.FC = ($$$) => { $$$ }'

# Find all TypeScript interfaces
sg --lang typescript -p 'interface $NAME { $$$ }'
sg --lang typescript -p 'type $NAME = { $$$ }'

# Find all async/await functions
sg --lang typescript -p 'async function $NAME($$$) { $$$ }'
sg --lang javascript -p 'const $NAME = async ($$$) => { $$$ }'

# Find all React hooks usage
sg --lang typescript -p 'const [$STATE, $SETTER] = useState($$$)'
sg --lang typescript -p 'useEffect(() => { $$$ }, [$$$])'

# Find all event handlers
sg --lang typescript -p 'const handle$NAME = ($$$) => { $$$ }'

# Find all exports
sg --lang typescript -p 'export function $NAME($$$) { $$$ }'
sg --lang typescript -p 'export const $NAME = $$$'
```

**When to use text search instead:**

- Simple string matching ("find all TODOs")
- Searching across non-code files (markdown, JSON)
- When pattern is simpler than AST representation

---

### 4. JSON Processing with `jq`

**When to use:** Parsing, querying, or transforming JSON data

**Common use cases:**

```bash
# Extract specific field
cat package.json | jq '.dependencies'

# Xcode build settings
xcodebuild -showBuildSettings -json | jq '.[] | .buildSettings.PRODUCT_NAME'

# Filter array items
jq '.[] | select(.status == "active")' data.json

# Pretty-print minified JSON
echo '{"a":1,"b":2}' | jq '.'

# Extract nested fields
jq '.user.profile.email' response.json
```

**MacAmp examples:**

```bash
# Parse npm dependencies
jq '.dependencies | keys[]' package.json

# Extract Xcode scheme info
xcodebuild -list -json | jq '.project.schemes[]'

# Parse MCP tool responses (if JSON)
jq '.tools[] | .name' mcp-response.json
```

---

### 5. YAML/XML Processing with `yq`

**When to use:** Parsing YAML configs or XML files (rare in Swift projects)

**Note:** For Xcode `.pbxproj` files, prefer `mcp__XcodeBuildMCP__*` tools over manual XML parsing.

**Examples:**

```bash
# Read YAML config
yq '.database.host' config.yml

# XML parsing (limited use)
yq -p xml '.plist.dict.key' Info.plist
```

**MacAmp context:** Minimal usage expected. Xcode project files are handled by MCP tools.

---

### Installation & Verification

**Install missing tools:**

```bash
# Optional only
brew install fzf       # Interactive selection (low priority for AI)
brew upgrade yq        # Upgrade to v4.x
```

**Verify installation:**

```bash
# Check all tools
command -v fd rg ast-grep jq yq

# Check versions
fd --version
rg --version
ast-grep --version
jq --version
yq --version
```

---

### Integration with Claude Code Tools

**Prefer built-in tools when available:**

| Task | Bash Command | Claude Tool | Recommendation |
|------|-------------|-------------|----------------|
| Find files | `fd pattern` | `Glob` | Use **Glob** for simple patterns, **fd** for complex queries |
| Search text | `rg pattern` | `Grep` | Use **Grep** for structured output, **rg** for ad-hoc searches |
| Read files | `cat file` | `Read` | **ALWAYS use Read** (supports images, PDFs, notebooks) |
| Edit files | `sed/awk` | `Edit` | **ALWAYS use Edit** (safer, tracks changes) |
| Write files | `echo > file` | `Write` | **ALWAYS use Write** (validation, no shell escaping) |

**When to use Bash tools:**

- Quick exploratory searches during research phase
- Piping multiple commands together (e.g., `fd | xargs rg | jq`)
- Performance-critical bulk operations
- Terminal operations requiring shell execution (git, npm, xcodebuild)

---

### Quick Reference Card

```bash
# FILES: Find Swift view files
fd -e swift -g "*View.swift"

# TEXT: Find all TODO comments in Swift
rg "// TODO" --type swift

# CODE: Find all @MainActor classes (Swift)
sg --lang swift -p '@MainActor class $NAME { $$$ }'

# CODE: Find React components (TypeScript/JavaScript)
sg --lang typescript -p 'const $NAME: React.FC = ($$$) => { $$$ }'

# JSON: Extract Xcode scheme names
xcodebuild -list -json | jq '.project.schemes[]'

# VERIFY: Check all tools installed
command -v fd rg ast-grep jq yq
```

**Remember:** Use `fd` for **files**, `rg` for **text**, `sg` for **code structure**, and built-in Claude tools when available.

## Handling of Deprecated or Legacy code

Instead of adding // Deprecated or // Legacy code to preserve you should always remove this code.

## Using Gemini CLI for Large Codebase Analysis

When analyzing large codebases or multiple files that might exceed context limits, use the Gemini CLI with its massive
context window. Use `gemini -p` to leverage Google Gemini's large context capacity.

## File and Directory Inclusion Syntax

Use the `@` syntax to include files and directories in your Gemini prompts. The paths should be relative to WHERE you run the
  gemini command:

### Examples

**Single file analysis:**

```bash
gemini -p "@src/main.py Explain this file's purpose and structure"
```

**Multiple files:**

```bash
gemini -p "@package.json @src/index.js Analyze the dependencies used in the code"
```

**Entire directory:**

```bash
gemini -p "@src/ Summarize the architecture of this codebase"
```

**Multiple directories:**

```bash
gemini -p "@src/ @tests/ Analyze test coverage for the source code"
```

**Current directory and subdirectories:**

```bash
gemini -p "@./ Give me an overview of this entire project"
```

**Or use --all_files flag:**

```bash
gemini --all_files -p "Analyze the project structure and dependencies"
```

### Implementation Verification Examples

**Check if a feature is implemented:**

```bash
gemini -p "@src/ @lib/ Has dark mode been implemented in this codebase? Show me the relevant files and functions"
```

**Verify authentication implementation:**

```bash
gemini -p "@src/ @middleware/ Is JWT authentication implemented? List all auth-related endpoints and middleware"
```

**Check for specific patterns:**

```bash
gemini -p "@src/ Are there any React hooks that handle WebSocket connections? List them with file paths"
```

**Verify error handling:**

```bash
gemini -p "@src/ @api/ Is proper error handling implemented for all API endpoints? Show examples of try-catch blocks"
```

**Check for rate limiting:**

```bash
gemini -p "@backend/ @middleware/ Is rate limiting implemented for the API? Show the implementation details"
```

**Verify caching strategy:**

```bash
gemini -p "@src/ @lib/ @services/ Is Redis caching implemented? List all cache-related functions and their usage"
```

**Check for specific security measures:**

```bash
gemini -p "@src/ @api/ Are SQL injection protections implemented? Show how user inputs are sanitized"
```

**Verify test coverage for features:**

```bash
gemini -p "@src/payment/ @tests/ Is the payment processing module fully tested? List all test cases"
```

### When to Use Gemini CLI

Use `gemini -p` when:

- Analyzing entire codebases or large directories
- Comparing multiple large files
- Need to understand project-wide patterns or architecture
- Current context window is insufficient for the task
- Working with files totaling more than 100KB
- Verifying if specific features, patterns, or security measures are implemented
- Checking for the presence of certain coding patterns across the entire codebase

### Important Notes

- Paths in @ syntax are relative to your current working directory when invoking gemini
- The CLI will include file contents directly in the context
- No need for --yolo flag for read-only analysis
- Gemini's context window can handle entire codebases that would overflow Claude's context
- When checking implementations, be specific about what you're looking for to get accurate results

## TypeScript/JavaScript Standards

When writing in javascript or typescript adhere to these rules so that we can avoid code written that we will have to fix later.

## Code Style

- TypeScript strict mode
- Single quotes, no semicolons
- Use functional patterns where possible

## Critical Rules

When writing TypeScript or JavaScript code:

- NEVER use `any` type - use `unknown` or specific interfaces
- NEVER use console.log in production - use console.warn/error or logger
- ALWAYS prefix unused parameters with underscore
- ALWAYS add explicit types for function parameters
- ALWAYS add newline at end of files
- ALWAYS remove unused imports before committing

## Type Assertions

For type assertions:

- Use `as unknown as SpecificType` pattern
- Define interfaces for request params/body/query
- Type all WebSocket and event data

See TYPESCRIPT_JAVASCRIPT_GUIDELINES.md for detailed patterns.

## Multi-step approach to Context Engineering

This is how we approach our work in multi-step context engineering

## 0. Tasks

- Operating on a task basis. Store all intermediate context in markdown files in `tasks/<task-id>/` folders.
- Use semantic task id slugs

## 1. Research

- Find existing patterns in this codebase
- Search internet if relevant
- Start by asking follow up questions to set the direction of research
- Report findings in research.md file

## 2. Planning

- Read the research.md in tasks for `<task-id>`.
- Based on the research come up with a plan for implementing the user request. We should reuse existing patterns, components and code where possible.
- If needed, ask clarifying questions to user to understand the scope of the task
- Write the comprehensive plan to plan.md. The plan should include all context required for an engineer to implement the feature.

## 3. State

- Write the state of the task in state.md. This should include all the information that is needed to understand the current state of the task.

## 4. Implementation

- Read. plan.md and create a todo-list with all items, then execute on the plan.
- Go for as long as possible. If ambiguous, leave all questions to the end and group them.
- If you put in any comment for stub or placeholder to implement something in the future you must document it in placeholder.md

## 5. Deprecated or legacy code

- Instead of adding // Deprecated or // Legacy code to preserve you should always remove this code.
- Report deprecated and legacy code findings in deprecated.md instead of marking code deprecated or legacy.

## 6. Verification

Create feedback loops to test that the plan was implemented correctly:

- Use separate agents when possible to do the verification so there is separation between what is being tested and what is testing.
- Verify that the feature works as expected

## 7. Documentation

Document the feature and its usage:

- Write the documentation in the docs/ directory
- Include usage examples and API documentation
- Include any relevant diagrams or screenshots
- Include any relevant links to external resources
- Include any relevant links to the codebase
- Include any relevant links to the research
- Include any relevant links to the plan
- Include any relevant links to the state
- Include any relevant links to the implementation
- Include any relevant links to the verification
- Include any relevant links to the documentation

## 8. PLaceholder

Document intentional placeholder/scaffolding code in the codebase that is part of planned features. Per project conventions, we use centralized `placeholder.md` files instead of in-code TODO comments.

  **Key Rules:**
    1. NO `// TODO` comments in production code
    2. Document placeholders in `tasks/<task-id>/placeholder.md`
    3. Include file:line, purpose, status, and action
    4. Review during task completion—remove or implement

## Swift & Apple Platform Development

### Xcode v26.0 Documentation Reference

When working with Swift and Apple platform development (iOS, macOS, watchOS, visionOS), reference the official Xcode documentation located at:

`/Applications/Xcode.app/Contents/PlugIns/IDEIntelligenceChat.framework/Versions/A/Resources/AdditionalDocumentation/`

### Available Documentation Files

**SwiftUI:**

- `SwiftUI-AlarmKit-Integration.md` - AlarmKit integration patterns
- `SwiftUI-Implementing-Liquid-Glass-Design.md` - Liquid Glass design system for SwiftUI
- `SwiftUI-New-Toolbar-Features.md` - Modern toolbar implementations
- `SwiftUI-Styled-Text-Editing.md` - Rich text editing capabilities
- `SwiftUI-WebKit-Integration.md` - WebKit integration patterns

**Swift Language & Data:**

- `Swift-Charts-3D-Visualization.md` - 3D visualization in Swift Charts
- `Swift-Concurrency-Updates.md` - Modern concurrency patterns
- `Swift-InlineArray-Span.md` - Performance optimizations with inline arrays
- `SwiftData-Class-Inheritance.md` - SwiftData inheritance patterns

**AppKit & UIKit:**

- `AppKit-Implementing-Liquid-Glass-Design.md` - Liquid Glass for AppKit
- `UIKit-Implementing-Liquid-Glass-Design.md` - Liquid Glass for UIKit

**Framework Updates:**

- `AppIntents-Updates.md` - App Intents framework updates
- `Foundation-AttributedString-Updates.md` - AttributedString improvements
- `FoundationModels-Using-on-device-LLM-in-your-app.md` - On-device LLM integration
- `StoreKit-Updates.md` - StoreKit framework updates
- `MapKit-GeoToolbox-PlaceDescriptors.md` - MapKit location features

**Widgets & Extensions:**

- `WidgetKit-Implementing-Liquid-Glass-Design.md` - Widget design patterns
- `Widgets-for-visionOS.md` - visionOS widget development

**Accessibility & Features:**

- `Implementing-Assistive-Access-in-iOS.md` - Assistive Access patterns
- `Implementing-Visual-Intelligence-in-iOS.md` - Visual Intelligence integration

### Using Xcode Documentation with Gemini CLI

When you need detailed information about Swift controls or Apple frameworks, use:

```bash
# Analyze specific framework documentation
gemini -p "@/Applications/Xcode.app/Contents/PlugIns/IDEIntelligenceChat.framework/Versions/A/Resources/AdditionalDocumentation/SwiftUI-New-Toolbar-Features.md Explain the new toolbar capabilities"

# Compare multiple framework docs
gemini -p "@/Applications/Xcode.app/Contents/PlugIns/IDEIntelligenceChat.framework/Versions/A/Resources/AdditionalDocumentation/SwiftUI-Implementing-Liquid-Glass-Design.md @/Applications/Xcode.app/Contents/PlugIns/IDEIntelligenceChat.framework/Versions/A/Resources/AdditionalDocumentation/AppKit-Implementing-Liquid-Glass-Design.md Compare Liquid Glass implementation between SwiftUI and AppKit"

# Analyze all documentation for a specific feature
gemini -p "@/Applications/Xcode.app/Contents/PlugIns/IDEIntelligenceChat.framework/Versions/A/Resources/AdditionalDocumentation/ Show all references to 'Liquid Glass' design patterns"
```

### Swift Development Best Practices

**Code Style:**

- Follow Swift API Design Guidelines
- Use SwiftUI for modern UI development where possible
- Leverage Swift Concurrency (async/await) for asynchronous operations
- Use SwiftData for data persistence in new projects

**CRITICAL: When writing Swift code:**

- ALWAYS use explicit types for public APIs
- NEVER force unwrap optionals (!) in production code - use guard/if let
- ALWAYS handle errors with proper do-catch or Result types
- ALWAYS mark async functions and use await
- ALWAYS use @MainActor for UI updates
- ALWAYS follow Apple's Human Interface Guidelines

**Apple Platform Patterns:**

- Use MVVM or modern SwiftUI architectures
- Leverage SwiftUI's declarative syntax
- Use Combine or async/await for reactive patterns
- Follow Apple's accessibility guidelines for inclusive design
- Implement proper memory management with weak/unowned references
